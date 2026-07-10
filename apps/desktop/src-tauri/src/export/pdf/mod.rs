use crate::error::{AppError, AppResult};

use super::{
    templates::Template,
    types::{DocumentType, ExportRequest},
};

// Typst engine — used by the live export path for all twelve templates.
use super::typst_engine::{
    render_letter_pdf, render_letter_svg_pages, render_pdf, render_pdf_with_photo,
    render_resume_svg_pages, render_resume_svg_pages_with_photo, resolve_photo, RenderOpts,
    TypstTemplate,
};
use crate::model::document::DocumentModel;

fn extract_section<'a>(text: &'a str, start_marker: &str, end_marker: Option<&str>) -> &'a str {
    let start = if let Some(idx) = text.find(start_marker) {
        let after = &text[idx + start_marker.len()..];
        after
            .find('\n')
            .map(|i| idx + start_marker.len() + i + 1)
            .unwrap_or(idx + start_marker.len())
    } else {
        return text;
    };
    let end = if let Some(em) = end_marker {
        text[start..]
            .find(em)
            .map(|i| start + i)
            .unwrap_or(text.len())
    } else {
        text.len()
    };
    text[start..end].trim()
}

// ─── Prepared render inputs (shared by PDF + SVG-preview emit) ─────────────────

/// Everything the Typst résumé render fns need, built once from an
/// [`ExportRequest`]. Shared by [`generate_pdf`] (PDF bytes) and
/// [`generate_preview_svg`] (per-page SVG) so the preview is byte-identical to
/// the export it mirrors — only the final emit (`render_pdf*` vs
/// `render_resume_svg_pages*`) differs.
struct ResumeRenderInputs {
    model: DocumentModel,
    template: Template,
    typst_template: TypstTemplate,
    opts: RenderOpts,
    photo_png: Option<Vec<u8>>,
}

/// Build the résumé render inputs from a request: section extraction,
/// `DocumentModel` build, name/contact header overrides, ATS linearization,
/// page geometry, and photo resolution. Behaviour-preserving extraction of the
/// résumé arm that previously lived inline in [`generate_pdf`].
fn prepare_resume_render(request: &ExportRequest) -> ResumeRenderInputs {
    let template = Template::get(request.template_id);

    let text = extract_section(
        &request.text,
        "### CANDIDATE RESUME ###",
        Some("### JOB ADVERTISEMENT ###"),
    );
    let text = if text.is_empty() { &request.text } else { text };

    // Build the DocumentModel from text + apply contact-profile header
    // override + optionally linearize for ATS.
    let mut model = crate::model::adapter::model_from_resume_text(text);

    // Apply candidate name from metadata.
    if let Some(name) = request
        .meta
        .as_ref()
        .and_then(|m| m.candidate_name.as_deref())
        .filter(|s| !s.trim().is_empty())
    {
        model.header.name = name.to_string();
    }

    // Override contact line from the named profile fields (URL-swap safety).
    if let Some(contact) = request.contact.as_ref() {
        contact.apply_to_header(&mut model.header, &request.target_lang());
    }

    // ATS mode: linearize section order to single-column reading order.
    if request.ats_mode {
        crate::model::transform::linearize(&mut model);
    }

    let page = request.page_geometry();
    let opts = RenderOpts {
        page,
        // Document-accent seam for the résumé-PDF path: forward the request's
        // optional 6-hex override here and nowhere else. `normalise_accent`
        // (in `prepare`) validates it; the parametric `.typ` prefers a non-empty
        // `data.opts.accent` over the template's built-in palette. `None` (the
        // default) leaves the palette untouched. The letter + DOCX paths thread
        // the same value through `Template::with_accent_override` instead.
        accent: request.accent.clone(),
        lang: request.target_lang(),
        ats: request.ats_mode,
    };

    // Resolved whenever a contact photo is present; only photo-capable templates
    // consume it.
    let contact_has_photo = request
        .contact
        .as_ref()
        .and_then(|c| c.photo.as_deref())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let photo_png = request
        .contact
        .as_ref()
        .and_then(|c| c.photo.as_deref())
        .and_then(resolve_photo);
    if contact_has_photo && photo_png.is_none() {
        log::warn!(
            "export: contact photo could not be loaded (format/size/path); exporting without it"
        );
    }

    let typst_template = TypstTemplate::from_template(&template);

    ResumeRenderInputs {
        model,
        template,
        typst_template,
        opts,
        photo_png,
    }
}

/// The finished cover-letter text plus the resolved (market, lang) for it.
/// Shared by the PDF and SVG-preview cover-letter paths.
fn prepare_letter_text(request: &ExportRequest) -> (&str, String) {
    let text = extract_section(&request.text, "### COMPLETE COVER LETTER ###", None);
    let text = if text.is_empty() { &request.text } else { text };
    let lang = request.target_lang();
    (text, lang)
}

// ─── Public entry point ───────────────────────────────────────────────────────

pub fn generate_pdf(request: &ExportRequest) -> AppResult<Vec<u8>> {
    match request.document_type {
        DocumentType::Resume => {
            let ResumeRenderInputs {
                model,
                template,
                typst_template,
                opts,
                photo_png,
            } = prepare_resume_render(request);

            let result = if photo_png.is_some() {
                render_pdf_with_photo(&model, typst_template, &opts, Some(&template), photo_png)
            } else {
                render_pdf(&model, typst_template, &opts, Some(&template))
            };

            result.map_err(|e| AppError::Parse(format!("Failed to generate resume PDF: {e}")))
        }
        DocumentType::CoverLetter => {
            // Document accent threads through the resume template's style, which
            // the cover letter inherits (`letter_style_from_template`).
            let template =
                Template::get(request.template_id).with_accent_override(request.accent.as_deref());
            let (text, lang) = prepare_letter_text(request);
            let meta_name = request
                .meta
                .as_ref()
                .and_then(|m| m.candidate_name.as_deref());
            let market = request.locale.as_deref().unwrap_or("intl");

            render_letter_pdf(
                text,
                &template,
                request.contact.as_ref(),
                meta_name,
                market,
                &lang,
            )
            .map_err(|e| AppError::Parse(format!("Failed to generate cover letter PDF: {e}")))
        }
    }
}

/// Live-preview sibling of [`generate_pdf`]: render the SAME document to one SVG
/// string per page instead of PDF bytes.
///
/// Builds inputs through the SAME helpers as the PDF path
/// ([`prepare_resume_render`] / [`prepare_letter_text`]) so preview fidelity is
/// identical to export — only the final engine call differs
/// (`render_*_svg_pages*` vs `render_pdf*`). Used by the
/// `documents_render_preview_images` command.
///
/// NOTE: this is the raw render. The export command runs requests through the
/// `validate/` gate (which also normalizes ATS-mode); the preview command reuses
/// the exact same request validation and normalization before calling this, so
/// the previewed bytes track what export would produce.
pub fn generate_preview_svg(request: &ExportRequest) -> AppResult<Vec<String>> {
    match request.document_type {
        DocumentType::Resume => {
            let ResumeRenderInputs {
                model,
                template,
                typst_template,
                opts,
                photo_png,
            } = prepare_resume_render(request);

            let result = if photo_png.is_some() {
                render_resume_svg_pages_with_photo(
                    &model,
                    typst_template,
                    &opts,
                    Some(&template),
                    photo_png,
                )
            } else {
                render_resume_svg_pages(&model, typst_template, &opts, Some(&template))
            };

            result.map_err(|e| AppError::Parse(format!("Failed to render resume preview: {e}")))
        }
        DocumentType::CoverLetter => {
            // Same document-accent threading as the PDF cover-letter path so the
            // live preview matches the export.
            let template =
                Template::get(request.template_id).with_accent_override(request.accent.as_deref());
            let (text, lang) = prepare_letter_text(request);
            let meta_name = request
                .meta
                .as_ref()
                .and_then(|m| m.candidate_name.as_deref());
            let market = request.locale.as_deref().unwrap_or("intl");

            render_letter_svg_pages(
                text,
                &template,
                request.contact.as_ref(),
                meta_name,
                market,
                &lang,
            )
            .map_err(|e| AppError::Parse(format!("Failed to render cover letter preview: {e}")))
        }
    }
}

#[cfg(test)]
mod test;

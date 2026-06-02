use anyhow::{Context, Result};

use super::{
    templates::Template,
    types::{DocumentType, ExportRequest},
};

// Typst engine — used by the live export path for all nine templates.
use super::typst_engine::{
    render_letter_pdf, render_pdf, render_pdf_with_photo, resolve_photo, RenderOpts, TypstTemplate,
};

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

// ─── Public entry point ───────────────────────────────────────────────────────

pub fn generate_pdf(request: &ExportRequest) -> Result<Vec<u8>> {
    let template = Template::get(request.template_id);

    match request.document_type {
        DocumentType::Resume => {
            let text = extract_section(
                &request.text,
                "### CANDIDATE RESUME ###",
                Some("### JOB ADVERTISEMENT ###"),
            );
            let text = if text.is_empty() { &request.text } else { text };

            // ── Typst résumé path (all nine live templates) ───────────────────
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
                // TODO(user-accent): RenderOpts.accent is always None — templates use their built-in accent. Wire from the export request when a user-settable accent color feature lands.
                accent: None,
                lang: request.target_lang(),
                ats: request.ats_mode,
            };

            // Resolve photo bytes (for photo-capable templates; None for others).
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

            let result = if photo_png.is_some() {
                render_pdf_with_photo(&model, typst_template, &opts, Some(&template), photo_png)
            } else {
                render_pdf(&model, typst_template, &opts, Some(&template))
            };

            result
                .map_err(|e| anyhow::anyhow!("{e}"))
                .context("Failed to generate resume PDF")
        }
        DocumentType::CoverLetter => {
            let text = extract_section(&request.text, "### COMPLETE COVER LETTER ###", None);
            let text = if text.is_empty() { &request.text } else { text };

            // ── Typst cover-letter path ───────────────────────────────────────
            let meta_name = request
                .meta
                .as_ref()
                .and_then(|m| m.candidate_name.as_deref());
            let market = request.locale.as_deref().unwrap_or("intl");
            let lang = request.target_lang();

            render_letter_pdf(
                text,
                &template,
                request.contact.as_ref(),
                meta_name,
                market,
                &lang,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("Failed to generate cover letter PDF")
        }
    }
}

#[cfg(test)]
mod test;

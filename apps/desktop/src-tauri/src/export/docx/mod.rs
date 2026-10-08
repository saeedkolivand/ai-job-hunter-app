use anyhow::{Context, Result};

use super::{
    templates::Template,
    types::{DocumentType, ExportRequest, LetterRender},
};
use crate::locale::LocaleProfile;

use super::docx_renderer::mm_to_dxa;

// Split (issue #1280 batch 5b): the cover-letter renderers each get their own
// module — `letter_classic` (the original, unmodified Classic path),
// `letter_style` (the per-layout `LetterDocxStyle` table + the shared header-
// band tint), `letter_layout` (the Refined/Banded/Navy/Sidebar/Monogram
// scanning loop) and `letter_layout_blocks` (its extracted paragraph
// builders). This file keeps the public entry point plus the small, shared
// page-geometry/section-extraction helpers.
mod letter_classic;
mod letter_layout;
mod letter_layout_blocks;
mod letter_style;

use letter_classic::generate_cover_letter_docx;

// Re-exported at the same path external callers already use
// (`crate::export::docx::band_tint_hex`, e.g. `model_docx`'s Awesome résumé
// header) — widened to `pub(in crate::export)` at its definition in
// `letter_style` so this re-export isn't widening beyond what the item itself
// grants.
pub(super) use letter_style::band_tint_hex;

/// Page size (in DOCX `dxa`) for the active locale. Defaults to the `en` profile
/// (A4), keeping DOCX on the same page geometry as the PDF backends. Set
/// explicitly rather than relying on the docx-rs default so the source of truth
/// is `LocaleProfile`/`PageGeometry`; per-request locale sizing arrives in a
/// later phase.
fn page_size_dxa() -> (u32, u32) {
    let geom = LocaleProfile::default().page_geometry();
    (mm_to_dxa(geom.width_mm), mm_to_dxa(geom.height_mm))
}

// ─── Extract section helper ───────────────────────────────────────────────────

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

pub fn generate_docx(request: &ExportRequest) -> Result<Vec<u8>> {
    // Document accent (ADR 0004): recolor the template's accent-derived fields
    // when a valid override is present. `setup_colors` reads `emphasis_color`, so
    // the accent surfaces on emphasized runs for both the résumé and cover-letter
    // DOCX paths (both derive from this `template`). No-op when absent/malformed.
    let template =
        Template::get(request.template_id).with_accent_override(request.accent.as_deref());

    // The DOCX path collapses two-column templates to a single column since
    // DOCX doesn't replicate the sidebar layout.
    let single_column = || {
        let mut t = template.clone();
        if crate::theme::is_two_column(request.template_id) {
            t.two_column = None;
            t.margin_in = 1.0;
        }
        t
    };

    let docx = match request.document_type {
        DocumentType::Resume => {
            let text = extract_section(
                &request.text,
                "### CANDIDATE RESUME ###",
                Some("### JOB ADVERTISEMENT ###"),
            );
            let text = if text.is_empty() {
                request.text.as_str()
            } else {
                text
            };
            // model_docx is the sole résumé-DOCX path. The legacy flat-parser arm
            // has been removed — it was only reachable with `--no-default-features`
            // and diverged from the model path.
            crate::export::model_docx::generate_resume_docx_in(
                text,
                request.meta.as_ref(),
                &template,
                request.ats_mode,
                request.page_geometry(),
                request.contact.as_ref(),
                &request.target_lang(),
                // "intl" (not "en", a language tag, not a market) — matches
                // the cover-letter path's fallback below;
                // `generate_resume_docx_in` canonicalises this through
                // `LocaleProfile::get` before it reaches `section_order_for`.
                request.locale.as_deref().unwrap_or("intl"),
            )
            .context("Failed to generate resume DOCX")?
        }
        DocumentType::CoverLetter => {
            let text = extract_section(&request.text, "### COMPLETE COVER LETTER ###", None);
            let text = if text.is_empty() {
                request.text.as_str()
            } else {
                text
            };
            // `market` drives only the Refined reference-line label (see
            // `letter_layout::generate_cover_letter_docx_layout`'s doc comment);
            // mirrors the PDF cover-letter path's `market` computation in
            // `pdf/mod.rs`.
            let market = request.locale.as_deref().unwrap_or("intl");
            let lang = request.target_lang();
            // `ats` mirrors the PDF cover-letter path: ATS mode drops each
            // layout's decorative tint in BOTH formats, so a golden-parity
            // check can't find a band in one and not the other.
            generate_cover_letter_docx(
                text,
                request.meta.as_ref(),
                &single_column(),
                request.contact.as_ref(),
                LetterRender {
                    market,
                    lang: &lang,
                    layout: request.letter_layout,
                    ats: request.ats_mode,
                },
            )
            .context("Failed to generate cover letter DOCX")?
        }
    };

    // docx-rs exposes no creator setter and stamps 1970 / "unknown" (#1398), so
    // replace the built core part with one carrying the candidate and the time.
    let creator = request.candidate_name().unwrap_or("AI Job Hunter");
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut built = docx.build();
    built.doc_props.core = core_properties_xml(creator, &now);

    let mut buffer = std::io::Cursor::new(Vec::new());
    built.pack(&mut buffer).context("Failed to pack DOCX")?;
    Ok(buffer.into_inner())
}

/// `docProps/core.xml` with a real creator and created/modified time (`now`, W3CDTF).
///
/// The XML mirrors docx-rs 0.4.22's `CoreProps` template (its `created_at` /
/// `updated_at` can set the dates but nothing can set the creator): re-check it
/// against `doc_props/core.rs` whenever docx-rs is bumped.
fn core_properties_xml(creator: &str, now: &str) -> Vec<u8> {
    // Keep only the XML 1.0 `Char` production; anything else corrupts the part.
    let creator: String = creator
        .chars()
        .filter(|c| matches!(c, '\t' | '\n' | '\r' | ' '..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'))
        .collect();
    let creator = if creator.trim().is_empty() {
        "AI Job Hunter".to_string()
    } else {
        creator
    };
    let creator = creator
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dcterms:created xsi:type="dcterms:W3CDTF">{now}</dcterms:created><dc:creator>{creator}</dc:creator><cp:lastModifiedBy>{creator}</cp:lastModifiedBy><dcterms:modified xsi:type="dcterms:W3CDTF">{now}</dcterms:modified><cp:revision>1</cp:revision></cp:coreProperties>"#
    )
    .into_bytes()
}

#[cfg(test)]
mod tests;

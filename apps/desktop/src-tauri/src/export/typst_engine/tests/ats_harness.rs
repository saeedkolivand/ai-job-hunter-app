//! Classic-template ATS harness: reading order, word boundaries, content presence.

use super::fixtures::opts_a4;
use super::resume_fixtures::FIXTURE_RESUME;
use crate::export::templates::Template;
use crate::export::types::TemplateId;
use crate::export::typst_engine::{render_pdf, RenderOpts, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

//
// Renders the fixture through Classic, extracts text with pdf-extract, and
// asserts three ATS-safety properties:
//
//   (a) READING ORDER — section headings appear in the expected top-to-bottom
//       order (SUMMARY before EXPERIENCE before EDUCATION before SKILLS).
//
//   (b) WORD BOUNDARIES — a known multi-word phrase from the fixture survives
//       WITH spaces and is not run together (e.g. "State University" not
//       "StateUniversity").
//
//   (c) CONTENT PRESENT — the candidate name, all major section headings, and
//       a representative bullet fragment are findable in the extracted text.

#[test]
fn ats_harness_classic_reading_order_word_boundaries_content() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) for ATS harness");

    let extracted =
        pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed on our output");

    let lower = extracted.to_lowercase();

    // ── (c) Content present ───────────────────────────────────────────────────
    assert!(
        lower.contains("jane doe"),
        "ATS: candidate name 'Jane Doe' missing from extracted text\n---\n{extracted}"
    );

    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "ATS: section heading '{heading}' missing from extracted text\n---\n{extracted}"
        );
    }

    // A bullet fragment that must survive intact.
    assert!(
        lower.contains("distributed task scheduler"),
        "ATS: bullet fragment 'distributed task scheduler' missing\n---\n{extracted}"
    );

    // ── (b) Word boundaries ───────────────────────────────────────────────────
    // "State University" must appear with a space, not run together.
    assert!(
        lower.contains("state university"),
        "ATS: 'state university' must appear with preserved word boundary\n---\n{extracted}"
    );

    // ── (a) Reading order ─────────────────────────────────────────────────────
    let order = ["summary", "experience", "education", "skills"];
    let mut last_pos = 0usize;
    for heading in &order {
        let pos = lower.find(heading).unwrap_or_else(|| {
            panic!("ATS reading order: '{heading}' not found in extracted text")
        });
        assert!(
            pos >= last_pos,
            "ATS reading order: '{heading}' (at {pos}) appeared before previous heading (at {last_pos})\n---\n{extracted}"
        );
        last_pos = pos;
    }
}

#[test]
fn render_opts_default_is_a4_en() {
    let opts = RenderOpts::default();
    assert_eq!(opts.page.width_mm, 210.0);
    assert_eq!(opts.page.height_mm, 297.0);
    assert_eq!(opts.lang, "en");
    assert!(!opts.ats);
    assert!(opts.accent.is_none());
}

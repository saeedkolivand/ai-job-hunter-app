//! Font-size / page-size / header-spacing invariants — the half-points-vs-dxa
//! regression class and the explicit name→contact paragraph spacing (#28).

use super::support::{all_font_sizes, build, part};
use crate::export::types::TemplateId;

/// The entry date run is italic (not bold) — matches the PDF path
/// (`single_column.typ`'s date-str run) so the duration reads as a
/// consistently distinguishable, fast-to-scan element across both export
/// formats, not just in the PDF.
#[test]
fn entry_date_run_is_italic_matching_pdf() {
    let bytes = build(TemplateId::Classic, false);
    let xml = part(&bytes, "word/document.xml");
    // "2020 - Present" is the right-aligned date run for the Acme Corp entry
    // (RESUME's legacy two-space format: `right_align_date` is true for
    // Classic's wide-flow layout). Find the run containing that text and
    // confirm it carries `<w:i/>`.
    let idx = xml
        .find("2020 - Present")
        .expect("date text must appear in document.xml");
    let run_start = xml[..idx].rfind("<w:r>").expect("enclosing run start");
    let run_end = idx + xml[idx..].find("</w:r>").expect("enclosing run end");
    let run_xml = &xml[run_start..run_end];
    assert!(
        run_xml.contains("<w:i "),
        "expected the date run to carry <w:i /> (italic); run xml: {run_xml:?}"
    );
    assert!(
        !run_xml.contains("<w:b "),
        "date run must stay non-bold; run xml: {run_xml:?}"
    );
}

#[test]
fn resume_docx_emits_half_point_sizes_not_dxa() {
    // SwissMinimal: name_pt 22.0, body_pt 10.5 (`templates/mod.rs`). The
    // regression this guards: routing font size through `pt_to_dxa` (×20)
    // instead of `pt_to_half_points` (×2) produced `w:sz w:val="440"`/`"210"` —
    // a 220pt/105pt name and body, the exact defect behind the 132-page export.
    let xml = part(&build(TemplateId::SwissMinimal, false), "word/document.xml");
    assert!(
        xml.contains(r#"w:sz w:val="44""#),
        "SwissMinimal name_pt 22.0 must emit w:sz w:val=\"44\" (half-points), not a dxa value: {xml}"
    );
    assert!(
        xml.contains(r#"w:sz w:val="21""#),
        "SwissMinimal body_pt 10.5 must emit w:sz w:val=\"21\" (half-points), not a dxa value: {xml}"
    );
}

#[test]
fn no_resume_font_size_exceeds_sane_ceiling() {
    // Independent of any one template's literal numbers: no half-point run size
    // should ever exceed 100 (50pt). This is the guard that would have caught
    // the pt_to_dxa/pt_to_half_points class outright — every real template's
    // pt sizes top out well under 40pt, so 50pt is a generous, stable ceiling.
    const MAX_HALF_POINTS: u32 = 100;
    for template_id in [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
        TemplateId::Atelier,
        TemplateId::Meridian,
        TemplateId::Throughline,
        TemplateId::Cadence,
        TemplateId::Regent,
        TemplateId::Jake,
        TemplateId::Awesome,
        TemplateId::Deedy,
    ] {
        for ats_mode in [false, true] {
            let xml = part(&build(template_id, ats_mode), "word/document.xml");
            for size in all_font_sizes(&xml) {
                assert!(
                    size <= MAX_HALF_POINTS,
                    "{template_id:?} (ats_mode={ats_mode}): w:sz={size} half-points ({}pt) exceeds \
                     the sane ceiling — likely a font size routed through pt_to_dxa instead of \
                     pt_to_half_points",
                    size as f32 / 2.0
                );
            }
        }
    }
}

#[test]
fn declares_a4_page_size_and_fallback_fonts() {
    // Academic: name/heading/body all SourceSerif4 → Georgia.
    let xml = part(&build(TemplateId::Academic, false), "word/document.xml");
    assert!(
        xml.contains(r#"w:w="11906""#) && xml.contains(r#"w:h="16838""#),
        "A4 page size"
    );
    assert!(
        xml.contains(r#"w:ascii="Georgia""#),
        "SourceSerif4 → Georgia"
    );
    let bundled = "Source Serif 4";
    assert!(
        !xml.contains(&format!(r#""{bundled}""#)),
        "bundled font {bundled:?} must not leak"
    );
}

/// #28 regression guard: the résumé DOCX header's candidate-name paragraph
/// used to carry NO explicit spacing at all, leaving the name→contact gap to
/// whatever Word's own default paragraph spacing happens to be. `add_header`
/// now sets `w:after="180"` (9pt = `pt_to_dxa(9.0)`, matching `_scale.typ`'s
/// `sp-name-below`) on that paragraph explicitly. Checked by locating the
/// `w:spacing` tag immediately preceding the name's own `w:t` run — DOCX has
/// no pixel geometry to measure (see this module's doc comment), so this
/// is the OOXML-part equivalent of the render-based checks in
/// `typst_engine::test`.
#[test]
fn resume_docx_header_name_paragraph_has_explicit_spacing_before_contact() {
    let xml = part(&build(TemplateId::SwissMinimal, false), "word/document.xml");
    let name_idx = xml
        .find(">Jane Doe<")
        .expect("candidate name run must be present in document.xml");
    let para_start = xml[..name_idx]
        .rfind("<w:p>")
        .or_else(|| xml[..name_idx].rfind("<w:p "))
        .expect("name run must sit inside a <w:p> paragraph");
    let para_head = &xml[para_start..name_idx];
    assert!(
        para_head.contains(r#"w:after="180""#),
        "the name paragraph must declare `w:after=\"180\"` (9pt) spacing \
         before the run reaches the contact line; paragraph head: {para_head}"
    );
}

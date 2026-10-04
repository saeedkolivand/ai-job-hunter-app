//! Two-column table / header-band / ATS-linearization invariants.

use super::support::{build, part, text_of};
use crate::export::types::TemplateId;

#[test]
fn two_column_renders_a_borderless_shaded_table() {
    let xml = part(&build(TemplateId::Atelier, false), "word/document.xml");
    assert!(xml.contains("<w:tbl"), "two-column DOCX must use a table");
    // Atelier sidebar tint (240,239,248) → F0EFF8 fill on the sidebar cell.
    assert!(
        xml.contains(r#"w:fill="F0EFF8""#),
        "sidebar cell should carry the Atelier tint"
    );
}

/// Awesome's PDF (`awesome.typ`) draws a full-width accent-tinted header band.
/// DOCX has no page-background primitive, so `add_header` approximates it with
/// the pairing the Banded cover-letter layout already established
/// (`docx::mod`'s `header_band` branch, pinned by
/// `cover_letter_docx_banded_shades_name_paragraph_and_uppercases`):
/// PARAGRAPH-level shading filled with the accent lightened 85 % toward white,
/// keeping the normal dark ink.
///
/// The shape this rules out is the inverse — run-level `w:shd` in the raw
/// accent behind hardcoded `FFFFFF` text. That tints only the glyph boxes, and
/// wherever run shading is not honoured the name is white on white: exactly the
/// hazard `awesome_matches_spec`'s `assert_ne!(name_color, white)` guards the
/// registry against, reintroduced in the renderer.
#[test]
fn awesome_name_paragraph_is_shaded_with_a_pale_tint_and_keeps_dark_ink() {
    let xml = part(&build(TemplateId::Awesome, false), "word/document.xml");

    // Accent #C41E3A lightened 85 % toward white → #F6DDE1
    // (196 + (255-196)*0.85 ≈ 246 = F6, 30 + 225*0.85 ≈ 221 = DD,
    //  58 + 197*0.85 ≈ 225 = E1 — `docx::band_tint_hex`).
    assert!(
        xml.contains(r#"w:fill="F6DDE1""#),
        "awesome header band must be the accent lightened 85% toward white: {xml}"
    );
    assert!(
        !xml.contains(r#"w:fill="C41E3A""#),
        "the raw accent is far too dark to read normal ink on — the band fill \
         must be the pale tint, not the accent itself: {xml}"
    );
    // Dark ink, never white: `name_color` is (26,26,26) → 1A1A1A.
    assert!(
        xml.contains(r#"w:color w:val="1A1A1A""#),
        "awesome name must keep the registry's dark ink: {xml}"
    );
    assert!(
        !xml.contains(r#"w:color w:val="FFFFFF""#),
        "white DOCX name text is invisible wherever the shading is dropped: {xml}"
    );

    // The shading must sit inside the paragraph properties (`w:pPr`), not in a
    // run — run-level `w:shd` tints only the glyph boxes, so it reads as a
    // highlighter stripe the width of the name rather than a header band.
    let ppr_open = xml.find("<w:pPr").expect("header paragraph must have pPr");
    let shd = xml.find("<w:shd").expect("header paragraph must be shaded");
    let ppr_close = xml.find("</w:pPr>").expect("pPr must close");
    assert!(
        ppr_open < shd && shd < ppr_close,
        "w:shd must be inside the first w:pPr (paragraph-level shading), not on a \
         run — got pPr@{ppr_open} shd@{shd} /pPr@{ppr_close}: {xml}"
    );

    // Control: a template with no band special-case must NOT gain shading —
    // this would fail if the `TemplateId::Awesome` branch leaked to everyone.
    let classic_xml = part(&build(TemplateId::Classic, false), "word/document.xml");
    assert!(
        !classic_xml.contains("w:shd"),
        "classic must not gain shading it never had: {classic_xml}"
    );
}

/// The band is decorative colour, so the ATS toggle drops it — the DOCX mirror
/// of the PDF's `awesome_ats_mode_drops_the_header_band_and_section_markers`.
/// `add_header` took no `ats_mode` at all, so an ATS-mode DOCX kept a band the
/// ATS-mode PDF had already dropped: the same request producing two documents
/// that disagree about what "ATS mode" means.
#[test]
fn awesome_ats_mode_drops_the_docx_header_band() {
    let ats_xml = part(&build(TemplateId::Awesome, true), "word/document.xml");
    assert!(
        !ats_xml.contains("w:shd"),
        "ATS mode must drop the Awesome header band entirely: {ats_xml}"
    );
    // …and the name must still be there, in dark ink.
    assert!(
        text_of(&ats_xml).contains("Jane Doe"),
        "dropping the band must not drop the name: {ats_xml}"
    );

    // Mutation guard: the non-ATS export of the very same fixture DOES shade,
    // so the assertion above is about `ats_mode`, not about shading never
    // being emitted.
    let banded_xml = part(&build(TemplateId::Awesome, false), "word/document.xml");
    assert!(
        banded_xml.contains("w:shd"),
        "non-ATS Awesome must still draw the band: {banded_xml}"
    );
}

#[test]
fn two_column_splits_sections_between_columns() {
    // SKILLS + EDUCATION are sidebar sections; EXPERIENCE is a main section.
    // All three must survive somewhere in the document.
    let text = text_of(&part(
        &build(TemplateId::Atelier, false),
        "word/document.xml",
    ));
    for needle in ["EXPERIENCE", "SKILLS", "EDUCATION", "Acme Corp", "Rust"] {
        assert!(text.contains(needle), "two-column lost content: {needle:?}");
    }
}

#[test]
fn ats_mode_emits_no_table() {
    // ATS mode linearizes to a single column — no two-column table.
    let xml = part(&build(TemplateId::Atelier, true), "word/document.xml");
    assert!(
        !xml.contains("<w:tbl"),
        "ATS mode must not emit a two-column table"
    );
    let text = text_of(&xml);
    for needle in ["EXPERIENCE", "SKILLS", "EDUCATION"] {
        assert!(
            text.contains(needle),
            "ATS linearization dropped {needle:?}"
        );
    }
}

#[test]
fn single_column_template_has_no_table() {
    let xml = part(&build(TemplateId::SwissMinimal, false), "word/document.xml");
    assert!(
        !xml.contains("<w:tbl"),
        "single-column template must not use a table"
    );
    let text = text_of(&xml);
    for needle in [
        "Jane Doe",
        "EXPERIENCE",
        "Rust, TypeScript, React",
        "BSc Computer Science",
    ] {
        assert!(
            text.contains(needle),
            "single-column lost content: {needle:?}"
        );
    }
}

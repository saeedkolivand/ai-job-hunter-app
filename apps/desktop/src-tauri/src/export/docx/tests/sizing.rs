//! Font-size / page-size / fallback-font / header-spacing invariants.

use super::support::{
    all_font_sizes, document_xml, letter_request, resume_request, REFINED_US_TEXT,
};
use crate::export::docx::generate_docx;
use crate::export::types::{DocumentType, ExportFormat, ExportRequest, LetterLayout, TemplateId};

#[test]
fn cover_letter_docx_emits_half_point_sizes_not_dxa() {
    // Classic template: name_pt 20.0, body_pt 10.5 (`templates/mod.rs`). The
    // regression this guards: routing font size through `pt_to_dxa` (×20)
    // instead of `pt_to_half_points` (×2) produced `w:sz w:val="400"`/`"210"` —
    // a 200pt/105pt name and body, the exact defect behind the 132-page export.
    let bytes = generate_docx(&letter_request(
        "Jane Doe\njane@example.com\n\nDear Hiring Manager,\n\nI am writing to apply.\n\nSincerely,\nJane Doe",
        LetterLayout::Classic,
    ))
    .expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:sz w:val="40""#),
        "Classic name_pt 20.0 must emit w:sz w:val=\"40\" (half-points), not a dxa value: {xml}"
    );
    assert!(
        xml.contains(r#"w:sz w:val="21""#),
        "Classic body_pt 10.5 must emit w:sz w:val=\"21\" (half-points), not a dxa value: {xml}"
    );
}

#[test]
fn no_cover_letter_font_size_exceeds_sane_ceiling() {
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
        for layout in [
            LetterLayout::Classic,
            LetterLayout::Refined,
            LetterLayout::Banded,
            LetterLayout::Navy,
            LetterLayout::Sidebar,
            LetterLayout::Monogram,
        ] {
            let request = ExportRequest {
                text: REFINED_US_TEXT.to_string(),
                format: ExportFormat::Docx,
                document_type: DocumentType::CoverLetter,
                template_id,
                meta: None,
                ats_mode: false,
                locale: None,
                contact: None,
                accent: None,
                letter_layout: layout,
            };
            let xml = document_xml(&generate_docx(&request).expect("docx"));
            for size in all_font_sizes(&xml) {
                assert!(
                    size <= MAX_HALF_POINTS,
                    "{template_id:?}/{layout:?}: w:sz={size} half-points ({}pt) exceeds the sane ceiling — \
                     likely a font size routed through pt_to_dxa instead of pt_to_half_points",
                    size as f32 / 2.0
                );
            }
        }
    }
}

#[test]
fn resume_docx_declares_a4_page_size() {
    let bytes = generate_docx(&resume_request(TemplateId::SwissMinimal)).expect("docx");
    let xml = document_xml(&bytes);
    // A4 in dxa, set explicitly from LocaleProfile rather than inherited.
    assert!(
        xml.contains(r#"w:w="11906""#) && xml.contains(r#"w:h="16838""#),
        "resume DOCX should declare an explicit A4 page size, got sectPr in: {xml}"
    );
}

#[test]
fn us_locale_drives_letter_page_size() {
    let mut request = resume_request(TemplateId::SwissMinimal);
    request.locale = Some("us".to_string());
    let xml = document_xml(&generate_docx(&request).expect("docx"));
    // US Letter in dxa (12240 × 15840), not the A4 default.
    assert!(
        xml.contains(r#"w:w="12240""#) && xml.contains(r#"w:h="15840""#),
        "US locale should yield a Letter page size"
    );

    // No locale → international A4.
    let a4 = document_xml(&generate_docx(&resume_request(TemplateId::SwissMinimal)).expect("docx"));
    assert!(a4.contains(r#"w:w="11906""#), "default stays A4");
}

#[test]
fn cover_letter_docx_declares_a4_page_size() {
    let request = ExportRequest {
        text: "Dear Hiring Manager,\n\nI am writing to apply.\n\nSincerely,\nJane Doe".to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_docx(&request).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:w="11906""#) && xml.contains(r#"w:h="16838""#),
        "cover-letter DOCX should declare an explicit A4 page size"
    );
}

#[test]
fn resume_docx_uses_fallback_fonts_not_bundled_names() {
    // Meridian: name/heading/body all Inter → Calibri.
    let bytes = generate_docx(&resume_request(TemplateId::Meridian)).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:ascii="Calibri""#),
        "Inter should fall back to Calibri"
    );
    // Both ranges are set so accented Latin renders in the same face.
    assert!(
        xml.contains(r#"w:hAnsi="Calibri""#),
        "fallback must also cover the high-ANSI range"
    );
    let bundled = "Inter";
    assert!(
        !xml.contains(&format!(r#""{bundled}""#)),
        "un-embedded bundled font {bundled:?} must not be referenced in the DOCX"
    );
}

#[test]
fn serif_and_display_templates_fall_back_predictably() {
    // Academic: Source Serif 4 → Georgia.
    let academic =
        document_xml(&generate_docx(&resume_request(TemplateId::Academic)).expect("docx"));
    assert!(
        academic.contains(r#"w:ascii="Georgia""#),
        "Source Serif 4 should fall back to Georgia"
    );
    assert!(
        !academic.contains(r#""Source Serif 4""#),
        "bundled Source Serif 4 must not leak"
    );

    // SwissMinimal: Manrope → Calibri.
    let swiss =
        document_xml(&generate_docx(&resume_request(TemplateId::SwissMinimal)).expect("docx"));
    assert!(
        swiss.contains(r#"w:ascii="Calibri""#),
        "Manrope should fall back to Calibri"
    );
    assert!(
        !swiss.contains(r#""Manrope""#),
        "bundled Manrope must not leak"
    );
}

/// #28 regression guard: the letterhead name paragraph used to carry only
/// `w:after="60"` (3pt) in both cover-letter DOCX renderers
/// (`generate_cover_letter_docx_classic` for `LetterLayout::Classic`,
/// `generate_cover_letter_docx_layout` for every other layout) — crammed
/// against the contact line below, the same shape as the PDF `letter_*.typ`
/// bug. Both now emit `w:after="180"` (9pt = `pt_to_dxa(9.0)`, matching
/// `_scale.typ`'s `sp-name-below`). Checked on one layout from each renderer.
#[test]
fn cover_letter_docx_name_paragraph_has_explicit_nine_point_spacing() {
    for layout in [LetterLayout::Classic, LetterLayout::Navy] {
        let xml =
            document_xml(&generate_docx(&letter_request(REFINED_US_TEXT, layout)).expect("docx"));
        // Navy (and Banded) uppercase the letterhead name (`uppercase_name`
        // in `LetterDocxStyle`), so search both cases and take whichever
        // occurs FIRST — the letterhead is always the earliest paragraph in
        // the document; a later, plain-cased "Jane Smith" also appears in
        // the signature block and would match the wrong paragraph.
        let name_idx = [">Jane Smith<", ">JANE SMITH<"]
            .into_iter()
            .filter_map(|needle| xml.find(needle))
            .min()
            .unwrap_or_else(|| panic!("{layout:?}: letterhead name run must be present"));
        let para_start = xml[..name_idx]
            .rfind("<w:p>")
            .or_else(|| xml[..name_idx].rfind("<w:p "))
            .unwrap_or_else(|| panic!("{layout:?}: name run must sit inside a <w:p> paragraph"));
        let para_head = &xml[para_start..name_idx];
        assert!(
            para_head.contains(r#"w:after="180""#),
            "{layout:?}: name paragraph must declare `w:after=\"180\"` (9pt) \
             spacing before the run reaches the contact line; paragraph head: {para_head}"
        );
    }
}

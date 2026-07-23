use std::io::{Cursor, Read};

use super::*;
use crate::export::types::{ExportFormat, LetterLayout, TemplateId};

/// Unzip a generated DOCX and return its `word/document.xml` (where the body
/// runs and the section's `pgSz` live).
fn document_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("docx is a zip archive");
    let mut file = zip
        .by_name("word/document.xml")
        .expect("docx contains word/document.xml");
    let mut xml = String::new();
    file.read_to_string(&mut xml).expect("read document.xml");
    xml
}

fn resume_request(template_id: TemplateId) -> ExportRequest {
    ExportRequest {
        // Name + contact + section + entry + bullet exercise name/heading/body fonts.
        text: "Jane Doe\njane@example.com\n\nEXPERIENCE\nAcme Corp  2020 - Present\nSenior Engineer\n- Built things that mattered".to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    }
}

/// Cover-letter request builder for the letter-layout DOCX tests (PR5).
fn letter_request(text: &str, layout: LetterLayout) -> ExportRequest {
    ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: layout,
    }
}

const REFINED_US_TEXT: &str = "Jane Smith\njane@example.com | https://linkedin.com/in/janesmith\n\nJune 2, 2025\n\nHiring Manager\nAcme Corp\n\nRe: Application for Platform Engineer (Ref PX-2291)\n\nDear Hiring Manager,\n\nI am writing to express my strong interest in the Platform Engineer position, bringing distributed systems experience.\n\nSincerely,\n\nJane Smith\nSoftware Engineer\n";

const REFINED_DE_TEXT: &str = "Max Müller\nmax@example.de | https://linkedin.com/in/maxmueller\n\nFrankfurt, 2. Juni 2025\n\nFrau Dr. Anna Weber\nMusterfirma GmbH\n\nBetreff: Bewerbung als Software Engineer\n\nSehr geehrte Frau Dr. Weber,\n\nmit großem Interesse habe ich Ihre Stellenausschreibung gelesen und bewerbe mich hiermit.\n\nMit freundlichen Grüßen,\n\nMax Müller\n";

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

#[test]
fn test_generate_simple_resume() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com\n\nEXPERIENCE\nSoftware Engineer  2020-2023".to_string(),
        format: super::super::types::ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };

    let result = generate_docx(&request);
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}

#[test]
fn test_extract_section_with_markers() {
    let text = "Header\n### START ###\nContent\n### END ###\nFooter";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "Content");
}

#[test]
fn test_extract_section_no_start() {
    let text = "Content\n### END ###\nFooter";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "Content\n### END ###\nFooter");
}

#[test]
fn test_extract_section_no_end() {
    let text = "Header\n### START ###\nContent\nMore";
    let result = extract_section(text, "### START ###", None);
    assert_eq!(result, "Content\nMore");
}

#[test]
fn test_extract_section_empty_text() {
    let text = "";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "");
}

#[test]
fn test_extract_section_no_markers() {
    let text = "Just some text";
    let result = extract_section(text, "NONEXISTENT", None);
    assert_eq!(result, "Just some text");
}

#[test]
fn test_generate_cover_letter() {
    let request = ExportRequest {
        text: "Dear Hiring Manager,\n\nI am writing to apply for the position.\n\nSincerely,\nJohn Doe".to_string(),
        format: super::super::types::ExportFormat::Docx,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };

    let result = generate_docx(&request);
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}

#[test]
fn document_accent_overrides_docx_emphasis_color() {
    use crate::export::docx_renderer::setup_colors;
    use crate::export::templates::Template;

    // The DOCX backend derives its emphasis color from the template's
    // `emphasis_color`, which `with_accent_override` recolors — so a document
    // accent surfaces on emphasized runs. Non-accent colors (e.g. section) stay put.
    let base = setup_colors(&Template::get(TemplateId::Classic));
    let accented =
        setup_colors(&Template::get(TemplateId::Classic).with_accent_override(Some("#AA0000")));
    assert_eq!(
        accented.emphasis, "AA0000",
        "accent must recolor DOCX emphasis"
    );
    assert_ne!(
        base.emphasis, accented.emphasis,
        "override must actually change the emphasis color"
    );
    assert_eq!(
        accented.section, base.section,
        "a non-accent color (section) must be untouched"
    );
}

#[test]
fn test_generate_resume_with_meta() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com".to_string(),
        format: super::super::types::ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: Some(GenerationMeta {
            candidate_name: Some("Jane Smith".to_string()),
            job_title: Some("Software Engineer".to_string()),
            company_name: Some("Test Corp".to_string()),
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };

    let result = generate_docx(&request);
    assert!(result.is_ok());
}

// ── PR5: Letter layout DOCX wiring ────────────────────────────────────────────
//
// `generate_cover_letter_docx` previously ignored `request.letter_layout`
// entirely, so a Banded/Refined choice never reached the DOCX export (a
// preview/export honesty violation). These tests lock in the fix.

#[test]
fn cover_letter_docx_classic_renders_and_omits_new_markup() {
    // Classic must stay on the untouched original renderer: no shading, no
    // extra paragraph borders introduced by the Refined/Banded wiring.
    let bytes =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Classic)).expect("docx");
    let xml = document_xml(&bytes);
    assert!(!xml.contains("w:shd"), "Classic must not carry any shading");
    assert!(
        !xml.contains("w:pBdr"),
        "Classic must not carry any paragraph borders"
    );
    assert!(
        xml.contains("Dear Hiring Manager") || xml.contains("Dear"),
        "Classic must still render the salutation"
    );
}

#[test]
fn cover_letter_docx_refined_right_aligns_contact_and_adds_bottom_border() {
    let bytes =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Refined)).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:jc w:val="right""#),
        "Refined must right-align the contact block: {xml}"
    );
    assert!(
        xml.contains("w:pBdr") && xml.contains("w:bottom"),
        "Refined must add a bottom-border rule under the header: {xml}"
    );
}

#[test]
fn cover_letter_docx_refined_shows_reference_line_from_subject_de() {
    // DE market: subject_line_label = "Betreff" — the market's own label, so
    // the caption is NOT suppressed and both the caption and the (label-
    // stripped) body must appear.
    let mut request = letter_request(REFINED_DE_TEXT, LetterLayout::Refined);
    request.locale = Some("de".to_string());
    let bytes = generate_docx(&request).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains("BETREFF"),
        "Refined DE must render the uppercase BETREFF caption: {xml}"
    );
    assert!(
        xml.contains("Bewerbung"),
        "Refined DE must render the (label-stripped) subject body: {xml}"
    );
}

#[test]
fn cover_letter_docx_refined_suppresses_redundant_reference_caption_us() {
    // US market: subject_line_label = "" but the text carries its own "Re:"
    // prefix — the caption must be suppressed to avoid "SUBJECT / Re: …".
    let mut request = letter_request(REFINED_US_TEXT, LetterLayout::Refined);
    request.locale = Some("us".to_string());
    let bytes = generate_docx(&request).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains("PX-2291"),
        "Refined US must still render the reference text itself: {xml}"
    );
    assert!(
        !xml.contains("SUBJECT"),
        "Refined US must suppress the redundant caption when the subject already opens with 'Re:': {xml}"
    );
}

#[test]
fn cover_letter_docx_banded_shades_name_paragraph_and_uppercases() {
    let bytes =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Banded)).expect("docx");
    let xml = document_xml(&bytes);
    // Classic's accent is #222222; lightened 85% toward white → #DEDEDE
    // (34 + (255-34)*0.85 ≈ 222 per channel — `lighten_rgb`).
    assert!(
        xml.contains("w:shd") && xml.contains(r#"w:fill="DEDEDE""#),
        "Banded must shade the name paragraph with the lightened accent: {xml}"
    );
    assert!(
        xml.contains("JANE SMITH"),
        "Banded must uppercase the candidate name: {xml}"
    );
}

#[test]
fn cover_letter_docx_banded_adds_right_aligned_contact_and_footer_border() {
    let bytes =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Banded)).expect("docx");
    let xml = document_xml(&bytes);
    assert!(
        xml.contains(r#"w:jc w:val="right""#),
        "Banded must right-align the contact block: {xml}"
    );
    assert!(
        xml.contains("w:pBdr") && xml.contains("w:bottom"),
        "Banded must add a bottom-border footer rule: {xml}"
    );
}

#[test]
fn cover_letter_docx_layouts_produce_distinct_bytes() {
    let classic =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Classic)).expect("classic");
    let refined =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Refined)).expect("refined");
    let banded =
        generate_docx(&letter_request(REFINED_US_TEXT, LetterLayout::Banded)).expect("banded");

    assert!(!classic.is_empty() && !refined.is_empty() && !banded.is_empty());
    assert_ne!(
        classic, refined,
        "Classic and Refined DOCX bytes must differ"
    );
    assert_ne!(classic, banded, "Classic and Banded DOCX bytes must differ");
    assert_ne!(refined, banded, "Refined and Banded DOCX bytes must differ");
}

/// A cover letter that opens directly at the salutation (no letterhead name/
/// contact lines) must keep its "Dear …" line and render the body normally.
///
/// The name block used to fire on the FIRST non-blank line whatever it was, so a
/// letterhead-less letter had its salutation consumed as the name (replaced with
/// `meta.candidate_name`) and, because `in_body` is set only in the salutation
/// arm, the whole body then rendered in the muted addressee style. Covers BOTH
/// letter renderers — `_classic` (Classic) and `_layout` (Refined/Banded).
#[test]
fn letterhead_less_letter_keeps_its_salutation_and_body() {
    for layout in [
        LetterLayout::Classic,
        LetterLayout::Refined,
        LetterLayout::Banded,
    ] {
        let request = ExportRequest {
            text: "Dear Hiring Manager,\n\nI am writing to apply for the role.\n\nSincerely,\nJane Smith".to_string(),
            format: ExportFormat::Docx,
            document_type: DocumentType::CoverLetter,
            template_id: TemplateId::Classic,
            // `candidate_name` is what the buggy name block substituted IN PLACE
            // of the salutation, so its presence makes the drop observable.
            meta: Some(GenerationMeta {
                candidate_name: Some("Jane Smith".to_string()),
                job_title: None,
                company_name: None,
                target_language: None,
            }),
            ats_mode: false,
            locale: None,
            contact: None,
            accent: None,
            letter_layout: layout,
        };
        let bytes = generate_docx(&request).expect("docx");
        let xml = document_xml(&bytes);
        assert!(
            xml.contains("Dear Hiring Manager"),
            "{layout:?}: the salutation was consumed as the letterhead name"
        );
    }
}

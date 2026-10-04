//! Smoke tests for the public entry point, the `extract_section` marker
//! helper, the document-accent override, and locale-driven section ordering.

use super::support::document_xml;
use crate::export::docx::{extract_section, generate_docx};
use crate::export::types::{
    DocumentType, ExportFormat, ExportRequest, GenerationMeta, LetterLayout, TemplateId,
};

/// Regression: a region-tagged locale (`de-DE`) reaching `ExportRequest` for
/// a DOCX résumé export must still resolve to the German `Lebenslauf`
/// section order (Certifications before Skills) — see the matching PDF-path
/// test in `export/pdf/test.rs` for the full rationale.
#[test]
fn ats_mode_resolves_region_tagged_german_locale_to_the_de_order_in_docx() {
    let request = ExportRequest {
        text: "\
Jane Doe
jane@example.com

EXPERIENCE
Acme Corp  2020 - Present
Did things

EDUCATION
Some University  2015 - 2019

CERTIFICATIONS
AWS Certified

SKILLS
Python, Rust"
            .to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: true,
        locale: Some("de-DE".to_string()),
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let xml = document_xml(&generate_docx(&request).expect("ats-mode german resume docx"));
    let cert = xml
        .find("CERTIFICATIONS")
        .expect("certifications section present");
    let skills = xml.find("SKILLS").expect("skills section present");
    assert!(
        cert < skills,
        "de-DE must resolve to the DE_ORDER (certifications before skills) \
         in the DOCX path too — document.xml: {xml}"
    );
}

#[test]
fn test_generate_simple_resume() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com\n\nEXPERIENCE\nSoftware Engineer  2020-2023".to_string(),
        format: ExportFormat::Docx,
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
        format: ExportFormat::Docx,
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

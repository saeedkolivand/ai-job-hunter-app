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
/// test in `export/pdf/tests/locale_and_market.rs` for the full rationale.
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

fn core_xml(bytes: &[u8]) -> String {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("docProps/core.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

/// Regression #1398: docx-rs defaults (1970 / "unknown") must not reach the file.
#[test]
fn core_properties_carry_the_candidate_and_the_current_time() {
    let mut request = super::support::resume_request(TemplateId::Classic);
    request.meta = Some(GenerationMeta {
        candidate_name: Some("Jane & Doe".into()),
        job_title: None,
        company_name: None,
        target_language: None,
    });
    let xml = core_xml(&generate_docx(&request).unwrap());
    assert!(
        xml.contains("<dc:creator>Jane &amp; Doe</dc:creator>"),
        "{xml}"
    );
    assert!(!xml.contains("1970") && !xml.contains("unknown"), "{xml}");
    let w3cdtf = regex::Regex::new(r#"W3CDTF">\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z<"#).unwrap();
    assert_eq!(w3cdtf.find_iter(&xml).count(), 2, "{xml}");

    request.meta = None;
    assert!(core_xml(&generate_docx(&request).unwrap()).contains("<dc:creator>AI Job Hunter<"));
}

#[test]
fn core_properties_creator_drops_xml_illegal_control_chars() {
    let xml = String::from_utf8(super::super::core_properties_xml("Jane\u{1}Doe", "t")).unwrap();
    assert!(xml.contains("<dc:creator>JaneDoe</dc:creator>"), "{xml}");
    // U+FFFE/U+FFFF are not XML Chars; an all-illegal name falls back to the default.
    let xml =
        String::from_utf8(super::super::core_properties_xml("A\u{FFFE}\u{FFFF}B", "t")).unwrap();
    assert!(xml.contains("<dc:creator>AB</dc:creator>"), "{xml}");
    let xml = String::from_utf8(super::super::core_properties_xml("\u{1}\u{FFFE}", "t")).unwrap();
    assert!(
        xml.contains("<dc:creator>AI Job Hunter</dc:creator>"),
        "{xml}"
    );
}

use std::io::{Cursor, Read};

use super::*;
use crate::export::types::{ExportFormat, TemplateId};

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
    }
}

#[test]
fn resume_docx_declares_a4_page_size() {
    let bytes = generate_docx(&resume_request(TemplateId::Modern)).expect("docx");
    let xml = document_xml(&bytes);
    // A4 in dxa, set explicitly from LocaleProfile rather than inherited.
    assert!(
        xml.contains(r#"w:w="11906""#) && xml.contains(r#"w:h="16838""#),
        "resume DOCX should declare an explicit A4 page size, got sectPr in: {xml}"
    );
}

#[test]
fn us_locale_drives_letter_page_size() {
    let mut request = resume_request(TemplateId::Modern);
    request.locale = Some("us".to_string());
    let xml = document_xml(&generate_docx(&request).expect("docx"));
    // US Letter in dxa (12240 × 15840), not the A4 default.
    assert!(
        xml.contains(r#"w:w="12240""#) && xml.contains(r#"w:h="15840""#),
        "US locale should yield a Letter page size"
    );

    // No locale → international A4.
    let a4 = document_xml(&generate_docx(&resume_request(TemplateId::Modern)).expect("docx"));
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
    // MonoTechnical: name/heading JetBrains Mono → Consolas, body Inter → Calibri.
    let bytes = generate_docx(&resume_request(TemplateId::MonoTechnical)).expect("docx");
    let xml = document_xml(&bytes);
    assert!(xml.contains(r#"w:ascii="Consolas""#), "JetBrains Mono should fall back to Consolas");
    assert!(xml.contains(r#"w:ascii="Calibri""#), "Inter should fall back to Calibri");
    // Both ranges are set so accented Latin renders in the same face.
    assert!(xml.contains(r#"w:hAnsi="Consolas""#), "fallback must also cover the high-ANSI range");
    for bundled in ["JetBrains Mono", "Inter"] {
        assert!(
            !xml.contains(&format!(r#""{bundled}""#)),
            "un-embedded bundled font {bundled:?} must not be referenced in the DOCX"
        );
    }
}

#[test]
fn serif_and_display_templates_fall_back_predictably() {
    // Academic: Source Serif 4 → Georgia.
    let academic = document_xml(&generate_docx(&resume_request(TemplateId::Academic)).expect("docx"));
    assert!(academic.contains(r#"w:ascii="Georgia""#), "Source Serif 4 should fall back to Georgia");
    assert!(!academic.contains(r#""Source Serif 4""#), "bundled Source Serif 4 must not leak");

    // RefinedExecutive: name Playfair Display → Cambria.
    let refined = document_xml(&generate_docx(&resume_request(TemplateId::RefinedExecutive)).expect("docx"));
    assert!(refined.contains(r#"w:ascii="Cambria""#), "Playfair Display should fall back to Cambria");
    assert!(!refined.contains(r#""Playfair Display""#), "bundled Playfair Display must not leak");

    // SwissMinimal: Manrope → Calibri.
    let swiss = document_xml(&generate_docx(&resume_request(TemplateId::SwissMinimal)).expect("docx"));
    assert!(swiss.contains(r#"w:ascii="Calibri""#), "Manrope should fall back to Calibri");
    assert!(!swiss.contains(r#""Manrope""#), "bundled Manrope must not leak");
}

#[test]
fn test_generate_simple_resume() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com\n\nEXPERIENCE\nSoftware Engineer  2020-2023".to_string(),
        format: super::super::types::ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Modern,
        meta: None,
        ats_mode: false,
        locale: None,
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
    };

    let result = generate_docx(&request);
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}

#[test]
fn test_generate_resume_with_meta() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com".to_string(),
        format: super::super::types::ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Executive,
        meta: Some(GenerationMeta {
            candidate_name: Some("Jane Smith".to_string()),
            job_title: Some("Software Engineer".to_string()),
            company_name: Some("Test Corp".to_string()),
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
    };

    let result = generate_docx(&request);
    assert!(result.is_ok());
}

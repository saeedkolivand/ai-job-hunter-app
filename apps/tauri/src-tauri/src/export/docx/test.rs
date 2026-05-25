use super::*;
use crate::export::types::TemplateId;

#[test]
fn test_generate_simple_resume() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com\n\nEXPERIENCE\nSoftware Engineer  2020-2023".to_string(),
        format: super::super::types::ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Modern,
        meta: None,
        ats_mode: false,
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
    };

    let result = generate_docx(&request);
    assert!(result.is_ok());
}

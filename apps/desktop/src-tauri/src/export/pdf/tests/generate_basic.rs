use crate::export::pdf::generate_pdf;
use crate::export::types::{
    DocumentType, ExportFormat, ExportRequest, GenerationMeta, LetterLayout, TemplateId,
};

#[test]
fn test_generate_pdf_resume_basic() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_cover_letter_basic() {
    let request = ExportRequest {
        text: "Dear Hiring Manager,\n\nI am writing to apply...".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_resume_with_meta() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com".to_string(),
        format: ExportFormat::Pdf,
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
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_resume_with_section_markers() {
    let text = "### CANDIDATE RESUME ###\nJohn Doe\njohn@example.com\n### JOB ADVERTISEMENT ###\nJob description here";
    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_cover_letter_with_section_markers() {
    let text =
        "Some header\n### COMPLETE COVER LETTER ###\nDear Hiring Manager,\n\nI am writing...";
    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

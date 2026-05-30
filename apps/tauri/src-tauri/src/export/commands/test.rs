use super::*;

#[test]
fn test_sanitize_filename() {
    assert_eq!(sanitize_filename("John Doe"), "John-Doe");
    assert_eq!(sanitize_filename("John@Doe!"), "JohnDoe");
    assert_eq!(sanitize_filename("  Spaces  "), "Spaces");
}

#[test]
fn test_generate_filename() {
    use super::super::types::{DocumentType, ExportFormat, GenerationMeta, TemplateId};

    let request = ExportRequest {
        text: "Test".to_string(),
        format: ExportFormat::Docx,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Modern,
        meta: Some(GenerationMeta {
            candidate_name: Some("John Doe".to_string()),
            job_title: Some("Software Engineer".to_string()),
            company_name: Some("Tech Corp".to_string()),
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
    };

    let filename = generate_filename(&request, "docx");
    assert!(filename.contains("John-Doe"));
    assert!(filename.contains("Software-Engineer"));
    assert!(filename.contains("resume"));
    assert!(filename.ends_with(".docx"));
}

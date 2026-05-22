use tauri::command;

use super::{
    docx::generate_docx,
    pdf::generate_pdf,
    types::{ExportFormat, ExportRequest, ExportResult},
};

/// Tauri command to export resume or cover letter
#[command]
pub async fn export_document(request: ExportRequest) -> Result<ExportResult, String> {
    // Validate input
    if request.text.trim().is_empty() {
        return Err("Cannot export empty document. Please generate content first.".to_string());
    }

    // Generate based on format
    let (data, mime_type, extension) = match request.format {
        ExportFormat::Docx => {
            let bytes = generate_docx(&request)
                .map_err(|e| format!("DOCX generation failed: {}", e))?;
            (
                bytes,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
                "docx",
            )
        }
        ExportFormat::Pdf => {
            let bytes = generate_pdf(&request)
                .map_err(|e| format!("PDF generation failed: {}", e))?;
            (bytes, "application/pdf".to_string(), "pdf")
        }
        ExportFormat::Txt => {
            let text = super::parser::strip_md(&request.text);
            (text.into_bytes(), "text/plain".to_string(), "txt")
        }
    };

    // Generate filename
    let filename = generate_filename(&request, extension);

    Ok(ExportResult {
        data,
        mime_type,
        filename,
    })
}

/// Generate filename from metadata
fn generate_filename(request: &ExportRequest, extension: &str) -> String {
    let name = request
        .meta
        .as_ref()
        .and_then(|m| m.candidate_name.as_ref())
        .map(|s| sanitize_filename(s))
        .unwrap_or_else(|| "Candidate".to_string());

    let role = request
        .meta
        .as_ref()
        .and_then(|m| m.job_title.as_ref())
        .map(|s| sanitize_filename(s))
        .unwrap_or_else(|| "Role".to_string());

    let company = request
        .meta
        .as_ref()
        .and_then(|m| m.company_name.as_ref())
        .map(|s| sanitize_filename(s))
        .unwrap_or_else(|| "Company".to_string());

    let doc_type = match request.document_type {
        super::types::DocumentType::Resume => "resume",
        super::types::DocumentType::CoverLetter => "cover-letter",
    };

    format!("{}-{}-{}-{}.{}", name, role, company, doc_type, extension)
}

/// Sanitize filename (remove invalid characters)
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .collect::<String>()
        .trim()
        .replace(' ', "-")
        .chars()
        .take(40)
        .collect()
}

#[cfg(test)]
mod tests {
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
        };

        let filename = generate_filename(&request, "docx");
        assert!(filename.contains("John-Doe"));
        assert!(filename.contains("Software-Engineer"));
        assert!(filename.contains("resume"));
        assert!(filename.ends_with(".docx"));
    }
}

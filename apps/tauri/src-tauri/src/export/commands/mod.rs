use tauri::command;
use tauri_plugin_dialog::DialogExt;

use super::{
    docx::generate_docx,
    pdf::generate_pdf,
    types::{ExportFormat, ExportRequest, ExportResult},
};

/// Tauri command to export resume or cover letter
#[command]
pub async fn documents_export_document(request: ExportRequest) -> Result<ExportResult, String> {
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

/// Tauri command to export and save document with file dialog
#[command]
pub async fn documents_export_and_save(
    app: tauri::AppHandle,
    request: ExportRequest,
) -> Result<String, String> {
    // Generate the document
    let result = documents_export_document(request).await?;

    // Extract extension for filter
    let ext = result.filename.split('.').last().unwrap_or("*").to_string();
    let filter_name = format!("{} files", ext.to_uppercase());

    // Open save dialog (blocking)
    let file_path = app
        .dialog()
        .file()
        .add_filter(&filter_name, &[&ext])
        .set_title("Save Document")
        .set_file_name(&result.filename)
        .blocking_save_file()
        .ok_or_else(|| "Save dialog was cancelled".to_string())?;

    // Resolve to PathBuf
    let path = match file_path {
        tauri_plugin_dialog::FilePath::Path(p) => p,
        #[allow(unreachable_patterns)]
        _ => return Err("Unsupported file path type".to_string()),
    };

    // Write bytes to file
    std::fs::write(&path, result.data)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(path.to_string_lossy().to_string())
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
mod test;

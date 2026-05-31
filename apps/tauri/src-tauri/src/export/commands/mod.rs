use tauri::command;
use tauri_plugin_dialog::DialogExt;

use super::{
    docx::generate_docx,
    pdf::generate_pdf,
    types::{ExportFormat, ExportRequest, ExportResult},
};
use crate::error::{AppError, AppResult};
use crate::validate::{validate_and_fix, ExportReport, Severity};

/// Tauri command to export resume or cover letter
#[command]
pub async fn documents_export_document(mut request: ExportRequest) -> AppResult<ExportResult> {
    // Validate input
    if request.text.trim().is_empty() {
        return Err(AppError::Validation(
            "Cannot export empty document. Please generate content first.".to_string(),
        ));
    }

    // Normalize Unicode before any rendering so unsupported glyphs don't appear
    // as replacement boxes in PDF/DOCX output.
    request.text = super::parser::normalize_unicode(&request.text);

    // Generate based on format. PDF/DOCX run through the validation gate, which
    // re-extracts the bytes, auto-fixes a two-column layout that doesn't survive
    // extraction, and reports what it found.
    let (data, mime_type, extension, report) = match request.format {
        ExportFormat::Docx => {
            let (bytes, report) = validate_and_fix(request.clone(), generate_docx)
                .map_err(|e| format!("DOCX generation failed: {}", e))?;
            (
                bytes,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(),
                "docx",
                Some(report),
            )
        }
        ExportFormat::Pdf => {
            let (bytes, report) = validate_and_fix(request.clone(), generate_pdf)
                .map_err(|e| format!("PDF generation failed: {}", e))?;
            (bytes, "application/pdf".to_string(), "pdf", Some(report))
        }
        ExportFormat::Txt => {
            let text = super::parser::strip_md(&request.text);
            (text.into_bytes(), "text/plain".to_string(), "txt", None)
        }
    };

    // Block only when a critical defect survived auto-fix.
    if let Some(report) = &report {
        if !report.ok {
            return Err(AppError::Validation(blocking_reason(report)));
        }
    }

    // Generate filename
    let filename = generate_filename(&request, extension);

    Ok(ExportResult {
        data,
        mime_type,
        filename,
        report,
    })
}

/// Plain-language reason an export was blocked, from its critical issues.
fn blocking_reason(report: &ExportReport) -> String {
    let reasons = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .map(|i| i.message.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if reasons.is_empty() {
        "Export blocked: the document failed validation.".to_string()
    } else {
        format!("Export blocked: {reasons}")
    }
}

/// Tauri command to export and save document with file dialog
#[command]
pub async fn documents_export_and_save(
    app: tauri::AppHandle,
    request: ExportRequest,
) -> AppResult<String> {
    // Generate the document
    let result = documents_export_document(request).await?;

    // Extract extension for filter
    let ext = result
        .filename
        .split('.')
        .next_back()
        .unwrap_or("*")
        .to_string();
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
        _ => return Err(AppError::Message("Unsupported file path type".to_string())),
    };

    // Write bytes to file
    std::fs::write(&path, result.data).map_err(|e| format!("Failed to write file: {}", e))?;

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

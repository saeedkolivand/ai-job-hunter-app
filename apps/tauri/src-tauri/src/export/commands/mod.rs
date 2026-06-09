use tauri::command;
use tauri_plugin_dialog::DialogExt;

use super::{
    docx::generate_docx,
    pdf::{generate_pdf, generate_preview_svg},
    types::{ExportFormat, ExportRequest, ExportResult, PreviewResult},
};
use crate::error::{AppError, AppResult};
use crate::validate::{validate_and_fix, ExportReport, Severity};

/// MIME type for every page string returned by the SVG live-preview path.
const SVG_MIME: &str = "image/svg+xml";

/// Reject empty input and run the same text-normalization passes every export
/// path uses (Unicode normalize → strip stray Markdown → dash typography), so
/// unsupported glyphs never appear as replacement boxes and no `*` / backtick or
/// mangled sentence-break dash reaches the page.
///
/// Shared by [`documents_export_document`] and
/// [`documents_render_preview_images`] so the preview is rendered from the EXACT
/// same validated + normalized request the export uses. The serde-tolerant
/// `TemplateId` fallback (unknown id → Classic) is applied during deserialization
/// of [`ExportRequest`] itself, so it covers both commands automatically.
fn validate_and_normalize(request: &mut ExportRequest) -> AppResult<()> {
    if request.text.trim().is_empty() {
        return Err(AppError::Validation(
            "Cannot export empty document. Please generate content first.".to_string(),
        ));
    }

    request.text = super::parser::normalize_unicode(&request.text);
    request.text = super::parser::sanitize_markdown(&request.text);
    request.text = super::parser::typography(&request.text);
    Ok(())
}

/// Tauri command to export resume or cover letter
#[command]
pub async fn documents_export_document(mut request: ExportRequest) -> AppResult<ExportResult> {
    // Validate + normalize input (empty-text guard + Unicode/Markdown/typography
    // passes). Shared with the preview command so they stay in lock-step.
    validate_and_normalize(&mut request)?;

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

/// Tauri command: render a résumé / cover letter to per-page SVG strings for the
/// live preview (shown via `<img>`), instead of producing downloadable bytes.
///
/// Accepts the SAME [`ExportRequest`] fields as [`documents_export_document`]
/// (`text`, `documentType`, `templateId`, `atsMode`, `locale`, `contact`,
/// `meta`; `format` is ignored — the preview always emits SVG) and reuses the
/// EXACT same input validation + normalization ([`validate_and_normalize`]) and
/// the serde-tolerant `TemplateId` fallback, so this new IPC surface is no looser
/// than export. The render itself goes through [`generate_preview_svg`], which
/// builds the identical model + Typst world as the PDF path — only the final emit
/// differs (SVG per page vs one PDF blob), so preview fidelity matches export.
///
/// Unlike the export command this does NOT run the `validate/` round-trip gate:
/// that gate re-extracts PDF *bytes* (it cannot read SVG) and exists to block a
/// bad *download*. A preview must always show the user's chosen layout; the
/// download path keeps the authoritative ATS/round-trip gate.
#[command]
pub async fn documents_render_preview_images(
    mut request: ExportRequest,
) -> AppResult<PreviewResult> {
    // Same empty-text guard + normalization passes as export.
    validate_and_normalize(&mut request)?;

    let pages = generate_preview_svg(&request)
        .map_err(|e| AppError::Message(format!("Preview rendering failed: {e}")))?;

    // The engine guards against a zero-page document, but assert at the command
    // boundary too so a future regression can't return an empty preview.
    if pages.is_empty() {
        return Err(AppError::Message(
            "Preview rendering produced no pages.".to_string(),
        ));
    }

    Ok(PreviewResult {
        pages,
        mime_type: SVG_MIME.to_string(),
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

    // Run the native save dialog OFF the async-runtime worker. `blocking_save_file()`
    // blocks its caller until the dialog closes; calling it directly inside this
    // `async` command stalls the runtime and dead-locks on a *subsequent* export —
    // the dialog never reappears and the invoke never resolves (the "spinner spins
    // forever on the 2nd export" symptom). `spawn_blocking` keeps the wait on a
    // dedicated blocking thread so repeat exports work.
    let filename = result.filename.clone();
    let file_path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter(&filter_name, &[&ext])
            .set_title("Save Document")
            .set_file_name(&filename)
            .blocking_save_file()
    })
    .await
    .map_err(|e| format!("Save dialog failed: {e}"))?
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

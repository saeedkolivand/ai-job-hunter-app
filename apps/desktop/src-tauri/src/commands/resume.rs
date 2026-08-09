//! Résumé extraction + content-quality-check IPC surface.
//!
//! Thin shell wrapper over [`crate::extraction`] and [`crate::validate::content`].
//! Both are Tauri-free (L1); keeping the `#[tauri::command]`s here makes the
//! shell layer the sole owner of command definitions (see
//! docs/architecture-rules.md R1).

use crate::applications::{clamp_to_bytes, MAX_JOB_DESCRIPTION_BYTES};
use crate::error::AppResult;
use crate::extraction::{self, types::ExtractedResume};
use crate::ipc_contracts::resume::ResumeValidateContentRequest;
use crate::observability::Span;
use crate::validate::content::{validate_content, ContentInput, ContentReport, DocKind};

/// Extract plain text + structured fields from a résumé file
/// (PDF/DOCX/TXT/RTF/HTML). Routes to the data-driven extractor registry.
#[tauri::command]
pub async fn extract_resume(path: String) -> AppResult<ExtractedResume> {
    extraction::extract_resume(path).await
}

/// A hostile/buggy direct IPC caller could otherwise hand the analyzer an
/// unbounded requirements list; the renderer's own JD-analysis step never
/// produces more than a handful. Mirrors the item-count caps used elsewhere
/// (e.g. `AutopilotTargetSchema.boards`).
const TOP_REQUIREMENTS_CAP: usize = 50;
/// Per-requirement byte cap — matches the Zod schema's `.max(300)` (renderer-side
/// only; this is the server-side mirror).
const TOP_REQUIREMENT_BYTES_CAP: usize = 300;

/// Deterministic content-quality checks (factual accuracy, ATS structure,
/// AI-voice tells) on an already-generated résumé/letter against its source
/// résumé and the job ad. Pure and fast — no AI call — so it's safe to run on
/// every save, not just on demand.
///
/// Server-side clamp mirrors the Zod caps (renderer-side only, see
/// `ResumeValidateContentSchema`): a direct IPC caller could otherwise hand the
/// analyzer unbounded text, the same trust boundary `resume_trim_suggestions`
/// and `applications_track`/`applications_update` enforce for résumé/job text.
#[tauri::command]
pub async fn resume_validate_content(
    req: ResumeValidateContentRequest,
) -> AppResult<ContentReport> {
    let span = Span::begin(
        "resume",
        format!(
            "validate_content kind={} requirements={}",
            req.doc_kind,
            req.top_requirements.len()
        ),
    );
    let generated = clamp_to_bytes(req.generated, MAX_JOB_DESCRIPTION_BYTES);
    let source = clamp_to_bytes(req.source, MAX_JOB_DESCRIPTION_BYTES);
    let job_ad = clamp_to_bytes(req.job_ad, MAX_JOB_DESCRIPTION_BYTES);
    let top_requirements: Vec<String> = req
        .top_requirements
        .into_iter()
        .take(TOP_REQUIREMENTS_CAP)
        .map(|r| clamp_to_bytes(r, TOP_REQUIREMENT_BYTES_CAP))
        .collect();
    // Wire form is exactly "resume" | "coverLetter" (`DocKind`'s `camelCase`
    // serde rename); an unrecognized value degrades to the more common case
    // rather than erroring, matching the tolerant-string convention used for
    // `mode`/`board` elsewhere on this IPC surface.
    let doc_kind = if req.doc_kind == "coverLetter" {
        DocKind::CoverLetter
    } else {
        DocKind::Resume
    };

    let report = validate_content(&ContentInput {
        generated: &generated,
        source_resume: &source,
        job_ad: &job_ad,
        top_requirements: &top_requirements,
        target_language: &req.target_language,
        doc_kind,
    });
    // Counts only — never résumé/job-ad/issue content in the span (ADR-027).
    span.end_with(
        &format!("issues={} ok={}", report.issues.len(), report.ok),
        true,
    );
    Ok(report)
}

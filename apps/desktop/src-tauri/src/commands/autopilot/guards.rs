//! Save-time and run-time guards for an autopilot's `dateFilter` and résumé text (#1492, #1493).

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::scraping::types::{is_known_date_filter, DATE_FILTER_OPTIONS};

/// Refuse a `dateFilter` outside the shared vocabulary instead of storing a value that silently
/// means "no date filter". `None` (any time) is valid.
pub(super) fn check_date_filter(date_filter: Option<&str>) -> AppResult<()> {
    match date_filter {
        Some(v) if !is_known_date_filter(v) => Err(AppError::Validation(format!(
            "invalid dateFilter; allowed values: {}",
            DATE_FILTER_OPTIONS.join(", ")
        ))),
        _ => Ok(()),
    }
}

/// The résumé text a run scores against: the autopilot's own when non-blank, else the default
/// document's text. `None` when neither has any text.
pub(super) fn effective_resume(own: Option<&str>, default_doc: Option<&str>) -> Option<String> {
    [own, default_doc]
        .into_iter()
        .flatten()
        .find(|t| !t.trim().is_empty())
        .map(str::to_owned)
}

fn default_document_text(app: &AppHandle) -> Option<String> {
    app.try_state::<crate::documents::DocumentStore>()?
        .default_document()
        .map(|d| d.text)
}

/// Resolve the résumé text for a run (see [`effective_resume`]).
pub(super) fn resume_for_run(app: &AppHandle, own: Option<&str>) -> Option<String> {
    effective_resume(own, default_document_text(app).as_deref())
}

/// Warning for the create/update reply when a run would have no résumé to score against.
/// Skips the default-document read when the autopilot has its own text.
pub(super) fn resume_warning(app: &AppHandle, own: Option<&str>) -> Option<&'static str> {
    if own.is_some_and(|t| !t.trim().is_empty()) {
        return None;
    }
    default_document_text(app)
        .is_none_or(|t| t.trim().is_empty())
        .then_some(NO_RESUME_WARNING)
}

pub(super) const NO_RESUME_WARNING: &str = "No resume text and no default document: runs will      score every job against an empty resume. Set resumeText or upload a default document.";

#[cfg(test)]
mod tests;

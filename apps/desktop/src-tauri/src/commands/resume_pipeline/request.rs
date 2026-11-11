//! `resume_pipeline_run`'s synchronous gate: what a request must satisfy BEFORE ids are
//! handed back (#1496).
//!
//! The Zod schema's `.refine`s never run on this transport and generated Rust cannot express
//! them, so an agent or direct-IPC caller could start a run that was doomed from the first
//! byte, get ids, and see it fail out of sight. Mirrors `commands::autopilot::guards`, which
//! closes the same gap for the autopilot contract.

use crate::autopilot::{Autopilot, FoundJob};
use crate::error::{AppError, AppResult};
use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;

fn blank(s: &str) -> bool {
    s.trim().is_empty()
}

/// The three `.refine`s of `ResumePipelineRunSchema`, plus `jobUrl` as a third way to name the
/// posting (resolved by [`resolve_found_job`]).
pub(super) fn validate(req: &ResumePipelineRunRequest) -> AppResult<()> {
    if blank(&req.resume_id) && blank(&req.resume_text) {
        return Err(AppError::Validation(
            "either resumeId or resumeText is required".to_string(),
        ));
    }
    if blank(&req.job_id) && blank(&req.job_ad_text) && blank(&req.job_url) {
        return Err(AppError::Validation(
            "either jobId, jobAdText or jobUrl is required".to_string(),
        ));
    }
    if !req.include_resume && !req.include_cover_letter {
        return Err(AppError::Validation(
            "a run must produce at least one document".to_string(),
        ));
    }
    Ok(())
}

/// Whether the posting must be looked up by `jobUrl`: only when neither an id nor text names it
/// (id wins over text wins over url, so nothing already working changes).
pub(super) fn needs_found_job(req: &ResumePipelineRunRequest) -> bool {
    blank(&req.job_id) && blank(&req.job_ad_text)
}

/// Fill the text path from the found job `req.job_url` names: its description becomes
/// `jobAdText`, and title/company/board fill whatever the caller left blank. A URL that matches
/// no found job — or one whose stored row has no description to write from — is refused by
/// name, never silently run against nothing.
pub(super) fn resolve_found_job(
    req: &mut ResumePipelineRunRequest,
    autopilots: &[Autopilot],
) -> AppResult<()> {
    let wanted = crate::applications::normalize_job_url(&req.job_url);
    if wanted.is_empty() {
        return Err(AppError::Validation(
            "jobUrl is not a valid http(s) link".to_string(),
        ));
    }
    let described = |j: &&FoundJob| j.description.as_deref().is_some_and(|d| !blank(d));
    let all = || autopilots.iter().flat_map(|a| a.found_jobs.iter());
    // An exact spelling wins. The normalizer can collapse two postings that differ only by a
    // query-string id on a host outside its allowlist, so a normalized-only match that spans
    // more than one distinct stored URL is ambiguous and refused rather than guessed.
    let exact_url = req.job_url.trim();
    let mut candidates: Vec<&FoundJob> = all().filter(|j| j.url == exact_url).collect();
    if candidates.is_empty() {
        candidates = all()
            .filter(|j| crate::applications::normalize_job_url(&j.url) == wanted)
            .collect();
        let distinct: std::collections::HashSet<&str> =
            candidates.iter().map(|j| j.url.as_str()).collect();
        if distinct.len() > 1 {
            return Err(AppError::Validation(
                "jobUrl matches more than one found job; pass jobId to pick one".to_string(),
            ));
        }
    }
    // The same posting can sit in several autopilots; take the one with the most text.
    let found = candidates
        .iter()
        .copied()
        .filter(described)
        .max_by_key(|j| j.description.as_deref().map_or(0, str::len))
        .ok_or_else(|| {
            AppError::Validation(if candidates.is_empty() {
                "jobUrl does not match any found job; pass jobId or jobAdText".to_string()
            } else {
                "the found job has no stored description; pass jobAdText".to_string()
            })
        })?;
    req.job_ad_text = found.description.clone().unwrap_or_default();
    if blank(&req.job_title) {
        req.job_title = found.title.clone();
    }
    if blank(&req.company_name) {
        req.company_name = found.company.clone();
    }
    if blank(&req.board) {
        req.board = found.board.clone().unwrap_or_default();
    }
    Ok(())
}

/// [`validate`], then the `jobUrl` lookup when it applies. The one entry `resume_pipeline_run`
/// calls before it mints ids.
pub(super) fn admit(
    app: &tauri::AppHandle,
    mut req: ResumePipelineRunRequest,
) -> AppResult<ResumePipelineRunRequest> {
    use tauri::Manager;
    validate(&req)?;
    if needs_found_job(&req) {
        let store = app
            .try_state::<std::sync::Arc<parking_lot::Mutex<crate::autopilot::AutopilotStore>>>()
            .ok_or_else(|| AppError::Config("the found-jobs store is not available".to_string()))?;
        let autopilots = store.lock().list();
        resolve_found_job(&mut req, &autopilots)?;
    }
    Ok(req)
}

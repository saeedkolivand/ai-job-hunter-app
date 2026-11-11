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
    let matching = || {
        autopilots
            .iter()
            .flat_map(|a| a.found_jobs.iter())
            .filter(|j| crate::applications::normalize_job_url(&j.url) == wanted)
    };
    // A row can match and still be unusable: it is held for an identity review (it may be a
    // different posting than the url says), or its stored text is the truncated snippet the
    // provisional score was computed over, not the full description.
    let usable = |j: &&FoundJob| {
        j.identity_review.is_none()
            && !j.score_provisional
            && j.description.as_deref().is_some_and(|d| !blank(d))
    };
    // An exact spelling wins. The normalizer can collapse two postings that differ only by a
    // query-string id on a host outside its allowlist, so a normalized-only match that spans
    // more than one distinct stored URL is ambiguous and refused rather than guessed.
    let exact_url = req.job_url.trim();
    let mut candidates: Vec<&FoundJob> = matching()
        .filter(|j| j.url == exact_url)
        .filter(usable)
        .collect();
    if candidates.is_empty() {
        candidates = matching().filter(usable).collect();
        let distinct: std::collections::HashSet<&str> =
            candidates.iter().map(|j| j.url.as_str()).collect();
        if distinct.len() > 1 {
            return Err(AppError::Validation(
                "jobUrl matches more than one found job; pass jobId to pick one".to_string(),
            ));
        }
    }
    // The same posting can sit in several autopilots: the most text wins, then the most recently
    // found, then the url — a total order, so the pick never depends on store iteration order.
    let found = candidates
        .iter()
        .copied()
        .max_by(|a, b| {
            let len = |j: &FoundJob| j.description.as_deref().map_or(0, str::len);
            (len(a), a.found_at, &a.url).cmp(&(len(b), b.found_at, &b.url))
        })
        .ok_or_else(|| AppError::Validation(refusal(matching())))?;
    // A stored row that arrived entity-escaped (#1497) is decoded once here, before any prompt.
    let stored = found.description.as_deref().unwrap_or_default();
    req.job_ad_text = if crate::scraping::http::has_escaped_markup(stored) {
        crate::scraping::http::html_to_markdown(stored)
    } else {
        stored.to_string()
    };
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

/// Why no matching found job was usable, naming the reason so the caller knows what to pass.
fn refusal<'a>(matches: impl Iterator<Item = &'a FoundJob>) -> String {
    let rows: Vec<&FoundJob> = matches.collect();
    if rows.is_empty() {
        "jobUrl does not match any found job; pass jobId or jobAdText".to_string()
    } else if rows.iter().all(|j| j.identity_review.is_some()) {
        "the found job is held for an identity review and may be a different posting; resolve it          or pass jobAdText"
            .to_string()
    } else if rows.iter().any(|j| j.score_provisional) {
        "the found job only has a truncated description; pass jobAdText".to_string()
    } else {
        "the found job has no stored description; pass jobAdText".to_string()
    }
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

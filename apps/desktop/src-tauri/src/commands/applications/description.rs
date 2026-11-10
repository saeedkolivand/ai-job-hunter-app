//! The ad text a Saved draft is created with (#1450).
//!
//! The renderer's Apply passes no description for a posting that lives only in
//! the persisted autopilot found jobs, so the backend resolves it by the same
//! identity rules as the `job` resource (#1166). Boilerplate is never persisted.

use crate::applications::{decode_unreserved, normalize_job_url};
use crate::autopilot::Autopilot;
use crate::documents::keywords::description_is_blank;
use crate::scraping::scrape_url::job_identity;

/// The description to store for a new/merged draft of `job_url`: the request's
/// own text when usable, else the matching found job's. Empty when neither is
/// usable, or when the existing row already has one (`stored`) — a re-save must
/// never replace user edits.
pub(super) fn draft_description(
    requested: Option<String>,
    job_url: &str,
    records: &[Autopilot],
    stored: &str,
) -> String {
    if !stored.trim().is_empty() {
        return String::new();
    }
    let usable = |d: &Option<String>| !description_is_blank(d.as_deref());
    if usable(&requested) {
        return requested.unwrap_or_default();
    }
    let decoded = decode_unreserved(job_url);
    let identity = job_identity(&decoded);
    let normalized = normalize_job_url(&decoded);
    records
        .iter()
        .flat_map(|ap| ap.found_jobs.iter())
        .find(|j| {
            let theirs = decode_unreserved(&j.url);
            (identity.is_some() && job_identity(&theirs) == identity)
                || (!normalized.is_empty() && normalize_job_url(&theirs) == normalized)
        })
        .filter(|j| usable(&j.description))
        .and_then(|j| j.description.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;

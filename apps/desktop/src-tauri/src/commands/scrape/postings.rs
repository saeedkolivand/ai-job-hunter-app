//! The live postings cache: list, clear and the description write-back. Split out
//! of `commands/scrape.rs` for R8 (issue #1280); `scrape.rs` re-exports the
//! commands, so each keeps its `commands::scrape::<name>` path.

use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::postings::{attach_interactions, InteractionStore, PostingsCache};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeUpdateDescriptionRequest {
    /// Renamed from `id` (issue #1106): the board-synthetic `id`
    /// `PostingsCache` upserts by has no meaning off that one in-memory
    /// cache, and no Agent/MCP read command ever exposed it — every reader
    /// (`job`/`best-matches`/`autopilot_best_matches`, and the renderer's own
    /// `JobDetailPane`) already has the posting's `url`. See
    /// `scrape_update_description`'s own doc for the two stores this now
    /// addresses by url.
    pub url: String,
    pub description: String,
}

/// Upper bound on a write-back description. A full JD is on the order of a few KB;
/// 256 KB is generous headroom while bounding a looping/XSS'd renderer from
/// ballooning a cached entry. Over-cap input is rejected, not silently truncated,
/// so a caller can tell the write didn't take effect as sent.
const MAX_DESCRIPTION_LEN: usize = 256 * 1024;

/// Write a freshly-resolved full description back into BOTH stores that can
/// carry a copy of this posting, addressed by `url` (issue #1106): the live
/// [`PostingsCache`] (session-lifetime, in-memory) AND every matching
/// `FoundJob` row across every persisted `Autopilot` record
/// (`AutopilotStore::update_found_job_descriptions`). `id` — this command's
/// old parameter — was a board-synthetic key with no meaning off
/// `PostingsCache`: no Agent/MCP read command ever exposed it, and even a
/// correct `id`-based lookup would still silently miss every posting
/// surfaced via `job`/`best-matches`/`autopilot_best_matches`, which read
/// `Autopilot.found_jobs` directly and never touch `PostingsCache` at all.
/// `url` is the one identity every surface already shares — see
/// `extension_bridge::agent_read`'s own module doc ("`url` is the
/// CROSS-RESOURCE KEY — not an id").
///
/// The detail pane resolves a fuller description on demand (see
/// [`scrape_resolve_url`]); without this, match scoring would continue
/// reading the truncated aggregator snippet from whichever store still held
/// it. The match-score cache is job-text-hash-keyed, so updating the
/// description invalidates cached scores for that job; on-demand scoring via
/// `useJobMatchScore` will recompute.
///
/// Both stores are tried independently and unconditionally — see
/// [`either_store_updated`] for the resulting `data: true`/`false` rule.
///
/// Validate the write-back inputs, returning the NORMALIZED url on success
/// (reused as-is for both stores, never re-derived per store). Pure (no
/// `AppHandle`) so the error paths are unit-tested directly. Rejects an empty
/// or non-http(s) url (an explicit `http://`/`https://` prefix is REQUIRED —
/// `normalize_job_url` alone passes a schemeless bare token like the
/// pre-rename `id` shape straight through unchanged, which would otherwise
/// validate and then miss both stores as a silent, honest-looking
/// `data: false`), an empty/whitespace-only description (this command
/// CORRECTS a description, it does not clear one — an empty string here
/// would otherwise wipe every matching row across both stores), and an
/// over-cap description rather than silently truncating, so the caller can
/// tell the write didn't take effect as sent.
///
/// Canonicalizes via [`crate::scraping::scrape_url::canonical_job_url`]
/// BEFORE normalizing — the exact two-line pipeline
/// `extension_bridge::agent_read::job_resource` uses — so a board-specific
/// search/SPA-view url (e.g. LinkedIn's `?currentJobId=` search view) lands
/// on the SAME identity a `job`/`answers.save` read resolves it to, rather
/// than a normalized value neither store was ever keyed by (issue #1106
/// follow-up).
fn validate_update_description(url: &str, description: &str) -> AppResult<String> {
    let url = url.trim();
    if url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }
    if description.trim().is_empty() {
        return Err(AppError::Validation(
            "description must not be empty — this command corrects a description, it does not \
             clear one"
                .to_string(),
        ));
    }
    if description.len() > MAX_DESCRIPTION_LEN {
        return Err(AppError::Validation(format!(
            "description exceeds the {MAX_DESCRIPTION_LEN}-byte cap"
        )));
    }
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(AppError::Validation(
            "url must have an explicit http(s) scheme".to_string(),
        ));
    }
    let canonical = crate::scraping::scrape_url::canonical_job_url(url);
    let effective = canonical.as_deref().unwrap_or(url);
    let normalized = crate::applications::normalize_job_url(effective);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "url is not a valid http(s) URL".to_string(),
        ));
    }
    Ok(normalized)
}

/// Whether either backing store had a matching row to correct — the `data`
/// bit `scrape_update_description` returns. Its own tiny pure fn so "success
/// iff EITHER `PostingsCache` or `Autopilot.found_jobs` matched, honest
/// failure only when NEITHER did" is asserted directly rather than only
/// implied by the command body.
fn either_store_updated(cache_hit: bool, found_jobs_updated: u32) -> bool {
    cache_hit || found_jobs_updated > 0
}

#[tauri::command]
pub fn scrape_update_description(
    app: AppHandle,
    req: ScrapeUpdateDescriptionRequest,
) -> AppResult<bool> {
    let normalized_url = validate_update_description(&req.url, &req.description)?;
    let cache_hit = {
        let cache = app.state::<Mutex<PostingsCache>>();
        cache
            .lock()
            .update_description(&normalized_url, &req.description)
    };
    let found_jobs_updated = crate::commands::autopilot::store(&app)
        .lock()
        .update_found_job_descriptions(&normalized_url, &req.description);
    Ok(either_store_updated(cache_hit, found_jobs_updated))
}

#[tauri::command]
pub fn scrape_list_postings(app: AppHandle) -> Value {
    // Snapshot the interactions first and DROP that guard before locking the
    // postings cache, so the two mutexes are never held at once (no lock-order
    // deadlock). `list` takes `&mut` because it lazily hydrates from disk.
    let interactions = {
        let store = app.state::<Mutex<InteractionStore>>();
        let mut guard = store.lock();
        guard.list(None)
    };
    // Now join the interactions onto the live postings so the jobs list can show
    // viewed/applied/saved badges (the cache items carry no interactions).
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock();
    json!(attach_interactions(guard.get_all(), &interactions))
}

#[tauri::command]
pub fn scrape_clear_postings(app: AppHandle) -> Value {
    app.state::<Mutex<PostingsCache>>().lock().clear_all();
    json!(null)
}

#[cfg(test)]
mod tests;

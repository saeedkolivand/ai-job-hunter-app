//! Import-time best-effort match score (`import.result.matchScore`) — split from `match_live.rs`
//! (R8 relief). Ungated (unlike the "Check fit" verb) — see `match_live`'s own module doc for the
//! consent-gate reasoning and the cache-key parity with the "Check fit" button.

use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::match_live_score::{adhoc_job_id, posting_job_text, resolve_resume, score_keyword_only};
use super::match_live_timeout::{timed, SCORE_TIMEOUT};
use crate::documents::DocumentStore;
use crate::scraping::types::JobPosting;

/// Best-effort keyword-only match score for `handle_import`'s
/// `import.result.matchScore` (the field has existed on the wire since the
/// import feature shipped — see `ExtensionImportResult`'s doc; this is the
/// first PR that ever populates it). Reuses the SAME résumé-resolution + ad-
/// hoc scoring path as [`resolve_match_live`]'s "Check fit" button, keyed by
/// the import's OWN already-normalized url so a later "Check fit" click on
/// the identical page hits the SAME self-invalidating result-cache row.
/// Returns `None` on ANY failure (no résumé yet, no usable posting text, the
/// document store unavailable) — `handle_import` has ALREADY persisted the
/// Application by the time this runs, so a scoring failure only omits the
/// field; it can never fail or block the import itself.
pub(super) async fn score_import_posting(
    app: &AppHandle,
    posting: &JobPosting,
    normalized_url: &str,
) -> Option<f64> {
    let job_text = posting_job_text(posting)?;
    let store = app.try_state::<DocumentStore>()?;
    let docs = store.list();
    let resume = resolve_resume(&docs)?;
    let job_id = adhoc_job_id(normalized_url);
    let result = score_keyword_only(app, store.inner(), resume, &job_id, job_text).await;
    result.get("combined").and_then(Value::as_f64)
}

/// [`score_import_posting`] bounded by [`SCORE_TIMEOUT`] — the function
/// `handle_import` actually calls. Logs at a level that matches how actionable
/// the outcome is: a genuine timeout (a slow/degraded scorer) is worth a
/// `warn`; an ordinary `None` — no résumé saved yet (the normal state for a
/// new user), unusable posting text, or a scoring failure — is expected noise
/// on plenty of imports and only worth a `debug`. Either way `matchScore` is
/// simply omitted; the import above has ALREADY succeeded by the time this runs.
pub(super) async fn score_import_posting_bounded(
    app: &AppHandle,
    posting: &JobPosting,
    normalized_url: &str,
) -> Option<f64> {
    match timed(
        SCORE_TIMEOUT,
        score_import_posting(app, posting, normalized_url),
    )
    .await
    {
        Err(_) => {
            log::warn!(
                "[extension_bridge] import-time match score exceeded {:?}; omitting matchScore",
                SCORE_TIMEOUT
            );
            None
        }
        Ok(None) => {
            log::debug!(
                "[extension_bridge] import-time match score unavailable (no résumé / unusable \
                 posting text / scoring failure); omitting matchScore"
            );
            None
        }
        Ok(Some(score)) => Some(score),
    }
}

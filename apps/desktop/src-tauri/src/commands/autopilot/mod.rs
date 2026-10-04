use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;

// `FoundJob`, `ScoreSource` and `CancellationToken` are not used in this file: `rerank` resolves them
// through its `use super::*`.
use crate::autopilot::{Autopilot, AutopilotStatus, AutopilotStore, FoundJob, ScoreSource};
// The save-path `country_code` backfill (a location saved without a geocode
// pick) — shared with the manual scrape path since trust-fix #2.
use crate::commands::geocoding::derive_country_code;
use crate::observability::sanitize_reason;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

// AutopilotCreateRequest / AutopilotUpdateRequest are generated from the Zod
// schemas in packages/shared by `pnpm gen:ipc`.
pub use crate::ipc_contracts::autopilot::{AutopilotCreateRequest, AutopilotUpdateRequest};

// `pub(super)` (issue #1106) — `commands::scrape::scrape_update_description`
// needs the SAME store handle to reach `Autopilot.found_jobs` on a
// description correction, rather than duplicating this state-extraction
// boilerplate a second time in a sibling module.
pub(super) fn store(app: &AppHandle) -> Arc<Mutex<AutopilotStore>> {
    app.state::<Arc<Mutex<AutopilotStore>>>().inner().clone()
}

/// Snapshot the set of normalized job urls that already have an Application
/// past `saved` (ADR 0001) — the one shared read `enrich_applied` and
/// `autopilot_best_matches` both need, expressed once instead of twice.
/// Best-effort: a missing store yields an empty set (nothing reads as
/// applied), never a failure.
// `pub(crate)` (issue #1167) — `agent_read::found_jobs` reuses this EXACT set (never a second
// hand-typed `ApplicationStore` read) so its `applied` filter/row-flag agrees with what
// `enrich_applied`/`mark_applied` already compute for `autopilot_list`/`best-matches`.
pub(crate) fn applied_job_urls(app: &AppHandle) -> HashSet<String> {
    app.try_state::<crate::applications::ApplicationStore>()
        .map(|s| s.applied_job_urls())
        .unwrap_or_default()
}

/// Like [`applied_job_urls`], but `None` means "cannot answer right now" —
/// either the store isn't managed, or it is and the query itself failed —
/// rather than collapsing both to the same empty set. `pub(crate)` (round-4
/// fix T3-cont, issue #1166/#1169) — `extension_bridge::agent_read`'s
/// `store_present` derives from this so a locked/corrupt applications DB
/// doesn't read as "the user applied to nothing" (see
/// `ApplicationStore::applied_job_urls_checked`'s own doc for the failure
/// mode this closes).
pub(crate) fn applied_job_urls_checked(app: &AppHandle) -> Option<HashSet<String>> {
    app.try_state::<crate::applications::ApplicationStore>()?
        .applied_job_urls_checked()
}

/// Fill each found job's `applied` from the set of `job_url`s that have a saved
/// generation — so the badge reflects a real link (a generation exists for that
/// job) rather than a hand-set flag that could drift.
fn enrich_applied(app: &AppHandle, list: &mut [crate::autopilot::Autopilot]) {
    // "Applied" is now derived from the Application aggregate (ADR 0001): a URL
    // counts as applied when it has an Application whose status is past `saved`.
    // The set is keyed by the SAME normalization the store applies on write, so
    // found-job urls must be normalized before the membership check below.
    let applied = applied_job_urls(app);
    if applied.is_empty() {
        return;
    }
    for ap in list.iter_mut() {
        for job in ap.found_jobs.iter_mut() {
            let key = crate::applications::normalize_job_url(&job.url);
            job.applied = applied.contains(&key);
        }
    }
}

#[tauri::command]
pub fn autopilot_list(app: AppHandle) -> Value {
    let mut list = store(&app).lock().list();
    enrich_applied(&app, &mut list);
    json!(list)
}

#[tauri::command]
pub fn autopilot_get(app: AppHandle, autopilot_id: String) -> Value {
    let ap = store(&app).lock().get(&autopilot_id).map(|a| {
        let mut one = [a];
        enrich_applied(&app, &mut one);
        let [ap] = one;
        ap
    });
    json!(ap)
}

#[tauri::command]
pub async fn autopilot_create(app: AppHandle, mut req: AutopilotCreateRequest) -> Value {
    if req.target.country_code.is_none() {
        req.target.country_code = derive_country_code(req.target.location.as_deref()).await;
    }
    let ap = store(&app)
        .lock()
        .create(serde_json::to_value(&req).unwrap_or_default());
    json!(ap)
}

#[tauri::command]
pub async fn autopilot_update(
    app: AppHandle,
    autopilot_id: String,
    mut req: AutopilotUpdateRequest,
) -> Value {
    if let Some(target) = req.target.as_mut() {
        if target.country_code.is_none() {
            target.country_code = derive_country_code(target.location.as_deref()).await;
        }
    }
    let patch = serde_json::to_value(&req).unwrap_or_default();
    let ap = mutate_record(&app, &autopilot_id, |records| {
        records.lock().update(&autopilot_id, patch)
    });
    json!(ap)
}

#[tauri::command]
pub fn autopilot_remove(app: AppHandle, autopilot_id: String) -> Value {
    mutate_record(&app, &autopilot_id, |records| {
        records.lock().remove(&autopilot_id)
    });
    json!(null)
}

/// Run a mutation of ONE autopilot record and drop whatever résumé-derived
/// cache rows it orphaned.
///
/// Both halves live here so a mutation path cannot ship with only the first: a
/// DELETE and a `resume_text` REPLACE orphan the identical rows, because the
/// cache identity IS the résumé's content hash. (An UPDATE shipped without this
/// exact defect and it took a second review round to notice.)
///
/// The "what is orphaned" question needs no diff: the list snapshot taken AFTER
/// the mutation still contains this record carrying its NEW text, so an
/// unchanged résumé is its own live producer and keeps its rows — the same
/// content-addressed rule that lets two autopilots share one row.
fn mutate_record<T>(
    app: &AppHandle,
    autopilot_id: &str,
    mutate: impl FnOnce(&Mutex<AutopilotStore>) -> T,
) -> T {
    let records = store(app);
    let previous_resume = records.lock().get(autopilot_id).and_then(|a| a.resume_text);
    let out = mutate(&records);
    // `try_state` because a mutation must never panic on an unmanaged store.
    if let Some(docs) = app.try_state::<crate::documents::DocumentStore>() {
        let remaining = records.lock().list();
        drop_orphaned_resume_cache(docs.inner(), previous_resume.as_deref(), &remaining);
    }
    out
}

/// Delete the cache rows derived from a résumé no autopilot carries any more —
/// its snapshot vector AND its cached match scores.
///
/// The re-rank caches the résumé under a content-addressed
/// `autopilot-resume:<sha256(text)>` id (see
/// `match_resume::autopilot_resume_id`), and every `match_scores` row that id
/// produced holds résumé-derived content of its own (gaps, recommendations, the
/// explanation). Both live in caches whose only bounds are a TTL and a row cap,
/// so without this they outlive the record they came from by up to the TTL (7
/// days at the default tier). The same-text check is what keeps a shared résumé
/// working: the id is the CONTENT, so another autopilot with the same résumé is
/// still a live producer of those rows.
///
/// Best-effort: a failed delete is logged, never surfaced — the caller has
/// already mutated the record.
fn drop_orphaned_resume_cache(
    docs: &crate::documents::DocumentStore,
    removed_resume: Option<&str>,
    remaining: &[Autopilot],
) {
    let Some(text) = removed_resume.filter(|t| !t.is_empty()) else {
        return;
    };
    if remaining
        .iter()
        .any(|a| a.resume_text.as_deref() == Some(text))
    {
        return; // another autopilot still produces these exact rows
    }
    let id = crate::commands::match_resume::autopilot_resume_id(text);
    if let Err(e) = docs.delete_posting_vector(&id) {
        log::warn!(
            "[autopilot] could not drop the résumé snapshot vector: {}",
            sanitize_reason(&e.to_string())
        );
    }
    if let Err(e) = docs.delete_match_scores_for_resume(&id) {
        log::warn!(
            "[autopilot] could not drop the résumé's cached match scores: {}",
            sanitize_reason(&e.to_string())
        );
    }
}

/// Run one autopilot: scrape, rank, optionally re-rank and annotate, then record. The pipeline lives
/// in [`run::autopilot_run`]; this is the IPC entry point.
#[tauri::command]
pub async fn autopilot_run(app: AppHandle, autopilot_id: String) -> Value {
    run::autopilot_run(app, autopilot_id).await
}

/// Take + clear the buffered autopilot-focus id. Split from the command so it's
/// unit-testable without a Tauri `State`. Atomic: the lock is held across the take.
pub(crate) fn take_pending_focus(buf: &crate::tray::PendingFocus) -> Option<String> {
    buf.0.lock().take()
}

/// Atomically take + clear the autopilot-focus intent buffered by
/// `tray::dispatch_focus` (a cold-start `ajh://autopilot/<id>` deep link fires
/// during Rust setup, before the renderer's `useAutopilotFocusNavigation`
/// listener attaches, so the `autopilot:focus` emit is lost). The renderer PULLS
/// this once its JS loop is provably live (on mount + on the emitted event). The
/// atomic take means an intent is delivered exactly once and can't re-fire on a
/// later unrelated focus. Returns `None` (the common case) when nothing is
/// buffered. Returns the `autopilotId` string. Infallible — just a lock take — so
/// no `AppResult`.
#[tauri::command]
pub fn autopilot_take_pending_focus(
    state: tauri::State<'_, crate::tray::PendingFocus>,
) -> Option<String> {
    take_pending_focus(state.inner())
}

#[tauri::command]
pub fn autopilot_pause(app: AppHandle, autopilot_id: String) -> Value {
    store(&app)
        .lock()
        .set_status(&autopilot_id, AutopilotStatus::Paused);
    json!(null)
}

#[tauri::command]
pub fn autopilot_resume(app: AppHandle, autopilot_id: String) -> Value {
    store(&app)
        .lock()
        .set_status(&autopilot_id, AutopilotStatus::Active);
    json!(null)
}

/// Cross-autopilot top-match surface (see `best_matches`'s module doc for the
/// recompute-vs-persist rationale). Thin I/O wrapper: every decision lives in
/// the pure, unit-tested `compute_best_matches`; this only resolves the
/// stores it needs and applies the one enrichment (`applied`) that can't be
/// expressed as a pure input, mirroring `enrich_applied`'s own
/// `ApplicationStore` read (a different row shape, so the pattern — not the
/// fn — is reused here).
///
/// Genuinely `async` + `spawn_blocking` (M4), not just the `async` keyword:
/// a plain sync `#[tauri::command]` fn runs INLINE on whichever thread
/// received the IPC call (the UI event-loop thread for a desktop webview —
/// see `commands::resume::resume_validate_content`'s doc for the traced
/// proof there is no Tauri-provided blocking pool for it), and clustering
/// the union is real CPU work: measured quadratic in the largest
/// title/company block (3.03s at 2000 items, 12.3s at 4000), unbounded
/// because `found_jobs` is never truncated.
///
/// On a `JoinError` this degrades to an empty result rather than propagating
/// — matching every other best-effort resolution in this command (a missing
/// store also degrades to an empty result). The join failure itself is
/// logged as a fixed category (`panicked` / `cancelled` / `failed`) — NEVER
/// the error's own `Display`: a `JoinError`'s panic-case message carries the
/// panic payload, arbitrary formatted text from whatever panicked inside a
/// closure that walks the user's data directory, which is exactly the shape
/// AGENTS.md's path-privacy rule exists to stop. The category is also the
/// only distinction anything here would act on differently, so nothing is
/// lost by not interpolating the raw error.
#[tauri::command]
pub async fn autopilot_best_matches(app: AppHandle) -> Value {
    tauri::async_runtime::spawn_blocking(move || autopilot_best_matches_blocking(&app))
        .await
        .unwrap_or_else(|e| {
            // `tauri::async_runtime::spawn_blocking`'s error is `tauri::Error`,
            // which only ever wraps a `tokio::task::JoinError` for this call —
            // matched out here rather than trusting `From` to have produced
            // anything else.
            let category = match &e {
                tauri::Error::JoinError(je) if je.is_panic() => "panicked",
                tauri::Error::JoinError(je) if je.is_cancelled() => "cancelled",
                _ => "failed",
            };
            log::error!("[autopilot] best_matches task {category}");
            json!({ "matches": [], "total": 0, "autopilotCount": 0 })
        })
}

fn autopilot_best_matches_blocking(app: &AppHandle) -> Value {
    let records = store(app).lock().list();
    let (tombstones, extra_agency) = snapshot_dedup_inputs(app);

    let dismissed_keys: HashSet<String> = app
        .try_state::<Mutex<crate::postings::InteractionStore>>()
        .map(|s| {
            s.lock()
                .list(Some("dismissed"))
                .into_iter()
                .map(|r| {
                    crate::scraping::boards::common::canonical_job_key(&r.url, &r.title, &r.company)
                })
                .collect()
        })
        .unwrap_or_default();

    let mut outcome = compute_best_matches(&records, &tombstones, &extra_agency, &dismissed_keys);
    mark_applied(&mut outcome.matches, &applied_job_urls(app));

    json!({
        "matches": outcome.matches,
        "total": outcome.total,
        "autopilotCount": outcome.autopilot_count,
    })
}

// ── Phase 2: optional semantic re-rank (ADR-020 addendum) ─────────────────────
//
// Split into a sibling module (see its doc) to keep this file under R8's LOC
// cap. Its items are `pub(super)`: `run` and the tests reach them as `super::rerank::…`.
mod rerank;

// ── Best Matches: cross-autopilot top-match surface ────────────────────────
//
// Same LOC-cap reasoning as `rerank` above (see that module's doc for the
// pattern); see `best_matches`'s own module doc for why membership is
// recomputed here rather than persisted.
mod best_matches;
use best_matches::*;

// ── LinkedIn-only post-discovery description enrichment (issue #1114) ──────
//
// L3 (Tauri/`AppHandle`-touching orchestration) — the pure "which URLs need a
// fetch" / "what does a fetch outcome mean" decisions live in the L2
// `autopilot_helpers::linkedin_enrich` instead (see its doc + this module's
// own doc for the boundary and why: `docs/architecture-rules.md` R2/R7).
mod linkedin_enrich;

// ── Phase 1 (keyword rank) + the run pipeline ──────────────────────────────
//
// Same LOC-cap reasoning as `rerank` above: `keyword_rank` holds the pure phase-1 helpers,
// `run` the pipeline that drives both phases (`autopilot_run` below is its IPC entry point).
mod keyword_rank;
mod phases;
mod run;
pub(crate) use keyword_rank::build_found_job;

/// Snapshot the durable dedup verdicts + agency extras from app state — the two
/// store-owned inputs every clustering call needs. Best-effort: a missing store
/// yields empty inputs (clustering degrades to "no splits / built-in agencies
/// only"), never a failure.
pub(crate) fn snapshot_dedup_inputs(app: &AppHandle) -> (HashSet<(String, String)>, Vec<String>) {
    let tombstones = app
        .try_state::<crate::dedup::DedupStore>()
        .map(|s| s.all_pairs())
        .unwrap_or_default();
    let extra_agency = app
        .try_state::<crate::job_preferences::JobPreferencesStore>()
        .map(|s| s.get().extra_agency_companies.unwrap_or_default())
        .unwrap_or_default();
    (tombstones, extra_agency)
}

/// Recompute + persist cluster annotations for one autopilot record after a
/// dedup split (`dedup_mark_not_duplicate` with an `autopilotId`). Snapshots the
/// current verdicts + extras and delegates to the store's per-record recompute.
pub(crate) fn recluster_autopilot_record(app: &AppHandle, autopilot_id: &str) {
    let (tombstones, extra_agency) = snapshot_dedup_inputs(app);
    store(app)
        .lock()
        .recompute_record_clusters(autopilot_id, &tombstones, &extra_agency);
}

#[cfg(test)]
mod tests;

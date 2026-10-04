//! Starting scrapes: the multi-board search, the single-URL import and the on-demand
//! description resolve. Split out of `commands/scrape.rs` for R8 (issue #1280);
//! `scrape.rs` re-exports the commands, so each keeps its `commands::scrape::<name>`
//! path (the `generate_handler!` list and the agent-CLI policy table are keyed on it).

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::clusters::recluster_postings_cache;
use super::{ScrapeBoardsRequest, ScrapeUrlRequest};
use crate::db::{new_job_id, now_ms};
use crate::error::AppResult;
use crate::events::{emit_event, JobEvent, JOBS_EVENT, SCRAPE_PROGRESS};
use crate::postings::PostingsCache;
use crate::scraping::{BoardSearchInput, ScraperEngine, WorkType};

/// Per-board page request budget. Each board clamps this down to its own page
/// cap; combined with the central `amount` cap, whichever limit is hit first
/// stops the scrape.
const MAX_PAGE_BUDGET: u32 = 10;

/// Take one slot on the shared scrape budget (rate + concurrency) under `op`. The
/// returned guard is held for the whole run and releases the slot when dropped.
fn acquire_scrape_slot(
    app: &AppHandle,
    op: &'static str,
) -> AppResult<crate::limits::ConcurrencyGuard> {
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    limiter.acquire(
        op,
        crate::limits::SCRAPE_RATE_MAX,
        crate::limits::SCRAPE_CONCURRENCY_MAX,
    )
}

/// Passively harvest ATS company slugs from posting URLs (parse-only, zero
/// network) — ADR-030 §c. Resolves the store at this shell boundary; a missing
/// store (startup failure) is a no-op.
fn harvest_ats_slugs(app: &AppHandle, refs: impl IntoIterator<Item = (String, String)>) {
    if let Some(store) = app.try_state::<crate::discovered::DiscoveredCompanyStore>() {
        crate::discovered::harvest_ats_refs(store.inner(), refs, "scrape");
    }
}

/// Fill `input.country_code` for a location the user TYPED instead of picking
/// from the geocode suggestions: the renderer only sends `countryCode` for a
/// picked suggestion, so a freehand "Germany"/"Amsterdam" arrives with none —
/// and the aggregator then hardcodes a `'de'` guess AND suppresses its
/// sparse-city broadening (a silent under-return). Same best-effort lookup
/// autopilot does on save; a network error / no match / 2s timeout just leaves
/// the field absent.
///
/// Returns **false when the run was cancelled** (the caller must abandon it),
/// true otherwise. Two cancellation concerns, both handled here:
/// - the lookup is raced against `token` with a `biased;` select, so a Stop
///   during the ≤2s geocode takes effect immediately instead of after it — and
///   an already-cancelled run never issues the request at all;
/// - a cancel that landed before this ran (between the command returning its
///   `jobId` and the spawned task waking) is caught by the same final check.
///
/// This narrows the window at the COMMAND layer; it is closed at the engine
/// layer by [`crate::scraping::ScraperEngine::cancel`] cancelling the job slot
/// in place instead of removing it, so a cancel landing after this returns
/// `true` is still honored by the engine rather than lost to a freshly minted
/// token. Takes a bare token + `&mut BoardSearchInput` so it is unit-testable
/// without an `AppHandle`.
async fn backfill_country_code(
    token: &tokio_util::sync::CancellationToken,
    input: &mut BoardSearchInput,
) -> bool {
    // Owned so the lookup future doesn't borrow `input` while we write to it.
    let location = input.location.clone();
    backfill_country_code_with(
        token,
        input,
        crate::commands::geocoding::derive_country_code(location.as_deref()),
    )
    .await
}

/// [`backfill_country_code`] with the geocode lookup injected, so the
/// cancellation behavior is testable against a hung / instant / never-polled
/// future instead of the real geocode lookup (no network, and no bundled-index
/// build, in tests).
///
/// `lookup` is only ever POLLED when `country_code` is absent — an existing
/// (picked) country is never clobbered and costs no request.
async fn backfill_country_code_with(
    token: &tokio_util::sync::CancellationToken,
    input: &mut BoardSearchInput,
    lookup: impl std::future::Future<Output = Option<String>>,
) -> bool {
    if input.country_code.is_none() {
        input.country_code = tokio::select! {
            biased;
            () = token.cancelled() => None,
            cc = lookup => cc,
        };
    }
    !token.is_cancelled()
}

#[tauri::command]
pub async fn scrape_boards(app: AppHandle, req: ScrapeBoardsRequest) -> Value {
    let job_id = new_job_id();

    // Anti-abuse: rate + concurrency cap. Rejected before a job is created so a
    // looping/XSS'd renderer can't drive unbounded scrape traffic. The guard is
    // moved into the spawned task and dropped when the scrape finishes.
    let guard = match acquire_scrape_slot(&app, "scrape_boards") {
        Ok(g) => g,
        Err(e) => return json!({ "error": e.to_string() }),
    };

    // "scrape.board" kept unchanged — renderer / use-worker-activity tests key on it.
    crate::commands::jobs::job_start(&app, &job_id, "scrape.board");

    let engine = app.state::<std::sync::Arc<ScraperEngine>>().inner().clone();
    // The count the USER actually typed, bounded once. Both budgets below are
    // THIS number on the manual path, so it is bound once rather than clamped
    // twice — two independent `.clamp` calls could silently drift apart.
    let requested_amount = req.amount.clamp(1, 100);
    let mut input = BoardSearchInput {
        query: req.query.clone(),
        location: req.location.clone(),
        // `amount` is the per-board cap: each board returns up to this many results.
        amount: requested_amount,
        pages: MAX_PAGE_BUDGET,
        // The ONLY path that sets a real provider spend target: here `amount` is
        // the count the USER actually typed, so a metered board (the aggregator)
        // may buy upstream calls up to it. Scheduled runs leave this `None` — see
        // `BoardSearchInput::provider_amount`.
        provider_amount: Some(requested_amount),
        date_filter: req.date_filter.clone(),
        // Structured search filters from the IPC request (ScrapeBoardsRequestSchema
        // in packages/shared). Optional, so absent fields stay None; LinkedIn's
        // search_paginated honors them and other boards ignore them. UI controls
        // for jobType/experienceLevel/etc. are a follow-up — only the contract +
        // propagation exist today. `work_types` is no longer in that bucket: it
        // normalises through the shared `WORK_TYPE_OPTIONS`/`WorkType` vocabulary.
        job_type: req.job_type.clone(),
        // Zod already restricts every entry to WORK_TYPE_OPTIONS; `WorkType::from_str`
        // is still the parser (never a raw cast) so an unrecognised entry is
        // dropped instead of silently miscoded — the same defensive posture the
        // rest of this module takes at an IPC boundary. A drop here is logged
        // (count only, never the raw string): silent on the manual-search path
        // today because Zod already blocks it, but this is the same
        // deserializer shape `AutopilotTarget` reuses on ITS persisted path,
        // where a future vocabulary rename landing here with nothing failing
        // is exactly the silent-widening failure mode this guards against.
        work_types: req.work_types.clone().map(|types| {
            let parsed: Vec<WorkType> = types.iter().filter_map(|s| s.parse().ok()).collect();
            let dropped = types.len() - parsed.len();
            if dropped > 0 {
                log::warn!(
                    "[scrape] dropped {dropped} unrecognised work-type entr{} from the request",
                    if dropped == 1 { "y" } else { "ies" }
                );
            }
            parsed
        }),
        experience_level: req.experience_level.clone(),
        easy_apply: req.easy_apply,
        actively_hiring: req.actively_hiring,
        verified: req.verified,
        sort_by: req.sort_by.clone(),
        country_code: req.country_code.clone(),
        latitude: req.latitude,
        longitude: req.longitude,
        radius_km: req.radius_km,
        // Company slugs for ATS boards with no global keyword search. Absent on
        // the wire → empty here, which is a no-op for every current board (none
        // read it yet); the 6 ATS boards will consume it in a follow-up.
        companies: req.companies.clone().unwrap_or_default(),
    };
    let boards = req.boards.clone();

    // First-item-clear: on a NEW search (replace=true) the live postings cache is
    // wiped under-lock the instant the first new result streams in, so a failed or
    // empty search leaves the previous results intact. The latch ensures we clear
    // exactly once across ALL boards. Append (replace omitted/false) leaves the
    // cache untouched.
    //
    // Exclusivity is a renderer contract: the Jobs page cancels the in-flight scrape
    // before starting a new one, so two concurrent replace=true scrapes don't race.
    let replace = req.replace.unwrap_or(false);
    let replaced_clone = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let app_progress = app.clone();
    let job_id_progress = job_id.clone();
    let on_progress: std::sync::Arc<dyn Fn(f32) + Send + Sync> =
        std::sync::Arc::new(move |p: f32| {
            emit_event(
                &app_progress,
                SCRAPE_PROGRESS,
                json!({ "jobId": job_id_progress, "progress": p }),
            );
            crate::commands::jobs::job_progress(&app_progress, &job_id_progress, p as f64);
        });

    let app_item = app.clone();
    let job_id_item = job_id.clone();
    let on_item: std::sync::Arc<dyn Fn(crate::scraping::JobPosting) + Send + Sync> =
        std::sync::Arc::new(move |item: crate::scraping::JobPosting| {
            if let Some(cache) = app_item.try_state::<Mutex<PostingsCache>>() {
                let mut guard = cache.lock();
                if replace && !replaced_clone.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    guard.clear_all();
                }
                if let Ok(item_json) = serde_json::to_value(&item) {
                    guard.add(item_json);
                }
            }

            emit_event(
                &app_item,
                JOBS_EVENT,
                JobEvent {
                    r#type: "job.stream".to_string(),
                    job_id: job_id_item.clone(),
                    data: Some(json!(item)),
                    ts: now_ms() as i64,
                },
            );
        });

    // F2 — register the cancellation token BEFORE spawning so that a fast
    // `jobs_cancel` call (arriving between this return and the spawn waking) is
    // never a no-op. `scrape_boards` detects the pre-registered slot and reuses
    // it (we_minted=false) and therefore will NOT remove it — we clean up below.
    let cancel_token = tokio_util::sync::CancellationToken::new();
    engine.register_token(&job_id, cancel_token.clone()).await;

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        // Hold the concurrency guard for the whole scrape; dropped on completion.
        let _guard = guard;

        // Fill in the market for a typed (not picked) location, unless the user
        // already cancelled — see [`backfill_country_code`].
        if !backfill_country_code(&cancel_token, &mut input).await {
            // `jobs_cancel` already emitted `job.cancelled`, so there is no
            // terminal event to report here — just release the slot and the
            // concurrency guard held by `_guard`.
            engine.unregister_token(&job_id_clone).await;
            return;
        }

        let result = engine
            .scrape_boards(
                &boards,
                input,
                job_id_clone.clone(),
                Some(on_progress),
                Some(on_item),
            )
            .await;

        // F2/F5 — we pre-registered the token, so scrape_boards left the slot
        // in place; clean it up now that the run is done.
        engine.unregister_token(&job_id_clone).await;

        match &result {
            Ok((postings, summaries)) => {
                // Cluster cross-board duplicates in the freshly-populated cache
                // BEFORE completion, so the jobs list renders grouped rows (and
                // the annotations are present when the renderer refetches).
                recluster_postings_cache(&app_clone);
                // Passively harvest ATS company slugs from every posting's URL.
                harvest_ats_slugs(
                    &app_clone,
                    postings.iter().map(|p| (p.url.clone(), p.company.clone())),
                );
                crate::commands::jobs::job_complete(
                    &app_clone,
                    &job_id_clone,
                    json!({ "count": postings.len(), "boards": summaries }),
                );
            }
            Err(e) => {
                crate::commands::jobs::job_fail(&app_clone, &job_id_clone, e.to_string());
            }
        }

        let _ = result;
    });

    json!({ "jobId": job_id })
}

#[tauri::command]
pub async fn scrape_url(app: AppHandle, req: ScrapeUrlRequest) -> Value {
    let url = req.url;
    if url.is_empty() {
        return json!({ "error": "url is required" });
    }

    // Anti-abuse: rate + concurrency cap (shares the scrape budget knobs). Checked
    // after the cheap empty-url guard so an invalid call costs no slot.
    let guard = match acquire_scrape_slot(&app, "scrape_url") {
        Ok(g) => g,
        Err(e) => return json!({ "error": e.to_string() }),
    };

    let job_id = new_job_id();
    crate::commands::jobs::job_start(&app, &job_id, "scrape.url");

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        // Hold the concurrency guard for the whole resolve; dropped on completion.
        let _guard = guard;
        let result = crate::scraping::scrape_url::resolve(&url).await;

        match result {
            Ok(Some(posting)) => {
                if let Some(cache) = app_clone.try_state::<Mutex<PostingsCache>>() {
                    {
                        let mut guard = cache.lock();
                        if let Ok(item_json) = serde_json::to_value(&posting) {
                            guard.add(item_json);
                        }
                    }
                }

                emit_event(
                    &app_clone,
                    JOBS_EVENT,
                    JobEvent {
                        r#type: "job.stream".to_string(),
                        job_id: job_id_clone.clone(),
                        data: Some(json!(posting)),
                        ts: now_ms() as i64,
                    },
                );

                // Re-cluster the cache now that the single resolved posting is in
                // it, so a URL-imported job picks up its cross-board group too.
                recluster_postings_cache(&app_clone);
                // Passively harvest the ATS slug from the resolved posting's URL.
                harvest_ats_slugs(
                    &app_clone,
                    std::iter::once((posting.url.clone(), posting.company.clone())),
                );
                crate::commands::jobs::job_complete(
                    &app_clone,
                    &job_id_clone,
                    json!({ "count": 1 }),
                );
            }
            Ok(None) => {
                crate::commands::jobs::job_fail(
                    &app_clone,
                    &job_id_clone,
                    "no scraper matched this URL".to_string(),
                );
            }
            Err(e) => {
                crate::commands::jobs::job_fail(&app_clone, &job_id_clone, e.to_string());
            }
        }
    });

    json!({ "jobId": job_id })
}

/// Resolve a single job posting (incl. full description) from its URL.
/// Synchronous request/response — used to fetch a description on demand for
/// boards whose list scrape omits it (LinkedIn, Glassdoor, etc.).
#[tauri::command]
pub async fn scrape_resolve_url(app: AppHandle, url: String) -> Value {
    if url.is_empty() {
        return json!(null);
    }
    // Anti-abuse: same rate + concurrency budget as the other scrape commands so a
    // looping/XSS'd renderer can't bypass the cap by hammering resolve directly.
    // NOTE: one slot here covers a single resolve, which may fan out a SHORT,
    // bounded redirect chain — `resolve` follows at most 2 hops
    // (get_guarded_following_redirects with max_hops=2 → up to 3 fetches: the
    // initial request + 2 redirect hops). The hop budget is kept small precisely so
    // one slot stays a small, honest, bounded number of outbound fetches.
    let _guard = match acquire_scrape_slot(&app, "scrape_url") {
        Ok(g) => g,
        Err(_) => return json!(null),
    };
    match crate::scraping::scrape_url::resolve(&url).await {
        Ok(Some(posting)) => {
            // ADR-031 §c: feed the resolved posting into the ADR-030 slug-harvest
            // seam (parse-only, zero new network) so a single-URL import populates
            // the slug typeahead like the scrape/autopilot/extension paths. Harvest
            // the posting's FINAL/canonical `url` (what got stored on it — an
            // aggregator click-tracker resolves to the board's real posting url),
            // not the raw request `url`, matching the other harvest sites. Resolve
            // the store at this shell boundary (missing store → no-op); the seam
            // itself degrades on an upsert error via log::warn.
            harvest_ats_slugs(
                &app,
                std::iter::once((posting.url.clone(), posting.company.clone())),
            );
            serde_json::to_value(&posting).unwrap_or(json!(null))
        }
        _ => json!(null),
    }
}

#[cfg(test)]
mod tests;

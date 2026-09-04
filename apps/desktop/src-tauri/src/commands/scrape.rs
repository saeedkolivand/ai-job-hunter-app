use std::collections::HashMap;

use crate::db::{new_job_id, now_ms};
use crate::error::{AppError, AppResult};
use crate::postings::{attach_interactions, InteractionRecord, InteractionStore, PostingsCache};
use crate::scraping::cluster::{
    assign_clusters, posting_cluster_input, ClusterAssignment, ClusterInput,
};
use crate::scraping::{BoardSearchInput, ScraperEngine, WorkType};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::events::{emit_event, JobEvent, JOBS_EVENT, SCRAPE_PROGRESS};

// ScrapeBoardsRequest and ScrapeUrlRequest are generated from the Zod schemas in
// packages/shared by `pnpm gen:ipc`. See crate::ipc_contracts::scrape.
pub use crate::ipc_contracts::scrape::{ScrapeBoardsRequest, ScrapeUrlRequest};

/// Per-board page request budget. Each board clamps this down to its own page
/// cap; combined with the central `amount` cap, whichever limit is hit first
/// stops the scrape.
const MAX_PAGE_BUDGET: u32 = 10;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobObject {
    pub id: Option<String>,
    pub title: Option<String>,
    pub company: Option<String>,
    pub url: Option<String>,
    pub source: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapePersistJobRequest {
    pub job: JobObject,
    pub interaction_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeListFilter {
    pub interaction_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeRemoveInteractionRequest {
    pub job_id: String,
    pub interaction_type: String,
}

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
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    let guard = match limiter.acquire(
        "scrape_boards",
        crate::limits::SCRAPE_RATE_MAX,
        crate::limits::SCRAPE_CONCURRENCY_MAX,
    ) {
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
                // Passively harvest ATS company slugs from every posting's URL
                // (parse-only, zero network) — ADR-030 §c. Resolve the store at this
                // shell boundary; a missing store (startup failure) is a no-op.
                if let Some(store) =
                    app_clone.try_state::<crate::discovered::DiscoveredCompanyStore>()
                {
                    crate::discovered::harvest_ats_refs(
                        store.inner(),
                        postings.iter().map(|p| (p.url.clone(), p.company.clone())),
                        "scrape",
                    );
                }
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
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    let guard = match limiter.acquire(
        "scrape_url",
        crate::limits::SCRAPE_RATE_MAX,
        crate::limits::SCRAPE_CONCURRENCY_MAX,
    ) {
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
                // Passively harvest the ATS slug from the resolved posting's URL
                // (parse-only, zero network) — ADR-030 §c. Resolve the store at this
                // shell boundary; a missing store (startup failure) is a no-op.
                if let Some(store) =
                    app_clone.try_state::<crate::discovered::DiscoveredCompanyStore>()
                {
                    crate::discovered::harvest_ats_refs(
                        store.inner(),
                        std::iter::once((posting.url.clone(), posting.company.clone())),
                        "scrape",
                    );
                }
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

/// Recompute cross-board clusters over the live [`PostingsCache`] and patch each
/// item's cluster annotations in place (ADR-029 §b). Store-aware — it needs the
/// user's tombstone verdicts, the agency extras, and the cached posting vectors —
/// so it lives in the L3 command layer, not the store-blind engine. Idempotent
/// and side-effect-free beyond the cache patch; safe to call after every ingest.
///
/// NEVER embeds: a cached-vector lookup is a MISS unless the row is present AND
/// in the active embedding space, in which case its vector feeds the cosine path;
/// otherwise the pair falls onto the trigram string path. A missing store, an
/// empty cache, or a read error degrades to "no annotations", never a failure.
pub fn recluster_postings_cache(app: &AppHandle) {
    // Durable + preference inputs (best-effort snapshots).
    let tombstones = app
        .try_state::<crate::dedup::DedupStore>()
        .map(|s| s.all_pairs())
        .unwrap_or_default();
    let extra_agency = app
        .try_state::<crate::job_preferences::JobPreferencesStore>()
        .map(|s| s.get().extra_agency_companies.unwrap_or_default())
        .unwrap_or_default();

    // Snapshot the cache items under-lock, then release before the per-item
    // DocumentStore vector reads (never hold two store locks at once).
    let Some(cache) = app.try_state::<Mutex<PostingsCache>>() else {
        return;
    };
    let items: Vec<Value> = cache.lock().get_all().to_vec();
    if items.is_empty() {
        return;
    }

    // Active embedding space — cached posting vectors in ANY other space are a
    // miss (mirrors `posting_vector_is_fresh`'s space check, without the
    // text-hash requirement, and WITHOUT ever embedding).
    let doc_store = app.try_state::<crate::documents::DocumentStore>();
    let active = doc_store.as_ref().map(|s| s.embedding_config());

    let mut ids: Vec<String> = Vec::with_capacity(items.len());
    let mut inputs: Vec<ClusterInput> = Vec::with_capacity(items.len());
    for item in &items {
        // The cache stores serialized `JobPosting`s; deserialize through the SAME
        // type production ingests so the cluster-input mapping can't drift (the
        // shared `posting_cluster_input` seam is also exercised by the aggregator
        // acceptance test). A cache entry that isn't a well-formed posting — or
        // carries no id to annotate — is skipped, never breaking the whole run.
        let Ok(posting) = serde_json::from_value::<crate::scraping::JobPosting>(item.clone())
        else {
            continue;
        };
        if posting.id.trim().is_empty() {
            continue;
        }

        let (vector, space) = match (doc_store.as_ref(), active.as_ref()) {
            (Some(store), Some(cfg)) => store
                .get_posting_vector(&posting.id)
                .filter(|(v, _)| cfg.matches(&v.space))
                .map(|(v, _)| (Some(v.values), Some(v.space.to_string())))
                .unwrap_or((None, None)),
            _ => (None, None),
        };

        ids.push(posting.id.clone());
        inputs.push(posting_cluster_input(&posting, vector, space));
    }

    let assignments = assign_clusters(inputs, &tombstones, &extra_agency);

    // Zip verdicts back onto ids by index (assign_clusters preserves input order).
    let by_id: HashMap<String, Value> = ids
        .into_iter()
        .zip(assignments.iter())
        .map(|(id, a)| (id, cluster_annotation_json(a)))
        .collect();
    cache.lock().apply_cluster_annotations(&by_id);
}

/// Serialize a [`ClusterAssignment`] to the annotation object patched onto a
/// cache item: `clusterId`, `clusterCanonical`, `clusterMembers` `[{key,board?,url}]`,
/// `isAgency` (ADR-029 §e). `board` is omitted when absent.
fn cluster_annotation_json(a: &ClusterAssignment) -> Value {
    let members: Vec<Value> = a
        .members
        .iter()
        .map(|m| {
            let mut obj = serde_json::Map::new();
            obj.insert("key".to_string(), json!(m.key));
            if let Some(board) = &m.board {
                obj.insert("board".to_string(), json!(board));
            }
            obj.insert("url".to_string(), json!(m.url));
            Value::Object(obj)
        })
        .collect();
    json!({
        "clusterId": a.cluster_id,
        "clusterCanonical": a.canonical,
        "clusterMembers": members,
        "isAgency": a.is_agency,
    })
}

/// Reverses `agent_call::fence_scraped_fields`'s wrapper for a field about
/// to be written into `InteractionStore` (security review round 4): a
/// caller that reads a job through a fenced surface (`scrape_list_postings`,
/// `autopilot_list`, …) and echoes the value straight back here would
/// otherwise persist the literal `<job_posting>…</job_posting>` markup into
/// the user's real interaction history. A no-op for the normal case — a
/// caller passing a clean value that was never fenced.
fn unfence_job_field(v: Option<String>) -> String {
    crate::prompt_fence::strip_fence_wrapper("job_posting", &v.unwrap_or_default())
}

#[tauri::command]
pub fn scrape_persist_job(app: AppHandle, req: ScrapePersistJobRequest) -> Value {
    let record = InteractionRecord {
        job_id: req.job.id.unwrap_or_default(),
        interaction_type: req.interaction_type,
        timestamp: now_ms(),
        title: unfence_job_field(req.job.title),
        company: unfence_job_field(req.job.company),
        url: req.job.url.unwrap_or_default(),
        source: req.job.source.unwrap_or_default(),
        location: unfence_job_field(req.job.location),
    };
    app.state::<Mutex<InteractionStore>>().lock().upsert(record);
    json!({ "success": true })
}

/// The real "undo" for [`scrape_persist_job`] — deletes the persisted
/// interaction instead of only hiding it client-side. Keys on the same
/// `(jobId, interactionType)` pair `upsert` writes; see
/// [`InteractionStore::remove`] for the "nothing to remove" distinction.
#[tauri::command]
pub fn scrape_remove_interaction(app: AppHandle, req: ScrapeRemoveInteractionRequest) -> bool {
    app.state::<Mutex<InteractionStore>>()
        .lock()
        .remove(&req.job_id, &req.interaction_type)
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
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    // NOTE: one slot here covers a single resolve, which may fan out a SHORT,
    // bounded redirect chain — `resolve` follows at most 2 hops
    // (get_guarded_following_redirects with max_hops=2 → up to 3 fetches: the
    // initial request + 2 redirect hops). The hop budget is kept small precisely so
    // one slot stays a small, honest, bounded number of outbound fetches.
    let _guard = match limiter.acquire(
        "scrape_url",
        crate::limits::SCRAPE_RATE_MAX,
        crate::limits::SCRAPE_CONCURRENCY_MAX,
    ) {
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
            if let Some(store) = app.try_state::<crate::discovered::DiscoveredCompanyStore>() {
                crate::discovered::harvest_ats_refs(
                    store.inner(),
                    std::iter::once((posting.url.clone(), posting.company.clone())),
                    "scrape",
                );
            }
            serde_json::to_value(&posting).unwrap_or(json!(null))
        }
        _ => json!(null),
    }
}

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

#[tauri::command]
pub fn scrape_list_interactions(app: AppHandle, filter: Option<ScrapeListFilter>) -> Value {
    let filter_type = filter.and_then(|f| f.interaction_type);
    let binding = app.state::<Mutex<InteractionStore>>();
    let mut store = binding.lock();
    json!(store.list(filter_type.as_deref()))
}

#[cfg(test)]
mod test {
    use super::*;

    // ── unfence_job_field (security review round 4) ──────────────────────
    // Pure — no AppHandle needed, unlike `scrape_persist_job` itself (this
    // crate has no `tauri::test` mock-app harness).

    #[test]
    fn unfence_job_field_strips_a_wrapper_a_caller_echoed_back_from_a_fenced_read() {
        let fenced = crate::prompt_fence::fenced("job_posting", "Senior Engineer", 1_000);
        assert_eq!(
            unfence_job_field(Some(fenced)),
            "Senior Engineer",
            "a value round-tripped from a fenced read must not persist the wrapper"
        );
    }

    #[test]
    fn unfence_job_field_leaves_a_clean_caller_supplied_value_alone() {
        assert_eq!(
            unfence_job_field(Some("Senior Engineer".to_string())),
            "Senior Engineer"
        );
    }

    #[test]
    fn unfence_job_field_defaults_a_missing_value_to_empty_string() {
        assert_eq!(unfence_job_field(None), "");
    }

    // The request must deserialize from the camelCase wire shape the renderer
    // sends (`url`/`description`). Pins the serde contract without an AppHandle.
    #[test]
    fn update_description_request_deserializes_camel_case() {
        let json = r#"{"url":"https://example.com/jobs/1","description":"full text"}"#;
        let req: ScrapeUpdateDescriptionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.url, "https://example.com/jobs/1");
        assert_eq!(req.description, "full text");
    }

    // The request must deserialize from the camelCase wire shape the renderer
    // sends (`jobId`/`interactionType`).
    #[test]
    fn remove_interaction_request_deserializes_camel_case() {
        let json = r#"{"jobId":"https://example.com/job/1","interactionType":"dismissed"}"#;
        let req: ScrapeRemoveInteractionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.job_id, "https://example.com/job/1");
        assert_eq!(req.interaction_type, "dismissed");
    }

    #[test]
    fn validate_rejects_empty_or_whitespace_url() {
        assert!(
            matches!(
                validate_update_description("", "text"),
                Err(AppError::Validation(_))
            ),
            "empty url must be a validation error"
        );
        assert!(
            matches!(
                validate_update_description("   ", "text"),
                Err(AppError::Validation(_))
            ),
            "whitespace-only url must be a validation error"
        );
    }

    #[test]
    fn validate_rejects_a_non_http_scheme() {
        assert!(
            matches!(
                validate_update_description("javascript:alert(1)", "text"),
                Err(AppError::Validation(_))
            ),
            "a non-http(s) scheme must normalize to empty and be rejected"
        );
    }

    // ── review round 2 (issue #1106 follow-up): schemeless input must be
    // rejected, not silently treated as a "valid" url ──────────────────────
    // `normalize_job_url("job-1")` returns `"job-1"` unchanged (no scheme to
    // strip), so without an explicit scheme check a stale caller sending the
    // pre-rename `id` shape would validate and then miss both stores as a
    // silent, honest-looking `data: false`.

    #[test]
    fn validate_rejects_a_schemeless_bare_token() {
        assert!(
            matches!(
                validate_update_description("job-1", "text"),
                Err(AppError::Validation(_))
            ),
            "a schemeless bare token must be rejected up front, not normalized \
             unchanged and looked up as if it were a valid url"
        );
    }

    #[test]
    fn validate_rejects_the_pre_rename_board_synthetic_id_shape() {
        // This particular shape happens to already be caught upstream (its
        // `greenhouse:` prefix parses as an explicit non-http(s) scheme, so
        // `normalize_job_url` alone already neutralizes it to ""); pinned
        // anyway as a regression guard on the exact shape called out in
        // review, alongside the schemeless-bare-token case above which the
        // NEW explicit-scheme check is what actually catches.
        assert!(
            matches!(
                validate_update_description("greenhouse:12345", "text"),
                Err(AppError::Validation(_))
            ),
            "a stale caller sending the OLD board-synthetic id format must get a \
             validation error, not a silent honest-looking data:false"
        );
    }

    #[test]
    fn validate_rejects_over_cap_description() {
        let too_long = "x".repeat(MAX_DESCRIPTION_LEN + 1);
        assert!(
            matches!(
                validate_update_description("https://example.com/jobs/1", &too_long),
                Err(AppError::Validation(_))
            ),
            "a description past the cap must be rejected, not truncated"
        );
    }

    // ── review round 3 (issue #1106 follow-up, MEDIUM/data-loss): an empty
    // description must be rejected up front, not silently wiped into every
    // matching row across both stores ───────────────────────────────────────

    #[test]
    fn validate_rejects_empty_or_whitespace_description() {
        assert!(
            matches!(
                validate_update_description("https://example.com/jobs/1", ""),
                Err(AppError::Validation(_))
            ),
            "an empty description must be a validation error, not a silent wipe"
        );
        assert!(
            matches!(
                validate_update_description("https://example.com/jobs/1", "   "),
                Err(AppError::Validation(_))
            ),
            "a whitespace-only description must be a validation error too"
        );
    }

    #[test]
    fn validate_accepts_valid_input_and_normalizes_the_url() {
        // At-cap is allowed (boundary): only strictly-over-cap is rejected.
        let at_cap = "x".repeat(MAX_DESCRIPTION_LEN);
        let normalized =
            validate_update_description("  HTTPS://Example.com/Jobs/1/?utm_source=x  ", &at_cap)
                .expect("a normalizable url with an at-cap description must validate");
        assert_eq!(
            normalized, "https://example.com/jobs/1",
            "the returned value is the NORMALIZED url (lowercase host, no trailing slash, \
             tracking params dropped), reused as-is for both stores"
        );
    }

    // ── review round 2 (issue #1106 follow-up, HIGH): the write-back identity
    // must canonicalize BEFORE normalizing, exactly like
    // `extension_bridge::agent_read::job_resource` does, so a board-specific
    // search/SPA-view url resolves to the same key a `job`/`answers.save`
    // read would use ──────────────────────────────────────────────────────

    #[test]
    fn validate_canonicalizes_a_linkedin_search_view_url_to_the_same_identity_job_resource_uses() {
        let search_view = "https://www.linkedin.com/jobs/search/?currentJobId=4185657072";
        let canonical_view = "https://www.linkedin.com/jobs/view/4185657072";

        let from_search = validate_update_description(search_view, "text")
            .expect("a recognised LinkedIn url must validate");
        let from_canonical = validate_update_description(canonical_view, "text")
            .expect("the canonical view url must validate");

        assert_eq!(
            from_search, from_canonical,
            "the search-view and canonical-view urls for the SAME job must normalize \
             to the identical identity — previously the search-view url normalized to \
             .../jobs/search with the id dropped entirely, matching neither store"
        );
        assert_eq!(
            from_search, "https://linkedin.com/jobs/view/4185657072",
            "must land on the canonical /jobs/view/<id> shape, not the raw search path"
        );
    }

    // ── either_store_updated (issue #1106 — the two-store OR semantics) ──────

    #[test]
    fn either_store_updated_is_true_when_only_the_cache_matched() {
        assert!(either_store_updated(true, 0));
    }

    #[test]
    fn either_store_updated_is_true_when_only_found_jobs_matched() {
        assert!(either_store_updated(false, 1));
    }

    #[test]
    fn either_store_updated_is_true_when_both_matched() {
        assert!(either_store_updated(true, 2));
    }

    #[test]
    fn either_store_updated_is_false_when_neither_matched() {
        assert!(!either_store_updated(false, 0));
    }

    // ── backfill_country_code (pre-scrape cancellation) ──────────────────────
    //
    // Driven through the injected-lookup seam so every case is hermetic: no
    // geocode round trip, no `AppHandle`, no timing sleeps.

    fn input_with(location: Option<&str>, country_code: Option<&str>) -> BoardSearchInput {
        BoardSearchInput {
            query: "rust".to_string(),
            location: location.map(str::to_string),
            amount: 25,
            pages: 1,
            provider_amount: None,
            date_filter: None,
            job_type: None,
            work_types: None,
            experience_level: None,
            easy_apply: None,
            actively_hiring: None,
            verified: None,
            sort_by: None,
            country_code: country_code.map(str::to_string),
            latitude: None,
            longitude: None,
            radius_km: None,
            companies: Vec::new(),
        }
    }

    /// Already cancelled when the task wakes → abandon the run AND never poll the
    /// lookup. The `biased;` ordering is what guarantees the second half: an
    /// unbiased select picks a ready branch at random and could fire the geocode
    /// request for a run nobody is waiting on.
    #[tokio::test]
    async fn pre_cancelled_run_is_abandoned_without_polling_the_lookup() {
        let token = tokio_util::sync::CancellationToken::new();
        token.cancel();
        let mut input = input_with(Some("Germany"), None);
        let polled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = polled.clone();

        let proceed = backfill_country_code_with(&token, &mut input, async move {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Some("de".to_string())
        })
        .await;

        assert!(!proceed, "a cancelled run must not proceed to the scrape");
        assert!(
            !polled.load(std::sync::atomic::Ordering::SeqCst),
            "the geocode lookup must never be issued for an already-cancelled run"
        );
        assert!(input.country_code.is_none());
    }

    /// A cancel landing WHILE the lookup is in flight wins immediately — the run
    /// is abandoned instead of waiting out the 2s geocode cap. `pending()` stands
    /// in for a hung Photon-fallback call: without the select the test would hang.
    #[tokio::test]
    async fn cancel_during_the_lookup_abandons_the_run() {
        let token = tokio_util::sync::CancellationToken::new();
        let mut input = input_with(Some("Amsterdam"), None);

        let canceller = token.clone();
        tokio::spawn(async move {
            tokio::task::yield_now().await;
            canceller.cancel();
        });

        let proceed = backfill_country_code_with(
            &token,
            &mut input,
            std::future::pending::<Option<String>>(),
        )
        .await;

        assert!(
            !proceed,
            "a cancel during the lookup must abandon the run, not wait it out"
        );
        assert!(
            input.country_code.is_none(),
            "an interrupted lookup must leave the field absent"
        );
    }

    /// The happy path: the lookup resolves, its country is written, and the run
    /// proceeds.
    #[tokio::test]
    async fn a_resolved_lookup_fills_the_country_and_proceeds() {
        let token = tokio_util::sync::CancellationToken::new();
        let mut input = input_with(Some("Amsterdam"), None);

        let proceed =
            backfill_country_code_with(&token, &mut input, std::future::ready(Some("nl".into())))
                .await;

        assert!(proceed);
        assert_eq!(input.country_code.as_deref(), Some("nl"));
    }

    /// A country the user PICKED is authoritative: no lookup is polled and the
    /// value is never overwritten (the backfill is for typed locations only).
    #[tokio::test]
    async fn an_existing_country_code_is_kept_and_costs_no_lookup() {
        let token = tokio_util::sync::CancellationToken::new();
        let mut input = input_with(Some("Austin, United States"), Some("us"));
        let polled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = polled.clone();

        let proceed = backfill_country_code_with(&token, &mut input, async move {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Some("de".to_string())
        })
        .await;

        assert!(proceed);
        assert_eq!(
            input.country_code.as_deref(),
            Some("us"),
            "a picked country must never be clobbered by the backfill"
        );
        assert!(
            !polled.load(std::sync::atomic::Ordering::SeqCst),
            "no geocode request may be issued when the country is already known"
        );
    }
}

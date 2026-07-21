use std::collections::HashMap;

use crate::db::{new_job_id, now_ms};
use crate::error::{AppError, AppResult};
use crate::postings::{attach_interactions, InteractionRecord, InteractionStore, PostingsCache};
use crate::scraping::cluster::{assign_clusters, posting_cluster_input, ClusterAssignment, ClusterInput};
use crate::scraping::{BoardSearchInput, ScraperEngine};
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
pub struct ScrapeUpdateDescriptionRequest {
    pub id: String,
    pub description: String,
}

/// Upper bound on a write-back description. A full JD is on the order of a few KB;
/// 256 KB is generous headroom while bounding a looping/XSS'd renderer from
/// ballooning a cached entry. Over-cap input is rejected, not silently truncated,
/// so a caller can tell the write didn't take effect as sent.
const MAX_DESCRIPTION_LEN: usize = 256 * 1024;

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
    let input = BoardSearchInput {
        query: req.query.clone(),
        location: req.location.clone(),
        // `amount` is the per-board cap: each board returns up to this many results.
        amount: req.amount.clamp(1, 100),
        pages: MAX_PAGE_BUDGET,
        date_filter: req.date_filter.clone(),
        // Structured search filters from the IPC request (ScrapeBoardsRequestSchema
        // in packages/shared). Optional, so absent fields stay None; LinkedIn's
        // search_paginated honors them and other boards ignore them. UI controls
        // for these are a follow-up — only the contract + propagation exist today.
        job_type: req.job_type.clone(),
        work_type: req.work_type.clone(),
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
    engine.register_token(&job_id, cancel_token).await;

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        // Hold the concurrency guard for the whole scrape; dropped on completion.
        let _guard = guard;
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
        let Ok(posting) = serde_json::from_value::<crate::scraping::JobPosting>(item.clone()) else {
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

#[tauri::command]
pub fn scrape_persist_job(app: AppHandle, req: ScrapePersistJobRequest) -> Value {
    let record = InteractionRecord {
        job_id: req.job.id.unwrap_or_default(),
        interaction_type: req.interaction_type,
        timestamp: now_ms(),
        title: req.job.title.unwrap_or_default(),
        company: req.job.company.unwrap_or_default(),
        url: req.job.url.unwrap_or_default(),
        source: req.job.source.unwrap_or_default(),
        location: req.job.location.unwrap_or_default(),
    };
    app.state::<Mutex<InteractionStore>>().lock().upsert(record);
    json!({ "success": true })
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
        Ok(Some(posting)) => serde_json::to_value(&posting).unwrap_or(json!(null)),
        _ => json!(null),
    }
}

/// Write a freshly-resolved full description back into the live postings cache,
/// keyed by posting id. The detail pane resolves a fuller description on demand
/// (see [`scrape_resolve_url`]); without this, match scoring would continue reading the
/// truncated aggregator snippet from the cache and produce incorrect scores.
///
/// Mutates the EXISTING cache entry in place (no new row, no persistence beyond
/// the in-memory cache — matching the cache's lifecycle). Returns `true` when an
/// entry was updated, `false` when the id isn't in the live cache (e.g. the cache
/// was cleared by a new search between resolve and write-back). The match-score
/// cache is job-text-hash-keyed, so updating the description invalidates cached
/// scores for that job; on-demand scoring via `useJobMatchScore` will recompute.
/// Validate the write-back inputs, returning the trimmed id on success. Pure (no
/// `AppHandle`) so the error paths are unit-tested directly. Rejects an empty id
/// and an over-cap description rather than silently truncating, so the caller can
/// tell the write didn't take effect as sent.
fn validate_update_description(id: &str, description: &str) -> AppResult<String> {
    let id = id.trim();
    if id.is_empty() {
        return Err(AppError::Validation("id is required".to_string()));
    }
    if description.len() > MAX_DESCRIPTION_LEN {
        return Err(AppError::Validation(format!(
            "description exceeds the {MAX_DESCRIPTION_LEN}-byte cap"
        )));
    }
    Ok(id.to_string())
}

#[tauri::command]
pub fn scrape_update_description(
    app: AppHandle,
    req: ScrapeUpdateDescriptionRequest,
) -> AppResult<bool> {
    let id = validate_update_description(&req.id, &req.description)?;
    let cache = app.state::<Mutex<PostingsCache>>();
    let updated = cache.lock().update_description(&id, &req.description);
    Ok(updated)
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

    // The request must deserialize from the camelCase wire shape the renderer
    // sends (`id`/`description`). Pins the serde contract without an AppHandle.
    #[test]
    fn update_description_request_deserializes_camel_case() {
        let json = r#"{"id":"job-1","description":"full text"}"#;
        let req: ScrapeUpdateDescriptionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, "job-1");
        assert_eq!(req.description, "full text");
    }

    #[test]
    fn validate_rejects_empty_or_whitespace_id() {
        assert!(
            matches!(
                validate_update_description("", "text"),
                Err(AppError::Validation(_))
            ),
            "empty id must be a validation error"
        );
        assert!(
            matches!(
                validate_update_description("   ", "text"),
                Err(AppError::Validation(_))
            ),
            "whitespace-only id must be a validation error"
        );
    }

    #[test]
    fn validate_rejects_over_cap_description() {
        let too_long = "x".repeat(MAX_DESCRIPTION_LEN + 1);
        assert!(
            matches!(
                validate_update_description("job-1", &too_long),
                Err(AppError::Validation(_))
            ),
            "a description past the cap must be rejected, not truncated"
        );
    }

    #[test]
    fn validate_accepts_valid_input_and_trims_id() {
        // At-cap is allowed (boundary): only strictly-over-cap is rejected.
        let at_cap = "x".repeat(MAX_DESCRIPTION_LEN);
        let id = validate_update_description("  job-1  ", &at_cap)
            .expect("a trimmed non-empty id with an at-cap description must validate");
        assert_eq!(id, "job-1", "the validated id must be trimmed");
    }
}

use serde::Deserialize;
use serde_json::{json, Value};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use crate::postings::{InteractionRecord, InteractionStore, PostingsCache};
use crate::scraping::{BoardSearchInput, ScraperEngine};

// ScrapeBoardRequest and ScrapeUrlRequest are generated from the Zod schemas in
// packages/shared by `pnpm gen:ipc`. See crate::ipc_contracts::scrape.
pub use crate::ipc_contracts::scrape::{ScrapeBoardRequest, ScrapeUrlRequest};

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

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("job-{t:x}")
}

#[tauri::command]
pub async fn scrape_board(app: AppHandle, req: ScrapeBoardRequest) -> Value {
    let job_id = uuid_v4();
    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .start(&job_id, "scrape.board");

    let engine = app.state::<std::sync::Arc<ScraperEngine>>().inner().clone();
    let input = BoardSearchInput {
        query: req.query.clone(),
        location: req.location.clone(),
        pages: req.pages,
        date_filter: req.date_filter.clone(),
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        locale: req.locale.clone(),
    };
    let board = req.board.clone();

    let app_progress = app.clone();
    let job_id_progress = job_id.clone();
    let on_progress = Box::new(move |p: f32| {
        let _ = app_progress.emit(
            "scrape.progress",
            json!({ "jobId": job_id_progress, "progress": p }),
        );
        if let Some(tracker) = app_progress.try_state::<Mutex<crate::jobs::JobTracker>>() {
            { let mut g = tracker.lock();
                g.update_progress(&job_id_progress, p as f64);
            }
        }
    });

    let app_item = app.clone();
    let job_id_item = job_id.clone();
    let on_item = Box::new(move |item: crate::scraping::JobPosting| {
        if let Some(cache) = app_item.try_state::<Mutex<PostingsCache>>() {
            { let mut guard = cache.lock();
                if let Ok(item_json) = serde_json::to_value(&item) {
                    guard.add(item_json);
                }
            }
        }
        
        let _ = app_item.emit(
            "jobs:event",
            json!({ "type": "job.stream", "jobId": job_id_item, "data": item, "ts": now_ms() })
        );
    });

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        let result = engine
            .scrape_board(&board, input, job_id_clone.clone(), Some(on_progress), Some(on_item))
            .await;

        let tracker = app_clone.state::<Mutex<crate::jobs::JobTracker>>();
        match &result {
            Ok(results) => {
                { let mut g = tracker.lock();
                    g.complete(&job_id_clone, json!({ "count": results.len() }));
                }
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.completed", "jobId": job_id_clone, "data": { "count": results.len() }, "ts": now_ms() })
                );
            }
            Err(e) => {
                { let mut g = tracker.lock();
                    g.fail(&job_id_clone, e.to_string());
                }
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.failed", "jobId": job_id_clone, "data": e.to_string(), "ts": now_ms() })
                );
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

    let job_id = uuid_v4();
    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .start(&job_id, "scrape.url");

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        let result = crate::scraping::scrape_url::resolve(&url).await;

        let tracker = app_clone.state::<Mutex<crate::jobs::JobTracker>>();
        match result {
            Ok(Some(posting)) => {
                if let Some(cache) = app_clone.try_state::<Mutex<PostingsCache>>() {
                    { let mut guard = cache.lock();
                        if let Ok(item_json) = serde_json::to_value(&posting) {
                            guard.add(item_json);
                        }
                    }
                }
                
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.stream", "jobId": job_id_clone, "data": posting, "ts": now_ms() })
                );

                { let mut g = tracker.lock();
                    g.complete(&job_id_clone, json!({ "ok": true }));
                }
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.completed", "jobId": job_id_clone, "data": { "count": 1 }, "ts": now_ms() })
                );
            }
            Ok(None) => {
                { let mut g = tracker.lock();
                    g.fail(&job_id_clone, "no scraper matched this URL".to_string());
                }
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.failed", "jobId": job_id_clone, "data": "no scraper matched this URL", "ts": now_ms() })
                );
            }
            Err(e) => {
                { let mut g = tracker.lock();
                    g.fail(&job_id_clone, e.to_string());
                }
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.failed", "jobId": job_id_clone, "data": e.to_string(), "ts": now_ms() })
                );
            }
        }
    });

    json!({ "jobId": job_id })
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
    app.state::<Mutex<InteractionStore>>()
        .lock()
        .upsert(record);
    json!({ "success": true })
}

/// Resolve a single job posting (incl. full description) from its URL.
/// Synchronous request/response — used to fetch a description on demand for
/// boards whose list scrape omits it (LinkedIn, Glassdoor, etc.).
#[tauri::command]
pub async fn scrape_resolve_url(url: String) -> Value {
    if url.is_empty() {
        return json!(null);
    }
    match crate::scraping::scrape_url::resolve(&url).await {
        Ok(Some(posting)) => serde_json::to_value(&posting).unwrap_or(json!(null)),
        _ => json!(null),
    }
}

#[tauri::command]
pub fn scrape_list_postings(app: AppHandle) -> Value {
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock();
    json!(guard.get_all())
}

#[tauri::command]
pub fn scrape_clear_postings(app: AppHandle) -> Value {
    app.state::<Mutex<PostingsCache>>()
        .lock()
        .clear_all();
    json!(null)
}

#[tauri::command]
pub fn scrape_list_interactions(app: AppHandle, filter: Option<ScrapeListFilter>) -> Value {
    let filter_type = filter.and_then(|f| f.interaction_type);
    let binding = app.state::<Mutex<InteractionStore>>();
    let mut store = binding.lock();
    json!(store.list(filter_type.as_deref()))
}


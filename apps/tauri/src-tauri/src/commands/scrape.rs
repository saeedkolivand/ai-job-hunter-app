use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use crate::postings::{InteractionRecord, InteractionStore, PostingsCache};
use crate::scraping::{BoardSearchInput, ScraperEngine};

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
pub async fn scrape_board(app: AppHandle, req: Value) -> Value {
    let board = req.get("board").and_then(|b| b.as_str()).unwrap_or("").to_string();
    let query = req.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string();
    let location = req.get("location").and_then(|l| l.as_str()).map(|s| s.to_string());
    let pages = req.get("pages").and_then(|p| p.as_u64()).unwrap_or(1) as u32;
    let date_filter = req.get("dateFilter").and_then(|d| d.as_str()).map(|s| s.to_string());
    let locale = req.get("locale").and_then(|l| l.as_str()).map(|s| s.to_string());

    let job_id = uuid_v4();
    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "scrape.board");

    let engine = app.state::<std::sync::Arc<ScraperEngine>>().inner().clone();
    let input = BoardSearchInput {
        query: query.clone(),
        location: location.clone(),
        pages,
        date_filter: date_filter.clone(),
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        locale: locale.clone(),
    };

    let app_progress = app.clone();
    let job_id_progress = job_id.clone();
    let on_progress = Box::new(move |p: f32| {
        let _ = app_progress.emit(
            "scrape.progress",
            json!({ "jobId": job_id_progress, "progress": p }),
        );
        if let Some(tracker) = app_progress.try_state::<Mutex<crate::jobs::JobTracker>>() {
            if let Ok(mut g) = tracker.lock() {
                g.update_progress(&job_id_progress, p as f64);
            }
        }
    });

    let app_item = app.clone();
    let job_id_item = job_id.clone();
    let on_item = Box::new(move |item: crate::scraping::JobPosting| {
        if let Some(cache) = app_item.try_state::<Mutex<PostingsCache>>() {
            if let Ok(mut guard) = cache.lock() {
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
                if let Ok(mut g) = tracker.lock() {
                    g.complete(&job_id_clone, json!({ "count": results.len() }));
                }
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.completed", "jobId": job_id_clone, "data": { "count": results.len() }, "ts": now_ms() })
                );
            }
            Err(e) => {
                if let Ok(mut g) = tracker.lock() {
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
pub async fn scrape_url(app: AppHandle, req: Value) -> Value {
    let url = req.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
    if url.is_empty() {
        return json!({ "error": "url is required" });
    }

    let job_id = uuid_v4();
    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "scrape.url");

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        let result = crate::scraping::scrape_url::resolve(&url).await;

        let tracker = app_clone.state::<Mutex<crate::jobs::JobTracker>>();
        match result {
            Ok(Some(posting)) => {
                if let Some(cache) = app_clone.try_state::<Mutex<PostingsCache>>() {
                    if let Ok(mut guard) = cache.lock() {
                        if let Ok(item_json) = serde_json::to_value(&posting) {
                            guard.add(item_json);
                        }
                    }
                }
                
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.stream", "jobId": job_id_clone, "data": posting, "ts": now_ms() })
                );

                if let Ok(mut g) = tracker.lock() {
                    g.complete(&job_id_clone, json!({ "ok": true }));
                }
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.completed", "jobId": job_id_clone, "data": { "count": 1 }, "ts": now_ms() })
                );
            }
            Ok(None) => {
                if let Ok(mut g) = tracker.lock() {
                    g.fail(&job_id_clone, "no scraper matched this URL".to_string());
                }
                let _ = app_clone.emit(
                    "jobs:event",
                    json!({ "type": "job.failed", "jobId": job_id_clone, "data": "no scraper matched this URL", "ts": now_ms() })
                );
            }
            Err(e) => {
                if let Ok(mut g) = tracker.lock() {
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
pub fn scrape_persist_job(app: AppHandle, req: Value) -> Value {
    if let (Some(job_obj), Some(interaction_type)) = (
        req.get("job"),
        req.get("interactionType").and_then(|v| v.as_str()),
    ) {
        let record = InteractionRecord {
            job_id: job_obj
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            interaction_type: interaction_type.to_string(),
            timestamp: now_ms(),
            title: job_obj
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            company: job_obj
                .get("company")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            url: job_obj
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            source: job_obj
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            location: job_obj
                .get("location")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        };
        app.state::<Mutex<InteractionStore>>()
            .lock()
            .unwrap()
            .upsert(record);
    }
    json!({ "success": true })
}

#[tauri::command]
pub fn scrape_list_postings(app: AppHandle) -> Value {
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock().unwrap();
    json!(guard.get_all())
}

#[tauri::command]
pub fn scrape_clear_postings(app: AppHandle) -> Value {
    app.state::<Mutex<PostingsCache>>()
        .lock()
        .unwrap()
        .clear_all();
    json!(null)
}

#[tauri::command]
pub fn scrape_list_interactions(app: AppHandle, filter: Option<Value>) -> Value {
    let filter_type = filter
        .as_ref()
        .and_then(|f| f.get("interactionType"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let binding = app.state::<Mutex<InteractionStore>>();
    let mut store = binding.lock().unwrap();
    json!(store.list(filter_type.as_deref()))
}

#[tauri::command]
pub async fn scrape_export_data(app: AppHandle) -> Value {
    use tauri_plugin_dialog::DialogExt;
    use tauri_plugin_dialog::FilePath;

    let interactions = {
        let binding = app.state::<Mutex<InteractionStore>>();
        let mut store = binding.lock().unwrap();
        store.export_all()
    };

    let default_name = format!("ajh-export-{}.json", chrono_date());
    let path = app
        .dialog()
        .file()
        .set_title("Export App Data")
        .set_file_name(&default_name)
        .add_filter("JSON", &["json"])
        .blocking_save_file();

    let Some(file_path) = path else {
        return json!({ "success": false });
    };

    let path_str = match file_path {
        FilePath::Path(p) => p.to_string_lossy().into_owned(),
        FilePath::Url(u) => u.to_string(),
    };

    let bundle = json!({
        "version": 1,
        "exportedAt": now_ms(),
        "interactions": interactions,
    });

    match std::fs::write(&path_str, serde_json::to_string_pretty(&bundle).unwrap_or_default()) {
        Ok(()) => json!({ "success": true, "filePath": path_str }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

#[tauri::command]
pub async fn scrape_import_data(app: AppHandle) -> Value {
    use tauri_plugin_dialog::DialogExt;
    use tauri_plugin_dialog::FilePath;

    let path = app
        .dialog()
        .file()
        .set_title("Import App Data")
        .add_filter("JSON", &["json"])
        .blocking_pick_file();

    let Some(file_path) = path else {
        return json!({ "success": false, "imported": 0 });
    };

    let path_str = match file_path {
        FilePath::Path(p) => p.to_string_lossy().into_owned(),
        FilePath::Url(u) => u.to_string(),
    };

    let raw = match std::fs::read_to_string(&path_str) {
        Ok(s) => s,
        Err(e) => return json!({ "success": false, "error": e.to_string(), "imported": 0 }),
    };

    let bundle: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            return json!({ "success": false, "error": "Invalid JSON", "imported": 0 })
        }
    };

    let interactions: Vec<InteractionRecord> = bundle
        .get("interactions")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let imported = app
        .state::<Mutex<InteractionStore>>()
        .lock()
        .unwrap()
        .import_bundle(interactions);

    json!({ "success": true, "imported": imported })
}

fn chrono_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    format!("{days}")
}

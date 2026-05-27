use serde_json::{json, Value};
use parking_lot::Mutex;
use tauri::{AppHandle, Manager};
use crate::jobs::JobTracker;
use crate::scraping::ScraperEngine;

#[tauri::command]
pub fn jobs_list(app: AppHandle) -> Value {
    let tracker = app.state::<Mutex<JobTracker>>();
    let guard = tracker.lock();
    json!(guard.list())
}

#[tauri::command]
pub fn jobs_get(app: AppHandle, job_id: String) -> Value {
    let tracker = app.state::<Mutex<JobTracker>>();
    let guard = tracker.lock();
    json!(guard.get(&job_id))
}

#[tauri::command]
pub async fn jobs_cancel(app: AppHandle, job_id: String) -> Value {
    let engine = app.state::<std::sync::Arc<ScraperEngine>>();
    engine.cancel(&job_id).await;

    app.state::<Mutex<JobTracker>>()
        .lock()
        .cancel(&job_id);
    json!({ "success": true })
}

#[tauri::command]
pub fn jobs_retry(app: AppHandle, job_id: String) -> Value {
    let tracker = app.state::<Mutex<JobTracker>>();
    let guard = tracker.lock();
    match guard.get(&job_id) {
        Some(rec) => json!({
            "success": true,
            "kind": rec.kind,
            "jobId": rec.id,
            "note": "renderer should re-dispatch this kind with the original payload",
        }),
        None => json!({ "success": false, "reason": "job id not found" }),
    }
}

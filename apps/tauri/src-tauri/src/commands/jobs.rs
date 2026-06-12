use crate::events::{emit_event, JobEvent, JOBS_EVENT};
use crate::jobs::JobTracker;
use crate::scraping::ScraperEngine;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

// ── L3 emit wrapper ───────────────────────────────────────────────────────────
//
// The `JobTracker` (L1) is AppHandle-free and must not emit. These thin wrappers
// are the single mutator boundary: they apply the in-memory/SQLite transition and
// then emit the matching `jobs:event` so the footer activity monitor reflects
// EVERY job of EVERY kind (ai.generate, autopilot.run, scrape.*, …), not just the
// few that used to emit ad-hoc. Call these instead of `tracker.lock().<mutator>()`.

fn emit_job_event(app: &AppHandle, kind: &str, job_id: &str, data: Option<Value>) {
    emit_event(
        app,
        JOBS_EVENT,
        JobEvent {
            r#type: kind.to_string(),
            job_id: job_id.to_string(),
            data,
            ts: crate::documents::now_ms() as i64,
        },
    );
}

/// Register a job as running and emit `job.started`.
pub fn job_start(app: &AppHandle, id: &str, kind: &str) {
    app.state::<Mutex<JobTracker>>().lock().start(id, kind);
    emit_job_event(app, "job.started", id, None);
}

/// Update a job's progress (0.0–1.0) and emit `job.progress`.
pub fn job_progress(app: &AppHandle, id: &str, p: f64) {
    app.state::<Mutex<JobTracker>>().lock().update_progress(id, p);
    emit_job_event(app, "job.progress", id, Some(json!({ "progress": p })));
}

/// Mark a job completed and emit `job.completed` (the result rides as `data`).
pub fn job_complete(app: &AppHandle, id: &str, result: Value) {
    app.state::<Mutex<JobTracker>>()
        .lock()
        .complete(id, result.clone());
    emit_job_event(app, "job.completed", id, Some(result));
}

/// Mark a job failed and emit `job.failed` (the error string rides as `data`).
pub fn job_fail(app: &AppHandle, id: &str, error: String) {
    app.state::<Mutex<JobTracker>>()
        .lock()
        .fail(id, error.clone());
    emit_job_event(app, "job.failed", id, Some(Value::String(error)));
}

/// Mark a job cancelled and emit `job.cancelled`.
pub fn job_cancel(app: &AppHandle, id: &str) {
    app.state::<Mutex<JobTracker>>().lock().cancel(id);
    emit_job_event(app, "job.cancelled", id, None);
}

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

    job_cancel(&app, &job_id);
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

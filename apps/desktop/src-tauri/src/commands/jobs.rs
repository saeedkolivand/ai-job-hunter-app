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
            ts: crate::db::now_ms() as i64,
        },
    );
}

/// Register a job as running and emit `job.started`.
pub fn job_start(app: &AppHandle, id: &str, kind: &str) {
    app.state::<Mutex<JobTracker>>().lock().start(id, kind);
    emit_job_event(app, "job.started", id, None);
}

/// Like [`job_start`], but refuses when a job of one of `exclusive_kinds` is
/// already active — returning that job's id instead of starting a second.
///
/// The scan and the insert share one lock (see
/// [`JobTracker::start_exclusive`]). Checking with a separate call and starting
/// after is check-then-act: two commands can both observe "nothing running"
/// before either registers, which for the embedding jobs means two concurrent
/// runs over the same documents and a cloud provider billed twice.
pub fn job_start_exclusive(
    app: &AppHandle,
    id: &str,
    kind: &str,
    exclusive_kinds: &[&str],
) -> Option<String> {
    let existing =
        app.state::<Mutex<JobTracker>>()
            .lock()
            .start_exclusive(id, kind, exclusive_kinds);
    if existing.is_none() {
        emit_job_event(app, "job.started", id, None);
    }
    existing
}

/// Park a job behind the concurrency limiter: status → `queued`, emitting
/// `job.queued` with how many callers are ahead of it.
///
/// The renderer suspends its stream deadline while a job is `queued` — see
/// `awaitAiStream`. Without that, a generation waiting its turn in a batch
/// counts down and fails having never sent a request.
pub fn job_queued(app: &AppHandle, id: &str, ahead: usize) {
    app.state::<Mutex<JobTracker>>()
        .lock()
        .set_waiting(id, true);
    emit_job_event(app, "job.queued", id, Some(json!({ "ahead": ahead })));
}

/// The counterpart to [`job_queued`]: a slot opened up, status → `running`.
pub fn job_dequeued(app: &AppHandle, id: &str) {
    app.state::<Mutex<JobTracker>>()
        .lock()
        .set_waiting(id, false);
    emit_job_event(app, "job.started", id, None);
}

/// Update a job's progress (0.0–1.0) and emit `job.progress`.
pub fn job_progress(app: &AppHandle, id: &str, p: f64) {
    app.state::<Mutex<JobTracker>>()
        .lock()
        .update_progress(id, p);
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

/// Like [`job_fail`], for a caller that can say more than the message text.
///
/// `message` still becomes the job's own tracked `error` (the
/// `jobs_get`/`jobs_list` surface, a `String` field with no i18n of its own —
/// see [`crate::commands::resume_pipeline::hooks::timeout_message`] for why
/// that stays plain English). `data` rides as `job.failed`'s event payload
/// INSTEAD of `message`, so a consumer that knows the shape (currently only
/// the staged pipeline's per-call timeout,
/// [`crate::commands::resume_pipeline::hooks::timeout_failure_data`]) can
/// render something better than the raw string — a localized message built
/// from structured fields, rather than an internal stage key spliced into
/// English prose.
pub fn job_fail_with_data(app: &AppHandle, id: &str, message: String, data: Value) {
    app.state::<Mutex<JobTracker>>().lock().fail(id, message);
    emit_job_event(app, "job.failed", id, Some(data));
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

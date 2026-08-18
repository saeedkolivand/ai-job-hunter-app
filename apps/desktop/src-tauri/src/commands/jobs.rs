use crate::events::{emit_event, JobEvent, JOBS_EVENT};
use crate::jobs::{JobTracker, KeyedExclusiveStart};
use crate::observability::sanitize_reason;
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

/// Like [`job_start_exclusive`], but the exclusion group is `kind` PLUS a
/// caller-supplied `key` (e.g. the model being pulled) rather than `kind`
/// alone — see [`JobTracker::start_exclusive_keyed`] for why kind-only
/// exclusivity is wrong for a command whose kind never varies but whose
/// target (the model) does, and for what each [`KeyedExclusiveStart`]
/// variant means to the caller.
pub fn job_start_exclusive_keyed(
    app: &AppHandle,
    id: &str,
    kind: &str,
    key_field: &str,
    key: &str,
) -> KeyedExclusiveStart {
    let outcome = app
        .state::<Mutex<JobTracker>>()
        .lock()
        .start_exclusive_keyed(id, kind, key_field, key);
    if matches!(outcome, KeyedExclusiveStart::Started) {
        emit_job_event(app, "job.started", id, None);
    }
    outcome
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

/// The AppHandle-free half of [`job_fail`]/[`job_fail_with_data`]: sanitize
/// the tracked error message, write it into the tracker, and hand back the
/// `job.failed` event payload the caller should still emit.
///
/// Split out so the sanitize step is testable without a `tauri::test` mock
/// app — this crate has none (see `documents::Embedder`'s doc comment for
/// why AppHandle-taking code goes through a seam like this one instead).
///
/// `error` is sanitized here, once, at the single mutator boundary — it
/// reaches the renderer through THREE surfaces (`jobs_get`, `jobs_list`, and
/// the `job.failed` event), and callers pass raw `e.to_string()` from
/// `reqwest`/`rusqlite`/keyring errors that can carry URLs, filesystem paths,
/// or hostnames. Sanitizing here catches every existing AND future caller
/// instead of relying on each one to remember. [`sanitize_reason`] is
/// conservative about plain English prose (see its own doc) so an
/// already-friendly message like
/// [`crate::commands::resume_pipeline::hooks::timeout_message`] passes
/// through unchanged.
///
/// `event_data` overrides what rides as the event payload (see
/// [`job_fail_with_data`]'s doc for why) — `None` reuses the sanitized error
/// string, mirroring the plain [`job_fail`] shape.
fn fail_in_tracker(
    tracker: &mut JobTracker,
    id: &str,
    error: String,
    event_data: Option<Value>,
) -> Value {
    let error = sanitize_reason(&error);
    tracker.fail(id, error.clone());
    event_data.unwrap_or(Value::String(error))
}

/// Mark a job failed and emit `job.failed` (the error string rides as `data`).
pub fn job_fail(app: &AppHandle, id: &str, error: String) {
    let data = fail_in_tracker(
        &mut app.state::<Mutex<JobTracker>>().lock(),
        id,
        error,
        None,
    );
    emit_job_event(app, "job.failed", id, Some(data));
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
/// English prose. `data` is a structured payload with its own meaning and is
/// NOT sanitized here — only `message`, the free-text half, is.
pub fn job_fail_with_data(app: &AppHandle, id: &str, message: String, data: Value) {
    let data = fail_in_tracker(
        &mut app.state::<Mutex<JobTracker>>().lock(),
        id,
        message,
        Some(data),
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    // A `reqwest`/keyring-shaped failure: a full URL carrying a query-string
    // credential, plus a Windows path an unwound filesystem error tacked on.
    // This is the PR #1036 leak class — sanitized in every `log::` call site,
    // but `job_fail` reached the renderer raw through `jobs_get`/`jobs_list`
    // and the `job.failed` event, a channel that sweep never touched.
    const RAW_ERROR: &str = r"error sending request to https://api.example.com/v1?api_key=SECRET123: connection reset while reading C:\Users\alice\AppData\Local\ajh\cache";

    #[test]
    fn job_fail_sanitizes_before_it_reaches_the_tracker_or_the_event() {
        let mut tracker = JobTracker::default();
        tracker.start("job-1", "test.kind");

        let event_data = fail_in_tracker(&mut tracker, "job-1", RAW_ERROR.to_string(), None);

        // The tracker's own `error` field — what `jobs_get`/`jobs_list` return.
        let tracked = tracker.get("job-1").and_then(|r| r.error.as_deref());
        for leaked in ["SECRET123", "api.example.com", "alice"] {
            assert!(
                !tracked.unwrap_or_default().contains(leaked),
                "tracker.error must not carry {leaked:?}: {tracked:?}"
            );
        }

        // The `job.failed` event payload.
        let emitted = event_data.as_str().unwrap_or_default();
        for leaked in ["SECRET123", "api.example.com", "alice"] {
            assert!(
                !emitted.contains(leaked),
                "job.failed data must not carry {leaked:?}: {emitted}"
            );
        }

        // MUTATION GUARD: a no-op passthrough (`error` written/emitted raw)
        // would leave both destinations byte-identical to `RAW_ERROR` — this
        // only passes when sanitization actually ran.
        assert_ne!(tracked, Some(RAW_ERROR));
        assert_ne!(emitted, RAW_ERROR);
    }

    #[test]
    fn job_fail_with_data_sanitizes_the_message_but_leaves_the_structured_data_alone() {
        let mut tracker = JobTracker::default();
        tracker.start("job-2", "test.kind");

        let structured = json!({ "kind": "timeout", "stage": "resume", "seconds": 45 });
        let event_data = fail_in_tracker(
            &mut tracker,
            "job-2",
            RAW_ERROR.to_string(),
            Some(structured.clone()),
        );

        let tracked = tracker.get("job-2").and_then(|r| r.error.as_deref());
        assert!(
            !tracked.unwrap_or_default().contains("SECRET123"),
            "tracker.error must not carry the credential: {tracked:?}"
        );
        // `data` rides as the event payload UNTOUCHED — it's a structured
        // payload with its own meaning, not free text.
        assert_eq!(event_data, structured);
    }

    #[test]
    fn job_fail_leaves_an_already_friendly_message_unchanged() {
        // `resume_pipeline::hooks::timeout_message`'s shape: plain English
        // prose with no path/URL/credential tokens. Sanitizing at the
        // `job_fail` boundary must not mangle it.
        let mut tracker = JobTracker::default();
        tracker.start("job-3", "test.kind");
        let msg = "The \"resume\" step didn't get a response within 45s. Try a faster model or \
                    a lower effort level.";

        let event_data = fail_in_tracker(&mut tracker, "job-3", msg.to_string(), None);

        assert_eq!(
            tracker.get("job-3").and_then(|r| r.error.as_deref()),
            Some(msg)
        );
        assert_eq!(event_data.as_str(), Some(msg));
    }
}

//! Application tracking commands — the IPC surface over [`crate::applications`].
//!
//! Seven capabilities (ADR 0001): list/get/set_status/update/delete plus the two
//! creation triggers that are NOT a generation save — `track` (manual create) and
//! `save_from_posting` (Jobs-page Save → `saved`). The Generate trigger lives in
//! [`crate::commands::ai_generations`] (it upserts the Application as a side-effect
//! of saving the document).
//!
//! All handlers use the centralized `AppResult`/`AppError` (serialized to the
//! existing string wire format) and open a trace [`Span`] for the mutating calls.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::applications::{ApplicationMeta, ApplicationStatus, ApplicationStore};
use crate::observability::Span;

// Generated from the Zod schemas in packages/shared by `pnpm gen:ipc`.
pub use crate::ipc_contracts::applications::{ApplicationTrackRequest, ApplicationUpdateRequest};

fn store(app: &AppHandle) -> tauri::State<'_, ApplicationStore> {
    app.state::<ApplicationStore>()
}

#[tauri::command]
pub async fn applications_list(app: AppHandle) -> Value {
    serde_json::to_value(store(&app).list()).unwrap_or(json!([]))
}

#[tauri::command]
pub async fn applications_get(app: AppHandle, id: String) -> Value {
    let s = store(&app);
    let app_rec = s.get(&id);
    let events = app_rec.as_ref().map(|_| s.events(&id)).unwrap_or_default();
    json!({ "application": app_rec, "events": events })
}

#[tauri::command]
pub async fn applications_set_status(
    app: AppHandle,
    id: String,
    status: String,
    note: Option<String>,
) -> Value {
    let span = Span::begin("applications", format!("set_status id={id} to={status}"));
    let to = ApplicationStatus::from_id(&status);
    let note = note.unwrap_or_default();
    match store(&app).set_status(&id, to, &note) {
        Ok(()) => {
            span.end(true);
            json!({ "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
}

#[tauri::command]
pub async fn applications_update(app: AppHandle, req: ApplicationUpdateRequest) -> Value {
    let span = Span::begin("applications", format!("update id={}", req.id));
    // `nextActionAt` is nullable+optional → generated as `Option<serde_json::Value>`.
    // Absent (None) = leave unchanged; explicit JSON `null` = clear the reminder;
    // a number = set it. Map that to the store's `Option<Option<u64>>` patch shape.
    let next_action_at: Option<Option<u64>> = req.next_action_at.map(|v| match v {
        serde_json::Value::Null => None,
        other => other.as_u64(),
    });
    let result = store(&app).update_fields(
        &req.id,
        req.notes,
        next_action_at,
        req.comp,
        req.contact_name,
        req.contact_email,
        req.job_summary,
    );
    match result {
        Ok(()) => {
            span.end(true);
            json!({ "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
}

#[tauri::command]
pub async fn applications_delete(app: AppHandle, id: String, keep_documents: bool) -> Value {
    let span = Span::begin(
        "applications",
        format!("delete id={id} keep_documents={keep_documents}"),
    );
    let s = store(&app);
    // When NOT keeping documents, delete the child generations linked to this
    // Application first; either way the Application + its history are removed.
    if !keep_documents {
        if let Some(gens) = app.try_state::<crate::ai_generations::AiGenerationStore>() {
            if let Err(e) = gens.remove_for_application(&id) {
                log::warn!("[applications] failed to delete child generations (non-fatal): {e}");
            }
        }
    } else if let Some(gens) = app.try_state::<crate::ai_generations::AiGenerationStore>() {
        // Keep documents: detach them so they survive as orphaned generations.
        if let Err(e) = gens.detach_application(&id) {
            log::warn!("[applications] failed to detach child generations (non-fatal): {e}");
        }
    }
    match s.delete(&id, keep_documents) {
        Ok(()) => {
            span.end(true);
            json!({ "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
}

#[tauri::command]
pub async fn applications_track(app: AppHandle, req: ApplicationTrackRequest) -> Value {
    let span = Span::begin("applications", "track (manual)".to_string());
    let meta = ApplicationMeta {
        company: req.company.unwrap_or_default(),
        title: req.title.unwrap_or_default(),
        candidate: req.candidate.unwrap_or_default(),
        brief: String::new(),
        answers: vec![],
        job_summary: String::new(),
    };
    let job_url = req.job_url.unwrap_or_default();
    let board = req.board.unwrap_or_default();
    match store(&app).track_manual(&job_url, &board, &meta) {
        Ok(id) => {
            span.end(true);
            json!({ "id": id, "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
}

#[tauri::command]
pub async fn applications_save_from_posting(app: AppHandle, req: ApplicationTrackRequest) -> Value {
    // Jobs-page "Save" → a `saved` (pre-apply) Application. Same request shape as
    // `track`, but the origin keeps it pre-apply instead of marking it applied.
    let span = Span::begin("applications", "save_from_posting".to_string());
    let meta = ApplicationMeta {
        company: req.company.unwrap_or_default(),
        title: req.title.unwrap_or_default(),
        candidate: req.candidate.unwrap_or_default(),
        brief: String::new(),
        answers: vec![],
        job_summary: String::new(),
    };
    let job_url = req.job_url.unwrap_or_default();
    let board = req.board.unwrap_or_default();
    match store(&app).upsert_for_origin(
        &job_url,
        &board,
        &meta,
        crate::applications::ApplicationOrigin::Saved,
        Some(false),
    ) {
        Ok(id) => {
            span.end(true);
            json!({ "id": id, "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
}

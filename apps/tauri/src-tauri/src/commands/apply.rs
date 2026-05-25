use crate::applying::types::{ApplyContext, ApplyStep};
use crate::apply_helpers::{decode_resume_to_temp, setup_apply_job};
use crate::scraping::ScraperEngine;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

#[tauri::command]
pub async fn apply_start(app: AppHandle, req: Value) -> Value {
    use crate::applying::registry::ApplierRegistry;

    let board = req.get("board").and_then(|b| b.as_str()).unwrap_or("");
    let url = req.get("url").and_then(|u| u.as_str()).unwrap_or("");
    if board.is_empty() || url.is_empty() {
        return json!({ "error": "board and url are required" });
    }

    let applier = match ApplierRegistry::get(board) {
        Some(a) => a,
        None => return json!({ "error": format!("no applier for board: {board}") }),
    };

    let temp_resume = decode_resume_to_temp(&req);
    let engine = app.state::<std::sync::Arc<ScraperEngine>>().inner().clone();
    let (job_id, cancel_token) = setup_apply_job(&app, engine.clone());

    let on_step = Some(create_step_callback(app.clone(), job_id.clone()));
    let on_progress = Some(create_progress_callback(app.clone(), job_id.clone()));

    let ctx = build_apply_context(
        &req,
        cancel_token,
        temp_resume.as_ref(),
        on_step,
        on_progress,
    );

    let result = applier.apply(url.to_string(), ctx).await;

    engine.unregister_token(&job_id).await;
    cleanup_temp_resume(temp_resume);

    let tracker = app.state::<Mutex<crate::jobs::JobTracker>>();
    match result {
        Ok(r) => {
            if let Ok(mut g) = tracker.lock() {
                g.complete(
                    &job_id,
                    json!({
                        "ok": r.ok,
                        "stage": r.stage,
                        "submitted": r.submitted,
                        "url": r.url,
                        "note": r.note,
                    }),
                );
            }
            json!({
                "jobId": job_id,
                "ok": r.ok,
                "stage": r.stage,
                "submitted": r.submitted,
                "url": r.url,
                "note": r.note,
            })
        }
        Err(e) => {
            if let Ok(mut g) = tracker.lock() {
                g.fail(&job_id, e.to_string());
            }
            json!({ "jobId": job_id, "error": e.to_string() })
        }
    }
}

#[tauri::command]
pub async fn apply_catalog(_app: AppHandle) -> Value {
    let catalog: Vec<Value> = crate::applying::registry::ApplierRegistry::catalog()
        .into_iter()
        .map(|(id, name)| json!({ "id": id, "displayName": name }))
        .collect();
    json!(catalog)
}

// Helper functions

fn create_step_callback(
    app: AppHandle,
    job_id: String,
) -> Box<dyn Fn(ApplyStep) + Send> {
    Box::new(move |step: ApplyStep| {
        let _ = app.emit(
            "apply.step",
            json!({
                "jobId": job_id,
                "stage": step.stage,
                "ok": step.ok,
                "note": step.note,
            }),
        );
    })
}

fn create_progress_callback(
    app: AppHandle,
    job_id: String,
) -> Box<dyn Fn(f32, String) + Send> {
    Box::new(move |p: f32, stage: String| {
        let _ = app.emit(
            "apply.progress",
            json!({ "jobId": job_id, "progress": p, "stage": stage }),
        );
    })
}

fn build_apply_context(
    req: &Value,
    cancel_token: CancellationToken,
    temp_resume: Option<&std::path::PathBuf>,
    on_step: Option<Box<dyn Fn(ApplyStep) + Send>>,
    on_progress: Option<Box<dyn Fn(f32, String) + Send>>,
) -> ApplyContext {
    ApplyContext {
        signal: cancel_token,
        cover_letter: req
            .get("coverLetter")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        resume_path: temp_resume.as_ref().map(|p| p.to_string_lossy().to_string()),
        auto_submit: req
            .get("autoSubmit")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        on_progress,
        on_step,
    }
}

fn cleanup_temp_resume(temp_resume: Option<std::path::PathBuf>) {
    if let Some(path) = temp_resume {
        let _ = std::fs::remove_file(path);
    }
}

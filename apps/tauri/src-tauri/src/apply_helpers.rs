use crate::applying::types::{ApplyContext, ApplyStep};
use crate::scraping::ScraperEngine;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Decode base64 resume bytes to a temporary file
pub fn decode_resume_to_temp(req: &Value) -> Option<std::path::PathBuf> {
    use base64::Engine;

    let (b64, name) = (
        req.get("resumeBytesBase64").and_then(|v| v.as_str()),
        req.get("resumeName").and_then(|v| v.as_str()),
    );

    match (b64, name) {
        (Some(b64), Some(name)) => {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                let ext = std::path::Path::new(name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("pdf");
                let path = std::env::temp_dir().join(format!("ajh-resume-{}.{ext}", Uuid::new_v4()));
                if std::fs::write(&path, &bytes).is_ok() {
                    return Some(path);
                }
            }
        }
        _ => {}
    }
    None
}

/// Setup job tracking and cancellation token
pub fn setup_apply_job(
    app: &AppHandle,
    engine: std::sync::Arc<ScraperEngine>,
) -> (String, CancellationToken) {
    let job_id = Uuid::new_v4().to_string();
    app.state::<std::sync::Mutex<crate::jobs::JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "apply.start");

    let cancel_token = CancellationToken::new();
    let engine_clone = engine.clone();
    let job_id_clone = job_id.clone();
    let token_clone = cancel_token.clone();
    tokio::spawn(async move {
        engine_clone.register_token(&job_id_clone, token_clone).await;
    });

    (job_id, cancel_token)
}

/// Create step callback for apply progress
pub fn create_step_callback(
    app: AppHandle,
    job_id: String,
) -> Box<dyn Fn(ApplyStep) + Send> {
    Box::new(move |step: ApplyStep| {
        let _ = app.emit(
            "apply.step",
            serde_json::json!({
                "jobId": job_id,
                "stage": step.stage,
                "ok": step.ok,
                "note": step.note,
            }),
        );
    })
}

/// Create progress callback for apply progress
pub fn create_progress_callback(
    app: AppHandle,
    job_id: String,
) -> Box<dyn Fn(f32, String) + Send> {
    Box::new(move |p: f32, stage: String| {
        let _ = app.emit(
            "apply.progress",
            serde_json::json!({ "jobId": job_id, "progress": p, "stage": stage }),
        );
    })
}

/// Build apply context from request
pub fn build_apply_context(
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

/// Cleanup temporary resume file
pub fn cleanup_temp_resume(temp_resume: Option<std::path::PathBuf>) {
    if let Some(path) = temp_resume {
        let _ = std::fs::remove_file(path);
    }
}

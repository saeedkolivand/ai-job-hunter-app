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

    if let (Some(b64), Some(name)) = (b64, name) {
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


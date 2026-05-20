/// Tauri command implementations for the AJH shell.
///
/// Real commands: system_health/version/platform/locale/openExternal,
///                scrape_board, scrape_url (proxy to scraper sidecar),
///                dialog_open_files.
///
/// Stub commands: everything else — they return null / empty list so the UI
/// renders with empty states. Parity is built incrementally by replacing
/// stubs with sidecar HTTP proxies.
use std::sync::Mutex;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::sidecar::ScraperSidecarState;

// ── Sidecar HTTP helpers ──────────────────────────────────────────────────────

/// Returns the scraper sidecar port if ready, or None.
fn sidecar_port(app: &AppHandle) -> Option<u16> {
    app.state::<Mutex<ScraperSidecarState>>()
        .lock()
        .ok()
        .and_then(|g| g.port)
}

/// POST a ScraperCommand to the sidecar and collect SSE ScraperEvents.
/// Forwards each event to the Tauri event bus so the renderer's onEvent /
/// onStream handlers fire just like they do in Electron.
async fn post_sidecar_command(
    app: &AppHandle,
    port: u16,
    cmd: &Value,
) -> Result<Value, String> {
    let url = format!("http://127.0.0.1:{port}/command");

    let response = reqwest::Client::new()
        .post(&url)
        .json(cmd)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body = response.text().await.map_err(|e| e.to_string())?;

    let mut last_done: Value = json!(null);

    for line in body.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(event) = serde_json::from_str::<Value>(data) {
                let kind = event.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                match kind {
                    "done" => {
                        last_done = event.get("result").cloned().unwrap_or(json!(null));
                        let job_id = event.get("jobId").cloned().unwrap_or(json!(""));
                        let _ = app.emit("jobs:event", json!({"type":"completed","jobId":job_id}));
                    }
                    "progress" => {
                        let _ = app.emit("jobs:event", event.clone());
                    }
                    "item" => {
                        let _ = app.emit("jobs:event", event.clone());
                    }
                    "error" => {
                        let msg = event.get("message").and_then(|m| m.as_str()).unwrap_or("sidecar error");
                        return Err(msg.to_string());
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(last_done)
}

use tauri::Manager;

// ── System ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn system_health(app: AppHandle) -> Value {
    let scraper_ready = sidecar_port(&app).is_some();
    json!({
        "status": "ok",
        "shell": "tauri",
        "scraper": { "mode": "http-sidecar", "ready": scraper_ready },
        "ai": { "available": false },
        "data": { "available": false }
    })
}

#[tauri::command]
pub fn system_get_version() -> Value {
    json!({ "version": env!("CARGO_PKG_VERSION"), "shell": "tauri" })
}

#[tauri::command]
pub fn system_get_locale() -> Value {
    json!("en")
}

#[tauri::command]
pub fn system_set_locale(_locale: String) -> Value {
    json!(null)
}

#[tauri::command]
pub fn system_get_platform() -> Value {
    json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "shell": "tauri"
    })
}

#[tauri::command]
pub async fn system_open_external(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn system_set_performance_mode(_mode: String) -> Value {
    json!(null)
}

#[tauri::command]
pub fn system_get_metrics() -> Value {
    json!({
        "shell": "tauri",
        "uptime": 0,
        "memoryMb": 0,
        "cpuPercent": 0
    })
}

// ── Jobs ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn jobs_list() -> Value {
    json!([])
}

#[tauri::command]
pub fn jobs_get(_job_id: String) -> Value {
    json!(null)
}

#[tauri::command]
pub fn jobs_cancel(_job_id: String) -> Value {
    json!(null)
}

#[tauri::command]
pub fn jobs_retry(_job_id: String) -> Value {
    json!(null)
}

// ── AI ───────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn ai_generate(_req: Value) -> Value {
    json!({ "error": "AI runtime not yet available in Tauri spike" })
}

#[tauri::command]
pub fn ai_list_models() -> Value {
    json!([])
}

#[tauri::command]
pub fn ai_pull_model(_model: String) -> Value {
    json!(null)
}

#[tauri::command]
pub fn ai_unload_model(_model: String) -> Value {
    json!(null)
}

#[tauri::command]
pub fn ai_embed(_req: Value) -> Value {
    json!(null)
}

// ── Documents ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn documents_list() -> Value {
    json!([])
}

#[tauri::command]
pub fn documents_import(_req: Value) -> Value {
    json!(null)
}

#[tauri::command]
pub fn documents_remove(_id: String) -> Value {
    json!(null)
}

// ── Search ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn search_hybrid(_req: Value) -> Value {
    json!({ "items": [], "total": 0 })
}

// ── Scrape ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn scrape_board(app: AppHandle, req: Value) -> Value {
    let Some(port) = sidecar_port(&app) else {
        return json!({ "error": "scraper sidecar not ready — start the scraper-runtime binary" });
    };
    let job_id = uuid_v4();
    let cmd = json!({ "kind": "scrape.board", "jobId": job_id, "payload": req });
    match post_sidecar_command(&app, port, &cmd).await {
        Ok(result) => result,
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
pub async fn scrape_url(app: AppHandle, req: Value) -> Value {
    let Some(port) = sidecar_port(&app) else {
        return json!({ "error": "scraper sidecar not ready — start the scraper-runtime binary" });
    };
    let job_id = uuid_v4();
    let url = req.get("url").and_then(|u| u.as_str()).unwrap_or("");
    let cmd = json!({ "kind": "scrape.url", "jobId": job_id, "payload": { "url": url } });
    match post_sidecar_command(&app, port, &cmd).await {
        Ok(result) => result,
        Err(e) => json!({ "error": e }),
    }
}

fn uuid_v4() -> String {
    // Simple random ID without pulling in uuid crate — sufficient for job IDs.
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("job-{t:x}")
}

#[tauri::command]
pub fn scrape_persist_job(_req: Value) -> Value {
    json!(null)
}

#[tauri::command]
pub fn scrape_list_postings() -> Value {
    json!([])
}

#[tauri::command]
pub fn scrape_clear_postings() -> Value {
    json!(null)
}

#[tauri::command]
pub fn scrape_list_interactions(_filter: Option<Value>) -> Value {
    json!([])
}

#[tauri::command]
pub fn scrape_export_data() -> Value {
    json!(null)
}

#[tauri::command]
pub fn scrape_import_data() -> Value {
    json!(null)
}

// ── Match ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn match_resume(_req: Value) -> Value {
    json!(null)
}

// ── Credentials ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn credentials_available() -> Value {
    json!(false)
}

#[tauri::command]
pub fn credentials_list() -> Value {
    json!([])
}

#[tauri::command]
pub fn credentials_set(_req: Value) -> Value {
    json!(null)
}

#[tauri::command]
pub fn credentials_remove(_board_id: String) -> Value {
    json!(null)
}

// ── LinkedIn / Boards ────────────────────────────────────────────────────────

#[tauri::command]
pub fn linkedin_connect() -> Value {
    json!({ "status": "not_connected" })
}

#[tauri::command]
pub fn linkedin_disconnect() -> Value {
    json!(null)
}

#[tauri::command]
pub fn linkedin_get_status() -> Value {
    json!({ "status": "not_connected" })
}

#[tauri::command]
pub fn boards_connect(_board_id: String) -> Value {
    json!({ "status": "not_connected" })
}

#[tauri::command]
pub fn boards_disconnect(_board_id: String) -> Value {
    json!(null)
}

#[tauri::command]
pub fn boards_get_status(_board_id: String) -> Value {
    json!({ "status": "not_connected" })
}

// ── Privacy ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn privacy_sign_out_all() -> Value {
    json!(null)
}

#[tauri::command]
pub fn privacy_clear_interactions() -> Value {
    json!(null)
}

// ── Apply ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn apply_start(_req: Value) -> Value {
    json!({ "error": "Apply not yet available in Tauri spike" })
}

#[tauri::command]
pub fn apply_catalog() -> Value {
    json!([])
}

// ── Resume ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn resume_extract_text(_req: Value) -> Value {
    json!({ "error": "Resume extraction not yet available in Tauri spike" })
}

// ── Support ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn support_export_diagnostics() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reload_ai_runtime() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_unload_all_models() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reset_model_configuration() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_rebuild_vector_indexes() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_clear_embeddings_cache() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reset_vector_database() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_clear_ocr_cache() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reindex_all_documents() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_reset_all_sessions() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_clear_scraping_queue() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_copy_environment_details() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_copy_app_version() -> Value {
    json!(null)
}

#[tauri::command]
pub fn support_copy_system_info() -> Value {
    json!(null)
}

// ── Conversations ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn conversations_get_or_create() -> Value {
    json!({ "id": "default", "createdAt": 0 })
}

#[tauri::command]
pub fn conversations_load_messages(_conversation_id: String) -> Value {
    json!([])
}

#[tauri::command]
pub fn conversations_save_message(_req: Value) -> Value {
    json!(null)
}

// ── Native dialogs ────────────────────────────────────────────────────────────

/// Open a native file picker and return the selected file paths.
/// Used by the document import flow instead of passing raw filesystem paths
/// over IPC — the renderer receives paths it can then read via the backend.
///
/// Replaces the Electron `dialog.showOpenDialog` equivalent.
#[tauri::command]
pub async fn dialog_open_files(
    app: AppHandle,
    title: Option<String>,
    filters: Option<Vec<DialogFilter>>,
) -> Vec<String> {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let mut builder = app.dialog().file();
    if let Some(t) = title {
        builder = builder.set_title(&t);
    }
    if let Some(fs) = filters {
        for f in fs {
            builder = builder.add_filter(&f.name, &f.extensions.iter().map(String::as_str).collect::<Vec<_>>());
        }
    }

    builder
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|p| match p {
            FilePath::Path(pb) => pb.to_string_lossy().into_owned(),
            FilePath::Url(u) => u.to_string(),
        })
        .collect()
}

#[derive(serde::Deserialize)]
pub struct DialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

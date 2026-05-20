/// Tauri command implementations for the AJH shell.
///
/// Real commands: system_health/version/platform/locale/openExternal,
///                scrape_board, scrape_url (proxy to scraper sidecar),
///                credentials_* (OS keychain via keyring crate),
///                dialog_open_files.
///
/// Stub commands: everything else — they return null / empty list so the UI
/// renders with empty states. Parity is built incrementally.
use std::sync::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use crate::autopilot::{AutopilotStatus, AutopilotStore};
use crate::credentials::CredentialStore;
use crate::jobs::JobTracker;
use crate::postings::{InteractionRecord, InteractionStore, PostingsCache};
use crate::sidecar::ScraperSidecarState;

// ── Sidecar HTTP helpers ──────────────────────────────────────────────────────

fn sidecar_port(app: &AppHandle) -> Option<u16> {
    app.state::<Mutex<ScraperSidecarState>>()
        .lock()
        .ok()
        .and_then(|g| g.port)
}

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

/// POST a ScraperCommand to the sidecar, stream SSE events back, and
/// update JobTracker + PostingsCache while events arrive.
async fn post_sidecar_command(
    app: &AppHandle,
    port: u16,
    cmd: &Value,
    job_id: &str,
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
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .unwrap()
                            .complete(job_id, last_done.clone());
                        let _ = app.emit("jobs:event", json!({"type":"completed","jobId":job_id}));
                    }
                    "progress" => {
                        let p = event.get("p").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .unwrap()
                            .update_progress(job_id, p);
                        let _ = app.emit("jobs:event", event.clone());
                    }
                    "item" => {
                        if let Some(item) = event.get("item").cloned() {
                            app.state::<Mutex<PostingsCache>>()
                                .lock()
                                .unwrap()
                                .add(item);
                        }
                        let _ = app.emit("jobs:event", event.clone());
                    }
                    "error" => {
                        let msg = event
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("sidecar error")
                            .to_string();
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .unwrap()
                            .fail(job_id, msg.clone());
                        return Err(msg);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(last_done)
}

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
pub fn jobs_list(app: AppHandle) -> Value {
    let tracker = app.state::<Mutex<JobTracker>>();
    let guard = tracker.lock().unwrap();
    json!(guard.list())
}

#[tauri::command]
pub fn jobs_get(app: AppHandle, job_id: String) -> Value {
    let tracker = app.state::<Mutex<JobTracker>>();
    let guard = tracker.lock().unwrap();
    json!(guard.get(&job_id))
}

#[tauri::command]
pub async fn jobs_cancel(app: AppHandle, job_id: String) -> Value {
    // Tell the sidecar to abort then update local tracker.
    if let Some(port) = sidecar_port(&app) {
        let cmd = json!({ "kind": "cancel", "jobId": job_id });
        reqwest::Client::new()
            .post(format!("http://127.0.0.1:{port}/command"))
            .json(&cmd)
            .send()
            .await
            .ok();
    }
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .cancel(&job_id);
    json!({ "success": true })
}

#[tauri::command]
pub fn jobs_retry(_app: AppHandle, _job_id: String) -> Value {
    // Retry is a no-op in the sidecar model — the caller re-enqueues.
    json!({ "success": false, "reason": "retry not supported in sidecar mode" })
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
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "scrape.board");
    let cmd = json!({ "kind": "scrape.board", "jobId": job_id, "payload": req });
    match post_sidecar_command(&app, port, &cmd, &job_id).await {
        Ok(_) => json!({ "jobId": job_id }),
        Err(e) => json!({ "error": e, "jobId": job_id }),
    }
}

#[tauri::command]
pub async fn scrape_url(app: AppHandle, req: Value) -> Value {
    let Some(port) = sidecar_port(&app) else {
        return json!({ "error": "scraper sidecar not ready — start the scraper-runtime binary" });
    };
    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "scrape.url");
    let url_str = req.get("url").and_then(|u| u.as_str()).unwrap_or("");
    let cmd = json!({ "kind": "scrape.url", "jobId": job_id, "payload": { "url": url_str } });
    match post_sidecar_command(&app, port, &cmd, &job_id).await {
        Ok(result) => result,
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
pub fn scrape_persist_job(app: AppHandle, req: Value) -> Value {
    // Record a user interaction with a job (viewed, applied, bookmarked, etc.)
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
    // Simple ISO date without chrono crate.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    // Approximate: good enough for a filename.
    format!("{days}")
}

// ── Match ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn match_resume(_req: Value) -> Value {
    json!(null)
}

// ── Credentials ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn credentials_available(app: AppHandle) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    json!(guard.is_available())
}

#[tauri::command]
pub fn credentials_list(app: AppHandle) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    json!(guard.list())
}

#[tauri::command]
pub fn credentials_set(app: AppHandle, req: Value) -> Value {
    let board_id = req.get("boardId").and_then(|v| v.as_str()).unwrap_or("");
    let username = req.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let password = req.get("password").and_then(|v| v.as_str()).unwrap_or("");

    if board_id.is_empty() || username.is_empty() || password.is_empty() {
        return json!({ "error": "boardId, username, and password are required" });
    }

    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    match guard.set(board_id, username, password) {
        Ok(()) => {
            // Forward to sidecar if it is running.
            let port = app
                .state::<Mutex<ScraperSidecarState>>()
                .lock()
                .ok()
                .and_then(|g| g.port);
            if let Some(port) = port {
                let cmd = serde_json::json!({
                    "kind": "set.credentials",
                    "boardId": board_id,
                    "username": username,
                    "password": password,
                });
                tauri::async_runtime::spawn(async move {
                    reqwest::Client::new()
                        .post(format!("http://127.0.0.1:{port}/command"))
                        .json(&cmd)
                        .send()
                        .await
                        .ok();
                });
            }
            json!({ "success": true })
        }
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
pub fn credentials_remove(app: AppHandle, board_id: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock().unwrap();
    match guard.remove(&board_id) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e }),
    }
}

// ── LinkedIn / Boards ─────────────────────────────────────────────────────────
// Both sets delegate to the sidecar's open.login / board.status / board.disconnect
// commands so the login flow runs in a headed Playwright browser.

async fn sidecar_open_login(app: &AppHandle, board_id: &str) -> Value {
    let Some(port) = sidecar_port(app) else {
        return json!({ "connected": false, "error": "scraper sidecar not ready" });
    };
    let job_id = uuid_v4();
    let cmd = json!({ "kind": "open.login", "boardId": board_id });
    match post_sidecar_command(app, port, &cmd, &job_id).await {
        Ok(result) => result,
        Err(e) => json!({ "connected": false, "error": e }),
    }
}

async fn sidecar_board_status(app: &AppHandle, board_id: &str) -> Value {
    let Some(port) = sidecar_port(app) else {
        return json!({ "connected": false });
    };
    let job_id = uuid_v4();
    let cmd = json!({ "kind": "board.status", "boardId": board_id });
    match post_sidecar_command(app, port, &cmd, &job_id).await {
        Ok(result) => result,
        Err(_) => json!({ "connected": false }),
    }
}

async fn sidecar_board_disconnect(app: &AppHandle, board_id: &str) -> Value {
    let Some(port) = sidecar_port(app) else {
        return json!(null);
    };
    let job_id = uuid_v4();
    let cmd = json!({ "kind": "board.disconnect", "boardId": board_id });
    post_sidecar_command(app, port, &cmd, &job_id).await.ok();
    json!(null)
}

#[tauri::command]
pub async fn linkedin_connect(app: AppHandle) -> Value {
    sidecar_open_login(&app, "linkedin").await
}

#[tauri::command]
pub async fn linkedin_disconnect(app: AppHandle) -> Value {
    sidecar_board_disconnect(&app, "linkedin").await
}

#[tauri::command]
pub async fn linkedin_get_status(app: AppHandle) -> Value {
    sidecar_board_status(&app, "linkedin").await
}

#[tauri::command]
pub async fn boards_connect(app: AppHandle, board_id: String) -> Value {
    sidecar_open_login(&app, &board_id).await
}

#[tauri::command]
pub async fn boards_disconnect(app: AppHandle, board_id: String) -> Value {
    sidecar_board_disconnect(&app, &board_id).await
}

#[tauri::command]
pub async fn boards_get_status(app: AppHandle, board_id: String) -> Value {
    sidecar_board_status(&app, &board_id).await
}

// ── Privacy ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn privacy_sign_out_all(app: AppHandle) -> Value {
    // Clear cached postings and interactions from the in-process stores.
    // Browser state directories are managed by the sidecar (Playwright storage).
    app.state::<Mutex<PostingsCache>>()
        .lock()
        .unwrap()
        .clear_all();
    app.state::<Mutex<InteractionStore>>()
        .lock()
        .unwrap()
        .clear_all();
    json!({ "success": true })
}

#[tauri::command]
pub fn privacy_clear_interactions(app: AppHandle) -> Value {
    app.state::<Mutex<InteractionStore>>()
        .lock()
        .unwrap()
        .clear_all();
    json!({ "success": true })
}

// ── Apply ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn apply_start(_req: Value) -> Value {
    json!({ "error": "Apply not yet available in Tauri spike" })
}

#[tauri::command]
pub async fn apply_catalog(app: AppHandle) -> Value {
    // Return the sidecar's scraper catalog — these are the boards the applier supports.
    let Some(port) = sidecar_port(&app) else {
        return json!([]);
    };
    let cmd = json!({ "kind": "catalog" });
    let job_id = uuid_v4();
    match post_sidecar_command(&app, port, &cmd, &job_id).await {
        Ok(_) => json!([]), // catalog reply comes via the event bus as catalog.reply
        Err(_) => json!([]),
    }
}

// ── Resume ───────────────────────────────────────────────────────────────────

/// Extract plain text from a resume file (PDF, DOCX, TXT, MD).
///
/// The renderer passes `{ name: string, bytes: Uint8Array }`.
/// Bytes are re-encoded as base64 before sending to the sidecar because the
/// sidecar protocol is JSON-over-HTTP and binary data needs encoding.
#[tauri::command]
pub async fn resume_extract_text(app: AppHandle, req: Value) -> Value {
    let Some(port) = sidecar_port(&app) else {
        return json!({ "error": "scraper sidecar not ready" });
    };

    let name = req.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // Bytes arrive as a JSON array of integers (Tauri serialises Uint8Array).
    let bytes_b64 = match req.get("bytes") {
        Some(serde_json::Value::Array(arr)) => {
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        }
        _ => return json!({ "error": "bytes field missing or invalid" }),
    };

    let job_id = uuid_v4();
    let cmd = json!({
        "kind": "extract.text",
        "jobId": job_id,
        "payload": { "name": name, "bytesBase64": bytes_b64 },
    });

    match post_sidecar_command(&app, port, &cmd, &job_id).await {
        Ok(result) => result,
        Err(e) => json!({ "error": e }),
    }
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
pub fn support_copy_environment_details(app: AppHandle) -> Value {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let text = format!(
        "AI Job Hunter (Tauri)\nVersion: {}\nShell: tauri\nOS: {} {}\nArch: {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        // Reading OS release is impractical from Rust without extra crates;
        // the OS name + arch is the practical diagnostic minimum.
        std::env::consts::FAMILY,
        std::env::consts::ARCH,
    );
    match app.clipboard().write_text(text) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub fn support_copy_app_version(app: AppHandle) -> Value {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    match app.clipboard().write_text(env!("CARGO_PKG_VERSION").to_string()) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub fn support_copy_system_info(app: AppHandle) -> Value {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let text = format!(
        "OS: {}\nFamily: {}\nArch: {}",
        std::env::consts::OS,
        std::env::consts::FAMILY,
        std::env::consts::ARCH,
    );
    match app.clipboard().write_text(text) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ── Conversations ────────────────────────────────────────────────────────────

// ── Autopilot ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn autopilot_list(app: AppHandle) -> Value {
    let binding = app.state::<Mutex<AutopilotStore>>();
    let list = binding.lock().unwrap().list();
    json!(list)
}

#[tauri::command]
pub fn autopilot_get(app: AppHandle, autopilot_id: String) -> Value {
    let binding = app.state::<Mutex<AutopilotStore>>();
    let ap = binding.lock().unwrap().get(&autopilot_id);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_create(app: AppHandle, req: Value) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    let ap = store.lock().unwrap().create(req);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_update(app: AppHandle, autopilot_id: String, req: Value) -> Value {
    let binding = app.state::<Mutex<AutopilotStore>>();
    let ap = binding.lock().unwrap().update(&autopilot_id, req);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_remove(app: AppHandle, autopilot_id: String) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    store.lock().unwrap().remove(&autopilot_id);
    json!(null)
}

#[tauri::command]
pub async fn autopilot_run(app: AppHandle, autopilot_id: String) -> Value {
    let target = {
        let store = app.state::<Mutex<AutopilotStore>>();
        let guard = store.lock().unwrap();
        guard.get(&autopilot_id).map(|ap| ap.target.clone())
    };

    let Some(target) = target else {
        return json!({ "error": format!("autopilot not found: {autopilot_id}") });
    };

    // Proxy to the sidecar as a scrape.board job — the result is tracked
    // via JobTracker so the renderer's job list updates in real time.
    let Some(port) = sidecar_port(&app) else {
        return json!({ "error": "scraper sidecar not ready" });
    };

    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "autopilot.run");

    let payload = serde_json::json!({
        "board": target.board,
        "query": target.query,
        "location": target.location,
        "pages": target.pages,
        "dateFilter": target.date_filter,
    });
    let cmd = serde_json::json!({ "kind": "scrape.board", "jobId": job_id, "payload": payload });

    let job_id_ret = job_id.clone();
    tauri::async_runtime::spawn(async move {
        post_sidecar_command(&app, port, &cmd, &job_id).await.ok();
    });

    json!({ "jobId": job_id_ret })
}

#[tauri::command]
pub fn autopilot_pause(app: AppHandle, autopilot_id: String) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    store.lock().unwrap().set_status(&autopilot_id, AutopilotStatus::Paused);
    json!(null)
}

#[tauri::command]
pub fn autopilot_resume(app: AppHandle, autopilot_id: String) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    store.lock().unwrap().set_status(&autopilot_id, AutopilotStatus::Active);
    json!(null)
}

// ── Conversations ─────────────────────────────────────────────────────────────

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

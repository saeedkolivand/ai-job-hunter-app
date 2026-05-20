/// Tauri command implementations for the AJH spike.
///
/// Real commands: system_health, system_get_version, system_get_platform,
///                system_get_locale, system_open_external.
///
/// Stub commands: everything else — they return null / empty list so the UI
/// renders (with empty states) rather than crashing. Parity is built
/// incrementally by replacing stubs with sidecar proxies.
use serde_json::{json, Value};
use tauri::AppHandle;

// ── System ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn system_health() -> Value {
    json!({
        "status": "ok",
        "shell": "tauri",
        "scraper": { "mode": "sidecar", "ready": false },
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
pub fn scrape_board(_req: Value) -> Value {
    json!({ "error": "Scraper sidecar not yet connected" })
}

#[tauri::command]
pub fn scrape_url(_req: Value) -> Value {
    json!({ "error": "Scraper sidecar not yet connected" })
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

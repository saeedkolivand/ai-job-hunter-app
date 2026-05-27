use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn support_export_logs(app: AppHandle) -> Value {
    match app.path().app_log_dir() {
        Ok(dir) => json!({ "success": true, "path": dir.to_string_lossy() }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

#[tauri::command]
pub async fn support_get_system_info(_app: AppHandle) -> Value {
    // Stub - implement when needed
    json!(null)
}

use serde_json::{json, Value};
use tauri::AppHandle;

#[tauri::command]
pub async fn support_export_logs(_app: AppHandle) -> Value {
    // Stub - implement when needed
    json!({ "success": false })
}

#[tauri::command]
pub async fn support_get_system_info(_app: AppHandle) -> Value {
    // Stub - implement when needed
    json!(null)
}

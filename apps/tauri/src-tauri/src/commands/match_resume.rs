use serde_json::{json, Value};

#[tauri::command]
pub async fn match_resume(_req: Value) -> Value {
    // Stub - implement when needed
    json!(null)
}

#[tauri::command]
pub async fn resume_extract_text(_req: Value) -> Value {
    // Stub - implement when needed
    json!(null)
}

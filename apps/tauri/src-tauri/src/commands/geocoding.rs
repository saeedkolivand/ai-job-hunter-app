use serde_json::{json, Value};

#[tauri::command]
pub async fn geocode_suggest(_query: String) -> Value {
    // Stub - implement when needed
    json!([])
}

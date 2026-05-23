use serde_json::{json, Value};

#[tauri::command]
pub async fn search_postings(_query: String) -> Value {
    // Stub - implement when needed
    json!([])
}

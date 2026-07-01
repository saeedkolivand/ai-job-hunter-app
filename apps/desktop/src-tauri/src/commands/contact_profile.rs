use serde_json::json;
use serde_json::Value;
use tauri::AppHandle;
use tauri::Manager;

use crate::contact_profile::{ContactProfile, ContactProfileStore};

#[tauri::command]
pub async fn contact_profile_get(app: AppHandle) -> Value {
    let store = app.state::<ContactProfileStore>();
    json!(store.get())
}

#[tauri::command]
pub async fn contact_profile_set(app: AppHandle, profile: Value) -> Value {
    let store = app.state::<ContactProfileStore>();
    let parsed: ContactProfile = match serde_json::from_value(profile) {
        Ok(p) => p,
        Err(e) => return json!({ "error": format!("invalid contact profile: {e}") }),
    };
    match store.set(&parsed) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

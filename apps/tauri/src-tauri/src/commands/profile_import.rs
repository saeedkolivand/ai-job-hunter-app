use serde_json::{json, Value};
use tauri::AppHandle;

#[tauri::command]
pub async fn profile_import_from_url(_app: AppHandle, url: String) -> Value {
    match crate::profile_import::import_from_url(&url).await {
        Ok(profile) => json!({
            "text": profile.to_resume_text(),
            "name": profile.name,
            "platform": profile.platform,
        }),
        Err(e) => json!({ "error": e }),
    }
}

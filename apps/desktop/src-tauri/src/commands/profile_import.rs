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
        Err(e) => {
            // Host only — never the profile path/slug (PII; diagnostics bundles
            // get shared with support). The deeper fetch/parse layers already log
            // HTTP status and session state for the LinkedIn path.
            let host = reqwest::Url::parse(&url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
                .unwrap_or_default();
            log::warn!("[profile_import] import failed: host={host} error={e}");
            json!({ "error": e })
        }
    }
}

use serde_json::json;
use serde_json::Value;
use tauri::AppHandle;
use tauri::Manager;

#[tauri::command]
pub async fn job_preferences_get(app: AppHandle) -> Value {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    let prefs = store.get();
    json!(prefs)
}

#[tauri::command]
pub async fn job_preferences_set(app: AppHandle, prefs: Value) -> Value {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    let job_prefs: crate::job_preferences::JobPreferences = serde_json::from_value(prefs)
        .unwrap_or(crate::job_preferences::JobPreferences {
            location: None,
            country_code: None,
            tech_stack: None,
        });
    match store.set(&job_prefs) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

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
            salary_expectation: None,
        });
    match store.set(&job_prefs) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Single-column salary-expectation write (review fix, PR #695) — mirrors
/// `job_preferences_set` but delegates to
/// `JobPreferencesStore::set_salary_expectation`, which touches ONLY that
/// column. Callers (`ApplicantDetailsSection`'s onChange, the boot-time sync
/// hook) that don't have a freshly-read `location`/`tech_stack`/`country_code`
/// on hand must use this, never `job_preferences_set` with a partial payload —
/// that full-row command would silently NULL every other field.
#[tauri::command]
pub async fn job_preferences_set_salary_expectation(
    app: AppHandle,
    salary_expectation: Option<String>,
) -> Value {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    match store.set_salary_expectation(salary_expectation) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

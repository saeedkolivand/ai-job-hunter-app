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
            extra_agency_companies: None,
        });
    match store.set(&job_prefs) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Single-column extra-agency-companies write (ADR-029 §i) — mirrors
/// `job_preferences_set_salary_expectation`: it delegates to
/// `JobPreferencesStore::set_extra_agency_companies`, touching ONLY that column,
/// so a Settings edit of the agency list can never NULL the user's saved
/// location/tech stack/country/salary via a stale full-row payload (PR #695).
#[tauri::command]
pub async fn job_preferences_set_extra_agency_companies(
    app: AppHandle,
    companies: Option<Vec<String>>,
) -> Value {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    match store.set_extra_agency_companies(companies) {
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

/// Single-column semantic-scoring write (ADR-020 addendum) — the renderer's
/// `semanticScoring` preference lives in the webview's `localStorage`, which no
/// Rust code can read, so the headless Autopilot scheduler needs this mirror to
/// know whether to run the semantic re-rank. Same single-column discipline as
/// `job_preferences_set_salary_expectation`: it can never NULL another column.
///
/// Returns `AppResult<()>` — the `email_watch_*` shape — NOT the sibling
/// setters' `Value`-with-an-`error`-key. That difference is load-bearing: a
/// `Value` return RESOLVES the invoke promise even for `{"error": …}`, so the
/// renderer's `onError` / `.catch` never runs and a failed write is invisible.
/// This particular write is the one whose silent failure diverges two scoring
/// surfaces (the user turns semantic scoring OFF, the mirror write fails, and
/// the headless scheduler keeps embedding), so it must REJECT. `AppError`
/// serializes as a plain string, so the rejection carries the store's message.
#[tauri::command]
pub async fn job_preferences_set_semantic_scoring(
    app: AppHandle,
    enabled: bool,
) -> crate::error::AppResult<()> {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    store.set_semantic_scoring(enabled)
}

#[cfg(test)]
mod test {
    use super::*;

    /// A compile-time pin on the wire contract above. Reverting this command to
    /// the sibling `Value` shape (`{"error": …}`) makes the crate's tests fail
    /// to build here — which is the only place the difference is observable
    /// in-process: `invoke`'s resolve-vs-reject behaviour is decided by this
    /// return type, and this crate has no `tauri::test` mock-app harness to
    /// drive the command end to end.
    #[test]
    fn the_semantic_scoring_mirror_rejects_instead_of_resolving_an_error_object() {
        fn assert_rejects_on_failure<F, Fut>(_command: F)
        where
            F: Fn(AppHandle, bool) -> Fut,
            Fut: std::future::Future<Output = crate::error::AppResult<()>>,
        {
        }
        assert_rejects_on_failure(job_preferences_set_semantic_scoring);
    }
}

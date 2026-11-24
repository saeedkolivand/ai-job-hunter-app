//! On-demand availability check for one found job (#1444). No queue, no background crawl:
//! one user action, one guarded fetch, one persisted verdict.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

use super::store;

/// Verify whether the posting at `url` still accepts applications and persist the verdict on
/// every found job at that exact url. The fetch goes through the SSRF-guarded net layer and
/// the shared scrape budget; a blocked/failed fetch records `unknown`, never `closed`.
pub(super) async fn check_availability(app: AppHandle, url: String) -> AppResult<Value> {
    let normalized = crate::applications::normalize_job_url(&url);
    if normalized.is_empty() {
        return Err(AppError::Validation("url is required".into()));
    }
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    let _guard = limiter.acquire(
        "autopilot_check_availability",
        crate::limits::SCRAPE_RATE_MAX,
        crate::limits::SCRAPE_CONCURRENCY_MAX,
    )?;

    let verdict = crate::scraping::availability::check_url(&url).await;
    let checked_at = crate::db::now_ms();
    let store = store(&app);
    tauri::async_runtime::spawn_blocking(move || {
        store
            .lock()
            .set_availability(&normalized, verdict, checked_at)
    })
    .await
    .map_err(|_| AppError::Storage("availability write did not complete".into()))?;

    Ok(json!({
        "availability": verdict.availability,
        "source": verdict.evidence,
        "checkedAt": checked_at,
    }))
}

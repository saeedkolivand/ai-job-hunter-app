use crate::autopilot::AutopilotTarget;
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, SCRAPE_ITEM, SCRAPE_PROGRESS};
use crate::scraping::{BoardSearchInput, JobPosting, ScraperEngine};
use tauri::AppHandle;

/// Scrape job postings from one or more boards for an autopilot run.
pub async fn autopilot_scrape(
    engine: &ScraperEngine,
    target: &AutopilotTarget,
    job_id: &str,
    app: &AppHandle,
) -> AppResult<Vec<JobPosting>> {
    let input = BoardSearchInput {
        query: target.query.clone(),
        location: target.location.clone(),
        // Autopilot expresses its target in pages, so let the page budget bind and
        // set the central item cap to the maximum (never caps autopilot to 0).
        amount: 100,
        pages: target.pages,
        date_filter: target.date_filter.clone(),
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: target.country_code.clone(),
        latitude: None,
        longitude: None,
        radius_km: None,
        // Autopilot has no per-company target; ATS company slugs are a manual
        // search affordance, so this stays empty (a no-op for every board).
        companies: Vec::new(),
    };

    let app_progress = app.clone();
    let job_id_progress = job_id.to_string();
    let on_progress: std::sync::Arc<dyn Fn(f32) + Send + Sync> =
        std::sync::Arc::new(move |p: f32| {
            emit_event(
                &app_progress,
                SCRAPE_PROGRESS,
                serde_json::json!({ "jobId": job_id_progress, "progress": p }),
            );
        });

    let app_item = app.clone();
    let job_id_item = job_id.to_string();
    let on_item: std::sync::Arc<dyn Fn(JobPosting) + Send + Sync> =
        std::sync::Arc::new(move |item: JobPosting| {
            emit_event(
                &app_item,
                SCRAPE_ITEM,
                serde_json::json!({ "jobId": job_id_item, "item": item }),
            );
        });

    let result = engine
        .scrape_boards(
            &target.boards,
            input,
            job_id.to_string(),
            Some(on_progress),
            Some(on_item),
        )
        .await;

    // Return only the postings; the caller (autopilot run) doesn't use per-board summaries.
    // Log any skipped or errored boards so operators can diagnose unexpected empty runs without
    // changing the result shape or IPC contract.
    result
        .map(|(postings, summaries)| {
            for s in &summaries {
                if let Some(ref reason) = s.skipped {
                    log::warn!(
                        "[autopilot] board '{}' skipped (reason='{}')",
                        s.board,
                        reason
                    );
                }
                if let Some(ref err) = s.error {
                    log::warn!(
                        "[autopilot] board '{}' failed (error='{}')",
                        s.board,
                        err
                    );
                }
            }
            postings
        })
        .map_err(AppError::from)
}

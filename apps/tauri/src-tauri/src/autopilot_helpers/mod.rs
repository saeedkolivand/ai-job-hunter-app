use crate::autopilot::AutopilotTarget;
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, SCRAPE_ITEM, SCRAPE_PROGRESS};
use crate::scraping::{BoardSearchInput, JobPosting, ScraperEngine};
use tauri::AppHandle;

/// Scrape job postings from a board
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
        locale: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
    };

    let app_progress = app.clone();
    let job_id_progress = job_id.to_string();
    let on_progress = Box::new(move |p: f32| {
        emit_event(
            &app_progress,
            SCRAPE_PROGRESS,
            serde_json::json!({ "jobId": job_id_progress, "progress": p }),
        );
    });

    let app_item = app.clone();
    let job_id_item = job_id.to_string();
    let on_item = Box::new(move |item: JobPosting| {
        emit_event(
            &app_item,
            SCRAPE_ITEM,
            serde_json::json!({ "jobId": job_id_item, "item": item }),
        );
    });

    let result = engine
        .scrape_board(
            &target.board,
            input,
            job_id.to_string(),
            Some(on_progress),
            Some(on_item),
        )
        .await;

    result.map_err(AppError::from)
}

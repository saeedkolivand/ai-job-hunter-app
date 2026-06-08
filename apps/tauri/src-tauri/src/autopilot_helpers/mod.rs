use crate::autopilot::AutopilotTarget;
use crate::error::{AppError, AppResult};
use crate::scraping::{BoardSearchInput, JobPosting, ScraperEngine};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

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
        let _ = app_progress.emit(
            "scrape.progress",
            serde_json::json!({ "jobId": job_id_progress, "progress": p }),
        );
    });

    let app_item = app.clone();
    let job_id_item = job_id.to_string();
    let on_item = Box::new(move |item: JobPosting| {
        let _ = app_item.emit(
            "scrape.item",
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

/// Rank job postings by semantic similarity to resume
#[allow(dead_code)]
pub async fn autopilot_rank(
    app: &AppHandle,
    postings: Vec<JobPosting>,
    resume_text: Option<&str>,
    min_match_score: f64,
    cancel_token: &CancellationToken,
) -> Vec<(f64, JobPosting)> {
    let resume_vec = match resume_text {
        Some(text) if !text.is_empty() => crate::documents::embed(app, text).await,
        _ => None,
    };

    let mut scored: Vec<(f64, JobPosting)> = Vec::new();
    for posting in postings {
        if cancel_token.is_cancelled() {
            break;
        }
        let score = match (&resume_vec, &posting.description) {
            (Some(rv), Some(desc)) if !desc.is_empty() => {
                match crate::documents::embed(app, desc).await {
                    // Space-checked: incompatible vectors score 0, never silently mixed.
                    Some(jv) => crate::commands::ai_provider::compare(rv, &jv)
                        .map(|s| (s * 100.0).round())
                        .unwrap_or(0.0),
                    None => 0.0,
                }
            }
            _ => 0.0,
        };
        if score >= min_match_score {
            scored.push((score, posting));
        }
    }

    // Sort descending by score
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    scored
}

use std::sync::Arc;

use parking_lot::Mutex;

use crate::autopilot::{AutopilotStatus, AutopilotStore, FoundJob};
use crate::autopilot_helpers::autopilot_scrape;
use crate::scraping::{JobPosting, ScraperEngine};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

// AutopilotCreateRequest / AutopilotUpdateRequest are generated from the Zod
// schemas in packages/shared by `pnpm gen:ipc`.
pub use crate::ipc_contracts::autopilot::{AutopilotCreateRequest, AutopilotUpdateRequest};

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("job-{t:x}")
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn store(app: &AppHandle) -> Arc<Mutex<AutopilotStore>> {
    app.state::<Arc<Mutex<AutopilotStore>>>().inner().clone()
}

#[tauri::command]
pub fn autopilot_list(app: AppHandle) -> Value {
    let list = store(&app).lock().list();
    json!(list)
}

#[tauri::command]
pub fn autopilot_get(app: AppHandle, autopilot_id: String) -> Value {
    let ap = store(&app).lock().get(&autopilot_id);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_create(app: AppHandle, req: AutopilotCreateRequest) -> Value {
    let ap = store(&app).lock().create(serde_json::to_value(&req).unwrap_or_default());
    json!(ap)
}

#[tauri::command]
pub fn autopilot_update(app: AppHandle, autopilot_id: String, req: AutopilotUpdateRequest) -> Value {
    let ap = store(&app).lock().update(&autopilot_id, serde_json::to_value(&req).unwrap_or_default());
    json!(ap)
}

#[tauri::command]
pub fn autopilot_remove(app: AppHandle, autopilot_id: String) -> Value {
    store(&app).lock().remove(&autopilot_id);
    json!(null)
}

#[tauri::command]
pub async fn autopilot_run(app: AppHandle, autopilot_id: String) -> Value {
    let autopilot = store(&app).lock().get(&autopilot_id);

    let Some(autopilot) = autopilot else {
        return json!({ "error": format!("autopilot not found: {autopilot_id}") });
    };

    let target = autopilot.target.clone();
    let filter = autopilot.filter.clone();

    let span = crate::observability::Span::begin(
        "autopilot",
        format!("run={autopilot_id} board={}", target.board),
    );

    let job_id = uuid_v4();
    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .start(&job_id, "autopilot.run");

    let engine = app.state::<Arc<ScraperEngine>>().inner().clone();
    let cancel_token = CancellationToken::new();
    engine.register_token(&job_id, cancel_token.clone()).await;

    let ap_id = autopilot_id.clone();
    let emit_step = move |app: &AppHandle, job_id: &str, step: &str, detail: &str| {
        let _ = app.emit(
            "autopilot.step",
            json!({ "jobId": job_id, "autopilotId": ap_id, "step": step, "detail": detail }),
        );
    };

    emit_step(&app, &job_id, "scrape_start", &format!("Scraping {}", target.board));

    let postings = match autopilot_scrape(&engine, &target, &job_id, &app).await {
        Ok(p) => p,
        Err(e) => {
            engine.unregister_token(&job_id).await;
            app.state::<Mutex<crate::jobs::JobTracker>>().lock().fail(&job_id, e.to_string());
            span.end(false);
            return json!({ "error": e, "jobId": job_id });
        }
    };

    let total_found = postings.len();
    emit_step(&app, &job_id, "scrape_done", &format!("Found {} postings", total_found));

    // Snapshot every scraped posting so the user can review what was found,
    // before `postings` is consumed by ranking.
    let found_at = now_ms();
    let mut found_jobs: Vec<FoundJob> = postings
        .iter()
        .map(|p| FoundJob {
            title: p.title.clone(),
            company: p.company.clone(),
            url: p.url.clone(),
            location: p.location.clone(),
            description: p.description.clone(),
            score: None,
            found_at,
        })
        .collect();

    let scored = autopilot_rank(
        postings,
        autopilot.resume_text.as_deref(),
        filter.min_match_score,
        &cancel_token,
    ).await;

    // Annotate the matched postings with their score.
    for (score, posting) in &scored {
        if let Some(fj) = found_jobs.iter_mut().find(|f| f.url == posting.url) {
            fj.score = Some(*score);
        }
    }

    let top_n = target.top_n.max(1) as usize;
    let candidates: Vec<_> = scored.into_iter().take(top_n).collect();

    emit_step(
        &app,
        &job_id,
        "rank_done",
        &format!("{} candidates above min_match_score {}", candidates.len(), filter.min_match_score),
    );

    let mut applied = 0u32;
    for (i, (score, posting)) in candidates.iter().enumerate() {
        if cancel_token.is_cancelled() {
            emit_step(&app, &job_id, "cancelled", "Autopilot cancelled by user");
            break;
        }

        emit_step(
            &app,
            &job_id,
            "apply_start",
            &format!("[{}/{}] Applying to {} (score {score})", i + 1, candidates.len(), posting.title),
        );

        let apply_req = crate::ipc_contracts::apply::ApplyStartRequest {
            board: target.board.clone(),
            url: posting.url.clone(),
            cover_letter: autopilot.cover_letter.clone(),
            resume_path: None,
            auto_submit: Some(autopilot.auto_submit),
        };
        let apply_result = crate::commands::apply::apply_start(app.clone(), apply_req).await;

        let ok = apply_result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        if ok {
            applied += 1;
        }

        emit_step(
            &app,
            &job_id,
            "apply_done",
            &format!("[{}/{}] {} — ok={ok}", i + 1, candidates.len(), posting.title),
        );
    }

    store(&app)
        .lock()
        .record_run(&autopilot_id, total_found as u32, applied, found_jobs);

    engine.unregister_token(&job_id).await;

    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .complete(&job_id, json!({ "found": total_found, "applied": applied }));

    emit_step(&app, &job_id, "complete", &format!("Found {total_found}, applied to {applied}"));

    span.end_with(&format!("found={total_found} applied={applied}"), true);
    json!({ "jobId": job_id, "found": total_found, "applied": applied })
}

#[tauri::command]
pub fn autopilot_pause(app: AppHandle, autopilot_id: String) -> Value {
    store(&app).lock().set_status(&autopilot_id, AutopilotStatus::Paused);
    json!(null)
}

#[tauri::command]
pub fn autopilot_resume(app: AppHandle, autopilot_id: String) -> Value {
    store(&app).lock().set_status(&autopilot_id, AutopilotStatus::Active);
    json!(null)
}

// Helper functions

pub async fn autopilot_rank(
    postings: Vec<JobPosting>,
    resume_text: Option<&str>,
    min_match_score: f64,
    cancel_token: &CancellationToken,
) -> Vec<(f64, JobPosting)> {
    let resume_text = resume_text.unwrap_or("");
    let mut scored = Vec::new();

    for posting in postings {
        if cancel_token.is_cancelled() {
            break;
        }

        let score = if let Some(description) = &posting.description {
            simple_similarity(resume_text, description)
        } else {
            0.0
        };

        if score >= min_match_score {
            scored.push((score, posting));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

fn simple_similarity(resume: &str, description: &str) -> f64 {
    let resume_lower = resume.to_lowercase();
    let desc_lower = description.to_lowercase();

    let resume_words: std::collections::HashSet<&str> = resume_lower
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();

    let desc_words: std::collections::HashSet<&str> = desc_lower
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();

    if resume_words.is_empty() || desc_words.is_empty() {
        return 0.0;
    }

    let intersection = resume_words.intersection(&desc_words).count();
    let union = resume_words.union(&desc_words).count();

    if union == 0 {
        0.0
    } else {
        (intersection as f64) / (union as f64)
    }
}

use crate::autopilot::{AutopilotStatus, AutopilotStore};
use crate::autopilot_helpers::autopilot_scrape;
use crate::scraping::{JobPosting, ScraperEngine};
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("job-{t:x}")
}

#[tauri::command]
pub fn autopilot_list(app: AppHandle) -> Value {
    let binding = app.state::<Mutex<AutopilotStore>>();
    let list = binding.lock().unwrap().list();
    json!(list)
}

#[tauri::command]
pub fn autopilot_get(app: AppHandle, autopilot_id: String) -> Value {
    let binding = app.state::<Mutex<AutopilotStore>>();
    let ap = binding.lock().unwrap().get(&autopilot_id);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_create(app: AppHandle, req: Value) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    let ap = store.lock().unwrap().create(req);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_update(app: AppHandle, autopilot_id: String, req: Value) -> Value {
    let binding = app.state::<Mutex<AutopilotStore>>();
    let ap = binding.lock().unwrap().update(&autopilot_id, req);
    json!(ap)
}

#[tauri::command]
pub fn autopilot_remove(app: AppHandle, autopilot_id: String) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    store.lock().unwrap().remove(&autopilot_id);
    json!(null)
}

#[tauri::command]
pub async fn autopilot_run(app: AppHandle, autopilot_id: String) -> Value {
    let autopilot = {
        let store = app.state::<Mutex<AutopilotStore>>();
        let guard = store.lock().unwrap();
        guard.get(&autopilot_id)
    };

    let Some(autopilot) = autopilot else {
        return json!({ "error": format!("autopilot not found: {autopilot_id}") });
    };

    let target = autopilot.target.clone();
    let filter = autopilot.filter.clone();

    let job_id = uuid_v4();
    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .unwrap()
        .start(&job_id, "autopilot.run");

    let engine = app.state::<std::sync::Arc<ScraperEngine>>().inner().clone();
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
            if let Ok(mut g) = app.state::<Mutex<crate::jobs::JobTracker>>().lock() {
                g.fail(&job_id, e.clone());
            }
            return json!({ "error": e, "jobId": job_id });
        }
    };

    let total_found = postings.len();
    emit_step(&app, &job_id, "scrape_done", &format!("Found {} postings", total_found));

    let scored = autopilot_rank(
        postings,
        autopilot.resume_text.as_deref(),
        filter.min_match_score,
        &cancel_token,
    ).await;

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

        let apply_req = json!({
            "board": target.board,
            "url": posting.url,
            "coverLetter": autopilot.cover_letter,
            "autoSubmit": autopilot.auto_submit,
        });
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

    {
        let store = app.state::<Mutex<AutopilotStore>>();
        let _ = store.lock().unwrap().update(
            &autopilot_id,
            json!({ "totalFound": total_found, "totalApplied": applied }),
        );
    }

    engine.unregister_token(&job_id).await;

    if let Ok(mut g) = app.state::<Mutex<crate::jobs::JobTracker>>().lock() {
        g.complete(&job_id, json!({ "found": total_found, "applied": applied }));
    }

    emit_step(&app, &job_id, "complete", &format!("Found {total_found}, applied to {applied}"));

    json!({ "jobId": job_id, "found": total_found, "applied": applied })
}

#[tauri::command]
pub fn autopilot_pause(app: AppHandle, autopilot_id: String) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    store.lock().unwrap().set_status(&autopilot_id, AutopilotStatus::Paused);
    json!(null)
}

#[tauri::command]
pub fn autopilot_resume(app: AppHandle, autopilot_id: String) -> Value {
    let store = app.state::<Mutex<AutopilotStore>>();
    store.lock().unwrap().set_status(&autopilot_id, AutopilotStatus::Active);
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

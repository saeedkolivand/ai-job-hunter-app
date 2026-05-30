use std::sync::Arc;

use parking_lot::Mutex;

use crate::autopilot::{AutopilotFilter, AutopilotStatus, AutopilotStore, FoundJob};
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

    // Apply the user's keyword filters to the scraped postings — must-include
    // (all keywords present) + exclude (any keyword present drops it). These were
    // dead config before; now they actually shape the fetched results.
    let postings: Vec<JobPosting> = postings
        .into_iter()
        .filter(|p| matches_keyword_filters(p, &filter))
        .collect();

    let total_found = postings.len();
    emit_step(&app, &job_id, "scrape_done", &format!("Found {total_found} postings after filters"));

    // Snapshot each posting, scored 0–100 against the resume when one is set, then
    // sorted highest-first. Applying is deferred (coming soon) — a run only fetches
    // and saves results for the user to review.
    let resume = autopilot.resume_text.as_deref().unwrap_or("");
    let found_at = now_ms();
    let mut found_jobs: Vec<FoundJob> = postings
        .iter()
        .map(|p| {
            let score = match &p.description {
                Some(desc) if !resume.is_empty() && !desc.is_empty() => {
                    Some(simple_similarity(resume, desc))
                }
                _ => None,
            };
            FoundJob {
                title: p.title.clone(),
                company: p.company.clone(),
                url: p.url.clone(),
                location: p.location.clone(),
                description: p.description.clone(),
                score,
                found_at,
            }
        })
        .collect();

    // Highest score first; unscored postings sort to the end.
    found_jobs.sort_by(|a, b| {
        b.score
            .unwrap_or(-1.0)
            .partial_cmp(&a.score.unwrap_or(-1.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let scored_count = found_jobs.iter().filter(|f| f.score.is_some()).count();
    emit_step(&app, &job_id, "rank_done", &format!("Scored {scored_count} of {total_found} jobs"));

    store(&app)
        .lock()
        .record_run(&autopilot_id, total_found as u32, 0, found_jobs);

    engine.unregister_token(&job_id).await;

    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .complete(&job_id, json!({ "found": total_found, "applied": 0 }));

    emit_step(&app, &job_id, "complete", &format!("Found {total_found}, saved for review"));

    span.end_with(&format!("found={total_found} applied=0"), true);
    json!({ "jobId": job_id, "found": total_found, "applied": 0 })
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

/// Whether a posting passes the autopilot's keyword filters: it must contain
/// **all** must-include keywords and **none** of the exclude keywords, matched
/// case-insensitively against the title + description. Empty/absent lists are
/// no-ops.
fn matches_keyword_filters(posting: &JobPosting, filter: &AutopilotFilter) -> bool {
    let haystack = format!(
        "{} {}",
        posting.title.to_lowercase(),
        posting.description.as_deref().unwrap_or_default().to_lowercase()
    );

    if let Some(excludes) = &filter.exclude_keywords {
        let hits_excluded = excludes.iter().any(|k| {
            let k = k.trim().to_lowercase();
            !k.is_empty() && haystack.contains(&k)
        });
        if hits_excluded {
            return false;
        }
    }

    if let Some(keywords) = &filter.keywords {
        let all_present = keywords.iter().all(|k| {
            let k = k.trim().to_lowercase();
            k.is_empty() || haystack.contains(&k)
        });
        if !all_present {
            return false;
        }
    }

    true
}

/// Word-overlap (Jaccard) similarity of a resume and a job description, scaled to
/// **0–100** to match the UI's percentage display and the `minMatchScore` range.
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
        ((intersection as f64) / (union as f64) * 100.0).round()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn posting(title: &str, description: Option<&str>) -> JobPosting {
        JobPosting {
            id: "id".into(),
            external_id: None,
            title: title.into(),
            company: "co".into(),
            location: None,
            url: "https://example.com/job".into(),
            source: "test".into(),
            description: description.map(String::from),
            requirements: None,
            posted_at: None,
            captured_at: 0,
            extra: HashMap::new(),
        }
    }

    fn filter(keywords: Option<&[&str]>, exclude: Option<&[&str]>) -> AutopilotFilter {
        AutopilotFilter {
            min_match_score: 0.0,
            keywords: keywords.map(|v| v.iter().map(|s| s.to_string()).collect()),
            exclude_keywords: exclude.map(|v| v.iter().map(|s| s.to_string()).collect()),
        }
    }

    #[test]
    fn no_filters_keep_everything() {
        let p = posting("Rust Engineer", Some("We use Rust and Go"));
        assert!(matches_keyword_filters(&p, &filter(None, None)));
        // Empty lists are also a no-op.
        assert!(matches_keyword_filters(&p, &filter(Some(&[]), Some(&[]))));
    }

    #[test]
    fn must_include_requires_all_keywords() {
        let p = posting("Rust Engineer", Some("We use Rust and Kubernetes"));
        assert!(matches_keyword_filters(&p, &filter(Some(&["rust", "kubernetes"]), None)));
        // Missing one required keyword → dropped.
        assert!(!matches_keyword_filters(&p, &filter(Some(&["rust", "elixir"]), None)));
    }

    #[test]
    fn exclude_drops_on_any_match() {
        let p = posting("Senior PHP Developer", Some("Legacy PHP codebase"));
        assert!(!matches_keyword_filters(&p, &filter(None, Some(&["php"]))));
        assert!(matches_keyword_filters(&p, &filter(None, Some(&["python"]))));
    }

    #[test]
    fn matching_is_case_insensitive_over_title_and_description() {
        let p = posting("Backend Role", Some("Postgres and REDIS"));
        // "Backend" only in title, "redis" only in description, different cases.
        assert!(matches_keyword_filters(&p, &filter(Some(&["Backend", "redis"]), None)));
    }

    #[test]
    fn similarity_is_scaled_0_to_100() {
        assert_eq!(simple_similarity("rust kubernetes docker", "rust kubernetes docker"), 100.0);
        assert_eq!(simple_similarity("rust", "java"), 0.0);
        let partial = simple_similarity("rust kubernetes", "rust docker");
        assert!(partial > 0.0 && partial < 100.0);
    }
}

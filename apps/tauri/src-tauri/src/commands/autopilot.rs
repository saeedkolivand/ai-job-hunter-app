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

/// Fill each found job's `applied` from the set of `job_url`s that have a saved
/// generation — so the badge reflects a real link (a generation exists for that
/// job) rather than a hand-set flag that could drift.
fn enrich_applied(app: &AppHandle, list: &mut [crate::autopilot::Autopilot]) {
    let applied = app
        .try_state::<crate::ai_generations::AiGenerationStore>()
        .map(|s| s.applied_job_urls())
        .unwrap_or_default();
    if applied.is_empty() {
        return;
    }
    for ap in list.iter_mut() {
        for job in ap.found_jobs.iter_mut() {
            job.applied = applied.contains(&job.url);
        }
    }
}

#[tauri::command]
pub fn autopilot_list(app: AppHandle) -> Value {
    let mut list = store(&app).lock().list();
    enrich_applied(&app, &mut list);
    json!(list)
}

#[tauri::command]
pub fn autopilot_get(app: AppHandle, autopilot_id: String) -> Value {
    let ap = store(&app).lock().get(&autopilot_id).map(|a| {
        let mut one = [a];
        enrich_applied(&app, &mut one);
        let [ap] = one;
        ap
    });
    json!(ap)
}

#[tauri::command]
pub fn autopilot_create(app: AppHandle, req: AutopilotCreateRequest) -> Value {
    let ap = store(&app)
        .lock()
        .create(serde_json::to_value(&req).unwrap_or_default());
    json!(ap)
}

#[tauri::command]
pub fn autopilot_update(
    app: AppHandle,
    autopilot_id: String,
    req: AutopilotUpdateRequest,
) -> Value {
    let ap = store(&app).lock().update(
        &autopilot_id,
        serde_json::to_value(&req).unwrap_or_default(),
    );
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

    emit_step(
        &app,
        &job_id,
        "scrape_start",
        &format!("Scraping {}", target.board),
    );

    let postings = match autopilot_scrape(&engine, &target, &job_id, &app).await {
        Ok(p) => p,
        Err(e) => {
            engine.unregister_token(&job_id).await;
            app.state::<Mutex<crate::jobs::JobTracker>>()
                .lock()
                .fail(&job_id, e.to_string());
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
    emit_step(
        &app,
        &job_id,
        "scrape_done",
        &format!("Found {total_found} postings after filters"),
    );

    // Snapshot each posting, scored 0–100 against the resume when one is set, then
    // sorted highest-first. Autopilot is a discovery agent: a run only finds, ranks,
    // and saves results — the user applies with the tailoring assistant.
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
                // Set by the dedup merge in `record_run`; `applied` is derived on read.
                is_new: false,
                applied: false,
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

    // Honour the autopilot's minimum match score: drop postings we *could*
    // score that fell below the bar. Unscored postings (no resume set, or no
    // description to compare) are always kept — the threshold only filters jobs
    // we were actually able to rank. Until now `minMatchScore` was dead config.
    let threshold = filter.min_match_score;
    found_jobs.retain(|j| passes_min_score(j, threshold));
    let kept = found_jobs.len();
    let dropped = total_found - kept;

    emit_step(
        &app,
        &job_id,
        "rank_done",
        &format!(
            "Scored {scored_count}/{total_found}; kept {kept} at or above {threshold:.0}% (dropped {dropped})"
        ),
    );

    // Bail cleanly if the run was cancelled (tray/UI) any time before we commit
    // — don't record results or fire a "new jobs" notification for an aborted
    // run. `cancel(job_id)` flips the token this run registered (engine reuses,
    // not overwrites, the slot), so cancels during scrape land here too.
    if cancel_token.is_cancelled() {
        engine.unregister_token(&job_id).await;
        app.state::<Mutex<crate::jobs::JobTracker>>()
            .lock()
            .cancel(&job_id);
        span.end_with("cancelled before recording results", false);
        return json!({ "jobId": job_id, "cancelled": true });
    }

    let new_count = store(&app)
        .lock()
        .record_run(&autopilot_id, kept as u32, 0, found_jobs);

    // Surface genuinely-new finds while the user is away: a permission-gated
    // notification + a "New jobs: N" tray counter that jumps back to this run.
    crate::tray::on_new_jobs(&app, &autopilot_id, &autopilot.name, new_count);

    engine.unregister_token(&job_id).await;

    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .complete(&job_id, json!({ "found": kept, "applied": 0 }));

    emit_step(
        &app,
        &job_id,
        "complete",
        &format!("Found {kept}, saved for review"),
    );

    span.end_with(&format!("found={kept} applied=0"), true);
    json!({ "jobId": job_id, "found": kept, "applied": 0 })
}

#[tauri::command]
pub fn autopilot_pause(app: AppHandle, autopilot_id: String) -> Value {
    store(&app)
        .lock()
        .set_status(&autopilot_id, AutopilotStatus::Paused);
    json!(null)
}

#[tauri::command]
pub fn autopilot_resume(app: AppHandle, autopilot_id: String) -> Value {
    store(&app)
        .lock()
        .set_status(&autopilot_id, AutopilotStatus::Active);
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
        posting
            .description
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
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

/// Whether a found job clears the autopilot's `min_match_score`. Postings we
/// could not score (no resume set, or no description to compare against) carry
/// no score and are always kept — the threshold only gates rankable jobs.
fn passes_min_score(job: &FoundJob, min_match_score: f64) -> bool {
    job.score.is_none_or(|s| s >= min_match_score)
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
        assert!(matches_keyword_filters(
            &p,
            &filter(Some(&["rust", "kubernetes"]), None)
        ));
        // Missing one required keyword → dropped.
        assert!(!matches_keyword_filters(
            &p,
            &filter(Some(&["rust", "elixir"]), None)
        ));
    }

    #[test]
    fn exclude_drops_on_any_match() {
        let p = posting("Senior PHP Developer", Some("Legacy PHP codebase"));
        assert!(!matches_keyword_filters(&p, &filter(None, Some(&["php"]))));
        assert!(matches_keyword_filters(
            &p,
            &filter(None, Some(&["python"]))
        ));
    }

    #[test]
    fn matching_is_case_insensitive_over_title_and_description() {
        let p = posting("Backend Role", Some("Postgres and REDIS"));
        // "Backend" only in title, "redis" only in description, different cases.
        assert!(matches_keyword_filters(
            &p,
            &filter(Some(&["Backend", "redis"]), None)
        ));
    }

    #[test]
    fn similarity_is_scaled_0_to_100() {
        assert_eq!(
            simple_similarity("rust kubernetes docker", "rust kubernetes docker"),
            100.0
        );
        assert_eq!(simple_similarity("rust", "java"), 0.0);
        let partial = simple_similarity("rust kubernetes", "rust docker");
        assert!(partial > 0.0 && partial < 100.0);
    }

    fn found(score: Option<f64>) -> FoundJob {
        FoundJob {
            title: "t".into(),
            company: "c".into(),
            url: "https://example.com/job".into(),
            location: None,
            description: None,
            score,
            found_at: 0,
            is_new: false,
            applied: false,
        }
    }

    #[test]
    fn min_score_gate_keeps_at_or_above_threshold() {
        assert!(passes_min_score(&found(Some(80.0)), 50.0));
        assert!(passes_min_score(&found(Some(50.0)), 50.0)); // boundary is inclusive
        assert!(!passes_min_score(&found(Some(49.9)), 50.0));
    }

    #[test]
    fn min_score_gate_keeps_unscored_jobs() {
        // No resume / no description → no score → never filtered out by the gate.
        assert!(passes_min_score(&found(None), 50.0));
        assert!(passes_min_score(&found(None), 100.0));
    }
}

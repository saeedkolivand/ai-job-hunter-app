use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ai_generations::sanitize_quality_report;
use crate::ipc_contracts::ai::{AiGenerationSaveRequest, AiGenerationUpdateRequest};

#[tauri::command]
pub async fn ai_generations_list(app: AppHandle) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();
    serde_json::to_value(store.list()).unwrap_or(json!([]))
}

#[tauri::command]
pub async fn ai_generations_save(app: AppHandle, req: AiGenerationSaveRequest) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();

    let mut rec = crate::ai_generations::AiGenerationRecord {
        id: crate::ai_generations::make_generation_id(),
        created_at: crate::db::now_ms(),
        candidate_name: req.candidate_name,
        job_title: req.job_title,
        company_name: req.company_name,
        resume_language: req.resume_language,
        job_ad_language: req.job_ad_language,
        target_language: req.target_language,
        mismatch: req.mismatch,
        top_requirements: req.top_requirements,
        mode: req.mode,
        resume_text: req.resume_text,
        cover_letter_text: req.cover_letter_text,
        job_ad: req.job_ad,
        job_url: req.job_url,
        board: req.board,
        application_answers: req
            .application_answers
            .into_iter()
            .map(|a| crate::ai_generations::ApplicationAnswer {
                id: a.id,
                question: a.question,
                answer: a.answer,
            })
            .collect(),
        company_brief: req.company_brief,
        interview_questions: req
            .interview_questions
            .into_iter()
            .map(|q| crate::ai_generations::InterviewQuestion {
                id: q.id,
                question: q.question,
                why: q.why,
                audience: q.audience,
            })
            .collect(),
        email_subject: req.email_subject,
        email_body: req.email_body,
        application_id: None,
        // Absent = "this save carries no fresh report" — `save_application`'s
        // merge (`merge_quality_report`) keeps whatever report is already on
        // the aggregate for exactly that case. An over-cap report hits the
        // same "no report" outcome deliberately: a byte-position clamp would
        // truncate mid-JSON instead, which `merge_quality_report` would then
        // treat as unparseable and silently drop with no signal at all.
        quality_report: sanitize_quality_report(
            req.quality_report.unwrap_or_default(),
            "ai_generations_save",
        ),
    };

    // ADR 0001: the Application aggregate is the source of truth for status +
    // "applied". A generation is its child Document. So before persisting the
    // generation we upsert/advance the Application for this job_url (Generate
    // origin → `applied`), then store the generation (which still carries its
    // own job/company/board copy for backward compatibility + offline export).
    let job_url = rec.job_url.clone();
    let board = rec.board.clone();
    let meta = crate::applications::ApplicationMeta {
        company: rec.company_name.clone(),
        title: rec.job_title.clone(),
        candidate: rec.candidate_name.clone(),
        brief: rec.company_brief.clone(),
        job_description: String::new(), // ponytail: JD persistence is scoped to import + the update IPC
        answers: rec.application_answers.clone(),
        job_summary: String::new(),
        salary_min: None,
        salary_max: None,
        salary_currency: None,
    };
    if let Some(apps) = app.try_state::<crate::applications::ApplicationStore>() {
        match apps.upsert_for_origin(
            &job_url,
            &board,
            &meta,
            crate::applications::ApplicationOrigin::Generate,
            None,
        ) {
            // Link the generation to its parent Application via the FK so the detail
            // page can join docs by `application_id` instead of a raw-vs-normalized
            // url string compare (which never matches for query-id boards like Indeed:
            // the Application stores the normalized url, the generation the raw one).
            Ok(app_id) => rec.application_id = Some(app_id),
            // Non-fatal: a failed Application upsert must not lose the generation the
            // user just produced. The generation save below is the user-visible action;
            // the aggregate (and the FK, via boot-time backfill) can be re-derived.
            Err(e) => log::warn!("[ai_generations] application upsert failed (non-fatal): {e}"),
        }
    }

    // Per-job aggregate (generation side): merge into that job's generation row so
    // résumé/cover/answers/brief from separate actions land on one document record.
    match store.save_application(rec) {
        Ok(id) => json!({ "id": id, "success": true }),
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
pub async fn ai_generations_update(app: AppHandle, req: AiGenerationUpdateRequest) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();
    // Direct overwrite of exactly the provided text fields, selected by id —
    // distinct from the save merge-upsert, so a user edit can replace text the
    // merge would have kept.
    match store.update_texts(&req.id, req.resume_text, req.cover_letter_text) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e }),
    }
}

/// Delete the PIPELINE RUN TRAIL of every posting these generations belonged
/// to — the cascade a generation delete owes.
///
/// **This is the PRIMARY delete.** `applications_delete` cascades already, but
/// the Documents page's "delete this generated résumé" is the button a user
/// actually reaches for, and it removed the `ai_generations` row while leaving
/// the run trail behind: a max-depth run persists its full re-seeded strategy
/// (the whole employment history) and its full evidence map (verbatim résumé
/// quotes) in `pipeline_run_events.artifact_json`. With the aggregate gone
/// those rows have no owner, no UI, and no eviction — retention partitions on
/// `(job_url, kind)` and only ever evicts the FOURTH run of a posting that is
/// still being run, which a deleted one never is — and `DataStore::export`
/// ships every one of them into the user's backups.
///
/// Called with the urls read BEFORE the delete (the row is what answers the
/// question) and only AFTER it succeeded (a failed delete must not take the
/// trail with it). Best-effort and non-fatal, like every other cascade here:
/// the store logs its own failures, and the user's delete already happened.
fn purge_run_trails(app: &AppHandle, job_urls: &[String]) {
    if job_urls.is_empty() {
        return;
    }
    if let Some(runs) = app.try_state::<crate::pipeline::runs::PipelineRunStore>() {
        runs.delete_for_jobs(job_urls);
    }
}

#[tauri::command]
pub async fn ai_generations_remove(app: AppHandle, id: String) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();
    let job_urls = store.job_urls_for(std::slice::from_ref(&id));
    match store.remove(&id) {
        Ok(()) => {
            purge_run_trails(&app, &job_urls);
            json!({ "success": true })
        }
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
pub async fn ai_generations_remove_bulk(app: AppHandle, ids: Vec<String>) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();
    let job_urls = store.job_urls_for(&ids);
    match store.remove_many(&ids) {
        Ok(count) => {
            purge_run_trails(&app, &job_urls);
            json!({ "success": true, "count": count })
        }
        Err(e) => json!({ "error": e }),
    }
}

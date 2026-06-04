use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ipc_contracts::ai::{AiGenerationSaveRequest, AiGenerationUpdateRequest};

#[tauri::command]
pub async fn ai_generations_list(app: AppHandle) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();
    serde_json::to_value(store.list()).unwrap_or(json!([]))
}

#[tauri::command]
pub async fn ai_generations_save(app: AppHandle, req: AiGenerationSaveRequest) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();

    let rec = crate::ai_generations::AiGenerationRecord {
        id: crate::ai_generations::make_generation_id(),
        created_at: crate::ai_generations::now_ms(),
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
    };

    // Per-job aggregate: when linked to a job, merge into that job's row so
    // résumé/cover/answers/brief from separate actions land on one record.
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

#[tauri::command]
pub async fn ai_generations_remove(app: AppHandle, id: String) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();
    match store.remove(&id) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
pub async fn ai_generations_remove_bulk(app: AppHandle, ids: Vec<String>) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();
    match store.remove_many(&ids) {
        Ok(count) => json!({ "success": true, "count": count }),
        Err(e) => json!({ "error": e }),
    }
}

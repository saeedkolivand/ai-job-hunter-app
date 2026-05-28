use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ipc_contracts::ai::AiGenerationSaveRequest;

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
    };

    match store.insert(&rec) {
        Ok(()) => json!({ "id": rec.id, "success": true }),
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

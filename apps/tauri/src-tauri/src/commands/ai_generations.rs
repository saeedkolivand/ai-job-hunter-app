use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn ai_generations_list(app: AppHandle) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();
    serde_json::to_value(store.list()).unwrap_or(json!([]))
}

#[tauri::command]
pub async fn ai_generations_save(app: AppHandle, req: Value) -> Value {
    let store = app.state::<crate::ai_generations::AiGenerationStore>();

    let top_requirements: Vec<String> = req
        .get("topRequirements")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let rec = crate::ai_generations::AiGenerationRecord {
        id: crate::ai_generations::make_generation_id(),
        created_at: crate::ai_generations::now_ms(),
        candidate_name: req.get("candidateName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        job_title: req.get("jobTitle").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        company_name: req.get("companyName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        resume_language: req.get("resumeLanguage").and_then(|v| v.as_str()).unwrap_or("en").to_string(),
        job_ad_language: req.get("jobAdLanguage").and_then(|v| v.as_str()).unwrap_or("en").to_string(),
        target_language: req.get("targetLanguage").and_then(|v| v.as_str()).unwrap_or("en").to_string(),
        mismatch: req.get("mismatch").and_then(|v| v.as_bool()).unwrap_or(false),
        top_requirements,
        mode: req.get("mode").and_then(|v| v.as_str()).unwrap_or("ats").to_string(),
        resume_text: req.get("resumeText").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        cover_letter_text: req.get("coverLetterText").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        job_ad: req.get("jobAd").and_then(|v| v.as_str()).unwrap_or("").to_string(),
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

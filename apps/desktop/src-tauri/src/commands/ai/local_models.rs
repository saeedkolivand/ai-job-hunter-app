//! Local (Ollama) model management — list, inspect, pull and unload. Split out of
//! `commands/ai/mod.rs` for R8 (issue #1280); `mod.rs` re-exports the commands, so
//! each keeps its `commands::ai::<name>` path (the `generate_handler!` list and the
//! agent-CLI policy table are keyed on it).

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai_provider::ollama;
use crate::db::new_job_id;
use crate::error::{AppError, AppResult};
use crate::jobs::KeyedExclusiveStart;

/// Local (Ollama) model list — powers the model picker's "Ollama (Local)"
/// section. Cloud models come from `ai_list_provider_models`.
#[tauri::command]
pub async fn ai_list_models() -> Value {
    json!(ollama::list_tag_models().await)
}

/// Inspect a local (Ollama) model's real context window + size via `/api/show`,
/// to suggest safe generation limits. Returns `Null` when Ollama is unreachable
/// or the model has no usable info — the UI only calls this for the local provider.
#[tauri::command]
pub async fn ai_inspect_model(model: String) -> Value {
    ollama::show_model(&model).await
}

#[tauri::command]
pub async fn ai_pull_model(app: AppHandle, model: String) -> AppResult<Value> {
    let job_id = new_job_id();
    // Exclusive per (kind, model), not kind alone: a returning caller (a
    // remounted onboarding step showing an idle Download button) must
    // re-attach to a pull of the SAME model already in flight — same
    // check-then-act reasoning as `claim_embed_job`. A pull of a DIFFERENT
    // model already running is refused rather than silently joined: joining
    // would hand the caller progress for a model it never asked for while its
    // own request never ran at all, and two concurrent multi-GB downloads
    // would compete for the same bandwidth and disk anyway.
    match crate::commands::jobs::job_start_exclusive_keyed(
        &app,
        &job_id,
        "ai.pull_model",
        "model",
        &model,
    ) {
        KeyedExclusiveStart::Joined(existing) => {
            return Ok(json!({ "jobId": existing }));
        }
        KeyedExclusiveStart::Started => {}
        KeyedExclusiveStart::Busy(active_model) => {
            return Err(AppError::Validation(format!(
                "Already downloading \"{active_model}\" — wait for it to finish before starting another model."
            )));
        }
    }

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        match ollama::pull(&app_clone, &job_id_clone, &model).await {
            Ok(()) => {
                crate::commands::jobs::job_complete(
                    &app_clone,
                    &job_id_clone,
                    json!({ "model": model, "done": true }),
                );
            }
            Err(e) => {
                crate::commands::jobs::job_fail(&app_clone, &job_id_clone, e.to_string());
            }
        }
    });

    Ok(json!({ "jobId": job_id }))
}

#[tauri::command]
pub fn ai_unload_model(_model: String) -> Value {
    json!({ "success": true })
}

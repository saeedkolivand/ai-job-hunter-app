use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::credentials::CredentialStore;
use crate::ipc_contracts::ai::AiEmbedRequest;
use crate::jobs::JobTracker;

use super::ai_provider::{
    emit_stream_error, ollama, resolve, resolve_by_name, AiGenerateRequest, ProviderId,
};

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("job-{t:x}")
}

/// Stream an AI generation from the explicitly-selected provider.
///
/// The provider is **required and validated** — unknown/missing providers and
/// model/provider mismatches fail with a clear error. There is no silent
/// fallback to Ollama.
#[tauri::command]
pub async fn ai_generate(app: AppHandle, req: AiGenerateRequest) -> Value {
    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .start(&job_id, "ai.generate");

    let fail = |app: &AppHandle, job_id: &str, msg: String| -> Value {
        emit_stream_error(app, job_id, &msg);
        app.state::<Mutex<JobTracker>>().lock().fail(job_id, msg);
        json!({ "jobId": job_id })
    };

    // 1. Provider must be present.
    let provider_str = match req.provider.as_deref() {
        Some(p) if !p.trim().is_empty() => p.to_string(),
        _ => {
            return fail(
                &app,
                &job_id,
                "No AI provider selected. Choose a provider in Settings → AI.".to_string(),
            );
        }
    };
    // 2. Provider must be known.
    let provider_id = match ProviderId::parse(&provider_str) {
        Ok(id) => id,
        Err(e) => return fail(&app, &job_id, e),
    };
    // 3. Model must belong to the active provider.
    if let Err(e) = provider_id.validate_model(&req.model) {
        return fail(&app, &job_id, e);
    }

    log::info!(
        "[ai] dispatch provider={} model={}",
        provider_id.as_str(),
        req.model
    );

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    let base_url = req.base_url.clone();
    tauri::async_runtime::spawn(async move {
        let provider = resolve(provider_id, base_url);
        if let Err(e) = provider.chat_stream(&app_clone, &job_id_clone, &req).await {
            emit_stream_error(&app_clone, &job_id_clone, &e);
            app_clone
                .state::<Mutex<JobTracker>>()
                .lock()
                .fail(&job_id_clone, e);
        }
    });

    json!({ "jobId": job_id })
}

pub(crate) fn get_provider_key(app: &AppHandle, provider: &str) -> Option<String> {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    guard
        .get_decrypted(&format!("ai:{provider}"))
        .map(|(_, password)| password)
}

#[tauri::command]
pub fn ai_set_provider_key(app: AppHandle, provider: String, api_key: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    match guard.set(&format!("ai:{provider}"), "apikey", &api_key) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn ai_remove_provider_key(app: AppHandle, provider: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    match guard.remove(&format!("ai:{provider}")) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn ai_has_provider_key(app: AppHandle, provider: String) -> Value {
    json!({ "has": get_provider_key(&app, &provider).is_some() })
}

#[tauri::command]
pub async fn ai_test_provider_key(app: AppHandle, provider: String) -> Value {
    let provider_client = match resolve_by_name(&provider, None) {
        Ok(p) => p,
        Err(e) => return json!({ "success": false, "error": e }),
    };
    let api_key = match get_provider_key(&app, &provider) {
        Some(k) => k,
        None => return json!({ "success": false, "error": "No API key found" }),
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return json!({ "success": false, "error": format!("Failed to create client: {}", e) })
        }
    };

    match provider_client.test_key(&client, &api_key).await {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub async fn ai_list_provider_models(app: AppHandle, provider: String) -> Value {
    let provider_client = match resolve_by_name(&provider, None) {
        Ok(p) => p,
        Err(_) => return json!([]),
    };
    let api_key = match get_provider_key(&app, &provider) {
        Some(k) => k,
        None => return json!([]),
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return json!([]),
    };

    json!(provider_client.list_models(&client, &api_key).await)
}

/// Local (Ollama) model list — powers the model picker's "Ollama (Local)"
/// section. Cloud models come from `ai_list_provider_models`.
#[tauri::command]
pub async fn ai_list_models() -> Value {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return json!([]),
    };
    json!(ollama::list_tag_models(&client).await)
}

#[tauri::command]
pub async fn ai_pull_model(app: AppHandle, model: String) -> Value {
    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .start(&job_id, "ai.pull_model");

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        match ollama::pull(&app_clone, &job_id_clone, &model).await {
            Ok(()) => {
                app_clone
                    .state::<Mutex<JobTracker>>()
                    .lock()
                    .complete(&job_id_clone, json!({ "model": model, "done": true }));
            }
            Err(e) => {
                app_clone
                    .state::<Mutex<JobTracker>>()
                    .lock()
                    .fail(&job_id_clone, e);
            }
        }
    });

    json!({ "jobId": job_id })
}

#[tauri::command]
pub fn ai_unload_model(_model: String) -> Value {
    json!({ "success": true })
}

/// Embed text. Embeddings remain Ollama-only pending the embeddings migration;
/// this is isolated inside the Ollama provider module.
#[tauri::command]
pub async fn ai_embed(req: AiEmbedRequest) -> Value {
    match ollama::embed(&req.text).await {
        Some(vector) => {
            let dim = vector.len();
            json!({ "vector": vector, "dim": dim })
        }
        None => json!(null),
    }
}

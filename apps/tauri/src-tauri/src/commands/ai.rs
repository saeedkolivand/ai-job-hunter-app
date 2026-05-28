use serde::Deserialize;
use serde_json::{json, Value};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use crate::credentials::CredentialStore;
use crate::jobs::JobTracker;

use super::ai_provider::{provider_for, AiGenerateRequest};

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("job-{t:x}")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEmbedRequest {
    pub text: String,
    pub model: Option<String>,
}

/// Stream an AI generation from the configured provider.
#[tauri::command]
pub async fn ai_generate(app: AppHandle, req: AiGenerateRequest) -> Value {
    let provider_name = req.provider.as_deref().unwrap_or("ollama").to_string();

    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .start(&job_id, "ai.generate");

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let provider = provider_for(&provider_name);
        let result = provider.stream_chat(&app_clone, &job_id_clone, &req).await;

        if let Err(e) = result {
            let _ = app_clone.emit(
                "ai:stream",
                json!({ "jobId": job_id_clone, "delta": "", "done": true, "error": { "code": "GENERATION_FAILED", "message": format!("{e}") } }),
            );
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
    let api_key = match get_provider_key(&app, &provider) {
        Some(k) => k,
        None => return json!({ "success": false, "error": "No API key found" }),
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => return json!({ "success": false, "error": format!("Failed to create client: {}", e) }),
    };

    match provider_for(&provider).test_key(&client, &api_key).await {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub async fn ai_list_provider_models(app: AppHandle, provider: String) -> Value {
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

    json!(provider_for(&provider).list_models(&client, &api_key).await)
}

#[tauri::command]
pub async fn ai_list_models() -> Value {
    let base = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return json!([]),
    };

    let resp = match client.get(format!("{base}/api/tags")).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return json!([]),
    };

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return json!([]),
    };

    let models: Vec<serde_json::Value> = body
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                .map(|name| json!({ "name": name }))
                .collect()
        })
        .unwrap_or_default();

    json!(models)
}

#[tauri::command]
pub async fn ai_pull_model(app: AppHandle, model: String) -> Value {
    let base = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

    let job_id = uuid_v4();
    app.state::<Mutex<JobTracker>>()
        .lock()
        .start(&job_id, "ai.pull_model");

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let result = pull_ollama_model(&app_clone, &base, &job_id_clone, &model).await;
        match result {
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

async fn pull_ollama_model(
    app: &AppHandle,
    base: &str,
    job_id: &str,
    model: &str,
) -> Result<(), String> {
    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{base}/api/pull"))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama {status}: {body}"));
    }

    let mut line_buf = String::new();

    while let Some(bytes) = response.chunk().await.map_err(|e| e.to_string())? {
        line_buf.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(nl) = line_buf.find('\n') {
            let line = line_buf[..nl].trim().to_string();
            line_buf = line_buf[nl + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            let event: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let status = event.get("status").and_then(|s| s.as_str()).unwrap_or("");
            let completed = event.get("completed").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let total = event.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let digest = event.get("digest").and_then(|v| v.as_str()).unwrap_or("");
            let p = if total > 0.0 { completed / total } else { 0.0 };

            let _ = app.emit("jobs:event", json!({ "type": "job.stream", "jobId": job_id, "data": { "status": status, "p": p, "completed": completed, "total": total, "digest": digest } }));

            if status == "success" {
                return Ok(());
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn ai_unload_model(_model: String) -> Value {
    json!({ "success": true })
}

#[tauri::command]
pub async fn ai_embed(req: AiEmbedRequest) -> Value {
    let base = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

    let model = req.model.as_deref().unwrap_or("nomic-embed-text");
    let body = json!({ "model": model, "prompt": req.text });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(_) => return json!(null),
    };

    let resp = match client.post(format!("{base}/api/embeddings")).json(&body).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return json!(null),
    };

    let data: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return json!(null),
    };

    let vector = data.get("embedding").cloned().unwrap_or(json!([]));
    let dim = vector.as_array().map(|a| a.len()).unwrap_or(0);
    json!({ "vector": vector, "dim": dim })
}

//! Ollama (local) provider.
//!
//! This is the ONLY module allowed to reference the Ollama host or its `/api/*`
//! endpoints. Everything Ollama-specific — chat, model list, pull, embeddings,
//! health — lives here so no hidden Ollama assumptions leak into the rest of the
//! codebase.

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::jobs::JobTracker;

use super::{
    AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, RequestTrace, TokenParam,
};

const DEFAULT_HOST: &str = "http://127.0.0.1:11434";
const EMBED_MODEL: &str = "nomic-embed-text";

/// Resolve the Ollama host (env override or localhost default).
pub fn host() -> String {
    std::env::var("OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string())
}

fn build_client(timeout_secs: u64) -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .ok()
}

// ── Provider impl ───────────────────────────────────────────────────────────────

pub struct OllamaClient;

#[async_trait]
impl AiProvider for OllamaClient {
    fn id(&self) -> ProviderId {
        ProviderId::Ollama
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: false,
            supports_tools: false,
            supports_json_mode: true,
            supports_embeddings: true,
            token_param: TokenParam::NumPredict,
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> Result<(), String> {
        stream_chat(app, job_id, req).await
    }

    async fn complete(
        &self,
        _app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> Result<String, String> {
        let base = host();
        let endpoint = format!("{base}/api/chat");
        let trace = RequestTrace::begin(ProviderId::Ollama, model, "/api/chat", &base, false);

        let mut body = json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
        });
        if let Some(t) = temperature {
            body["options"] = json!({ "temperature": t });
        }

        let client = build_client(300).ok_or_else(|| "Failed to build Ollama client".to_string())?;
        let resp = match client.post(&endpoint).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(format!("Ollama unreachable: {e}"));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(format!("Ollama {status}: {body_text}"));
        }
        let data: Value = resp.json().await.map_err(|e| format!("Ollama parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        data.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(String::from)
            .ok_or_else(|| "Ollama: unexpected response shape".to_string())
    }

    async fn embed(&self, _app: &AppHandle, model: &str, text: &str) -> Result<Vec<f64>, String> {
        embed_with(model, text).await
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some(EMBED_MODEL)
    }

    async fn list_models(&self, client: &reqwest::Client, _api_key: &str) -> Vec<Value> {
        list_tag_models(client).await
    }

    async fn test_key(&self, client: &reqwest::Client, _api_key: &str) -> Result<(), String> {
        // Ollama needs no key — a reachable host counts as healthy.
        match client.get(format!("{}/api/tags", host())).send().await {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(format!("Ollama returned status: {}", r.status())),
            Err(e) => Err(format!("Ollama unreachable: {e}")),
        }
    }
}

// ── Shared Ollama helpers (used by the AI commands, health, embeddings) ─────────

/// `{ name }` list from `/api/tags`.
pub async fn list_tag_models(client: &reqwest::Client) -> Vec<Value> {
    let resp = match client.get(format!("{}/api/tags", host())).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return vec![],
    };
    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    body.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                .map(|name| json!({ "name": name }))
                .collect()
        })
        .unwrap_or_default()
}

/// `(reachable, first_model_name)` for the system health probe.
pub async fn reachable_model() -> (bool, Option<String>) {
    let Some(client) = build_client(3) else {
        return (false, None);
    };
    match client.get(format!("{}/api/tags", host())).send().await {
        Ok(r) if r.status().is_success() => {
            let body: Value = r.json().await.unwrap_or_default();
            let model = body
                .get("models")
                .and_then(|m| m.as_array())
                .and_then(|arr| arr.first())
                .and_then(|m| m.get("name"))
                .and_then(|n| n.as_str())
                .map(String::from);
            (true, model)
        }
        _ => (false, None),
    }
}

/// Embed `text` with a specific Ollama embedding model. Returns a clear error
/// (not `None`) so callers can surface why embedding failed.
pub async fn embed_with(model: &str, text: &str) -> Result<Vec<f64>, String> {
    let client = build_client(15).ok_or_else(|| "Failed to build Ollama client".to_string())?;
    // Char-boundary-safe truncation (avoids panics on multi-byte input).
    let truncated: String = text.chars().take(8000).collect();
    let body = json!({ "model": model, "prompt": truncated });
    let resp = client
        .post(format!("{}/api/embeddings", host()))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama {status}: {body_text}"));
    }
    let data: Value = resp.json().await.map_err(|e| format!("Ollama parse: {e}"))?;
    let arr = data
        .get("embedding")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "Ollama: missing embedding in response".to_string())?;
    Ok(arr.iter().filter_map(|v| v.as_f64()).collect())
}

/// Stream a model pull, emitting `jobs:event` progress. Returns when complete.
pub async fn pull(app: &AppHandle, job_id: &str, model: &str) -> Result<(), String> {
    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{}/api/pull", host()))
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

// ── Chat streaming ──────────────────────────────────────────────────────────────

async fn stream_chat(
    app: &AppHandle,
    job_id: &str,
    req: &AiGenerateRequest,
) -> Result<(), String> {
    let base = host();
    let endpoint = format!("{base}/api/chat");
    let trace = RequestTrace::begin(ProviderId::Ollama, &req.model, "/api/chat", &base, true);

    let messages = serde_json::to_value(
        &req.messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect::<Vec<_>>(),
    )
    .unwrap_or(json!([]));

    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    let mut options = serde_json::Map::new();
    if let Some(t) = req.temperature {
        options.insert("temperature".to_string(), json!(t));
    }
    if let Some(mt) = req.max_tokens {
        options.insert("num_predict".to_string(), json!(mt));
    }
    if !options.is_empty() {
        body["options"] = Value::Object(options);
    }

    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?
        .post(&endpoint)
        .json(&body)
        .send()
        .await;

    let mut response = match response {
        Ok(r) => r,
        Err(e) => {
            trace.end(None, false);
            return Err(format!("Ollama unreachable: {e}"));
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        trace.end(Some(status.as_u16()), false);
        return Err(format!("Ollama {status}: {body_text}"));
    }

    let mut line_buf = String::new();
    loop {
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                let _ = response.error_for_status_ref();
                trace.end(Some(status.as_u16()), false);
                return Err("Job cancelled".to_string());
            }
        }

        match response.chunk().await {
            Ok(Some(bytes)) => {
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
                    let delta = event
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    let done = event.get("done").and_then(|d| d.as_bool()).unwrap_or(false);
                    let _ = app.emit(
                        "ai:stream",
                        json!({ "jobId": job_id, "delta": delta, "done": done }),
                    );
                    if done {
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .complete(job_id, json!({ "done": true }));
                        trace.end(Some(status.as_u16()), true);
                        return Ok(());
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                trace.end(Some(status.as_u16()), false);
                return Err(format!("Stream error: {e}"));
            }
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>()
        .lock()
        .complete(job_id, json!({ "done": true }));
    trace.end(Some(status.as_u16()), true);
    Ok(())
}

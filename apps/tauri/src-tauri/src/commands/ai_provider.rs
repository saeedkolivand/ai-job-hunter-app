//! AI provider abstraction.
//!
//! Each supported backend (Ollama, OpenAI-compatible, Anthropic, Gemini)
//! implements the `AiProvider` trait. `provider_for()` is the single routing
//! point — adding a provider means adding one struct, one impl, and one match
//! arm here, instead of editing the `match` in three separate commands.

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::jobs::JobTracker;
use parking_lot::Mutex;

use super::ai::get_provider_key;
pub use crate::ipc_contracts::ai::AiGenerateRequest;

/// A chat/embedding backend. Object-safe so the registry can return
/// `Box<dyn AiProvider>`.
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Stream a chat completion, emitting `ai:stream` events for each delta and
    /// marking the job complete/failed on the `JobTracker`. Resolves the API
    /// key itself (from the credential store or env) so callers stay uniform.
    async fn stream_chat(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> Result<(), String>;

    /// Validate the provider's stored API key. Returns Ok(()) when reachable.
    async fn test_key(&self, client: &reqwest::Client, api_key: &str) -> Result<(), String>;

    /// List the models this provider exposes for the given key.
    async fn list_models(&self, client: &reqwest::Client, api_key: &str) -> Vec<Value>;
}

/// Single routing point for all AI providers.
pub fn provider_for(name: &str) -> Box<dyn AiProvider> {
    match name {
        "openai" | "openai-compatible" => {
            Box::new(OpenAiProvider { provider: name.to_string() })
        }
        "anthropic" => Box::new(AnthropicProvider),
        "gemini" => Box::new(GeminiProvider),
        _ => Box::new(OllamaProvider),
    }
}

// ── Ollama ──────────────────────────────────────────────────────────────────

pub struct OllamaProvider;

fn ollama_host() -> String {
    std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string())
}

#[async_trait]
impl AiProvider for OllamaProvider {
    async fn stream_chat(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> Result<(), String> {
        stream_ollama_chat(app, &ollama_host(), job_id, req).await
    }

    async fn test_key(&self, client: &reqwest::Client, _api_key: &str) -> Result<(), String> {
        // Ollama needs no key — a reachable host counts as healthy.
        match client.get(format!("{}/api/tags", ollama_host())).send().await {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(format!("Ollama returned status: {}", r.status())),
            Err(e) => Err(format!("Ollama unreachable: {e}")),
        }
    }

    async fn list_models(&self, client: &reqwest::Client, _api_key: &str) -> Vec<Value> {
        let resp = match client.get(format!("{}/api/tags", ollama_host())).send().await {
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
}

// ── OpenAI / OpenAI-compatible ────────────────────────────────────────────────

pub struct OpenAiProvider {
    provider: String,
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    async fn stream_chat(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> Result<(), String> {
        let api_key = get_provider_key(app, &self.provider).unwrap_or_default();
        let base_url = req
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1")
            .to_string();
        stream_openai_chat(app, &base_url, &api_key, job_id, req).await
    }

    async fn test_key(&self, client: &reqwest::Client, api_key: &str) -> Result<(), String> {
        let resp = client
            .get("https://api.openai.com/v1/models")
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("API returned status: {}", resp.status()))
        }
    }

    async fn list_models(&self, client: &reqwest::Client, api_key: &str) -> Vec<Value> {
        let resp = client
            .get("https://api.openai.com/v1/models")
            .bearer_auth(api_key)
            .send()
            .await;
        if let Ok(r) = resp {
            if let Ok(body) = r.json::<Value>().await {
                if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                    return data
                        .iter()
                        .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                        .filter(|id| {
                            id.starts_with("gpt-")
                                || id.starts_with("o1")
                                || id.starts_with("o3")
                                || id.starts_with("o4")
                        })
                        .map(|id| json!({ "name": id }))
                        .collect();
                }
            }
        }
        vec![]
    }
}

// ── Anthropic ─────────────────────────────────────────────────────────────────

pub struct AnthropicProvider;

#[async_trait]
impl AiProvider for AnthropicProvider {
    async fn stream_chat(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> Result<(), String> {
        let api_key = get_provider_key(app, "anthropic").unwrap_or_default();
        stream_anthropic_chat(app, &api_key, job_id, req).await
    }

    async fn test_key(&self, client: &reqwest::Client, api_key: &str) -> Result<(), String> {
        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&json!({
                "model": "claude-3-haiku-20240307",
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": "test" }]
            }))
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;
        // 400 means the key is valid but our minimal request was rejected.
        if resp.status().is_success() || resp.status() == 400 {
            Ok(())
        } else {
            Err(format!("API returned status: {}", resp.status()))
        }
    }

    async fn list_models(&self, client: &reqwest::Client, api_key: &str) -> Vec<Value> {
        let resp = client
            .get("https://api.anthropic.com/v1/models")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await;
        if let Ok(r) = resp {
            if let Ok(body) = r.json::<Value>().await {
                if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                    return data
                        .iter()
                        .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                        .filter(|id| id.starts_with("claude-"))
                        .map(|id| json!({ "name": id }))
                        .collect();
                }
            }
        }
        vec![]
    }
}

// ── Gemini ────────────────────────────────────────────────────────────────────

pub struct GeminiProvider;

#[async_trait]
impl AiProvider for GeminiProvider {
    async fn stream_chat(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> Result<(), String> {
        let api_key = get_provider_key(app, "gemini").unwrap_or_default();
        stream_gemini_chat(app, &api_key, job_id, req).await
    }

    async fn test_key(&self, client: &reqwest::Client, api_key: &str) -> Result<(), String> {
        let resp = client
            .get(format!(
                "https://generativelanguage.googleapis.com/v1/models?key={api_key}"
            ))
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("API returned status: {}", resp.status()))
        }
    }

    async fn list_models(&self, client: &reqwest::Client, api_key: &str) -> Vec<Value> {
        let resp = client
            .get(format!(
                "https://generativelanguage.googleapis.com/v1/models?key={api_key}"
            ))
            .send()
            .await;
        if let Ok(r) = resp {
            if let Ok(body) = r.json::<Value>().await {
                if let Some(models) = body.get("models").and_then(|d| d.as_array()) {
                    return models
                        .iter()
                        .filter_map(|m| m.get("name").and_then(|id| id.as_str()))
                        .filter(|id| id.starts_with("models/"))
                        .map(|id| json!({ "name": id.strip_prefix("models/").unwrap_or(id) }))
                        .collect();
                }
            }
        }
        vec![]
    }
}

// ── Streaming implementations ─────────────────────────────────────────────────

async fn stream_ollama_chat(
    app: &AppHandle,
    base: &str,
    job_id: &str,
    req: &AiGenerateRequest,
) -> Result<(), String> {
    let messages = serde_json::to_value(
        &req.messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect::<Vec<_>>(),
    )
    .unwrap_or(json!([]));

    let mut body = json!({
        "model": req.model,
        "messages": messages,
        "stream": true,
    });
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

    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{base}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama {status}: {body_text}"));
    }

    let mut line_buf = String::new();

    loop {
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                let _ = response.error_for_status_ref();
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
                        return Ok(());
                    }
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("Stream error: {e}")),
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>()
        .lock()
        .complete(job_id, json!({ "done": true }));

    Ok(())
}

async fn stream_openai_chat(
    app: &AppHandle,
    base_url: &str,
    api_key: &str,
    job_id: &str,
    req: &AiGenerateRequest,
) -> Result<(), String> {
    let messages = req
        .messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect::<Vec<_>>();
    let temperature = req.temperature.unwrap_or(0.7);

    let mut body = json!({
        "model": req.model,
        "messages": messages,
        "stream": true,
        "temperature": temperature,
    });
    if let Some(mt) = req.max_tokens {
        body["max_tokens"] = json!(mt);
    }

    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI {status}: {body_text}"));
    }

    let mut line_buf = String::new();

    loop {
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                drop(response);
                return Err("Job cancelled".to_string());
            }
        }

        match response.chunk().await {
            Ok(Some(bytes)) => {
                line_buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(nl) = line_buf.find('\n') {
                    let line = line_buf[..nl].trim().to_string();
                    line_buf = line_buf[nl + 1..].to_string();

                    let data = match line.strip_prefix("data: ") {
                        Some(d) => d.trim(),
                        None => continue,
                    };

                    if data == "[DONE]" {
                        let _ = app.emit(
                            "ai:stream",
                            json!({ "jobId": job_id, "delta": "", "done": true }),
                        );
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .complete(job_id, json!({ "done": true }));
                        return Ok(());
                    }

                    let event: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let delta = event
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("");

                    if !delta.is_empty() {
                        let _ = app.emit(
                            "ai:stream",
                            json!({ "jobId": job_id, "delta": delta, "done": false }),
                        );
                    }
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("Stream error: {e}")),
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>()
        .lock()
        .complete(job_id, json!({ "done": true }));
    Ok(())
}

async fn stream_anthropic_chat(
    app: &AppHandle,
    api_key: &str,
    job_id: &str,
    req: &AiGenerateRequest,
) -> Result<(), String> {
    let temperature = req.temperature.unwrap_or(0.7);
    let max_tokens = req.max_tokens.unwrap_or(4096);

    let system_content: String = req
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let messages: Vec<Value> = req
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();

    // Enable extended thinking for balanced effort and above (max_tokens >= 2048).
    // Thinking requires temperature=1; budget is half the output quota.
    let thinking_budget = if max_tokens >= 2048 { max_tokens / 2 } else { 0 };
    let actual_max_tokens = max_tokens + thinking_budget;

    let mut body = json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": actual_max_tokens,
        "stream": true,
        "temperature": if thinking_budget > 0 { 1.0 } else { temperature },
    });
    if thinking_budget > 0 {
        body["thinking"] = json!({ "type": "enabled", "budget_tokens": thinking_budget });
    }
    if !system_content.is_empty() {
        body["system"] = json!(system_content);
    }

    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("Anthropic {status}: {body_text}"));
    }

    let mut line_buf = String::new();
    let mut last_event = String::new();

    loop {
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                drop(response);
                return Err("Job cancelled".to_string());
            }
        }

        match response.chunk().await {
            Ok(Some(bytes)) => {
                line_buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(nl) = line_buf.find('\n') {
                    let line = line_buf[..nl].trim().to_string();
                    line_buf = line_buf[nl + 1..].to_string();

                    if let Some(event) = line.strip_prefix("event: ") {
                        last_event = event.trim().to_string();
                        continue;
                    }

                    let data = match line.strip_prefix("data: ") {
                        Some(d) => d.trim(),
                        None => continue,
                    };

                    if last_event == "message_stop" || data.contains("\"type\":\"message_stop\"") {
                        let _ = app.emit(
                            "ai:stream",
                            json!({ "jobId": job_id, "delta": "", "done": true }),
                        );
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .complete(job_id, json!({ "done": true }));
                        return Ok(());
                    }

                    let event: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let delta_obj = event.get("delta");
                    let delta_type = delta_obj
                        .and_then(|d| d.get("type"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    match delta_type {
                        "thinking_delta" => {
                            let thinking = delta_obj
                                .and_then(|d| d.get("thinking"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if !thinking.is_empty() {
                                let _ = app.emit(
                                    "ai:stream",
                                    json!({
                                        "jobId": job_id,
                                        "delta": thinking,
                                        "done": false,
                                        "thinking": true,
                                    }),
                                );
                            }
                        }
                        "text_delta" => {
                            let text = delta_obj
                                .and_then(|d| d.get("text"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if !text.is_empty() {
                                let _ = app.emit(
                                    "ai:stream",
                                    json!({
                                        "jobId": job_id,
                                        "delta": text,
                                        "done": false,
                                    }),
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("Stream error: {e}")),
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>()
        .lock()
        .complete(job_id, json!({ "done": true }));
    Ok(())
}

async fn stream_gemini_chat(
    app: &AppHandle,
    api_key: &str,
    job_id: &str,
    req: &AiGenerateRequest,
) -> Result<(), String> {
    let temperature = req.temperature.unwrap_or(0.7);

    let system_text: String = req
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let contents: Vec<Value> = req
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| {
            let role = if m.role == "assistant" { "model" } else { "user" };
            json!({ "role": role, "parts": [{ "text": m.content }] })
        })
        .collect();

    let mut generation_config = json!({ "temperature": temperature });
    if let Some(mt) = req.max_tokens {
        generation_config["maxOutputTokens"] = json!(mt);
    }
    let mut body = json!({
        "contents": contents,
        "generationConfig": generation_config,
    });
    if !system_text.is_empty() {
        body["systemInstruction"] = json!({ "parts": [{ "text": system_text }] });
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}",
        req.model, api_key
    );

    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return Err(format!("Gemini {status}: {body_text}"));
    }

    let mut buf = String::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;

    loop {
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                drop(response);
                return Err("Job cancelled".to_string());
            }
        }

        match response.chunk().await {
            Ok(Some(bytes)) => {
                let chunk = String::from_utf8_lossy(&bytes).to_string();
                for ch in chunk.chars() {
                    if escape {
                        escape = false;
                        buf.push(ch);
                        continue;
                    }
                    if ch == '\\' && in_string {
                        escape = true;
                        buf.push(ch);
                        continue;
                    }
                    if ch == '"' {
                        in_string = !in_string;
                    }
                    if !in_string {
                        if ch == '{' {
                            depth += 1;
                        } else if ch == '}' {
                            depth -= 1;
                        }
                    }
                    buf.push(ch);

                    if depth == 0 && buf.trim_start().starts_with('{') && !buf.trim().is_empty() {
                        if let Ok(event) = serde_json::from_str::<Value>(buf.trim()) {
                            let delta = event
                                .get("candidates")
                                .and_then(|c| c.get(0))
                                .and_then(|c| c.get("content"))
                                .and_then(|c| c.get("parts"))
                                .and_then(|p| p.get(0))
                                .and_then(|p| p.get("text"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if !delta.is_empty() {
                                let _ = app.emit(
                                    "ai:stream",
                                    json!({ "jobId": job_id, "delta": delta, "done": false }),
                                );
                            }
                        }
                        buf.clear();
                    }
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("Stream error: {e}")),
        }
    }

    let _ = app.emit("ai:stream", json!({ "jobId": job_id, "delta": "", "done": true }));
    app.state::<Mutex<JobTracker>>()
        .lock()
        .complete(job_id, json!({ "done": true }));
    Ok(())
}

//! Anthropic provider — Messages API only.

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::ai::get_provider_key;
use crate::jobs::JobTracker;

use crate::error::{AppError, AppResult};

use super::{
    friendly_api_error, AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, RequestTrace,
    TokenParam,
};

const BASE: &str = "https://api.anthropic.com/v1";
const VERSION: &str = "2023-06-01";

pub struct AnthropicClient;

#[async_trait]
impl AiProvider for AnthropicClient {
    fn id(&self) -> ProviderId {
        ProviderId::Anthropic
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            // Anthropic carries the system prompt as a top-level field, not a role.
            supports_system_role: false,
            supports_streaming: true,
            supports_reasoning: true,
            supports_tools: true,
            supports_json_mode: false,
            supports_embeddings: false,
            token_param: TokenParam::MaxTokens,
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(ProviderId::Anthropic, &req.model, "/messages", BASE, true);

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

        // Extended thinking for balanced effort and above; requires temperature=1.
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

        let response = crate::net::http::shared()
            .post(&endpoint)
            .timeout(std::time::Duration::from_secs(300))
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
            .json(&body)
            .send()
            .await;

        let mut response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Anthropic unreachable: {e}")));
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Anthropic, status, &body_text));
        }

        let mut line_buf = String::new();
        let mut last_event = String::new();
        loop {
            if let Some(job) = app.state::<Mutex<JobTracker>>().lock().get(job_id) {
                if job.status == crate::jobs::JobStatus::Cancelled {
                    drop(response);
                    trace.end(Some(status.as_u16()), false);
                    return Err(AppError::Message("Job cancelled".to_string()));
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
                        if last_event == "message_stop"
                            || data.contains("\"type\":\"message_stop\"")
                        {
                            let _ = app.emit(
                                "ai:stream",
                                json!({ "jobId": job_id, "delta": "", "done": true }),
                            );
                            app.state::<Mutex<JobTracker>>()
                                .lock()
                                .complete(job_id, json!({ "done": true }));
                            trace.end(Some(status.as_u16()), true);
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
                                        json!({ "jobId": job_id, "delta": thinking, "done": false, "thinking": true }),
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
                                        json!({ "jobId": job_id, "delta": text, "done": false }),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    trace.end(Some(status.as_u16()), false);
                    return Err(AppError::Network(format!("Stream error: {e}")));
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

    async fn complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String> {
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(ProviderId::Anthropic, model, "/messages", BASE, false);

        let mut body = json!({
            "model": model,
            "max_tokens": 4096,
            "messages": [ { "role": "user", "content": user } ],
            "temperature": temperature.unwrap_or(0.7),
        });
        if !system.is_empty() {
            body["system"] = json!(system);
        }

        let resp = crate::net::http::shared()
            .post(&endpoint)
            .timeout(std::time::Duration::from_secs(120))
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Anthropic unreachable: {e}")));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Anthropic, status, &body_text));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        // Concatenate all text blocks in the content array.
        data.get("content")
            .and_then(|c| c.as_array())
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::Provider("Anthropic: unexpected response shape".to_string()))
    }

    async fn embed(&self, _app: &AppHandle, _model: &str, _text: &str) -> AppResult<Vec<f64>> {
        Err(AppError::Provider(
            "Anthropic has no embeddings API. Use OpenAI, Gemini, or Ollama for embeddings.".to_string(),
        ))
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        None
    }

    async fn list_models(&self, client: &reqwest::Client, api_key: &str) -> Vec<Value> {
        let resp = client
            .get(format!("{BASE}/models"))
            .header("x-api-key", api_key)
            .header("anthropic-version", VERSION)
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

    async fn test_key(&self, client: &reqwest::Client, api_key: &str) -> AppResult<()> {
        let resp = client
            .post(format!("{BASE}/messages"))
            .header("x-api-key", api_key)
            .header("anthropic-version", VERSION)
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
            Err(AppError::Provider(format!("API returned status: {}", resp.status())))
        }
    }
}

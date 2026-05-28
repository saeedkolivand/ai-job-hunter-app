//! OpenAI and OpenAI-compatible providers (LM Studio, vLLM, OpenRouter, Groq,
//! Together, DeepSeek, Azure-style gateways…). One client, parameterized by the
//! `ProviderId` and an optional base URL.

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::ai::get_provider_key;
use crate::jobs::JobTracker;

use super::{
    friendly_api_error, AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, RequestTrace,
    TokenParam,
};

const DEFAULT_BASE: &str = "https://api.openai.com/v1";

/// OpenAI reasoning families (o1/o3/o4) reject `temperature` and require
/// `max_completion_tokens` instead of `max_tokens`.
fn is_reasoning_model(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.starts_with("o1") || m.starts_with("o3") || m.starts_with("o4")
}

pub struct OpenAiClient {
    id: ProviderId,
    base_url: String,
}

impl OpenAiClient {
    pub fn new(id: ProviderId, base_url: Option<String>) -> Self {
        Self {
            id,
            base_url: base_url
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_BASE.to_string()),
        }
    }
}

#[async_trait]
impl AiProvider for OpenAiClient {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        let reasoning = is_reasoning_model(model);
        ModelCapabilities {
            supports_temperature: !reasoning,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: reasoning,
            supports_tools: true,
            supports_json_mode: true,
            supports_embeddings: true,
            token_param: if reasoning {
                TokenParam::MaxCompletionTokens
            } else {
                TokenParam::MaxTokens
            },
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> Result<(), String> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let caps = self.capabilities(&req.model);
        let endpoint = format!("{}/chat/completions", self.base_url);
        let trace = RequestTrace::begin(self.id, &req.model, "/chat/completions", &self.base_url, true);

        let messages = req
            .messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect::<Vec<_>>();

        let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
        if caps.supports_temperature {
            body["temperature"] = json!(req.temperature.unwrap_or(0.7));
        }
        if let Some(mt) = req.max_tokens {
            let field = match caps.token_param {
                TokenParam::MaxCompletionTokens => "max_completion_tokens",
                _ => "max_tokens",
            };
            body[field] = json!(mt);
        }

        let response = crate::net::http::shared()
            .post(&endpoint)
            .timeout(std::time::Duration::from_secs(300))
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await;

        let mut response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(format!("{} unreachable: {e}", self.id.as_str()));
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(self.id, status, &body_text));
        }

        let mut line_buf = String::new();
        loop {
            if let Some(job) = app.state::<Mutex<JobTracker>>().lock().get(job_id) {
                if job.status == crate::jobs::JobStatus::Cancelled {
                    drop(response);
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
                            trace.end(Some(status.as_u16()), true);
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

    async fn complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> Result<String, String> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let caps = self.capabilities(model);
        let endpoint = format!("{}/chat/completions", self.base_url);
        let trace = RequestTrace::begin(self.id, model, "/chat/completions", &self.base_url, false);

        let mut body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
            "stream": false,
        });
        if caps.supports_temperature {
            body["temperature"] = json!(temperature.unwrap_or(0.7));
        }

        let resp = crate::net::http::shared()
            .post(&endpoint)
            .timeout(std::time::Duration::from_secs(120))
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(format!("{} unreachable: {e}", self.id.as_str()));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(self.id, status, &body_text));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        data.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .map(String::from)
            .ok_or_else(|| format!("{}: unexpected response shape", self.id.as_str()))
    }

    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> Result<Vec<f64>, String> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let endpoint = format!("{}/embeddings", self.base_url);
        let trace = RequestTrace::begin(self.id, model, "/embeddings", &self.base_url, false);
        let resp = crate::net::http::shared()
            .post(&endpoint)
            .timeout(std::time::Duration::from_secs(30))
            .bearer_auth(&api_key)
            .json(&json!({ "model": model, "input": text }))
            .send()
            .await
            .map_err(|e| format!("{} unreachable: {e}", self.id.as_str()))?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(self.id, status, &body_text));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        data.get("data")
            .and_then(|d| d.get(0))
            .and_then(|e| e.get("embedding"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .ok_or_else(|| format!("{}: missing embedding in response", self.id.as_str()))
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some("text-embedding-3-small")
    }

    async fn list_models(&self, client: &reqwest::Client, api_key: &str) -> Vec<Value> {
        let resp = client
            .get(format!("{}/models", self.base_url))
            .bearer_auth(api_key)
            .send()
            .await;
        if let Ok(r) = resp {
            if let Ok(body) = r.json::<Value>().await {
                if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                    return data
                        .iter()
                        .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                        // OpenAI proper: only chat-capable families. Compatible
                        // servers expose arbitrary names, so don't filter those.
                        .filter(|id| {
                            self.id == ProviderId::OpenAiCompatible
                                || id.starts_with("gpt-")
                                || id.starts_with("o1")
                                || id.starts_with("o3")
                                || id.starts_with("o4")
                                || id.starts_with("chatgpt")
                        })
                        .map(|id| json!({ "name": id }))
                        .collect();
                }
            }
        }
        vec![]
    }

    async fn test_key(&self, client: &reqwest::Client, api_key: &str) -> Result<(), String> {
        let resp = client
            .get(format!("{}/models", self.base_url))
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
}

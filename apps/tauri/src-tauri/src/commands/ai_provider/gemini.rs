//! Google Gemini provider — generateContent (streaming) API.

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

const BASE: &str = "https://generativelanguage.googleapis.com";

pub struct GeminiClient;

#[async_trait]
impl AiProvider for GeminiClient {
    fn id(&self) -> ProviderId {
        ProviderId::Gemini
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            supports_system_role: true, // mapped to systemInstruction
            supports_streaming: true,
            supports_reasoning: false,
            supports_tools: true,
            supports_json_mode: true,
            supports_embeddings: true,
            token_param: TokenParam::MaxOutputTokens,
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> Result<(), String> {
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint_label = format!("/v1beta/models/{}:streamGenerateContent", req.model);
        let trace = RequestTrace::begin(ProviderId::Gemini, &req.model, &endpoint_label, BASE, true);

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
        let mut body = json!({ "contents": contents, "generationConfig": generation_config });
        if !system_text.is_empty() {
            body["systemInstruction"] = json!({ "parts": [{ "text": system_text }] });
        }

        let url = format!("{BASE}{endpoint_label}?key={api_key}");
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| e.to_string())?
            .post(&url)
            .json(&body)
            .send()
            .await;

        let mut response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(format!("Gemini unreachable: {e}"));
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }

        // Gemini streams a JSON array; parse complete top-level objects as they arrive.
        let mut buf = String::new();
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escape = false;
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

                        if depth == 0 && buf.trim_start().starts_with('{') && !buf.trim().is_empty()
                        {
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
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:generateContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, false);

        let mut body = json!({
            "contents": [ { "role": "user", "parts": [{ "text": user }] } ],
            "generationConfig": { "temperature": temperature.unwrap_or(0.7) },
        });
        if !system.is_empty() {
            body["systemInstruction"] = json!({ "parts": [{ "text": system }] });
        }

        let url = format!("{BASE}{endpoint_label}?key={api_key}");
        let resp = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?
            .post(&url)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(format!("Gemini unreachable: {e}"));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        data.get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Gemini: unexpected response shape".to_string())
    }

    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> Result<Vec<f64>, String> {
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:embedContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, false);
        let body = json!({
            "model": format!("models/{m}"),
            "content": { "parts": [{ "text": text }] },
        });
        let url = format!("{BASE}{endpoint_label}?key={api_key}");
        let resp = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Gemini unreachable: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        data.get("embedding")
            .and_then(|e| e.get("values"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .ok_or_else(|| "Gemini: missing embedding in response".to_string())
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some("text-embedding-004")
    }

    async fn list_models(&self, client: &reqwest::Client, api_key: &str) -> Vec<Value> {
        let resp = client
            .get(format!("{BASE}/v1/models?key={api_key}"))
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

    async fn test_key(&self, client: &reqwest::Client, api_key: &str) -> Result<(), String> {
        let resp = client
            .get(format!("{BASE}/v1/models?key={api_key}"))
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

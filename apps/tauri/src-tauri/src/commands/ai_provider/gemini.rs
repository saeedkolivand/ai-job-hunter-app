//! Google Gemini provider — generateContent (streaming) API.

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::ai::get_provider_key;
use crate::jobs::JobTracker;

use crate::error::{AppError, AppResult};

use super::research;
use super::{
    friendly_api_error, AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, RequestTrace,
    TokenParam,
};

const BASE: &str = "https://generativelanguage.googleapis.com";

/// Concatenate every `parts[].text` of the first candidate (non-streaming
/// `generateContent`, incl. grounded responses) into one string. Pure +
/// unit-tested.
fn join_parts_text(data: &Value) -> String {
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
        .unwrap_or_default()
}

/// Whether to request `thinkingConfig.includeThoughts`. Gemini 1.5 and the GA
/// 2.0 models reject `thinkingConfig` with a 400, so we only enable it for the
/// 2.5 family and any explicit `*-thinking-*` model. Unknown future models simply
/// don't surface thoughts (a graceful miss, never a broken request).
fn gemini_supports_thinking(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("2.5") || m.contains("thinking")
}

/// Extract a Gemini chunk's streamed parts as `(is_thought, text)` pairs. 2.5
/// thinking models flag reasoning parts with `"thought": true`; the rest are
/// normal answer text. Pure + unit-tested so the streaming loop stays a thin
/// emitter.
fn parse_gemini_parts(event: &Value) -> Vec<(bool, &str)> {
    event
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    let text = part.get("text").and_then(|t| t.as_str())?;
                    let thought = part
                        .get("thought")
                        .and_then(|t| t.as_bool())
                        .unwrap_or(false);
                    Some((thought, text))
                })
                .collect()
        })
        .unwrap_or_default()
}

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
    ) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint_label = format!("/v1beta/models/{}:streamGenerateContent", req.model);
        let trace =
            RequestTrace::begin(ProviderId::Gemini, &req.model, &endpoint_label, BASE, true);

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
                let role = if m.role == "assistant" {
                    "model"
                } else {
                    "user"
                };
                json!({ "role": role, "parts": [{ "text": m.content }] })
            })
            .collect();

        let mut generation_config = json!({ "temperature": temperature });
        if let Some(mt) = req.max_tokens {
            generation_config["maxOutputTokens"] = json!(mt);
        }
        // Ask thinking-capable models to stream their reasoning as `thought` parts.
        if gemini_supports_thinking(&req.model) {
            generation_config["thinkingConfig"] = json!({ "includeThoughts": true });
        }
        let mut body = json!({ "contents": contents, "generationConfig": generation_config });
        if !system_text.is_empty() {
            body["systemInstruction"] = json!({ "parts": [{ "text": system_text }] });
        }

        let url = format!("{BASE}{endpoint_label}?key={api_key}");
        let response = crate::net::http::shared()
            .post(&url)
            .timeout(std::time::Duration::from_secs(300))
            .json(&body)
            .send()
            .await;

        let mut response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Gemini unreachable: {e}")));
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
                    return Err(AppError::Message("Job cancelled".to_string()));
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
                                // Split every part: `thought` parts stream as reasoning,
                                // the rest as the normal answer.
                                for (thought, text) in parse_gemini_parts(&event) {
                                    if text.is_empty() {
                                        continue;
                                    }
                                    let chunk = if thought {
                                        json!({ "jobId": job_id, "delta": text, "done": false, "thinking": true })
                                    } else {
                                        json!({ "jobId": job_id, "delta": text, "done": false })
                                    };
                                    let _ = app.emit("ai:stream", chunk);
                                }
                            }
                            buf.clear();
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

        let _ = app.emit(
            "ai:stream",
            json!({ "jobId": job_id, "delta": "", "done": true }),
        );
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
        let resp = crate::net::http::shared()
            .post(&url)
            .timeout(std::time::Duration::from_secs(120))
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Gemini unreachable: {e}")));
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
        let text = join_parts_text(&data);
        if text.is_empty() {
            return Err(AppError::Provider(
                "Gemini: unexpected response shape".to_string(),
            ));
        }
        Ok(text)
    }

    async fn research(
        &self,
        app: &AppHandle,
        model: &str,
        company: &str,
        role: &str,
    ) -> AppResult<String> {
        let api_key = match get_provider_key(app, self.id().credential_key()) {
            Some(k) if !k.trim().is_empty() => k,
            _ => return Ok(String::new()),
        };
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:generateContent");
        let trace = RequestTrace::begin(
            ProviderId::Gemini,
            model,
            "/generateContent google_search",
            BASE,
            false,
        );

        // Grounding with Google Search: the model searches and writes the brief.
        let body = json!({
            "contents": [ { "role": "user", "parts": [{ "text": research::native_user(company, role) }] } ],
            "systemInstruction": { "parts": [{ "text": research::NATIVE_SYSTEM }] },
            "generationConfig": { "temperature": 0.2 },
            "tools": [{ "google_search": {} }],
        });
        let url = format!("{BASE}{endpoint_label}?key={api_key}");
        let resp = crate::net::http::shared()
            .post(&url)
            .timeout(std::time::Duration::from_secs(25))
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                tracing::warn!("gemini research unreachable: {e}");
                return Ok(String::new());
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            tracing::warn!("gemini research {status}: {body_text}");
            return Ok(String::new());
        }
        let data: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => {
                trace.end(Some(status.as_u16()), false);
                return Ok(String::new());
            }
        };
        trace.end(Some(status.as_u16()), true);
        Ok(join_parts_text(&data))
    }

    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>> {
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:embedContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, false);
        let body = json!({
            "model": format!("models/{m}"),
            "content": { "parts": [{ "text": text }] },
        });
        let url = format!("{BASE}{endpoint_label}?key={api_key}");
        let resp = crate::net::http::shared()
            .post(&url)
            .timeout(std::time::Duration::from_secs(30))
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
            .ok_or_else(|| AppError::Provider("Gemini: missing embedding in response".to_string()))
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some("text-embedding-004")
    }

    async fn list_models(&self, app: &AppHandle) -> Vec<Value> {
        let api_key = match get_provider_key(app, self.id().credential_key()) {
            Some(k) => k,
            None => return vec![],
        };
        let client = match super::probe_client() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
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

    async fn test_key(&self, app: &AppHandle) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id().credential_key())
            .ok_or_else(|| AppError::Config("No API key found".to_string()))?;
        let client = super::probe_client()?;
        let resp = client
            .get(format!("{BASE}/v1/models?key={api_key}"))
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AppError::Provider(format!(
                "API returned status: {}",
                resp.status()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{gemini_supports_thinking, join_parts_text, parse_gemini_parts};
    use serde_json::json;

    #[test]
    fn join_parts_text_concatenates_first_candidate_parts() {
        let data = json!({
            "candidates": [{
                "content": { "parts": [{ "text": "Acme is " }, { "text": "a widget maker." }] },
                "groundingMetadata": { "webSearchQueries": ["Acme"] }
            }]
        });
        assert_eq!(join_parts_text(&data), "Acme is a widget maker.");
        assert_eq!(join_parts_text(&json!({})), "");
        assert_eq!(join_parts_text(&json!({ "candidates": [] })), "");
    }

    #[test]
    fn thinking_gate_enables_only_known_models() {
        for m in [
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "gemini-2.0-flash-thinking",
        ] {
            assert!(gemini_supports_thinking(m), "{m} should enable thinking");
        }
        for m in ["gemini-1.5-pro", "gemini-1.5-flash", "gemini-2.0-flash"] {
            assert!(
                !gemini_supports_thinking(m),
                "{m} must not request thinkingConfig (it 400s)"
            );
        }
    }

    #[test]
    fn parse_parts_splits_thought_from_answer() {
        let ev = json!({
            "candidates": [{
                "content": { "parts": [
                    { "text": "reasoning…", "thought": true },
                    { "text": "the answer" }
                ] }
            }]
        });
        assert_eq!(
            parse_gemini_parts(&ev),
            vec![(true, "reasoning…"), (false, "the answer")]
        );
    }

    #[test]
    fn parse_parts_empty_without_candidates() {
        assert!(parse_gemini_parts(&json!({})).is_empty());
        assert!(parse_gemini_parts(&json!({ "candidates": [] })).is_empty());
    }
}

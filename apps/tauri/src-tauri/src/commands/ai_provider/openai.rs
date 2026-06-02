//! OpenAI and OpenAI-compatible providers (LM Studio, vLLM, OpenRouter, Groq,
//! Together, DeepSeek, Azure-style gateways…). One client, parameterized by the
//! `ProviderId` and an optional base URL.

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

const DEFAULT_BASE: &str = "https://api.openai.com/v1";

/// Concatenate the assistant text from a Responses API result. The `output`
/// array interleaves `web_search_call` items with the final `message`; we take
/// the `output_text` blocks of message items. Pure + unit-tested.
fn join_responses_text(data: &Value) -> String {
    data.get("output")
        .and_then(|o| o.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|it| it.get("type").and_then(|t| t.as_str()) == Some("message"))
                .filter_map(|it| it.get("content").and_then(|c| c.as_array()))
                .flatten()
                .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Whether a model id returned by `/v1/models` should be offered in the picker.
/// Native OpenAI exposes a large non-chat catalog (embeddings, audio, image,
/// moderation…), so restrict it to chat-capable families. Every *other*
/// OpenAI-compatible backend (custom gateways, Ollama Cloud, …) returns a curated
/// catalog of its own models under arbitrary names, so pass those through
/// unfiltered — that way a new composed provider lists its full catalog with no
/// code change here.
fn should_list_model(provider: ProviderId, id: &str) -> bool {
    provider != ProviderId::OpenAi
        || id.starts_with("gpt-")
        || id.starts_with("o1")
        || id.starts_with("o3")
        || id.starts_with("o4")
        || id.starts_with("chatgpt")
}

/// OpenAI reasoning families (the `o`-series: o1, o3, o4, … and future `o`N)
/// reject `temperature` and require `max_completion_tokens` instead of
/// `max_tokens`. Matched by the `o`+digit convention so new o-series models are
/// handled without a code change.
fn is_reasoning_model(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    let mut bytes = m.bytes();
    matches!((bytes.next(), bytes.next()), (Some(b'o'), Some(d)) if d.is_ascii_digit())
}

/// Split one streaming chunk into `(reasoning, content)` deltas.
///
/// OpenAI-compatible servers that expose chain-of-thought put it on
/// `delta.reasoning_content` (DeepSeek-R1, vLLM, LM Studio, Ollama's OpenAI
/// shim) or `delta.reasoning` (OpenRouter); the visible answer stays on
/// `delta.content`. Either may be empty/absent. Pure + unit-tested so the
/// streaming loop stays a thin emitter.
///
/// Honest limitation: OpenAI's own o-series hide their reasoning text over Chat
/// Completions, so there is nothing to surface there — only the answer streams.
fn parse_openai_delta(event: &Value) -> (&str, &str) {
    let delta = event
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));
    let reasoning = delta
        .and_then(|d| d.get("reasoning_content").or_else(|| d.get("reasoning")))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let content = delta
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    (reasoning, content)
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
    ) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let caps = self.capabilities(&req.model);
        let endpoint = format!("{}/chat/completions", self.base_url);
        let trace = RequestTrace::begin(
            self.id,
            &req.model,
            "/chat/completions",
            &self.base_url,
            true,
        );

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
                return Err(AppError::Network(format!(
                    "{} unreachable: {e}",
                    self.id.as_str()
                )));
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
                    return Err(AppError::Message("Job cancelled".to_string()));
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
                        let (reasoning, delta) = parse_openai_delta(&event);
                        if !reasoning.is_empty() {
                            let _ = app.emit(
                                "ai:stream",
                                json!({ "jobId": job_id, "delta": reasoning, "done": false, "thinking": true }),
                            );
                        }
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
                return Err(AppError::Network(format!(
                    "{} unreachable: {e}",
                    self.id.as_str()
                )));
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
            .ok_or_else(|| {
                AppError::Provider(format!("{}: unexpected response shape", self.id.as_str()))
            })
    }

    async fn research(
        &self,
        app: &AppHandle,
        model: &str,
        company: &str,
        role: &str,
    ) -> AppResult<String> {
        // Only native OpenAI exposes the Responses `web_search` tool. Generic
        // OpenAI-compatible gateways can't be assumed to support it (and Ollama
        // Cloud overrides `research()` in its own client), so they degrade to "".
        if self.id != ProviderId::OpenAi {
            return Ok(String::new());
        }
        let api_key = match get_provider_key(app, self.id.credential_key()) {
            Some(k) if !k.trim().is_empty() => k,
            _ => return Ok(String::new()),
        };
        let endpoint = format!("{}/responses", self.base_url);
        let trace = RequestTrace::begin(
            self.id,
            model,
            "/responses web_search",
            &self.base_url,
            false,
        );

        let body = json!({
            "model": model,
            "instructions": research::NATIVE_SYSTEM,
            "input": research::native_user(company, role),
            "tools": [{ "type": "web_search" }],
        });
        let resp = crate::net::http::shared()
            .post(&endpoint)
            .timeout(std::time::Duration::from_secs(25))
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                tracing::warn!("openai research unreachable: {e}");
                return Ok(String::new());
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            tracing::warn!("openai research {status}: {body_text}");
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
        Ok(join_responses_text(&data))
    }

    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>> {
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
            .ok_or_else(|| {
                AppError::Provider(format!(
                    "{}: missing embedding in response",
                    self.id.as_str()
                ))
            })
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some("text-embedding-3-small")
    }

    async fn list_models(&self, app: &AppHandle) -> Vec<Value> {
        let api_key = match get_provider_key(app, self.id.credential_key()) {
            Some(k) => k,
            None => return vec![],
        };
        let client = match super::probe_client() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let resp = client
            .get(format!("{}/models", self.base_url))
            .bearer_auth(&api_key)
            .send()
            .await;
        if let Ok(r) = resp {
            if let Ok(body) = r.json::<Value>().await {
                if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                    return data
                        .iter()
                        .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                        // OpenAI proper: only chat-capable families. Every other
                        // OpenAI-compatible backend (incl. Ollama Cloud) lists its
                        // own curated catalog, so pass those through unfiltered.
                        .filter(|id| should_list_model(self.id, id))
                        .map(|id| json!({ "name": id }))
                        .collect();
                }
            }
        }
        vec![]
    }

    async fn test_key(&self, app: &AppHandle) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id.credential_key())
            .ok_or_else(|| AppError::Config("No API key found".to_string()))?;
        let client = super::probe_client()?;
        let resp = client
            .get(format!("{}/models", self.base_url))
            .bearer_auth(&api_key)
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
    use super::{is_reasoning_model, join_responses_text, parse_openai_delta, should_list_model};
    use crate::commands::ai_provider::ProviderId;
    use serde_json::json;

    #[test]
    fn list_filter_only_restricts_native_openai() {
        // Native OpenAI exposes a large non-chat catalog — keep only chat families.
        assert!(should_list_model(ProviderId::OpenAi, "gpt-4o"));
        assert!(should_list_model(ProviderId::OpenAi, "o3-mini"));
        assert!(should_list_model(ProviderId::OpenAi, "chatgpt-4o-latest"));
        for non_chat in ["text-embedding-3-small", "dall-e-3", "whisper-1", "tts-1"] {
            assert!(
                !should_list_model(ProviderId::OpenAi, non_chat),
                "{non_chat} should be filtered out for native OpenAI"
            );
        }

        // Ollama Cloud + generic OpenAI-compatible servers return their own
        // curated catalog under arbitrary names — never filter those, so the
        // full Ollama Cloud list (not just gpt-oss:*) reaches the picker.
        for id in [
            "gpt-oss:120b",
            "qwen3-coder:480b",
            "deepseek-v3.1:671b",
            "kimi-k2:1t",
            "glm-4.6",
        ] {
            assert!(should_list_model(ProviderId::OllamaCloud, id), "{id}");
            assert!(should_list_model(ProviderId::OpenAiCompatible, id), "{id}");
        }
    }

    #[test]
    fn join_responses_text_takes_message_items_only() {
        // The Responses `output` array interleaves the web_search_call with the
        // final assistant message.
        let data = json!({
            "output": [
                { "type": "web_search_call", "id": "ws_1", "status": "completed" },
                { "type": "message", "role": "assistant", "content": [
                    { "type": "output_text", "text": "Acme is a ", "annotations": [] },
                    { "type": "output_text", "text": "widget maker.", "annotations": [] }
                ]}
            ]
        });
        assert_eq!(join_responses_text(&data), "Acme is a widget maker.");
        assert_eq!(join_responses_text(&json!({})), "");
        assert_eq!(join_responses_text(&json!({ "output": [] })), "");
    }

    #[test]
    fn detects_o_series_including_future_models() {
        for m in ["o1", "o1-mini", "o3", "o3-mini", "o4-mini", "o5", "o9-pro"] {
            assert!(is_reasoning_model(m), "{m} should be a reasoning model");
        }
        for m in [
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-3.5-turbo",
            "omni",
            "chatgpt-4o",
        ] {
            assert!(
                !is_reasoning_model(m),
                "{m} should not be a reasoning model"
            );
        }
    }

    #[test]
    fn parse_delta_splits_reasoning_from_content() {
        // DeepSeek-R1 / vLLM style: reasoning on `reasoning_content`.
        let ev = json!({ "choices": [{ "delta": { "reasoning_content": "let me think" } }] });
        assert_eq!(parse_openai_delta(&ev), ("let me think", ""));

        // OpenRouter style: reasoning on `reasoning`.
        let ev = json!({ "choices": [{ "delta": { "reasoning": "pondering" } }] });
        assert_eq!(parse_openai_delta(&ev), ("pondering", ""));

        // Normal answer content.
        let ev = json!({ "choices": [{ "delta": { "content": "the answer" } }] });
        assert_eq!(parse_openai_delta(&ev), ("", "the answer"));
    }

    #[test]
    fn parse_delta_empty_when_no_choices_or_fields() {
        assert_eq!(parse_openai_delta(&json!({})), ("", ""));
        assert_eq!(
            parse_openai_delta(&json!({ "choices": [{ "delta": {} }] })),
            ("", "")
        );
    }
}

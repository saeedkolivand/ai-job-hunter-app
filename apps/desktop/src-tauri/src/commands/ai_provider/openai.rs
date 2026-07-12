//! OpenAI and OpenAI-compatible providers (LM Studio, vLLM, OpenRouter, Groq,
//! Together, DeepSeek, Azure-style gateways…). One client, parameterized by the
//! `ProviderId` and an optional base URL.

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;

use crate::error::{AppError, AppResult};

use super::research;
use super::retry::send_with_retry;
use super::stream::{stream_response, StreamPiece};
use super::timeouts;
use super::{
    friendly_api_error, single_shot_turn, AgentTurn, AiGenerateRequest, AiProvider, ChatMsg,
    ModelCapabilities, ProviderId, RequestTrace, StopReason, TokenParam, ToolCall, ToolSpec, Usage,
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

/// Parse a non-streaming Chat Completions response into an [`AgentTurn`]:
/// `choices[0].message.content` is the text (may be null when tool calls are
/// present), each `choices[0].message.tool_calls[]` maps to a [`ToolCall`] (its
/// `function.arguments` is a JSON *string* — decoded here; malformed → `{}`), and
/// `finish_reason` maps to the stop reason (`tool_calls`→ToolUse, `stop`→End,
/// `length`→Length, else Other). Pure + unit-tested.
fn parse_openai_turn(data: &Value) -> AgentTurn {
    let choice = data.get("choices").and_then(|c| c.get(0));
    let message = choice.and_then(|c| c.get("message"));
    let text = message
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string();
    let tool_calls = message
        .and_then(|m| m.get("tool_calls"))
        .and_then(|c| c.as_array())
        .map(|calls| {
            calls
                .iter()
                .filter_map(|c| {
                    let func = c.get("function")?;
                    let name = func.get("name").and_then(|n| n.as_str())?.to_string();
                    let args = func
                        .get("arguments")
                        .and_then(|a| a.as_str())
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .unwrap_or_else(|| json!({}));
                    Some(ToolCall {
                        id: c
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        name,
                        args,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = match choice
        .and_then(|c| c.get("finish_reason"))
        .and_then(|f| f.as_str())
    {
        Some("tool_calls") => StopReason::ToolUse,
        Some("stop") => StopReason::End,
        Some("length") => StopReason::Length,
        _ => StopReason::Other,
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
        usage: parse_openai_usage(data).unwrap_or_default(),
    }
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
/// Extract `usage.{prompt_tokens,completion_tokens}` from an OpenAI Chat
/// Completions response/chunk — always present on the non-streaming response,
/// and (with `stream_options.include_usage: true`, set by
/// [`build_chat_stream_body`]) on ONE extra streamed chunk carrying no delta,
/// emitted right before `[DONE]`. `None` on every other streamed chunk. Pure +
/// unit-tested.
fn parse_openai_usage(data: &Value) -> Option<Usage> {
    let usage = data.get("usage")?;
    Some(Usage {
        input_tokens: usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    })
}

/// Extract real token usage from an OpenAI `/embeddings` response:
/// `usage.prompt_tokens` (falling back to `usage.total_tokens`, which some
/// OpenAI-compatible servers send instead), and `output_tokens: 0` — an
/// embedding call has no completion tokens. Zero on both fields when `usage`
/// is entirely absent (never fabricated). Pure + unit-tested.
fn parse_openai_embed_usage(data: &Value) -> Usage {
    let usage = data.get("usage");
    let input_tokens = usage
        .and_then(|u| u.get("prompt_tokens").or_else(|| u.get("total_tokens")))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    Usage {
        input_tokens,
        output_tokens: 0,
    }
}

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

/// Drain complete `data:`-prefixed SSE lines from the accumulated stream buffer
/// into [`StreamPiece`]s, leaving any partial trailing line for the next chunk.
/// `data: [DONE]` yields a terminal sentinel; other lines split into reasoning +
/// content via [`parse_openai_delta`]. Pure + unit-tested; this is the `parse`
/// closure handed to [`stream_response`], so OpenAI's SSE framing lives here only.
fn parse_openai_frames(buf: &mut String) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    // Walk by a `consumed` offset and `drain(..consumed)` once at the end, instead
    // of reallocating the whole tail per line (O(n²) on a big frame).
    let mut consumed = 0;
    while let Some(rel) = buf[consumed..].find('\n') {
        let nl = consumed + rel;
        let line = buf[consumed..nl].trim().to_string();
        consumed = nl + 1;

        let data = match line.strip_prefix("data: ") {
            Some(d) => d.trim(),
            None => continue,
        };
        if data == "[DONE]" {
            buf.drain(..consumed);
            out.push(StreamPiece::done(""));
            return out;
        }
        let event: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(usage) = parse_openai_usage(&event) {
            out.push(StreamPiece::usage(usage));
        }
        let (reasoning, delta) = parse_openai_delta(&event);
        if !reasoning.is_empty() {
            out.push(StreamPiece::thinking(reasoning));
        }
        if !delta.is_empty() {
            out.push(StreamPiece::text(delta));
        }
    }
    // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
    buf.drain(..consumed);
    out
}

/// Build the `/chat/completions` streaming request body for a given
/// [`AiGenerateRequest`] + capability matrix. Pure + unit-tested — this is the
/// shared body shape for native OpenAI, any OpenAI-compatible gateway, and
/// Ollama Cloud (all backed by [`OpenAiClient`]). `top_p`/`frequency_penalty`/
/// `presence_penalty` are the detector-resistance sampling knobs (RAID, ACL
/// 2024) the renderer sets only for prose generation surfaces — each is only
/// ever added when `Some` (never sent as `null`), and skipped entirely on
/// reasoning models that reject `temperature`.
fn build_chat_stream_body(req: &AiGenerateRequest, caps: ModelCapabilities) -> Value {
    let messages = req
        .messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect::<Vec<_>>();

    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    // Real per-call token usage (AI-spend visibility, `crate::spend`): asks for
    // one extra streamed chunk carrying `usage` right before `[DONE]`. Every
    // OpenAI-compatible server this client talks to (native OpenAI, LM
    // Studio/vLLM/OpenRouter/Groq/…, Ollama Cloud) either honors this or
    // silently ignores the unknown field — never a 400.
    body["stream_options"] = json!({ "include_usage": true });
    if caps.supports_temperature {
        body["temperature"] = json!(req.temperature.unwrap_or(0.7));
        if let Some(top_p) = req.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(fp) = req.frequency_penalty {
            body["frequency_penalty"] = json!(fp);
        }
        if let Some(pp) = req.presence_penalty {
            body["presence_penalty"] = json!(pp);
        }
    }
    if let Some(mt) = req.max_tokens {
        let field = match caps.token_param {
            TokenParam::MaxCompletionTokens => "max_completion_tokens",
            _ => "max_tokens",
        };
        body[field] = json!(mt);
    }
    body
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

    /// Whether this client's provider id exposes OpenAI's native `web_search`
    /// tool — only native OpenAI does; every OpenAI-compatible gateway can't be
    /// assumed to support it, and Ollama Cloud overrides `research()`/
    /// `research_salary()` on its own client. Factored to a pure, `AppHandle`-free
    /// predicate purely so the gate stays unit-testable (this crate has no
    /// `tauri::test` mock-app harness to drive `web_search_complete` itself end
    /// to end — see the same note on `salary_research::SalaryResearch::enrich`).
    fn supports_web_search(&self) -> bool {
        self.id == ProviderId::OpenAi
    }

    /// Shared body of `complete`/`complete_with_usage`: one non-streaming
    /// `/chat/completions` call, parsed once into `(text, usage)` so the two
    /// trait methods never duplicate the HTTP round-trip.
    async fn complete_impl(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
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

        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&endpoint)
                .timeout(timeouts::COMPLETION)
                .bearer_auth(&api_key)
                .json(&body)
        })
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
        let text = data
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .map(String::from)
            .ok_or_else(|| {
                AppError::Provider(format!("{}: unexpected response shape", self.id.as_str()))
            })?;
        let usage = parse_openai_usage(&data).unwrap_or_default();
        Ok((text, usage))
    }

    /// Shared body of `embed`/`embed_with_usage`: one `/embeddings` call,
    /// parsed once into `(vector, usage)` so the two trait methods never
    /// duplicate the HTTP round-trip.
    async fn embed_impl(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let endpoint = format!("{}/embeddings", self.base_url);
        let trace = RequestTrace::begin(self.id, model, "/embeddings", &self.base_url, false);
        let body = json!({ "model": model, "input": text });
        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&endpoint)
                .timeout(timeouts::EMBED)
                .bearer_auth(&api_key)
                .json(&body)
        })
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
        let vector: Vec<f64> = data
            .get("data")
            .and_then(|d| d.get(0))
            .and_then(|e| e.get("embedding"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .ok_or_else(|| {
                AppError::Provider(format!(
                    "{}: missing embedding in response",
                    self.id.as_str()
                ))
            })?;
        Ok((vector, parse_openai_embed_usage(&data)))
    }

    /// Shared transport for every `research*` facet: the Responses API with the
    /// native `web_search` tool, `system`/`user` supplied by the caller. Every
    /// non-OpenAI id degrades to `""`, exactly like a missing key or a failed
    /// call.
    async fn web_search_complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<String> {
        if !self.supports_web_search() {
            return Ok(String::new());
        }
        let api_key = match get_provider_key(app, self.id.credential_key()) {
            Some(k) if !k.trim().is_empty() => k,
            _ => return Ok(String::new()),
        };
        self.web_search_transport(&api_key, model, system, user)
            .await
    }

    /// The `/responses` HTTP transport itself — no `AppHandle`/keychain, so it's
    /// directly testable against a `wiremock::MockServer` (see the tests below).
    /// Behavior-preserving extraction from `web_search_complete`: a transport
    /// failure, a non-2xx status, and a non-JSON body all degrade to `""` (never
    /// an error) — the same gentle-degrade contract the caller already promises.
    async fn web_search_transport(
        &self,
        api_key: &str,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<String> {
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
            "instructions": system,
            "input": user,
            "tools": [{ "type": "web_search" }],
        });
        let resp = crate::net::http::shared()
            .post(&endpoint)
            .timeout(timeouts::WEB_SEARCH)
            .bearer_auth(api_key)
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
            // Only native OpenAI exposes the `web_search` tool; any
            // OpenAI-compatible gateway (LM Studio, OpenRouter, …) can't be
            // assumed to — see `supports_web_search()`.
            supports_web_search: self.supports_web_search(),
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

        let body = build_chat_stream_body(req, caps);

        let response = crate::net::http::shared()
            .post(&endpoint)
            .timeout(timeouts::STREAM)
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await;

        let response = match response {
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

        // The shared loop owns cancel-check + chunk read + emit + complete; this
        // closure is the only OpenAI-specific part (its `data:`-prefixed SSE framing).
        stream_response(
            app,
            job_id,
            &trace,
            response,
            status.as_u16(),
            self.id,
            &req.model,
            &self.base_url,
            parse_openai_frames,
        )
        .await
    }

    async fn complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String> {
        self.complete_impl(app, model, system, user, temperature)
            .await
            .map(|(text, _)| text)
    }

    async fn complete_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
        self.complete_impl(app, model, system, user, temperature)
            .await
    }

    async fn research(
        &self,
        app: &AppHandle,
        model: &str,
        company: &str,
        role: &str,
    ) -> AppResult<String> {
        self.web_search_complete(
            app,
            model,
            research::NATIVE_SYSTEM,
            &research::native_user(company, role),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn research_salary(
        &self,
        app: &AppHandle,
        model: &str,
        role: &str,
        company: &str,
        location: &str,
        country: &str,
        currency: &str,
    ) -> AppResult<String> {
        self.web_search_complete(
            app,
            model,
            &research::salary_system(currency),
            &research::salary_user(role, company, location, country, currency),
        )
        .await
    }

    async fn research_answer(
        &self,
        app: &AppHandle,
        model: &str,
        question: &str,
        role: &str,
        company: &str,
    ) -> AppResult<String> {
        self.web_search_complete(
            app,
            model,
            research::ANSWER_SYSTEM,
            &research::answer_user(question, role, company),
        )
        .await
    }

    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>> {
        self.embed_impl(app, model, text).await.map(|(v, _)| v)
    }

    async fn embed_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        self.embed_impl(app, model, text).await
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some("text-embedding-3-small")
    }

    fn max_embedding_input_chars(&self) -> usize {
        // text-embedding-3-* enforce a hard 8191-TOKEN limit and ERROR (no
        // auto-truncate) when exceeded. The old 32k-char cap assumed ~4 chars/token
        // (English); for token-dense scripts (CJK ≈ 1 char/token) 32k chars ≈ 32k
        // tokens — far over 8191 — so the request would FAIL. Cap at 8000 chars: in
        // the worst case (≈1 char/token) that stays under 8191 tokens for every
        // language. Slightly over-truncates very long English, but safety (never a
        // failed request) wins over maximizing English length.
        8_000
    }

    async fn list_models(&self, app: &AppHandle) -> Vec<Value> {
        let api_key = match get_provider_key(app, self.id.credential_key()) {
            Some(k) => k,
            None => return vec![],
        };
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{}/models", self.base_url))
            .bearer_auth(&api_key)
            .timeout(timeouts::LIST_MODELS)
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
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{}/models", self.base_url))
            .bearer_auth(&api_key)
            .timeout(timeouts::LIST_MODELS)
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

    async fn chat_with_tools(
        &self,
        app: &AppHandle,
        model: &str,
        messages: &[ChatMsg],
        tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        let caps = self.capabilities(model);
        if !caps.supports_tools {
            return single_shot_turn(self, app, model, messages, temperature).await;
        }
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let endpoint = format!("{}/chat/completions", self.base_url);
        let trace = RequestTrace::begin(
            self.id,
            model,
            "/chat/completions tools",
            &self.base_url,
            false,
        );

        let wire_messages: Vec<Value> = messages
            .iter()
            .map(|m| json!({ "role": m.role.wire(), "content": m.content }))
            .collect();
        // OpenAI function-tool shape. The schema is trusted, fixed input — never
        // built from scraped/model text.
        let tool_specs: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.schema,
                    },
                })
            })
            .collect();

        let mut body = json!({
            "model": model,
            "messages": wire_messages,
            "stream": false,
            "tools": tool_specs,
            "tool_choice": "auto",
        });
        if caps.supports_temperature {
            body["temperature"] = json!(temperature.unwrap_or(0.7));
        }

        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&endpoint)
                .timeout(timeouts::COMPLETION)
                .bearer_auth(&api_key)
                .json(&body)
        })
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
        Ok(parse_openai_turn(&data))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_chat_stream_body, is_reasoning_model, join_responses_text, parse_openai_delta,
        parse_openai_embed_usage, parse_openai_frames, parse_openai_turn, parse_openai_usage,
        should_list_model, OpenAiClient,
    };
    use crate::commands::ai_provider::{
        AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, StopReason, TokenParam,
        ToolCall,
    };
    use crate::ipc_contracts::ai::AiGenerateRequestMessage;
    use serde_json::json;

    fn base_request() -> AiGenerateRequest {
        AiGenerateRequest {
            model: "gpt-4o".to_string(),
            messages: vec![AiGenerateRequestMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
            locale: "en".to_string(),
            temperature: Some(0.8),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            repeat_penalty: None,
            max_tokens: None,
            context_window: None,
            provider: None,
            base_url: None,
            effort: None,
        }
    }

    fn chat_caps(supports_temperature: bool) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: !supports_temperature,
            supports_tools: true,
            supports_json_mode: true,
            supports_embeddings: true,
            supports_web_search: false,
            token_param: TokenParam::MaxTokens,
        }
    }

    #[test]
    fn chat_stream_body_always_requests_streamed_usage() {
        // AI-spend visibility depends on this flag being sent on every OpenAI
        // Chat Completions stream (native, OpenAI-compatible, and Ollama Cloud).
        let body = build_chat_stream_body(&base_request(), chat_caps(true));
        assert_eq!(body["stream_options"], json!({ "include_usage": true }));
    }

    #[test]
    fn parse_usage_extracts_real_token_counts() {
        let data = json!({ "usage": { "prompt_tokens": 42, "completion_tokens": 17 } });
        let usage = parse_openai_usage(&data).expect("usage present");
        assert_eq!(usage.input_tokens, 42);
        assert_eq!(usage.output_tokens, 17);
    }

    #[test]
    fn parse_usage_is_none_when_absent() {
        // Every streamed chunk except the final one has no `usage` field.
        assert!(parse_openai_usage(&json!({ "choices": [] })).is_none());
        assert!(parse_openai_usage(&json!({})).is_none());
    }

    #[test]
    fn parse_embed_usage_prefers_prompt_tokens() {
        let data = json!({ "usage": { "prompt_tokens": 12, "total_tokens": 12 } });
        let usage = parse_openai_embed_usage(&data);
        assert_eq!(usage.input_tokens, 12);
        assert_eq!(usage.output_tokens, 0, "an embed call has no output tokens");
    }

    #[test]
    fn parse_embed_usage_falls_back_to_total_tokens() {
        // Some OpenAI-compatible embed servers send only `total_tokens`.
        let data = json!({ "usage": { "total_tokens": 9 } });
        assert_eq!(parse_openai_embed_usage(&data).input_tokens, 9);
    }

    #[test]
    fn parse_embed_usage_zero_when_absent() {
        let usage = parse_openai_embed_usage(&json!({}));
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
    }

    #[test]
    fn chat_stream_body_serializes_sampling_params_when_set() {
        let mut req = base_request();
        req.top_p = Some(0.95);
        req.frequency_penalty = Some(0.3);
        req.presence_penalty = Some(0.2);
        let body = build_chat_stream_body(&req, chat_caps(true));
        assert_eq!(body["top_p"], json!(0.95));
        assert_eq!(body["frequency_penalty"], json!(0.3));
        assert_eq!(body["presence_penalty"], json!(0.2));
    }

    #[test]
    fn chat_stream_body_omits_sampling_params_when_none() {
        let body = build_chat_stream_body(&base_request(), chat_caps(true));
        assert!(body.get("top_p").is_none());
        assert!(body.get("frequency_penalty").is_none());
        assert!(body.get("presence_penalty").is_none());
    }

    #[test]
    fn chat_stream_body_skips_sampling_params_on_reasoning_models() {
        // o-series models reject `temperature` entirely — sampling knobs must be
        // skipped alongside it, never sent to a model that 400s on them.
        let mut req = base_request();
        req.top_p = Some(0.95);
        req.frequency_penalty = Some(0.3);
        req.presence_penalty = Some(0.2);
        let body = build_chat_stream_body(&req, chat_caps(false));
        assert!(body.get("temperature").is_none());
        assert!(body.get("top_p").is_none());
        assert!(body.get("frequency_penalty").is_none());
        assert!(body.get("presence_penalty").is_none());
    }

    #[test]
    fn embedding_cap_is_token_safe_for_every_language() {
        // text-embedding-3-* hard-error past 8191 tokens. Token-dense scripts (CJK)
        // run ≈1 char/token, so the char cap must itself stay under 8191 — otherwise
        // a full-cap CJK input exceeds the token limit and the request FAILS.
        let cap = OpenAiClient::new(ProviderId::OpenAi, None).max_embedding_input_chars();
        assert!(
            cap <= 8191,
            "char cap {cap} can exceed 8191 tokens for ~1-char/token languages"
        );
        // Sanity: still a useful amount of text (not collapsed to near-zero).
        assert!(cap >= 4_000, "cap {cap} truncates too aggressively");
    }

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

    #[test]
    fn parse_frames_splits_sse_lines_into_pieces() {
        use super::StreamPiece;
        // Two complete data lines (reasoning then content) + a partial trailing line.
        let mut buf = String::from(
            "data: {\"choices\":[{\"delta\":{\"reasoning\":\"think\"}}]}\n\
             data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\
             data: {\"choices\":[{\"delta\":{\"con",
        );
        let pieces = parse_openai_frames(&mut buf);
        assert_eq!(
            pieces,
            vec![StreamPiece::thinking("think"), StreamPiece::text("hello")]
        );
        // The incomplete final line is left buffered for the next chunk.
        assert!(buf.starts_with("data: {\"choices\""));
        assert!(!buf.contains('\n'));
    }

    #[test]
    fn parse_frames_emits_done_sentinel_on_done_marker() {
        use super::StreamPiece;
        let mut buf = String::from(
            "data: {\"choices\":[{\"delta\":{\"content\":\"last\"}}]}\n\
             data: [DONE]\n",
        );
        let pieces = parse_openai_frames(&mut buf);
        assert_eq!(
            pieces,
            vec![StreamPiece::text("last"), StreamPiece::done("")]
        );
    }

    #[test]
    fn parse_frames_skips_non_data_and_unparseable_lines() {
        // Comment/keepalive lines and malformed JSON are ignored, not errors.
        let mut buf = String::from(": keepalive\ndata: not-json\n\n");
        assert!(parse_openai_frames(&mut buf).is_empty());
    }

    #[test]
    fn parse_turn_decodes_tool_calls_with_stringified_arguments() {
        // Chat Completions puts function args in a JSON *string* — it must be decoded.
        let data = json!({
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "match_resume", "arguments": "{\"resumeId\":\"r1\",\"jobId\":\"j1\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let turn = parse_openai_turn(&data);
        assert_eq!(turn.text, "");
        assert_eq!(turn.stop, StopReason::ToolUse);
        assert_eq!(
            turn.tool_calls,
            vec![ToolCall {
                id: "call_1".to_string(),
                name: "match_resume".to_string(),
                args: json!({ "resumeId": "r1", "jobId": "j1" }),
            }]
        );
    }

    #[test]
    fn parse_turn_plain_answer_has_no_tool_calls() {
        let data = json!({
            "choices": [{ "message": { "content": "Here is the answer." }, "finish_reason": "stop" }]
        });
        let turn = parse_openai_turn(&data);
        assert_eq!(turn.text, "Here is the answer.");
        assert!(turn.tool_calls.is_empty());
        assert_eq!(turn.stop, StopReason::End);
    }

    #[test]
    fn parse_turn_malformed_arguments_degrade_to_empty_object() {
        // A truncated/invalid arguments string must not error the whole turn.
        let data = json!({
            "choices": [{
                "message": { "tool_calls": [{ "id": "c", "function": { "name": "f", "arguments": "{not json" } }] },
                "finish_reason": "tool_calls"
            }]
        });
        let turn = parse_openai_turn(&data);
        assert_eq!(turn.tool_calls.len(), 1);
        assert_eq!(turn.tool_calls[0].args, json!({}));
    }

    // ── web_search_transport (wiremock against `crate::net::http::shared()`,
    // mirroring the pattern in `retry.rs`'s `retry_loop_tests`) ────────────────

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn web_search_transport_degrades_to_empty_on_http_500() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
        let text = client
            .web_search_transport("dummy-key", "gpt-4o", "system", "user")
            .await
            .expect("never an error, only degrades to empty");
        assert_eq!(text, "");
    }

    #[tokio::test]
    async fn web_search_transport_degrades_to_empty_on_non_json_200() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
        let text = client
            .web_search_transport("dummy-key", "gpt-4o", "system", "user")
            .await
            .expect("never an error, only degrades to empty");
        assert_eq!(text, "");
    }

    #[tokio::test]
    async fn web_search_transport_extracts_text_from_a_realistic_responses_payload() {
        let server = MockServer::start().await;
        let payload = json!({
            "output": [
                { "type": "web_search_call", "id": "ws_1", "status": "completed" },
                { "type": "message", "role": "assistant", "content": [
                    { "type": "output_text", "text": "Acme is a ", "annotations": [] },
                    { "type": "output_text", "text": "widget maker.", "annotations": [] }
                ]}
            ]
        });
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(payload))
            .mount(&server)
            .await;

        let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
        let text = client
            .web_search_transport("dummy-key", "gpt-4o", "system", "user")
            .await
            .expect("ok");
        assert_eq!(text, "Acme is a widget maker.");
    }

    #[test]
    fn supports_web_search_gate_only_allows_native_openai() {
        // Regression guard against silently dropping the provider gate in a
        // future refactor: a non-OpenAI id must never reach `/responses` (a
        // generic OpenAI-compatible gateway can't be assumed to support the
        // native `web_search` tool). `web_search_complete` itself can't be
        // driven end to end here — it needs a live `AppHandle`, and this crate
        // has no `tauri::test` mock-app harness (see its doc comment, and the
        // same note on `salary_research::SalaryResearch::enrich`) — so this
        // exercises the pure gate predicate it's built on before any HTTP call.
        assert!(OpenAiClient::new(ProviderId::OpenAi, None).supports_web_search());
        for other in [
            ProviderId::OpenAiCompatible,
            ProviderId::OllamaCloud,
            ProviderId::Ollama,
            ProviderId::Anthropic,
            ProviderId::Gemini,
        ] {
            assert!(
                !OpenAiClient::new(other, None).supports_web_search(),
                "{other:?} must not pass the web_search gate"
            );
        }
    }

    /// `ModelCapabilities::supports_web_search` (what `ai_research_answer` gates
    /// the daily-budget charge on) must mirror the private gate predicate above —
    /// this is the field a caller actually reads.
    #[test]
    fn capabilities_supports_web_search_mirrors_the_gate() {
        assert!(
            OpenAiClient::new(ProviderId::OpenAi, None)
                .capabilities("gpt-4o")
                .supports_web_search
        );
        assert!(
            !OpenAiClient::new(ProviderId::OpenAiCompatible, None)
                .capabilities("some-model")
                .supports_web_search
        );
    }
}

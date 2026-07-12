//! Anthropic provider — Messages API only.

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
    friendly_api_error, single_shot_turn, split_system, AgentTurn, AiGenerateRequest, AiProvider,
    ChatMsg, ModelCapabilities, ProviderId, RequestTrace, StopReason, TokenParam, ToolCall,
    ToolSpec, Usage,
};

const BASE: &str = "https://api.anthropic.com/v1";
const VERSION: &str = "2023-06-01";

/// Whether a model should be sent the `thinking` block (extended thinking).
///
/// Anthropic's extended-thinking mode forces `temperature=1.0` and consumes extra
/// output tokens; a model that does **not** support it answers a `thinking`
/// request with a 400. Only the Claude 3.7+ / 4.x families support it, so gate on
/// the model id (mirrors [`gemini_supports_thinking`](super::gemini)). Older 3.0–
/// 3.5 models and any explicitly-tagged non-thinking name are excluded. Unknown
/// future names default to **off** — a graceful miss (no thinking) is always safe;
/// a wrongful `thinking` request 400s the whole generation.
fn anthropic_supports_thinking(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    // Claude 3.7 (the first extended-thinking model) and the 4.x families.
    m.contains("claude-3-7")
        || m.contains("claude-3.7")
        || m.contains("claude-4")
        || m.contains("claude-opus-4")
        || m.contains("claude-sonnet-4")
        || m.contains("claude-haiku-4")
}

/// Concatenate every `type:"text"` block in an Anthropic Messages `content` array
/// into one string (web-search responses interleave `server_tool_use` /
/// `web_search_tool_result` blocks, which have no `text` field and are skipped).
/// Pure + unit-tested.
fn join_text_blocks(data: &Value) -> String {
    data.get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Parse a non-streaming Anthropic Messages response into an [`AgentTurn`]:
/// concatenate the `type:"text"` blocks for the visible text, map every
/// `type:"tool_use"` block to a [`ToolCall`] (`id`, `name`, `input`→`args`), and
/// map `stop_reason` (`tool_use`→ToolUse, `end_turn`→End, `max_tokens`→Length,
/// else Other). Pure + unit-tested — this is the error-prone per-vendor shape, so
/// it lives here with no I/O.
fn parse_anthropic_turn(data: &Value) -> AgentTurn {
    let text = join_text_blocks(data);
    let tool_calls = data
        .get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                .filter_map(|b| {
                    let name = b.get("name").and_then(|n| n.as_str())?.to_string();
                    Some(ToolCall {
                        id: b
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        name,
                        args: b.get("input").cloned().unwrap_or_else(|| json!({})),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = match data.get("stop_reason").and_then(|s| s.as_str()) {
        Some("tool_use") => StopReason::ToolUse,
        Some("end_turn") => StopReason::End,
        Some("max_tokens") => StopReason::Length,
        _ => StopReason::Other,
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
        usage: parse_anthropic_usage(data),
    }
}

/// Drain complete SSE lines from the accumulated stream buffer into
/// [`StreamPiece`]s. Anthropic emits paired `event:`/`data:` lines; we track the
/// most recent `event:` in `last_event` (carried across chunk boundaries by the
/// caller). `message_stop` (by event name or embedded `type`) yields a terminal
/// sentinel; `thinking_delta` / `text_delta` map to reasoning / answer pieces.
///
/// Real token usage (`crate::spend`) arrives split across two events:
/// `message_start` carries `message.usage.input_tokens` (once, at the top of
/// the stream) and each `message_delta` carries a running `usage.output_tokens`
/// total (the LAST one is authoritative). `usage` is caller-carried mutable
/// state (like `last_event`) so the two halves combine into one [`Usage`]; a
/// [`StreamPiece::usage`] piece is emitted whenever either half updates.
///
/// Pure + unit-tested; this is the OpenAI-style `parse` closure for Anthropic, so
/// its SSE framing lives here only.
fn parse_anthropic_frames(
    buf: &mut String,
    last_event: &mut String,
    usage: &mut Usage,
) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    // Walk the buffer by a `consumed` offset and `drain(..consumed)` once at the end,
    // instead of reallocating the whole tail per line (O(n²) on a big frame).
    let mut consumed = 0;
    while let Some(rel) = buf[consumed..].find('\n') {
        let nl = consumed + rel;
        let line = buf[consumed..nl].trim().to_string();
        consumed = nl + 1;

        if let Some(event) = line.strip_prefix("event: ") {
            *last_event = event.trim().to_string();
            continue;
        }
        let data = match line.strip_prefix("data: ") {
            Some(d) => d.trim(),
            None => continue,
        };
        if last_event == "message_stop" || data.contains("\"type\":\"message_stop\"") {
            buf.drain(..consumed);
            out.push(StreamPiece::done(""));
            return out;
        }
        let event: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match last_event.as_str() {
            "message_start" => {
                if let Some(input) = event
                    .get("message")
                    .and_then(|m| m.get("usage"))
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|v| v.as_u64())
                {
                    usage.input_tokens = input as u32;
                    out.push(StreamPiece::usage(*usage));
                }
            }
            "message_delta" => {
                if let Some(output) = event
                    .get("usage")
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|v| v.as_u64())
                {
                    usage.output_tokens = output as u32;
                    out.push(StreamPiece::usage(*usage));
                }
            }
            _ => {}
        }
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
                    out.push(StreamPiece::thinking(thinking));
                }
            }
            "text_delta" => {
                let text = delta_obj
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if !text.is_empty() {
                    out.push(StreamPiece::text(text));
                }
            }
            _ => {}
        }
    }
    // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
    buf.drain(..consumed);
    out
}

/// Build the `/messages` streaming request body for a given [`AiGenerateRequest`].
/// Pure + unit-tested. `top_p` is Anthropic's only sampling knob beyond
/// temperature (no frequency/presence penalty in this API) — set only when the
/// caller supplied it (prose surfaces). CRITICAL: extended thinking forces
/// `temperature=1.0` and the Anthropic API rejects `top_p` alongside `thinking`
/// (400), so `top_p` is skipped whenever thinking is enabled.
fn build_chat_stream_body(req: &AiGenerateRequest) -> Value {
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

    // Extended thinking for balanced effort and above — but ONLY on models that
    // support it. Enabling it forces temperature=1 and spends extra output
    // tokens; an unsupported model 400s on a `thinking` block, so we gate on the
    // model id. A caller can opt out by selecting a non-thinking model.
    let thinking_budget = if max_tokens >= 2048 && anthropic_supports_thinking(&req.model) {
        max_tokens / 2
    } else {
        0
    };
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
    } else if let Some(top_p) = req.top_p {
        // Anthropic 400s if `top_p` rides alongside `thinking` — only add it on
        // the non-thinking path.
        body["top_p"] = json!(top_p);
    }
    if !system_content.is_empty() {
        body["system"] = json!(system_content);
    }
    body
}

/// Extract `usage.{input_tokens,output_tokens}` from a non-streaming Anthropic
/// Messages response — always present on a successful response. Pure +
/// unit-tested.
fn parse_anthropic_usage(data: &Value) -> Usage {
    let usage = data.get("usage");
    Usage {
        input_tokens: usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    }
}

pub struct AnthropicClient;

impl AnthropicClient {
    /// Shared body of `complete`/`complete_with_usage`: one non-streaming
    /// `/messages` call, parsed once into `(text, usage)` so the two trait
    /// methods never duplicate the HTTP round-trip.
    async fn complete_impl(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
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

        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&endpoint)
                .timeout(timeouts::COMPLETION)
                .header("x-api-key", &api_key)
                .header("anthropic-version", VERSION)
                .json(&body)
        })
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
            return Err(friendly_api_error(
                ProviderId::Anthropic,
                status,
                &body_text,
            ));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        let text = join_text_blocks(&data);
        if text.is_empty() {
            return Err(AppError::Provider(
                "Anthropic: unexpected response shape".to_string(),
            ));
        }
        Ok((text, parse_anthropic_usage(&data)))
    }

    /// Shared transport for every `research*` facet: a non-streaming Messages
    /// call with the server-side web-search tool, `system`/`user` supplied by the
    /// caller. Capped at 3 searches (a brief, not deep research); the enricher
    /// also bounds the whole call with a timeout. Requires the org to enable web
    /// search, and degrades to `""` (never an error) on any failure so
    /// generation always proceeds.
    async fn web_search_complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<String> {
        let api_key = match get_provider_key(app, self.id().credential_key()) {
            Some(k) if !k.trim().is_empty() => k,
            _ => return Ok(String::new()),
        };
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(
            ProviderId::Anthropic,
            model,
            "/messages web_search",
            BASE,
            false,
        );

        let body = json!({
            "model": model,
            "max_tokens": 1024,
            "system": system,
            "messages": [{ "role": "user", "content": user }],
            "temperature": 0.2,
            "tools": [{ "type": "web_search_20250305", "name": "web_search", "max_uses": 3 }],
        });

        let resp = crate::net::http::shared()
            .post(&endpoint)
            .timeout(timeouts::WEB_SEARCH)
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                tracing::warn!("anthropic research unreachable: {e}");
                return Ok(String::new());
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            tracing::warn!("anthropic research {status}: {body_text}");
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
        Ok(join_text_blocks(&data))
    }
}

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
            // Native server-side web_search tool (account-key gated at call time).
            supports_web_search: true,
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

        let body = build_chat_stream_body(req);

        let response = crate::net::http::shared()
            .post(&endpoint)
            .timeout(timeouts::STREAM)
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
            .json(&body)
            .send()
            .await;

        let response = match response {
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
            return Err(friendly_api_error(
                ProviderId::Anthropic,
                status,
                &body_text,
            ));
        }

        // The shared loop owns cancel-check + chunk read + emit + complete; the
        // closure is the only Anthropic-specific part (paired `event:`/`data:` SSE
        // framing, with `last_event`/`usage` carried across chunks).
        let mut last_event = String::new();
        let mut usage = Usage::default();
        stream_response(
            app,
            job_id,
            &trace,
            response,
            status.as_u16(),
            ProviderId::Anthropic,
            &req.model,
            BASE,
            move |buf| parse_anthropic_frames(buf, &mut last_event, &mut usage),
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

    async fn embed(&self, _app: &AppHandle, _model: &str, _text: &str) -> AppResult<Vec<f64>> {
        Err(AppError::Provider(
            "Anthropic has no embeddings API. Use OpenAI, Gemini, or Ollama for embeddings."
                .to_string(),
        ))
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        None
    }

    async fn list_models(&self, app: &AppHandle) -> Vec<Value> {
        let api_key = match get_provider_key(app, self.id().credential_key()) {
            Some(k) => k,
            None => return vec![],
        };
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{BASE}/models"))
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
            .timeout(timeouts::LIST_MODELS)
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

    async fn test_key(&self, app: &AppHandle) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id().credential_key())
            .ok_or_else(|| AppError::Config("No API key found".to_string()))?;
        // Liveness probe via `GET /v1/models` (the same endpoint `list_models`
        // uses). A key-only authenticated GET avoids pinning a specific chat model
        // snapshot — the old probe hardcoded `claude-3-haiku-20240307`, so key
        // validation would have broken the day that snapshot is retired.
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{BASE}/models"))
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
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
        // Unknown / non-tool models degrade to a single-shot answer rather than
        // 400-ing on a `tools` field they don't understand.
        if !self.capabilities(model).supports_tools {
            return single_shot_turn(self, app, model, messages, temperature).await;
        }
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint = format!("{BASE}/messages");
        let trace =
            RequestTrace::begin(ProviderId::Anthropic, model, "/messages tools", BASE, false);

        let (system, rest) = split_system(messages);
        let wire_messages: Vec<Value> = rest
            .iter()
            .map(|m| json!({ "role": m.role.wire(), "content": m.content }))
            .collect();
        // Map each ToolSpec to Anthropic's tool shape (`input_schema`). The caller's
        // schema is a trusted, fixed JSON-Schema object — never built from scraped
        // or model-supplied text.
        let tool_specs: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({ "name": t.name, "description": t.description, "input_schema": t.schema })
            })
            .collect();

        let mut body = json!({
            "model": model,
            "max_tokens": 4096,
            "messages": wire_messages,
            "temperature": temperature.unwrap_or(0.7),
            "tools": tool_specs,
        });
        if !system.is_empty() {
            body["system"] = json!(system);
        }

        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&endpoint)
                .timeout(timeouts::COMPLETION)
                .header("x-api-key", &api_key)
                .header("anthropic-version", VERSION)
                .json(&body)
        })
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
            return Err(friendly_api_error(
                ProviderId::Anthropic,
                status,
                &body_text,
            ));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        Ok(parse_anthropic_turn(&data))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        anthropic_supports_thinking, build_chat_stream_body, join_text_blocks,
        parse_anthropic_frames, parse_anthropic_turn, parse_anthropic_usage, StreamPiece,
    };
    use crate::commands::ai_provider::{AiGenerateRequest, StopReason, ToolCall, Usage};
    use crate::ipc_contracts::ai::AiGenerateRequestMessage;
    use serde_json::json;

    fn base_request(model: &str) -> AiGenerateRequest {
        AiGenerateRequest {
            model: model.to_string(),
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

    #[test]
    fn chat_stream_body_serializes_top_p_when_set_on_a_non_thinking_model() {
        let mut req = base_request("claude-3-5-sonnet-20241022");
        req.top_p = Some(0.95);
        let body = build_chat_stream_body(&req);
        assert_eq!(body["top_p"], json!(0.95));
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn chat_stream_body_omits_top_p_when_none() {
        let body = build_chat_stream_body(&base_request("claude-3-5-sonnet-20241022"));
        assert!(body.get("top_p").is_none());
    }

    #[test]
    fn chat_stream_body_skips_top_p_when_extended_thinking_is_enabled() {
        // Extended thinking forces temperature=1 and the API rejects `top_p`
        // alongside `thinking` — must never be sent together, even if the caller
        // (an application-answer/cover-letter prose call) supplied top_p.
        let mut req = base_request("claude-opus-4-20250514");
        req.top_p = Some(0.95);
        req.max_tokens = Some(4096); // >= 2048 → thinking budget kicks in
        let body = build_chat_stream_body(&req);
        assert!(body.get("thinking").is_some(), "thinking should be enabled");
        assert_eq!(body["temperature"], json!(1.0));
        assert!(
            body.get("top_p").is_none(),
            "top_p must be omitted when thinking is enabled"
        );
    }

    #[test]
    fn thinking_gate_enables_only_extended_thinking_models() {
        for m in [
            "claude-3-7-sonnet-20250219",
            "claude-3.7-sonnet",
            "claude-opus-4-20250514",
            "claude-sonnet-4-5",
            "claude-haiku-4",
        ] {
            assert!(anthropic_supports_thinking(m), "{m} should enable thinking");
        }
        // Pre-3.7 models 400 on a `thinking` block — must stay off.
        for m in [
            "claude-3-haiku-20240307",
            "claude-3-5-sonnet-20241022",
            "claude-3-opus-20240229",
            "claude-2.1",
        ] {
            assert!(
                !anthropic_supports_thinking(m),
                "{m} must not request thinking (it 400s)"
            );
        }
    }

    #[test]
    fn parse_frames_splits_thinking_and_text_deltas() {
        let mut last = String::new();
        let mut usage = Usage::default();
        let mut buf = String::from(
            "event: content_block_delta\n\
             data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hmm\"}}\n\
             event: content_block_delta\n\
             data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n",
        );
        let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
        assert_eq!(
            pieces,
            vec![StreamPiece::thinking("hmm"), StreamPiece::text("hi")]
        );
    }

    #[test]
    fn parse_frames_done_on_message_stop_event() {
        let mut last = String::new();
        let mut usage = Usage::default();
        let mut buf = String::from("event: message_stop\ndata: {\"type\":\"message_stop\"}\n");
        assert_eq!(
            parse_anthropic_frames(&mut buf, &mut last, &mut usage),
            vec![StreamPiece::done("")]
        );
    }

    #[test]
    fn parse_frames_done_when_event_line_split_across_chunks() {
        // The `event:` line arrives in one chunk, the `data:` in the next — the
        // caller carries `last_event`, so message_stop is still detected.
        let mut last = String::new();
        let mut usage = Usage::default();
        let mut buf = String::from("event: message_stop\n");
        assert!(parse_anthropic_frames(&mut buf, &mut last, &mut usage).is_empty());
        assert_eq!(last, "message_stop");
        buf.push_str("data: {}\n");
        assert_eq!(
            parse_anthropic_frames(&mut buf, &mut last, &mut usage),
            vec![StreamPiece::done("")]
        );
    }

    #[test]
    fn parse_frames_leaves_partial_trailing_line_buffered() {
        let mut last = String::new();
        let mut usage = Usage::default();
        let mut buf = String::from("data: {\"type\":\"content_block_de");
        assert!(parse_anthropic_frames(&mut buf, &mut last, &mut usage).is_empty());
        assert_eq!(buf, "data: {\"type\":\"content_block_de");
    }

    #[test]
    fn parse_frames_drains_consumed_lines_keeping_partial_tail() {
        // The in-place `drain(..consumed)` must drop exactly the fully-parsed lines
        // (incl. a multi-byte char before the newline) and keep the partial tail —
        // the offset arithmetic stays on char boundaries.
        let mut last = String::new();
        let mut usage = Usage::default();
        let mut buf = String::from(
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"café\"}}\n\
             data: {\"type\":\"content_block_de",
        );
        let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
        assert_eq!(pieces, vec![StreamPiece::text("café")]);
        // Only the unterminated trailing line survives the drain.
        assert_eq!(buf, "data: {\"type\":\"content_block_de");
    }

    #[test]
    fn parse_frames_combines_message_start_input_and_message_delta_output_tokens() {
        let mut last = String::new();
        let mut usage = Usage::default();
        let mut buf = String::from(
            "event: message_start\n\
             data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25,\"output_tokens\":1}}}\n",
        );
        let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
        assert_eq!(
            pieces,
            vec![StreamPiece::usage(Usage {
                input_tokens: 25,
                output_tokens: 0
            })]
        );

        buf.push_str(
            "event: message_delta\n\
             data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":91}}\n",
        );
        let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
        assert_eq!(
            pieces,
            vec![StreamPiece::usage(Usage {
                input_tokens: 25,
                output_tokens: 91
            })]
        );
    }

    #[test]
    fn parse_usage_reads_a_non_streaming_response() {
        let data = json!({ "usage": { "input_tokens": 12, "output_tokens": 34 } });
        let usage = parse_anthropic_usage(&data);
        assert_eq!(usage.input_tokens, 12);
        assert_eq!(usage.output_tokens, 34);
    }

    #[test]
    fn parse_usage_defaults_to_zero_when_absent() {
        let usage = parse_anthropic_usage(&json!({}));
        assert_eq!(usage, Usage::default());
    }

    #[test]
    fn join_text_blocks_concatenates_only_text_blocks() {
        // Web-search responses interleave tool blocks among the text blocks.
        let data = json!({
            "content": [
                { "type": "text", "text": "Acme is a " },
                { "type": "server_tool_use", "name": "web_search", "input": { "query": "Acme" } },
                { "type": "web_search_tool_result", "content": [{ "url": "x", "title": "y" }] },
                { "type": "text", "text": "widget maker." }
            ]
        });
        assert_eq!(join_text_blocks(&data), "Acme is a widget maker.");
    }

    #[test]
    fn join_text_blocks_empty_on_missing_or_error() {
        assert_eq!(join_text_blocks(&json!({})), "");
        assert_eq!(join_text_blocks(&json!({ "content": [] })), "");
    }

    #[test]
    fn parse_turn_extracts_text_and_tool_use_blocks() {
        // Assistant text interleaved with a `tool_use` block; stop_reason=tool_use.
        let data = json!({
            "content": [
                { "type": "text", "text": "Let me look that up." },
                { "type": "tool_use", "id": "toolu_1", "name": "research_company",
                  "input": { "company": "Acme", "jobAd": "..." } }
            ],
            "stop_reason": "tool_use"
        });
        let turn = parse_anthropic_turn(&data);
        assert_eq!(turn.text, "Let me look that up.");
        assert_eq!(turn.stop, StopReason::ToolUse);
        assert_eq!(
            turn.tool_calls,
            vec![ToolCall {
                id: "toolu_1".to_string(),
                name: "research_company".to_string(),
                args: json!({ "company": "Acme", "jobAd": "..." }),
            }]
        );
    }

    #[test]
    fn parse_turn_no_tool_calls_is_a_plain_end_turn() {
        let data = json!({
            "content": [{ "type": "text", "text": "All done." }],
            "stop_reason": "end_turn"
        });
        let turn = parse_anthropic_turn(&data);
        assert_eq!(turn.text, "All done.");
        assert!(turn.tool_calls.is_empty());
        assert_eq!(turn.stop, StopReason::End);
    }

    #[test]
    fn parse_turn_maps_max_tokens_and_missing_input() {
        // `max_tokens` → Length; a tool_use with no `input` still parses (args = {}).
        let data = json!({
            "content": [{ "type": "tool_use", "id": "t", "name": "match_resume" }],
            "stop_reason": "max_tokens"
        });
        let turn = parse_anthropic_turn(&data);
        assert_eq!(turn.stop, StopReason::Length);
        assert_eq!(turn.tool_calls.len(), 1);
        assert_eq!(turn.tool_calls[0].args, json!({}));
    }
}

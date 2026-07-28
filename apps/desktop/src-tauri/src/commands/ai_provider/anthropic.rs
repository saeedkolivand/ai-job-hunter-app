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

/// Whether a model should be sent the classic `thinking: {type:"enabled",
/// budget_tokens}` block (extended thinking).
///
/// Anthropic's extended-thinking mode forces `temperature=1.0` and consumes extra
/// output tokens; a model that does **not** support it answers a `thinking`
/// request with a 400. Only the Claude 3.7+ / 4.x families (incl. Haiku 4.5) use
/// this classic budget-token mechanism, so gate on the model id (mirrors
/// [`gemini_supports_thinking`](super::gemini)). Older 3.0–3.5 models are
/// excluded (never supported it).
///
/// `claude-opus-4-7` and `claude-opus-4-8` are **deliberately excluded** even
/// though they match the `claude-opus-4` substring below: per Anthropic's
/// thinking docs they are adaptive-only models ("Extended thinking: No" in the
/// per-model table) — see [`anthropic_uses_adaptive_thinking`] instead.
///
/// The Claude 5 family (`claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`,
/// Mythos, and later adaptive families) is excluded too: it replaced classic
/// budget-token thinking with adaptive thinking, which this predicate does not
/// gate — see [`anthropic_uses_adaptive_thinking`]. Unknown future names
/// default to **off** — a graceful miss (no thinking) is always safe; a
/// wrongful `thinking` request 400s the whole generation.
fn anthropic_supports_thinking(model: &str) -> bool {
    let m = normalize_model_id(model);
    // Opus 4.7/4.8 are adaptive-only (see doc comment) — carve them out before
    // the "claude-opus-4" substring below would otherwise catch them.
    if m.contains("opus-4-7") || m.contains("opus-4-8") {
        return false;
    }
    // Claude 3.7 (the first extended-thinking model) and the 4.x families.
    // Deliberately does NOT match "claude-opus-5"/"claude-sonnet-5"/
    // "claude-fable-5"/mythos — the 5 family (and later adaptive families)
    // use adaptive thinking, not this mechanism. Do not widen this to a bare
    // "claude-" match.
    m.contains("claude-3-7")
        || m.contains("claude-4")
        || m.contains("claude-opus-4")
        || m.contains("claude-sonnet-4")
        || m.contains("claude-haiku-4")
}

/// Whether a model should be sent Anthropic's **adaptive** thinking block
/// (`thinking: {"type":"adaptive","display":"summarized"}`): `claude-opus-4-7`,
/// `claude-opus-4-8` (adaptive-only per the extended-thinking per-model table),
/// and the Claude 5 family (`claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`,
/// Mythos) and later adaptive families.
///
/// `display` defaults to `"omitted"` on every one of these models (per
/// Anthropic's thinking docs) — an empty `thinking` field, signature only. We
/// opt into `"summarized"` explicitly so the app's thinking view actually
/// receives text; without this, "sending nothing extra" silently regresses the
/// thinking view to blank on every adaptive model, even though the model is
/// still thinking (and billing for it) under the hood. This is also the sole
/// source of truth for [`AnthropicClient::capabilities`]'s `supports_temperature`:
/// every model here 400s on ANY non-default temperature/top_p/top_k, on every
/// request, not just while thinking (Anthropic's "Sampling parameters" note).
///
/// New adaptive families need a new substring added here — this predicate IS
/// the adapter's model-classification layer (the zero-change rule protects
/// business logic/callers, not this file). Unknown names default to **off** —
/// a graceful miss (no adaptive block, `display` stays omitted) is always
/// safe; guessing wrong never 400s here (unlike the classic gate above),
/// since adaptive thinking is already on by default on every model in this set.
fn anthropic_uses_adaptive_thinking(model: &str) -> bool {
    let m = normalize_model_id(model);
    m.contains("opus-4-7")
        || m.contains("opus-4-8")
        || m.contains("opus-5")
        || m.contains("sonnet-5")
        || m.contains("fable-5")
        || m.contains("mythos")
}

/// Shared normalization for the two thinking-mode predicates above: lowercase,
/// then collapse dot-form version separators to dashes, so a model id spelled
/// `claude-opus-4.7` (dot form) still matches the `opus-4-7` needle instead of
/// falling through to the classic `claude-opus-4` gate and 400ing (adaptive
/// models reject the classic `thinking.enabled` shape).
fn normalize_model_id(model: &str) -> String {
    model.to_ascii_lowercase().replace('.', "-")
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

/// Build the `/messages` streaming request body for a given [`AiGenerateRequest`]
/// and capability matrix (mirrors `openai.rs`'s `build_chat_stream_body`'s
/// `caps.supports_temperature` gate). Pure + unit-tested. `top_p` is
/// Anthropic's only sampling knob beyond temperature (no frequency/presence
/// penalty in this API) — set only when the caller supplied it (prose
/// surfaces), and only when `caps.supports_temperature` (false for every
/// adaptive-thinking model — see [`AnthropicClient::capabilities`]): those
/// models reject *any* non-default `temperature`/`top_p`/`top_k` on every
/// request per Anthropic's docs ("Sampling parameters"), and we don't know
/// each model's own default value, so omitting the field entirely is the only
/// universally-safe choice. Classic extended thinking also omits `temperature`
/// (Anthropic forces it to 1.0 internally; omitting IS that default, and is
/// one fewer place to get the number wrong) and never gets `top_p` (400s
/// alongside `thinking`).
fn build_chat_stream_body(req: &AiGenerateRequest, caps: ModelCapabilities) -> Value {
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

    // Thinking-token budget headroom: on BOTH classic and adaptive models,
    // thinking tokens are billed as output tokens and count toward
    // `max_tokens` alongside the visible response, so `max_tokens` must be
    // inflated to leave room for both. Gate on `max_tokens >= 2048` (a "big
    // enough task to warrant it" heuristic, not a support check) and on the
    // model id (an unsupported/non-thinking model just gets no inflation and
    // no `thinking` key — always safe; a wrongful `thinking` block 400s the
    // whole generation on classic-only models).
    let is_classic = anthropic_supports_thinking(&req.model);
    let is_adaptive = anthropic_uses_adaptive_thinking(&req.model);
    let thinking_budget = if max_tokens >= 2048 && (is_classic || is_adaptive) {
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
    });
    // Adaptive checked FIRST: a future model id that (incorrectly) matches
    // both predicates must fail toward the safe adaptive shape, not toward
    // the classic shape that 400s on an adaptive-only model.
    if is_adaptive {
        // Opt into "summarized" display — it defaults to "omitted" (empty
        // thinking blocks) on every adaptive model, which would silently
        // blank the app's thinking view. `temperature`/`top_p` stay omitted
        // (see fn doc comment above; `caps.supports_temperature` is false here).
        body["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
    } else if is_classic && thinking_budget > 0 {
        body["thinking"] = json!({ "type": "enabled", "budget_tokens": thinking_budget });
    } else if caps.supports_temperature {
        body["temperature"] = json!(req.temperature.unwrap_or(0.7));
        if let Some(top_p) = req.top_p {
            body["top_p"] = json!(top_p);
        }
    }
    if !system_content.is_empty() {
        body["system"] = json!(system_content);
    }
    body
}

/// Thinking-aware `max_tokens` for the three NON-streaming builders below,
/// which (unlike [`build_chat_stream_body`]) hardcode a fixed cap rather than
/// taking one from the caller. Adaptive thinking is on by default and can't be
/// turned off on several models (Fable can't disable it at all) — it still
/// counts toward `max_tokens` even though these builders never send a
/// `thinking` key (no thinking-view display concern on these paths; default
/// `"omitted"` is correct and gives faster time-to-first-text per Anthropic's
/// docs). Unlike the streaming builder's inflation, this is **not** gated on
/// a size threshold: these caps are fixed/small (e.g. 1024 for web search),
/// and thinking still fires by default regardless of how small the cap is.
fn adaptive_max_tokens(model: &str, base: u32) -> u32 {
    if anthropic_uses_adaptive_thinking(model) {
        base + base / 2
    } else {
        base
    }
}

/// Build the non-streaming `/messages` body shared by `complete`/
/// `complete_with_usage`. Pure + unit-tested — mirrors
/// [`build_chat_stream_body`]'s `caps.supports_temperature` gate but never
/// sends a `thinking` key at all (no thinking-view display concern on this
/// single-shot completion path).
fn build_complete_body(
    model: &str,
    system: &str,
    user: &str,
    temperature: Option<f64>,
    caps: ModelCapabilities,
) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": adaptive_max_tokens(model, 4096),
        "messages": [ { "role": "user", "content": user } ],
    });
    if caps.supports_temperature {
        body["temperature"] = json!(temperature.unwrap_or(0.7));
    }
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    body
}

/// Build the non-streaming `/messages` body shared by every `research*`
/// facet (native `web_search` tool). Pure + unit-tested — same
/// `caps.supports_temperature` gate as [`build_complete_body`]; the hardcoded
/// `0.2` (favor precision over creativity for a research brief) is simply
/// skipped instead of overridden on adaptive models.
fn build_web_search_body(model: &str, system: &str, user: &str, caps: ModelCapabilities) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": adaptive_max_tokens(model, 1024),
        "system": system,
        "messages": [{ "role": "user", "content": user }],
        "tools": [{ "type": "web_search_20250305", "name": "web_search", "max_uses": 3 }],
    });
    if caps.supports_temperature {
        body["temperature"] = json!(0.2);
    }
    body
}

/// Build the non-streaming `/messages` body for [`AnthropicClient::chat_with_tools`].
/// Pure + unit-tested — same `caps.supports_temperature` gate as
/// [`build_complete_body`].
fn build_tools_body(
    model: &str,
    system: &str,
    wire_messages: Vec<Value>,
    tool_specs: Vec<Value>,
    temperature: Option<f64>,
    caps: ModelCapabilities,
) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": adaptive_max_tokens(model, 4096),
        "messages": wire_messages,
        "tools": tool_specs,
    });
    if caps.supports_temperature {
        body["temperature"] = json!(temperature.unwrap_or(0.7));
    }
    if !system.is_empty() {
        body["system"] = json!(system);
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
        let caps = self.capabilities(model);
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(ProviderId::Anthropic, model, "/messages", BASE, false);

        let body = build_complete_body(model, system, user, temperature, caps);

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
        let caps = self.capabilities(model);
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(
            ProviderId::Anthropic,
            model,
            "/messages web_search",
            BASE,
            false,
        );

        let body = build_web_search_body(model, system, user, caps);

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

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        // Every adaptive-thinking model 400s on ANY non-default temperature/
        // top_p/top_k on EVERY request, thinking or not (Anthropic's "Sampling
        // parameters" note) — so `supports_temperature` must be model-aware,
        // not a blanket `true`. All four request builders in this file gate
        // on this field instead of re-deriving the check themselves.
        let adaptive = anthropic_uses_adaptive_thinking(model);
        if !adaptive && !anthropic_supports_thinking(model) {
            // Always safe (no `thinking` key sent, never a 400) — but if this
            // is actually a new Anthropic family this adapter hasn't learned
            // about yet, its thinking view silently stays blank with no
            // signal. Debug-only: never user-facing, never blocks the call.
            tracing::debug!(
                model,
                "anthropic: model matches neither the classic nor adaptive thinking gate — \
                 thinking view (if any) stays blank until this adapter learns the family"
            );
        }
        ModelCapabilities {
            supports_temperature: !adaptive,
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
        let caps = self.capabilities(&req.model);
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(ProviderId::Anthropic, &req.model, "/messages", BASE, true);

        let body = build_chat_stream_body(req, caps);

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
        let caps = self.capabilities(model);
        // Unknown / non-tool models degrade to a single-shot answer rather than
        // 400-ing on a `tools` field they don't understand.
        if !caps.supports_tools {
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

        let body = build_tools_body(model, &system, wire_messages, tool_specs, temperature, caps);

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
        anthropic_supports_thinking, anthropic_uses_adaptive_thinking, build_chat_stream_body,
        build_complete_body, build_tools_body, build_web_search_body, join_text_blocks,
        parse_anthropic_frames, parse_anthropic_turn, parse_anthropic_usage, AnthropicClient,
        StreamPiece,
    };
    use crate::commands::ai_provider::{
        AiGenerateRequest, AiProvider, ModelCapabilities, StopReason, ToolCall, Usage,
    };
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
            effort: None,
        }
    }

    /// The real capability matrix for `model`, exactly as every trait method
    /// computes it — tests must exercise the same `caps.supports_temperature`
    /// gate the adapter actually uses, not a hand-rolled stand-in.
    fn caps_for(model: &str) -> ModelCapabilities {
        AnthropicClient.capabilities(model)
    }

    #[test]
    fn chat_stream_body_serializes_top_p_when_set_on_a_non_thinking_model() {
        let mut req = base_request("claude-3-5-sonnet-20241022");
        req.top_p = Some(0.95);
        let body = build_chat_stream_body(&req, caps_for(&req.model));
        assert_eq!(body["top_p"], json!(0.95));
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn chat_stream_body_omits_top_p_when_none() {
        let req = base_request("claude-3-5-sonnet-20241022");
        let body = build_chat_stream_body(&req, caps_for(&req.model));
        assert!(body.get("top_p").is_none());
    }

    #[test]
    fn chat_stream_body_skips_top_p_when_extended_thinking_is_enabled() {
        // The API rejects `top_p` alongside `thinking` — must never be sent
        // together, even if the caller (an application-answer/cover-letter
        // prose call) supplied top_p. `temperature` is omitted entirely on the
        // classic-thinking path too (Anthropic forces it to 1.0 internally;
        // omitting IS that default — see `build_chat_stream_body`'s doc comment).
        let mut req = base_request("claude-opus-4-20250514");
        req.top_p = Some(0.95);
        req.max_tokens = Some(4096); // >= 2048 → thinking budget kicks in
        let body = build_chat_stream_body(&req, caps_for(&req.model));
        assert!(body.get("thinking").is_some(), "thinking should be enabled");
        assert!(
            body.get("temperature").is_none(),
            "temperature must be omitted, not forced to 1.0, when thinking is enabled"
        );
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
    fn thinking_gates_normalize_dot_form_version_ids() {
        // A dot-form id ("claude-opus-4.7") must normalize the same as its
        // dash-form equivalent — otherwise it misses the "opus-4-7" needle,
        // falls through to the classic "claude-opus-4" gate, and 400s (Opus
        // 4.7 is adaptive-only; it rejects the classic `thinking.enabled` shape).
        assert!(anthropic_uses_adaptive_thinking("claude-opus-4.7"));
        assert!(anthropic_uses_adaptive_thinking("claude-opus-4.8"));
        assert!(!anthropic_supports_thinking("claude-opus-4.7"));
        assert!(!anthropic_supports_thinking("claude-opus-4.8"));
    }

    #[test]
    fn thinking_gate_excludes_the_claude_5_family() {
        // The 5 family (Opus 5, Sonnet 5, Fable 5) replaced classic
        // budget-token thinking with adaptive thinking — sending the classic
        // `thinking` block to them would 400, so the gate must stay off.
        for m in [
            "claude-opus-5",
            "claude-sonnet-5",
            "claude-fable-5",
            "claude-fable-5-20260201",
        ] {
            assert!(
                !anthropic_supports_thinking(m),
                "{m} must not request classic thinking (it 400s; uses adaptive thinking instead)"
            );
        }
    }

    #[test]
    fn thinking_gate_excludes_opus_4_7_and_4_8_despite_matching_claude_opus_4() {
        // Opus 4.7/4.8 are adaptive-ONLY per Anthropic's per-model table
        // ("Extended thinking: No") — they must not fall into the classic
        // gate just because they match the "claude-opus-4" substring.
        for m in ["claude-opus-4-7", "claude-opus-4-8"] {
            assert!(
                !anthropic_supports_thinking(m),
                "{m} is adaptive-only; must not receive classic thinking"
            );
        }
    }

    #[test]
    fn adaptive_gate_matches_opus_4_7_4_8_and_the_5_family() {
        for m in [
            "claude-opus-4-7",
            "claude-opus-4-8",
            "claude-opus-5",
            "claude-opus-5-20260201",
            "claude-sonnet-5",
            "claude-fable-5",
            "claude-fable-5-20260201",
            "claude-mythos-5",
        ] {
            assert!(
                anthropic_uses_adaptive_thinking(m),
                "{m} should use adaptive thinking"
            );
        }
        // Pre-5/pre-4.7 models must never be misclassified as adaptive — in
        // particular "claude-sonnet-4-5" (Sonnet 4.5) must not match on a bare
        // "-5" substring.
        for m in [
            "claude-opus-4-20250514",
            "claude-sonnet-4-5",
            "claude-3-5-sonnet-20241022",
            "claude-3-haiku-20240307",
        ] {
            assert!(
                !anthropic_uses_adaptive_thinking(m),
                "{m} must not be classified as adaptive"
            );
        }
    }

    #[test]
    fn chat_stream_body_sends_adaptive_thinking_with_summarized_display_for_claude_5_family() {
        // End-to-end: the request body builder must attach the adaptive
        // thinking block (opting into visible "summarized" display, which
        // defaults to "omitted"/empty otherwise) and inflate max_tokens the
        // same way the classic-thinking path does.
        for m in [
            "claude-opus-5",
            "claude-sonnet-5",
            "claude-fable-5",
            "claude-fable-5-20260201",
        ] {
            let mut req = base_request(m);
            req.max_tokens = Some(4096);
            let body = build_chat_stream_body(&req, caps_for(&req.model));
            assert_eq!(
                body["thinking"],
                json!({ "type": "adaptive", "display": "summarized" }),
                "{m} must opt into summarized display"
            );
            assert_eq!(
                body["max_tokens"],
                json!(4096 + 4096 / 2),
                "{m}: thinking tokens count toward max_tokens on adaptive models too"
            );
            // Anthropic 400s on ANY non-default temperature/top_p for every
            // adaptive-thinking model — both must be entirely omitted.
            assert!(
                body.get("temperature").is_none(),
                "{m} must not send temperature"
            );
            assert!(body.get("top_p").is_none(), "{m} must not send top_p");
        }
    }

    #[test]
    fn chat_stream_body_sends_adaptive_thinking_for_opus_4_7_and_4_8() {
        for m in ["claude-opus-4-7", "claude-opus-4-8"] {
            let mut req = base_request(m);
            req.max_tokens = Some(4096);
            let body = build_chat_stream_body(&req, caps_for(&req.model));
            assert_eq!(
                body["thinking"],
                json!({ "type": "adaptive", "display": "summarized" }),
                "{m} is adaptive-only — must never get the classic enabled+budget shape"
            );
            assert!(body.get("temperature").is_none());
        }
    }

    #[test]
    fn chat_stream_body_omits_top_p_for_adaptive_models_even_when_caller_supplies_it() {
        let mut req = base_request("claude-sonnet-5");
        req.top_p = Some(0.95);
        req.max_tokens = Some(4096);
        let body = build_chat_stream_body(&req, caps_for(&req.model));
        assert!(
            body.get("top_p").is_none(),
            "adaptive models 400 on a non-default top_p"
        );
    }

    #[test]
    fn chat_stream_body_sends_no_thinking_key_for_unknown_models() {
        // Unknown/other models get nothing extra: no thinking block, no
        // max_tokens inflation, and the plain temperature/top_p path.
        let mut req = base_request("some-future-claude-model");
        req.max_tokens = Some(8192);
        let body = build_chat_stream_body(&req, caps_for(&req.model));
        assert!(body.get("thinking").is_none());
        assert_eq!(
            body["max_tokens"],
            json!(8192),
            "no inflation for an unknown model"
        );
        assert_eq!(body["temperature"], json!(0.8));
    }

    #[test]
    fn build_complete_body_omits_temperature_and_inflates_max_tokens_for_fable_5() {
        let caps = caps_for("claude-fable-5");
        let body = build_complete_body("claude-fable-5", "", "hi", Some(0.8), caps);
        assert!(
            body.get("temperature").is_none(),
            "adaptive models 400 on a non-default temperature"
        );
        assert_eq!(
            body["max_tokens"],
            json!(4096 + 4096 / 2),
            "thinking is on by default and counts toward max_tokens even with no thinking key sent"
        );
        // No thinking-view display concern on this single-shot completion path.
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn build_complete_body_keeps_temperature_for_a_classic_model() {
        let caps = caps_for("claude-opus-4-20250514");
        let body = build_complete_body("claude-opus-4-20250514", "sys", "hi", Some(0.3), caps);
        assert_eq!(body["temperature"], json!(0.3));
        assert_eq!(
            body["max_tokens"],
            json!(4096),
            "no inflation for a classic model here"
        );
        assert_eq!(body["system"], json!("sys"));
    }

    #[test]
    fn build_web_search_body_omits_temperature_and_inflates_max_tokens_for_fable_5() {
        let caps = caps_for("claude-fable-5");
        let body = build_web_search_body("claude-fable-5", "sys", "hi", caps);
        assert!(
            body.get("temperature").is_none(),
            "adaptive models 400 on a non-default temperature"
        );
        assert_eq!(
            body["max_tokens"],
            json!(1024 + 1024 / 2),
            "adaptive headroom applies even to this small hardcoded 1024 cap"
        );
    }

    #[test]
    fn build_web_search_body_keeps_its_hardcoded_temperature_for_a_classic_model() {
        let caps = caps_for("claude-opus-4-20250514");
        let body = build_web_search_body("claude-opus-4-20250514", "sys", "hi", caps);
        assert_eq!(body["temperature"], json!(0.2));
        assert_eq!(body["max_tokens"], json!(1024));
    }

    #[test]
    fn build_tools_body_omits_temperature_and_inflates_max_tokens_for_fable_5() {
        let caps = caps_for("claude-fable-5");
        let body = build_tools_body("claude-fable-5", "", vec![], vec![], Some(0.8), caps);
        assert!(
            body.get("temperature").is_none(),
            "adaptive models 400 on a non-default temperature"
        );
        assert_eq!(body["max_tokens"], json!(4096 + 4096 / 2));
    }

    #[test]
    fn build_tools_body_keeps_temperature_for_a_classic_model() {
        let caps = caps_for("claude-opus-4-20250514");
        let body = build_tools_body(
            "claude-opus-4-20250514",
            "",
            vec![],
            vec![],
            Some(0.4),
            caps,
        );
        assert_eq!(body["temperature"], json!(0.4));
        assert_eq!(body["max_tokens"], json!(4096));
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

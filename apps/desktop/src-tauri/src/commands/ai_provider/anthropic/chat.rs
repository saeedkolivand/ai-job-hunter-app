//! Anthropic `/messages` transport: plain completion, web-search research,
//! streaming chat, and tool-calling. Split out of `anthropic.rs` (R8 LOC cap)
//! — a pure move (each fn keeps its original body; only the enclosing `impl`
//! block moved, and the trait methods in `anthropic.rs` now delegate here).

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::{AppError, AppResult};

use super::super::pagination::checked_response;
use super::super::retry::send_with_retry;
use super::super::stream::{collect, open, stream_response, StreamLimits};
use super::super::timeouts;
use super::super::{
    map_completion_transport_error, resolve_intent, single_shot_turn, split_system, AgentTurn,
    AiGenerateRequest, AiProvider, ChatMsg, ProviderId, RequestTrace, ToolSpec, Usage,
};
use super::body::{
    build_chat_stream_body, build_structured_body, build_tools_body, build_web_search_body,
};
use super::wire::{join_text_blocks, parse_anthropic_frames, parse_anthropic_turn};
use super::{AnthropicClient, BASE, VERSION};

impl AnthropicClient {
    /// Shared body of `complete`/`complete_with_usage`: one `/messages` call,
    /// STREAMED and re-assembled into `(text, usage)` (idle timeout instead of
    /// a whole-request wall, #1353) so the trait methods never duplicate the
    /// HTTP round-trip. `output_config` (format + effort) is accepted on the
    /// stream request exactly as on the one-shot one.
    pub(super) async fn complete_impl(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
        output_config: Option<Value>,
    ) -> AppResult<(String, Usage)> {
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(ProviderId::Anthropic, model, "/messages", BASE, true);

        // Truncated JSON must fail, not reach `repair_json`; plain text may be cut.
        let json = output_config
            .as_ref()
            .is_some_and(|o| o.get("format").is_some());
        let body = build_structured_body(model, system, user, temperature, output_config);

        let limits = StreamLimits::new(timeouts::COMPLETION);
        let resp = open(
            || {
                crate::net::http::shared()
                    .post(&endpoint)
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", VERSION)
                    .json(&body)
            },
            limits,
            "Anthropic",
            |e| map_completion_transport_error(e, "Anthropic", limits.idle),
        )
        .await;
        let mut resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(e);
            }
        };
        resp = checked_response(resp, ProviderId::Anthropic, &trace).await?;
        let status = resp.status();
        let mut last_event = String::new();
        let mut usage = Usage::default();
        let collected = collect(
            &mut resp,
            move |buf| parse_anthropic_frames(buf, &mut last_event, &mut usage),
            limits,
            "Anthropic",
            json,
        )
        .await;
        trace.end(Some(status.as_u16()), collected.is_ok());
        collected
    }

    /// Shared transport for every `research*` facet: a non-streaming Messages
    /// call with the server-side web-search tool, `system`/`user` supplied by the
    /// caller. Capped at 3 searches (a brief, not deep research); the enricher
    /// also bounds the whole call with a timeout. Requires the org to enable web
    /// search, and degrades to `""` (never an error) on any failure so
    /// generation always proceeds.
    pub(super) async fn web_search_complete(
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

        let body = build_web_search_body(model, system, user);

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
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            tracing::warn!(
                "anthropic research {status}: {}",
                crate::commands::ai_provider::redact_body_for_log(&body_text)
            );
            return Ok(String::new());
        }
        let data: Value = match crate::net::http::read_json_capped(
            resp,
            crate::net::http::DEFAULT_MAX_BODY_BYTES,
        )
        .await
        {
            Ok(v) => v,
            Err(_) => {
                trace.end(Some(status.as_u16()), false);
                return Ok(String::new());
            }
        };
        trace.end(Some(status.as_u16()), true);
        Ok(join_text_blocks(&data))
    }

    /// Body of `AiProvider::chat_stream` for [`AnthropicClient`] — moved out
    /// of the trait method so it stays a thin delegator.
    pub(super) async fn chat_stream_impl(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(ProviderId::Anthropic, &req.model, "/messages", BASE, true);

        let sampling = self
            .sampling_profile(&req.model, resolve_intent(req))
            .resolve(req);
        let body = build_chat_stream_body(req, sampling);

        // Retried on a transient 429/5xx: this is only the handshake, so a retry
        // re-sends a request that emitted no deltas. Treating it as terminal is
        // what turned a provider rate-limit into a lost multi-minute generation.
        let limits = StreamLimits::new(timeouts::stream_deadline(req.effort.as_deref()));
        let response = open(
            || {
                crate::net::http::shared()
                    .post(&endpoint)
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", VERSION)
                    .json(&body)
            },
            limits,
            "Anthropic",
            |e| AppError::Network(format!("Anthropic unreachable: {e}")),
        )
        .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(e);
            }
        };

        let response = checked_response(response, ProviderId::Anthropic, &trace).await?;
        let status = response.status();

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
            limits,
            move |buf| parse_anthropic_frames(buf, &mut last_event, &mut usage),
        )
        .await
    }

    /// Body of `AiProvider::chat_with_tools` for [`AnthropicClient`] — moved
    /// out of the trait method so it stays a thin delegator.
    pub(super) async fn chat_with_tools_impl(
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

        let body = build_tools_body(model, &system, wire_messages, tool_specs, temperature);

        let resp = send_with_retry(
            || {
                crate::net::http::shared()
                    .post(&endpoint)
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", VERSION)
                    .json(&body)
            },
            timeouts::COMPLETION,
        )
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Anthropic unreachable: {e}")));
            }
        };
        let resp = checked_response(resp, ProviderId::Anthropic, &trace).await?;
        let status = resp.status();
        let data: Value =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        Ok(parse_anthropic_turn(&data))
    }
}

//! Gemini `generateContent` tool-calling transport. Split out of
//! `gemini.rs` (R8 LOC cap) — a pure move.

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::error::{AppError, AppResult};

use super::super::pagination::checked_response;
use super::super::timeouts;
use super::super::{
    single_shot_turn, split_system, AgentTurn, AiProvider, ChatMsg, ProviderId, RequestTrace, Role,
    StopReason, ToolSpec,
};
use super::thinking::gemini_effective_temperature;
use super::transport::require_gemini_key;
use super::wire::parse_gemini_turn;
use super::{GeminiClient, BASE};

impl GeminiClient {
    /// Body of `AiProvider::chat_with_tools` for [`GeminiClient`] — moved
    /// out of the trait method so it stays a thin delegator.
    pub(super) async fn chat_with_tools_impl(
        &self,
        app: &AppHandle,
        model: &str,
        messages: &[ChatMsg],
        tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        if !self.capabilities(model).supports_tools {
            return single_shot_turn(self, app, model, messages, temperature).await;
        }
        let api_key = require_gemini_key(app)?;
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:generateContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, false);

        let (system, rest) = split_system(messages);
        let contents: Vec<Value> = rest
            .iter()
            .map(|msg| {
                // Gemini's assistant role is "model"; user + (folded) tool results are "user".
                let role = if msg.role == Role::Assistant {
                    "model"
                } else {
                    "user"
                };
                json!({ "role": role, "parts": [{ "text": msg.content }] })
            })
            .collect();
        // Trusted, fixed function declarations — never built from scraped/model text.
        let function_declarations: Vec<Value> = tools
            .iter()
            .map(
                |t| json!({ "name": t.name, "description": t.description, "parameters": t.schema }),
            )
            .collect();

        let mut generation_config = json!({});
        if let Some(t) = gemini_effective_temperature(model, temperature, 0.7) {
            generation_config["temperature"] = json!(t);
        }
        let mut body = json!({
            "contents": contents,
            "generationConfig": generation_config,
            "tools": [{ "functionDeclarations": function_declarations }],
        });
        if !system.is_empty() {
            body["systemInstruction"] = json!({ "parts": [{ "text": system }] });
        }

        let url = format!("{BASE}{endpoint_label}");
        let resp = super::super::retry::send_with_retry(
            || {
                crate::net::http::shared()
                    .post(&url)
                    .header("x-goog-api-key", &api_key)
                    .json(&body)
            },
            timeouts::COMPLETION,
        )
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Gemini unreachable: {e}")));
            }
        };
        let resp = checked_response(resp, ProviderId::Gemini, &trace).await?;
        let status = resp.status();
        let data: Value =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        let turn = parse_gemini_turn(&data);
        // Mirror `complete()`'s empty-response guard: a missing/blocked candidate
        // (e.g. a safety block with no `candidates`) parses to blank text and no
        // tool calls. Exclude `Length` — a `MAX_TOKENS`/`MALFORMED_FUNCTION_CALL`
        // turn can legitimately have no usable text or calls yet, and that is
        // already handled by the controller's truncation path, not an error here.
        if turn.text.is_empty() && turn.tool_calls.is_empty() && turn.stop != StopReason::Length {
            return Err(AppError::Provider(
                "Gemini: unexpected response shape".to_string(),
            ));
        }
        Ok(turn)
    }
}

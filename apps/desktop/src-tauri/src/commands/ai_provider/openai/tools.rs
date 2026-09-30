//! OpenAI-compatible `/chat/completions` tool-calling transport. Split out of
//! `openai.rs` (R8 LOC cap) — a pure move.

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::{AppError, AppResult};

use super::super::pagination::checked_response;
use super::super::timeouts;
use super::super::{single_shot_turn, AgentTurn, AiProvider, ChatMsg, RequestTrace, ToolSpec};
use super::transport::scrub_url_secret;
use super::wire::parse_openai_turn;
use super::OpenAiClient;

impl OpenAiClient {
    /// Body of `AiProvider::chat_with_tools` for [`OpenAiClient`] — moved out
    /// of the trait method so it stays a thin delegator.
    pub(super) async fn chat_with_tools_impl(
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
        let endpoint = self.endpoint_url("chat/completions")?;
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

        let resp = super::super::retry::send_with_retry(
            || {
                crate::net::http::shared()
                    .post(endpoint.clone())
                    .bearer_auth(&api_key)
                    .json(&body)
            },
            timeouts::COMPLETION,
        )
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!(
                    "{} unreachable: {}",
                    self.id.as_str(),
                    scrub_url_secret(e)
                )));
            }
        };
        let resp = checked_response(resp, self.id, &trace).await?;
        let status = resp.status();
        let data: Value = match crate::net::http::read_json_capped(
            resp,
            crate::net::http::DEFAULT_MAX_BODY_BYTES,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                trace.end(Some(status.as_u16()), false);
                return Err(AppError::Message(format!("parse: {e}")));
            }
        };
        trace.end(Some(status.as_u16()), true);
        Ok(parse_openai_turn(&data))
    }
}

//! Ollama `/api/chat` tool-calling transport. Split out of `ollama.rs`
//! (R8 LOC cap) — a pure move.

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::error::AppResult;

use super::super::{
    single_shot_turn, AgentTurn, AiProvider, ChatMsg, ProviderId, RequestTrace, ToolSpec,
};
use super::wire::parse_ollama_turn;
use super::{host, OllamaClient};

impl OllamaClient {
    /// Body of `AiProvider::chat_with_tools` for [`OllamaClient`] — moved out
    /// of the trait method so it stays a thin delegator.
    pub(super) async fn chat_with_tools_impl(
        &self,
        app: &AppHandle,
        model: &str,
        messages: &[ChatMsg],
        tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        // Only tool-capable local models attempt native tool-calling; the rest
        // degrade to a single-shot answer.
        if !self.capabilities(model).supports_tools {
            return single_shot_turn(self, app, model, messages, temperature).await;
        }
        // Held for the whole call — see `LOCAL_CHAT_INFLIGHT`'s doc. The
        // `single_shot_turn` fallback above is covered separately: it calls
        // `complete_with_usage` -> `complete_impl`, which guards itself.
        let _chat_guard = super::local_chat::ChatInFlight::begin();
        let base = host();
        let endpoint = format!("{base}/api/chat");
        let trace = RequestTrace::begin(ProviderId::Ollama, model, "/api/chat tools", &base, false);

        let wire_messages: Vec<Value> = messages
            .iter()
            .map(|m| json!({ "role": m.role.wire(), "content": m.content }))
            .collect();
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
            "stream": false,
            "messages": wire_messages,
            "tools": tool_specs,
        });
        if let Some(t) = temperature {
            body["options"] = json!({ "temperature": t });
        }
        body["keep_alive"] = json!(crate::performance::ollama_keep_alive());

        let resp = match super::super::retry::send_with_retry(
            || crate::net::http::shared().post(&endpoint).json(&body),
            super::super::timeouts::OLLAMA_COMPLETION_BASELINE,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(super::super::map_completion_transport_error(
                    e,
                    "Ollama",
                    super::super::timeouts::OLLAMA_COMPLETION_BASELINE,
                ));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(crate::error::AppError::Provider(format!(
                "Ollama {status}: {body_text}"
            )));
        }
        let data: Value =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| format!("Ollama parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        Ok(parse_ollama_turn(&data))
    }
}

//! OpenAI's native `/responses` web-search transport. Split out of
//! `openai.rs` (R8 LOC cap) — a pure move.

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::AppResult;

use super::super::timeouts;
use super::super::RequestTrace;
use super::wire::join_responses_text;
use super::OpenAiClient;

impl OpenAiClient {
    /// Shared transport for every `research*` facet: the Responses API with the
    /// native `web_search` tool, `system`/`user` supplied by the caller. Every
    /// non-OpenAI id degrades to `""`, exactly like a missing key or a failed
    /// call.
    pub(super) async fn web_search_complete(
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
    /// directly testable against a `wiremock::MockServer` (see the tests).
    /// Behavior-preserving extraction from `web_search_complete`: a transport
    /// failure, a non-2xx status, and a non-JSON body all degrade to `""` (never
    /// an error) — the same gentle-degrade contract the caller already promises.
    pub(super) async fn web_search_transport(
        &self,
        api_key: &str,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<String> {
        let endpoint = match self.endpoint_url("responses") {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("openai research: {e}");
                return Ok(String::new());
            }
        };
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
            .post(endpoint)
            .timeout(timeouts::WEB_SEARCH)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                tracing::warn!(
                    "openai research unreachable: {}",
                    super::transport::scrub_url_secret(e)
                );
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
                "openai research {status}: {}",
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
        Ok(join_responses_text(&data))
    }
}

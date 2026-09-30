//! Gemini's native Google Search grounding transport for every `research*`
//! facet. Split out of `gemini.rs` (R8 LOC cap) — a pure move.

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::AppResult;

use super::super::timeouts;
use super::super::{AiProvider, ProviderId, RequestTrace};
use super::thinking::gemini_effective_temperature;
use super::wire::join_parts_text;
use super::{GeminiClient, BASE};

impl GeminiClient {
    /// Shared transport for every `research*` facet: `generateContent` grounded
    /// with the native Google Search tool, `system`/`user` supplied by the
    /// caller. Degrades to `""` (never an error) on a missing key or any
    /// transport/response failure, so generation always proceeds.
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
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:generateContent");
        let trace = RequestTrace::begin(
            ProviderId::Gemini,
            model,
            "/generateContent google_search",
            BASE,
            false,
        );

        // 0.2 favors precision over creativity for a research brief — but on
        // Gemini 3+ this is exactly the case Google's docs warn against (see
        // `gemini_effective_temperature`'s doc comment): research/synthesis is
        // itself a "complex reasoning task", so forcing a below-1.0 value here
        // was pushing v3+ research into the documented degradation case, not
        // improving its precision. Omitted entirely on v3+ (API applies its
        // own 1.0); kept for pre-v3 models where this concern doesn't apply.
        let mut generation_config = json!({});
        if let Some(t) = gemini_effective_temperature(model, None, 0.2) {
            generation_config["temperature"] = json!(t);
        }
        let body = json!({
            "contents": [ { "role": "user", "parts": [{ "text": user }] } ],
            "systemInstruction": { "parts": [{ "text": system }] },
            "generationConfig": generation_config,
            "tools": [{ "google_search": {} }],
        });
        let url = format!("{BASE}{endpoint_label}");
        let resp = crate::net::http::shared()
            .post(&url)
            .timeout(timeouts::WEB_SEARCH)
            .header("x-goog-api-key", &api_key)
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
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            tracing::warn!("gemini research {status}: {body_text}");
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
        Ok(join_parts_text(&data))
    }
}

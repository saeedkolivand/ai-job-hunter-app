//! Gemini `generateContent`/`embedContent`/`streamGenerateContent` transport:
//! plain completion, embeddings, and streaming chat. Split out of
//! `gemini.rs` (R8 LOC cap) — a pure move (each fn keeps its original body;
//! only the enclosing `impl` block moved, and the trait methods in
//! `gemini.rs` now delegate here).

use serde_json::Value;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};

use super::super::pagination::checked_response;
use super::super::stream::{collect, open, stream_response, StreamLimits};
use super::super::timeouts;
use super::super::{
    map_completion_transport_error, resolve_intent, AiGenerateRequest, AiProvider, ProviderId,
    RequestTrace, Usage,
};
use super::body::{build_chat_stream_body, build_complete_body, build_embed_body, StructuredCall};
use super::transport::require_gemini_key;
use super::wire::{parse_gemini_embed_usage, parse_gemini_frames, GeminiScanner};
use super::{GeminiClient, BASE, EMBED_OUTPUT_DIMENSIONALITY};

impl GeminiClient {
    /// Shared body of `complete`/`complete_with_usage`: one
    /// `streamGenerateContent` call, re-assembled into `(text, usage)` (idle
    /// timeout instead of a whole-request wall, #1353) so the trait methods
    /// never duplicate the HTTP round-trip. `responseMimeType`/`responseSchema`
    /// are `generationConfig` fields, accepted on the stream endpoint exactly as
    /// on `generateContent`. `structured` is
    /// `Some` only on the structured path — see [`StructuredCall`], which
    /// carries everything that path has and the other two do not (JSON mode,
    /// the translated schema, the request's effort).
    pub(super) async fn complete_impl(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
        structured: Option<StructuredCall<'_>>,
    ) -> AppResult<(String, Usage)> {
        let api_key = require_gemini_key(app)?;
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:streamGenerateContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, true);

        // Truncated JSON must fail, not reach `repair_json`; plain text may be cut.
        let json = structured.as_ref().is_some_and(|s| s.json);
        let body = build_complete_body(model, system, user, temperature, structured);

        let url = format!("{BASE}{endpoint_label}");
        let limits = StreamLimits::new(timeouts::COMPLETION);
        let resp = open(
            || {
                crate::net::http::shared()
                    .post(&url)
                    .header("x-goog-api-key", &api_key)
                    .json(&body)
            },
            limits,
            "Gemini",
            |e| map_completion_transport_error(e, "Gemini", limits.idle),
        )
        .await;
        let mut resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(e);
            }
        };
        resp = checked_response(resp, ProviderId::Gemini, &trace).await?;
        let status = resp.status();
        let mut state = GeminiScanner::default();
        let collected = collect(
            &mut resp,
            move |buf| parse_gemini_frames(buf, &mut state),
            limits,
            "Gemini",
            json,
        )
        .await;
        trace.end(Some(status.as_u16()), collected.is_ok());
        collected
    }

    /// Shared body of `embed`/`embed_with_usage`: one `embedContent` call,
    /// parsed once into `(vector, usage)` so the two trait methods never
    /// duplicate the HTTP round-trip.
    pub(super) async fn embed_impl(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        let api_key = require_gemini_key(app)?;
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:embedContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, false);
        let body = build_embed_body(m, text);
        let url = format!("{BASE}{endpoint_label}");
        // The embed entry point (per-attempt bound ≠ sequence budget) so a
        // timed-out first attempt is still retried — see `retry::
        // send_embed_with_retry`.
        let resp = super::super::retry::send_embed_with_retry(
            || {
                crate::net::http::shared()
                    .post(&url)
                    .header("x-goog-api-key", &api_key)
                    .json(&body)
            },
            timeouts::EMBED,
        )
        .await
        .map_err(|e| format!("Gemini unreachable: {e}"))?;
        let resp = checked_response(resp, ProviderId::Gemini, &trace).await?;
        let status = resp.status();
        let data: Value =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        let vector: Vec<f64> = data
            .get("embedding")
            .and_then(|e| e.get("values"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .ok_or_else(|| {
                AppError::Provider("Gemini: missing embedding in response".to_string())
            })?;
        // Self-verify the wire shape rather than trust it: see
        // `build_embed_body`'s doc comment. If `outputDimensionality` were
        // ever silently ignored (wrong nesting, a future API change, proto3
        // tolerating an unknown field), the API would return its full
        // default dimension instead of what was requested — catch that HERE,
        // loudly, instead of silently storing an oversized vector.
        if vector.len() != EMBED_OUTPUT_DIMENSIONALITY as usize {
            return Err(AppError::Provider(format!(
                "Gemini: requested a {EMBED_OUTPUT_DIMENSIONALITY}-dim embedding but the API \
                 returned {} dims — outputDimensionality was not honored",
                vector.len()
            )));
        }
        Ok((vector, parse_gemini_embed_usage(&data)))
    }

    /// Body of `AiProvider::chat_stream` for [`GeminiClient`] — moved out of
    /// the trait method so it stays a thin delegator.
    pub(super) async fn chat_stream_impl(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        let api_key = require_gemini_key(app)?;
        let endpoint_label = format!("/v1beta/models/{}:streamGenerateContent", req.model);
        let trace =
            RequestTrace::begin(ProviderId::Gemini, &req.model, &endpoint_label, BASE, true);

        let sampling = self
            .sampling_profile(&req.model, resolve_intent(req))
            .resolve(req);
        let body = build_chat_stream_body(req, sampling);

        let url = format!("{BASE}{endpoint_label}");
        // Retried on a transient 429/5xx: this is only the handshake, so a retry
        // re-sends a request that emitted no deltas. Treating it as terminal is
        // what turned a provider rate-limit into a lost multi-minute generation.
        let limits = StreamLimits::new(timeouts::stream_deadline(req.effort.as_deref()));
        let response = open(
            || {
                crate::net::http::shared()
                    .post(&url)
                    .header("x-goog-api-key", &api_key)
                    .json(&body)
            },
            limits,
            "Gemini",
            |e| AppError::Network(format!("Gemini unreachable: {e}")),
        )
        .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(e);
            }
        };

        let response = checked_response(response, ProviderId::Gemini, &trace).await?;
        let status = response.status();

        // The shared loop owns cancel-check + chunk read + emit + complete; the
        // closure is the only Gemini-specific part — it scans the streamed JSON
        // array for complete top-level objects (`state` carries brace depth across
        // chunk boundaries). Gemini has no in-band done sentinel, so the loop
        // completes on end-of-body.
        let mut state = GeminiScanner::default();
        stream_response(
            app,
            job_id,
            &trace,
            response,
            status.as_u16(),
            ProviderId::Gemini,
            &req.model,
            BASE,
            limits,
            move |buf| parse_gemini_frames(buf, &mut state),
        )
        .await
    }
}

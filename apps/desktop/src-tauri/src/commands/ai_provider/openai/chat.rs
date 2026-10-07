//! OpenAI-compatible `/chat/completions` transport: plain completion,
//! embeddings, and streaming chat. Split out of `openai.rs` (R8 LOC cap) —
//! a pure move (each fn keeps its original body; only the enclosing `impl`
//! block moved, and the trait methods in `openai.rs` now delegate here).

use serde_json::Value;
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::{AppError, AppResult};

use super::super::pagination::checked_response;
use super::super::stream::{collect, open, stream_response, StreamLimits};
use super::super::timeouts;
use super::super::{
    map_completion_transport_error, resolve_intent, AiGenerateRequest, AiProvider, RequestTrace,
    Usage,
};
use super::body::{build_chat_stream_body, build_complete_body, StructuredCall};
use super::transport::scrub_url_secret;
use super::wire::{parse_openai_embed_usage, parse_openai_frames};
use super::OpenAiClient;

impl OpenAiClient {
    /// Shared body of `complete`/`complete_with_usage`: one `/chat/completions`
    /// call, STREAMED and re-assembled into `(text, usage)` (idle timeout
    /// instead of a whole-request wall, #1353) so the trait methods never
    /// duplicate the HTTP round-trip. `response_format` is accepted on the
    /// stream request exactly as on the one-shot one. `structured` is
    /// `Some` only on the structured path (see `AiProvider::complete_structured`)
    /// — it is the only non-streaming entry point handed the whole
    /// [`AiGenerateRequest`], so the other two have nothing to pass. Its
    /// `effort` arrives RAW (the user's per-provider preference) and is gated
    /// against this model's own capabilities inside [`build_complete_body`],
    /// exactly as `chat_stream` gates it.
    pub(super) async fn complete_impl(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
        structured: Option<StructuredCall<'_>>,
    ) -> AppResult<(String, Usage)> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let caps = self.capabilities(model);
        let endpoint = self.endpoint_url("chat/completions")?;
        let trace = RequestTrace::begin(self.id, model, "/chat/completions", &self.base_url, true);

        // Truncated JSON must fail, not reach `repair_json`; plain text may be cut.
        let json = structured
            .as_ref()
            .is_some_and(|s| s.response_format.is_some());
        let body = build_complete_body(model, system, user, temperature, caps, structured);

        let limits = StreamLimits::new(timeouts::COMPLETION);
        let resp = open(
            || {
                crate::net::http::shared()
                    .post(endpoint.clone())
                    .bearer_auth(&api_key)
                    .json(&body)
            },
            limits,
            self.id.as_str(),
            // Scrub BEFORE mapping: some OpenAI-compatible gateways put the
            // API key in the base URL's own query string, and
            // `reqwest::Error`'s `Display` embeds the request URL verbatim
            // (see `scrub_url_secret`'s own doc) — the timeout branch never
            // reads `e`'s `Display`, so scrubbing unconditionally is safe.
            |e| map_completion_transport_error(scrub_url_secret(e), self.id.as_str(), limits.idle),
        )
        .await;
        let mut resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(e);
            }
        };
        resp = checked_response(resp, self.id, &trace).await?;
        let status = resp.status();
        let collected = collect(
            &mut resp,
            parse_openai_frames,
            limits,
            self.id.as_str(),
            json,
        )
        .await;
        trace.end(Some(status.as_u16()), collected.is_ok());
        collected
    }

    /// Shared body of `embed`/`embed_with_usage`: one `/embeddings` call,
    /// parsed once into `(vector, usage)` so the two trait methods never
    /// duplicate the HTTP round-trip.
    pub(super) async fn embed_impl(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let endpoint = self.endpoint_url("embeddings")?;
        let trace = RequestTrace::begin(self.id, model, "/embeddings", &self.base_url, false);
        let body = serde_json::json!({ "model": model, "input": text });
        // The embed entry point (per-attempt bound ≠ sequence budget) so a
        // timed-out first attempt is still retried — see `retry::
        // send_embed_with_retry`.
        let resp = super::super::retry::send_embed_with_retry(
            || {
                crate::net::http::shared()
                    .post(endpoint.clone())
                    .bearer_auth(&api_key)
                    .json(&body)
            },
            timeouts::EMBED,
        )
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Message(format!(
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

    /// Body of `AiProvider::chat_stream` for [`OpenAiClient`] — moved out of
    /// the trait method so it stays a thin delegator.
    pub(super) async fn chat_stream_impl(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let caps = self.capabilities(&req.model);
        let sampling = self
            .sampling_profile(&req.model, resolve_intent(req))
            .resolve(req);
        let endpoint = self.endpoint_url("chat/completions")?;
        let trace = RequestTrace::begin(
            self.id,
            &req.model,
            "/chat/completions",
            &self.base_url,
            true,
        );

        let body = build_chat_stream_body(req, caps, sampling);

        // Retried on a transient 429/5xx: this is only the handshake, so a retry
        // re-sends a request that emitted no deltas. Treating it as terminal is
        // what turned a provider rate-limit into a lost multi-minute generation.
        let limits = StreamLimits::new(timeouts::stream_deadline(req.effort.as_deref()));
        let response = open(
            || {
                crate::net::http::shared()
                    .post(endpoint.clone())
                    .bearer_auth(&api_key)
                    .json(&body)
            },
            limits,
            self.id.as_str(),
            |e| {
                AppError::Network(format!(
                    "{} unreachable: {}",
                    self.id.as_str(),
                    scrub_url_secret(e)
                ))
            },
        )
        .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(e);
            }
        };

        let response = checked_response(response, self.id, &trace).await?;
        let status = response.status();

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
            limits,
            parse_openai_frames,
        )
        .await
    }
}

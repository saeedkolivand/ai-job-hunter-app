//! Ollama `/api/chat` request-body builders + transport: streaming chat and
//! plain completion. Split out of `ollama.rs` (R8 LOC cap) — a pure move
//! (each fn keeps its original body; only the enclosing module moved, and
//! the trait methods in `ollama.rs` now delegate here).

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::error::AppResult;

use super::super::stream::{collect, open, stream_response, StreamLimits};
use super::super::timeouts;
use super::super::{
    map_completion_transport_error, AiGenerateRequest, ProviderId, RequestTrace, SamplingProfile,
    Usage,
};
use super::wire::parse_ollama_frames;
use super::{
    host, ollama_family_supports_thinking, ollama_think_is_level_only, OLLAMA_EFFORT_LEVELS,
    OLLAMA_OFF,
};

/// Build the `/api/chat` streaming request body for a given
/// [`AiGenerateRequest`] + resolved [`SamplingProfile`] (already merged with
/// the request's explicit numeric overrides — see [`SamplingProfile::resolve`]).
/// Pure + unit-tested. Every `options.*` sampling field (`temperature`,
/// `top_p`, `repeat_penalty`) is added only when `sampling` carries `Some`
/// (never sent as `null`) — an unset field lets `/api/chat` fall back to the
/// model's own Modelfile default, which is NOT a safe default this app
/// controls (see `AiProvider::sampling_profile`'s doc on `OllamaClient`
/// for empirical evidence: some current default-tier local models default to
/// `temperature: 1`, one as high as `presence_penalty: 1.5`). `repeat_penalty`
/// uses Ollama's own field/semantics — it is NEVER a remap of
/// `frequency_penalty` (different math, different field).
pub(super) fn build_chat_stream_body(req: &AiGenerateRequest, sampling: SamplingProfile) -> Value {
    let messages = serde_json::to_value(
        req.messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect::<Vec<_>>(),
    )
    .unwrap_or(json!([]));

    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    if let Some(think) = think_level(&req.model, req.effort.as_deref()) {
        body["think"] = think;
    }
    let mut options = serde_json::Map::new();
    if let Some(t) = sampling.temperature {
        options.insert("temperature".to_string(), json!(t));
    }
    if let Some(top_p) = sampling.top_p {
        options.insert("top_p".to_string(), json!(top_p));
    }
    if let Some(rp) = sampling.repeat_penalty {
        options.insert("repeat_penalty".to_string(), json!(rp));
    }
    if let Some(mt) = req.max_tokens {
        options.insert("num_predict".to_string(), json!(mt));
    }
    // Context window (num_ctx) — large résumé/job-ad prompts overflow Ollama's
    // small default context and get silently truncated without this.
    if let Some(ctx) = req.context_window {
        options.insert("num_ctx".to_string(), json!(ctx));
    }
    if !options.is_empty() {
        body["options"] = Value::Object(options);
    }
    body["keep_alive"] = json!(crate::performance::ollama_keep_alive());
    body
}

pub(super) async fn stream_chat(
    app: &AppHandle,
    job_id: &str,
    req: &AiGenerateRequest,
    sampling: SamplingProfile,
) -> AppResult<()> {
    // Held for the whole streamed call (dropped on every exit path, including
    // `?` and the final `stream_response` return) — see `LOCAL_CHAT_INFLIGHT`'s
    // doc. The GPU is busy for the WHOLE stream, not just the handshake.
    let _chat_guard = super::local_chat::ChatInFlight::begin();
    let base = host();
    let endpoint = format!("{base}/api/chat");
    let trace = RequestTrace::begin(ProviderId::Ollama, &req.model, "/api/chat", &base, true);

    let body = build_chat_stream_body(req, sampling);

    // Retried on a transient 429/5xx: this is only the handshake, so a retry
    // re-sends a request that emitted no deltas. Treating it as terminal is what
    // turned a provider rate-limit into a lost multi-minute generation.
    // An IDLE bound (+ ceiling), not a whole-request wall — see `timeouts::stream_deadline`.
    let limits = StreamLimits::new(timeouts::stream_deadline(req.effort.as_deref()));
    let response = open(
        || crate::net::http::shared().post(&endpoint).json(&body),
        limits,
        "Ollama",
        |e| map_completion_transport_error(e, "Ollama", limits.idle),
    )
    .await;

    let response = match response {
        Ok(r) => r,
        Err(e) => {
            trace.end(None, false);
            return Err(e);
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body_text =
            crate::net::http::read_text_capped(response, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
        trace.end(Some(status.as_u16()), false);
        return Err(crate::error::AppError::Provider(format!(
            "Ollama {status}: {}",
            crate::commands::ai_provider::redact_upstream_text(&body_text)
        )));
    }

    // The shared loop owns cancel-check + chunk read + emit + complete; the closure
    // is the only Ollama-specific part (newline-delimited JSON framing). Structured
    // reasoning from thinking models rides on `message.thinking`; models that embed
    // <think>…</think> in `content` are split renderer-side. `think` is only sent
    // (by `build_chat_stream_body`) when the caller set an effort AND the model is
    // in the known thinking family — it 400s on a non-thinking model otherwise.
    stream_response(
        app,
        job_id,
        &trace,
        response,
        status.as_u16(),
        ProviderId::Ollama,
        &req.model,
        &base,
        limits,
        parse_ollama_frames,
    )
    .await
}

/// The `think` value a request may send on this model — `None` when it must
/// not be sent at all. `think` is a top-level request field (NOT nested under
/// `options`), and only safe to send when it is one of [`OLLAMA_EFFORT_LEVELS`]
/// or the `off` tier (which becomes `think: false`, or `"low"` on gpt-oss)
/// (see its doc comment) AND the model is in the known thinking family: it 400s
/// on a non-thinking model (see `ollama_family_supports_thinking`) or on an
/// unrecognized value.
///
/// Shared by BOTH body builders rather than re-written per call site.
/// `complete_structured` is the one non-streaming path that receives the whole
/// [`AiGenerateRequest`], and it silently dropped `effort` for its entire
/// existence while `chat_stream` honored it — a second hand-written copy of
/// this gate is precisely how that happens again.
pub(super) fn think_level(model: &str, effort: Option<&str>) -> Option<Value> {
    let effort = effort.map(str::trim)?;
    if !ollama_family_supports_thinking(model) {
        return None;
    }
    if effort == OLLAMA_OFF {
        // `false` switches thinking off on qwen3-style models; gpt-oss ignores
        // it, so its cheapest tier is the level string.
        return Some(if ollama_think_is_level_only(model) {
            json!("low")
        } else {
            json!(false)
        });
    }
    OLLAMA_EFFORT_LEVELS
        .contains(&effort)
        .then(|| json!(effort))
}

/// What `OllamaClient::complete_structured` adds to a streamed
/// `/api/chat` call and the plain `complete`/`complete_with_usage` path cannot:
/// those two take no [`AiGenerateRequest`], so they have neither a `format` nor
/// any of the request-level knobs below. `Some(..)` IS the JSON-mode switch, so
/// the switch and the knobs can never disagree — the mirror of
/// `super::super::gemini::body::StructuredCall`.
pub(super) struct StructuredCall<'a> {
    /// Ollama's own constrained-decoding field: the caller's JSON Schema
    /// verbatim, or the `"json"` string (see `structured::ollama_format`).
    /// `None` is the plain-text call that only carries an effort/limits
    /// (`AiProvider::complete_with_effort`).
    pub(super) format: Option<Value>,
    /// The request's RAW reasoning effort, gated here by [`think_level`].
    pub(super) effort: Option<&'a str>,
    /// `req.max_tokens` → `options.num_predict`.
    pub(super) max_tokens: Option<u32>,
    /// `req.context_window` → `options.num_ctx`.
    pub(super) context_window: Option<u32>,
}

/// Build the streamed `/api/chat` body shared by `complete`/
/// `complete_with_usage`/`complete_structured`. Pure + unit-tested.
///
/// Every `options.*` entry is added only when `Some` — an unset field lets
/// `/api/chat` fall back to the model's own Modelfile default, exactly as on
/// [`build_chat_stream_body`], whose gating this mirrors field for field.
/// `num_predict`/`num_ctx` reach only this path's structured caller for the
/// same reason `think` does (the two plain paths carry no request), and they
/// matter most here: a résumé + job ad is precisely the prompt that overflows
/// Ollama's small default context and comes back silently truncated.
///
/// `think` puts the model's reasoning in `message.thinking`, a SEPARATE field
/// from the `message.content` this path parses, so it never pollutes the JSON
/// answer.
pub(super) fn build_complete_body(
    model: &str,
    system: &str,
    user: &str,
    temperature: Option<f64>,
    structured: Option<StructuredCall<'_>>,
) -> Value {
    // Streamed and re-assembled by `complete_impl` (idle timeout, #1353) — `format`
    // (the JSON schema) is accepted on the stream endpoint exactly as on the
    // one-shot one.
    let mut body = json!({
        "model": model,
        "stream": true,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
    });
    let mut options = serde_json::Map::new();
    if let Some(t) = temperature {
        options.insert("temperature".to_string(), json!(t));
    }
    if let Some(structured) = structured {
        if let Some(mt) = structured.max_tokens {
            options.insert("num_predict".to_string(), json!(mt));
        }
        if let Some(ctx) = structured.context_window {
            options.insert("num_ctx".to_string(), json!(ctx));
        }
        if let Some(format) = structured.format {
            body["format"] = format;
        }
        if let Some(think) = think_level(model, structured.effort) {
            body["think"] = think;
        }
    }
    if !options.is_empty() {
        body["options"] = Value::Object(options);
    }
    body["keep_alive"] = json!(crate::performance::ollama_keep_alive());
    body
}

/// One content-free line per call naming what the body actually carries: the
/// `think` value (or `absent`), the `format` kind and the option KEYS. Never the
/// prompt, the schema or any value other than `think` (#1382: tells a "thinking
/// ran although effort was off" report apart from a model that ignored `think`).
fn log_request_shape(model: &str, body: &Value) {
    let think = body
        .get("think")
        .map_or_else(|| "absent".to_string(), Value::to_string);
    let format = match body.get("format") {
        None => "none",
        Some(Value::String(_)) => "json",
        Some(_) => "schema",
    };
    let options: Vec<&str> = body
        .get("options")
        .and_then(Value::as_object)
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    log::info!(
        "[ai] ollama request shape model={model} think={think} format={format} options={options:?}"
    );
}

/// Shared body of `AiProvider::complete`/`AiProvider::complete_with_usage`
/// for [`OllamaClient`](super::OllamaClient): one streamed `/api/chat`
/// call, parsed once into `(text, usage)` so the two trait methods never
/// duplicate the HTTP round-trip. A free function (no `&self`) since
/// `OllamaClient` is a unit struct with no other state.
pub(super) async fn complete_impl(
    model: &str,
    system: &str,
    user: &str,
    temperature: Option<f64>,
    structured: Option<StructuredCall<'_>>,
) -> AppResult<(String, Usage)> {
    // Held for the whole call (dropped on every exit path, including `?`) —
    // see `LOCAL_CHAT_INFLIGHT`'s doc.
    let _chat_guard = super::local_chat::ChatInFlight::begin();
    let base = host();
    let endpoint = format!("{base}/api/chat");
    let trace = RequestTrace::begin(ProviderId::Ollama, model, "/api/chat", &base, true);

    // The IDLE bound, scaled by the SAME effort that governs `chat_stream`'s — see
    // `timeouts::ollama_completion_deadline`'s doc for why only the callers that
    // carry a `StructuredCall` can raise it above the baseline: `complete`/
    // `complete_with_usage` have no `AiGenerateRequest` to read an effort off.
    let limits = StreamLimits::new(timeouts::ollama_completion_deadline(
        structured.as_ref().and_then(|s| s.effort),
    ));
    // Truncated JSON must fail, not reach `repair_json`; plain text may be cut.
    let json = structured.as_ref().is_some_and(|s| s.format.is_some());
    let body = build_complete_body(model, system, user, temperature, structured);
    log_request_shape(model, &body);

    let resp = match open(
        || crate::net::http::shared().post(&endpoint).json(&body),
        limits,
        "Ollama",
        |e| map_completion_transport_error(e, "Ollama", limits.idle),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            trace.end(None, false);
            return Err(e);
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
            "Ollama {status}: {}",
            crate::commands::ai_provider::redact_upstream_text(&body_text)
        )));
    }
    // Reasoning rides `message.thinking` and is dropped by `collect` (as the
    // one-shot path never read it); the answer is `message.content` only.
    let mut resp = resp;
    let collected = collect(&mut resp, parse_ollama_frames, limits, "Ollama", json).await;
    trace.end(Some(status.as_u16()), collected.is_ok());
    collected
}

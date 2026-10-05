//! Error mapping: turning a provider's HTTP/transport failure into a clear,
//! privacy-safe [`AppError`]. Split out of `mod.rs` (R8 line-budget split).

use serde_json::Value;
use tauri::AppHandle;

use crate::error::AppError;
use crate::events::{emit_event, AiStreamChunk, AiStreamChunkError, AI_STREAM};

use super::ProviderId;

/// Pull a human-readable message out of a provider's JSON error body.
pub fn extract_error_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(msg) = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .or_else(|| v.get("message").and_then(|m| m.as_str()))
            .or_else(|| {
                v.get("error")
                    .and_then(|e| e.get(0))
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
            })
        {
            return msg.to_string();
        }
    }
    body.trim().chars().take(200).collect()
}

/// Map a provider HTTP error to a clear, actionable message.
pub fn friendly_api_error(
    provider: ProviderId,
    status: reqwest::StatusCode,
    body: &str,
) -> AppError {
    let name = provider.as_str();
    let code = status.as_u16();
    let detail = extract_error_message(body);
    match code {
        401 | 403 => AppError::Config(format!("{name}: invalid or unauthorized API key.")),
        404 => AppError::Provider(format!("{name}: model or endpoint not found — {detail}")),
        413 => AppError::Provider(format!(
            "{name}: request too large — try a smaller resume/job ad."
        )),
        422 => AppError::Provider(format!(
            "{name}: this model rejected the request — {detail}"
        )),
        429 => AppError::Network(format!(
            "{name}: rate limit or quota reached. Wait a moment or check your plan."
        )),
        400 => AppError::Provider(format!("{name}: request rejected — {detail}")),
        500..=599 => AppError::Network(format!(
            "{name}: service error ({code}). Try again shortly."
        )),
        _ => AppError::Provider(format!("{name} {code}: {detail}")),
    }
}

/// Map a *transport* failure (the `send()` that raced an HTTP call never got a
/// response at all) to `AppError::Timeout` or `AppError::Network`. Distinct
/// from [`friendly_api_error`], which maps a response the server DID send
/// back.
///
/// Named for its original completion call sites but equally correct for any
/// other request against the same pooled client (list-models, model-pull,
/// embeddings, tool-calling): the classification below depends only on
/// `is_timeout()`, never on what the request was *for*. Ollama's embed path
/// (`ollama::embed_with`) once mapped every transport failure — including a
/// real embed-timeout while the daemon was up and busy — to "unreachable",
/// which sent two separate investigations to the wrong root cause; it now
/// routes through here too.
///
/// `is_timeout()` walks `e`'s WHOLE source chain, not just this crate's own
/// `.timeout()` call: it also matches an inner `hyper::Error::is_timeout()`
/// and a raw `io::ErrorKind::TimedOut` — so it is reqwest's general "gave up
/// waiting" signal, not narrowly "the client's own configured deadline
/// fired". That is still the right classification here: `net::http::shared`
/// (the sole pooled client every adapter uses) sets no separate
/// connect/read timeout of its own — see its module doc, "no global
/// timeout" — so every request's ONLY timing bound is the SAME per-call
/// `.timeout()` these call sites set, and that bound covers connect through
/// the last streamed byte. Whichever inner layer is the one that actually
/// noticed the wait (reqwest's own timer, hyper's, or the OS socket's) is
/// noticing the SAME deadline elapsing, so `is_timeout() == true` reliably
/// means this call's own `deadline` is why it stopped waiting, and a retry
/// against that same deadline would time out again — `AppError::Timeout`,
/// not `Network`, either way.
///
/// `label` names the provider in the message — a fixed string for a
/// single-provider adapter (Anthropic/Gemini/Ollama), or `self.id.as_str()`
/// for [`crate::commands::ai_provider::openai::OpenAiClient`], which serves
/// several [`ProviderId`]s from one client. `deadline` is the per-call bound
/// that just expired — not read off a shared table here because it varies by
/// call (Ollama's non-streaming completion scales it by the request's
/// reasoning effort; see `timeouts::ollama_completion_deadline`).
pub fn map_completion_transport_error(
    e: reqwest::Error,
    label: &str,
    deadline: std::time::Duration,
) -> AppError {
    if e.is_timeout() {
        AppError::Timeout(format!(
            "{label}: no response within {}s",
            deadline.as_secs()
        ))
    } else {
        AppError::Network(format!("{label} unreachable: {e}"))
    }
}

/// Redact a generation-failure message before it reaches the renderer.
///
/// This is the choke point every generation-failure path funnels through
/// (`ai_generate` in `commands/ai/generate.rs`, `generate_pipeline` in
/// `commands/pipeline.rs` — both call [`emit_stream_error`] on their `Err`
/// branch with a raw `AppError`/`e.to_string()`). A provider or transport
/// error can carry a `base_url` with query-string auth (the #935 shape), an
/// absolute filesystem path, or a bare host — none of which may reach the
/// screen. Reuses the diagnostics-bundle redactor (`commands::support::redact_lines`,
/// ADR-027) rather than a second one: both are "text about to reach outside
/// the machine's trust boundary" and must not drift into differing strength.
/// Deliberately conservative (URL/path/host/credential/email shapes only) so
/// an ordinary message like `"429 Too Many Requests"` or `"model not found"`
/// survives byte-for-byte. Pure + unit-tested (see `mod tests`).
fn redact_stream_error_message(message: &str) -> String {
    crate::commands::support::redact_lines(message)
}

/// Emit the terminal `ai:stream` error event the renderer's stream reader expects.
pub fn emit_stream_error(app: &AppHandle, job_id: &str, message: &str) {
    emit_event(
        app,
        AI_STREAM,
        AiStreamChunk {
            job_id: job_id.to_string(),
            delta: String::new(),
            done: true,
            error: Some(AiStreamChunkError {
                code: "GENERATION_FAILED".to_string(),
                message: redact_stream_error_message(message),
            }),
            thinking: None,
        },
    );
}

#[cfg(test)]
mod tests;

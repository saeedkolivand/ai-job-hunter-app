//! Error mapping: turning a provider's HTTP/transport failure into a clear,
//! privacy-safe [`AppError`]. Split out of `mod.rs` (R8 line-budget split).

use serde_json::Value;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
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

/// ponytail: byte ceiling applied to upstream text BEFORE the shape redactor
/// runs (which is a per-token scan, so its cost grows with input; the JSON
/// branch of [`extract_error_message`] is otherwise uncapped). 8 KiB is ~40x
/// the old 200-char cap and far past where any provider puts its
/// context-length wording (the first sentence of the message), so
/// `embed::is_context_length_error` keeps matching; it is NOT the 200-char
/// `sanitize_reason` cap, which would cut that keyword off a long message.
const MAX_UPSTREAM_TEXT_BYTES: usize = 8 * 1024;

/// ponytail: log lines keep only `MAX_REASON_LEN` chars after redaction, so
/// bounding the input to 1 KiB first loses nothing and caps the redactor's work.
const MAX_LOG_BODY_BYTES: usize = 1024;

/// Truncate `s` to at most `max` bytes on a char boundary (never panics on a
/// multi-byte char straddling the ceiling).
fn bound_bytes(s: &str, max: usize) -> &str {
    &s[..s.floor_char_boundary(max)]
}

/// Untrusted upstream provider text -> bounded (byte ceiling, char-boundary
/// safe) then shape-redacted. Ordinary text passes unchanged (modulo the
/// redactor's whitespace normalisation). Deliberately not length-capped to
/// 200 chars: see [`MAX_UPSTREAM_TEXT_BYTES`].
pub fn redact_upstream_text(s: &str) -> String {
    redact_stream_error_message(bound_bytes(s, MAX_UPSTREAM_TEXT_BYTES))
}

/// Upstream response body about to be written to a log: bounded, shape-redacted
/// and capped to `MAX_REASON_LEN` chars (`sanitize_reason`).
pub fn redact_body_for_log(s: &str) -> String {
    crate::observability::sanitize_reason(bound_bytes(s, MAX_LOG_BODY_BYTES))
}

/// Map a provider HTTP error to a clear, actionable message.
pub fn friendly_api_error(
    provider: ProviderId,
    status: reqwest::StatusCode,
    body: &str,
) -> AppError {
    let name = provider.as_str();
    let code = status.as_u16();
    let detail = redact_upstream_text(&extract_error_message(body));
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

/// Shortest secret stripped verbatim: an empty/tiny needle would mangle the
/// whole message (and a 1-3 char "key" is not a credential worth matching).
const MIN_SECRET_LEN: usize = 8;

/// Redact a model-list / key-probe failure before it crosses IPC into the
/// settings UI (`ModelPicker` shows the string verbatim). Provider error text
/// is upstream-controlled and can echo the request. Two passes: (1) every
/// known `secrets` value is replaced verbatim (shape-independent, so a bare
/// `AIza…`/`gsk_…` key or an `x-api-key:` header echo cannot survive), skipping
/// any shorter than [`MIN_SECRET_LEN`]; (2) the shape redactor
/// ([`redact_stream_error_message`]) plus a 200-char bound, since the upstream
/// body is arbitrary. Always `AppError::Provider` (the wire is a string).
pub fn redact_provider_error(e: AppError, secrets: &[&str]) -> AppError {
    AppError::Provider(crate::observability::sanitize_reason(
        &redact_stream_error_message(&strip_text(&e.to_string(), secrets)),
    ))
}

/// Replace every `secrets` value (>= [`MIN_SECRET_LEN`], longest first so a
/// secret that contains a shorter one is removed whole) in `text` verbatim.
/// Cost: one `str::replace` pass per qualifying secret (a handful per call).
fn strip_text(text: &str, secrets: &[&str]) -> String {
    let mut needles: Vec<&str> = secrets
        .iter()
        .copied()
        .filter(|s| s.len() >= MIN_SECRET_LEN)
        .collect();
    needles.sort_by_key(|s| std::cmp::Reverse(s.len()));
    needles.into_iter().fold(text.to_string(), |t, n| {
        t.replace(n, "<credential-redacted>")
    })
}

/// Verbatim strip that keeps the error's VARIANT and does not cap its length —
/// the form the provider-call seams (`Completer`, `embed_text`) need:
/// `pipeline/stage.rs` matches `Timeout`, `is_empty_answer_length_cut` compares
/// the exact `Provider` text, and `retriable()` keys on `Network`/`RateLimited`.
/// A message holding no secret comes back byte-identical.
pub fn strip_secrets_in_place(e: AppError, secrets: &[&str]) -> AppError {
    let s = |m: String| strip_text(&m, secrets);
    match e {
        AppError::Config(m) => AppError::Config(s(m)),
        AppError::Network(m) => AppError::Network(s(m)),
        AppError::Provider(m) => AppError::Provider(s(m)),
        AppError::Storage(m) => AppError::Storage(s(m)),
        AppError::Parse(m) => AppError::Parse(s(m)),
        AppError::Validation(m) => AppError::Validation(s(m)),
        AppError::RateLimited(m) => AppError::RateLimited(s(m)),
        AppError::Timeout(m) => AppError::Timeout(s(m)),
        AppError::Message(m) => AppError::Message(s(m)),
        AppError::Cancelled => AppError::Cancelled,
    }
}

/// The secrets a provider call holds: the stored key (raw and trimmed), plus
/// the base URL's userinfo password and query values. Only compared, never
/// logged.
fn provider_secrets(stored_key: Option<&str>, base_url: Option<&str>) -> Vec<String> {
    let mut secrets: Vec<String> = Vec::new();
    if let Some(k) = stored_key {
        secrets.push(k.to_string());
        secrets.push(k.trim().to_string());
    }
    if let Some(url) = base_url.and_then(|u| reqwest::Url::parse(u).ok()) {
        secrets.extend(url.password().map(str::to_string));
        secrets.extend(url.query_pairs().map(|(_, v)| v.into_owned()));
    }
    secrets
}

/// [`strip_secrets_in_place`] with the secrets derived from what a provider
/// call holds (see [`provider_secrets`]).
pub fn strip_provider_secrets(
    e: AppError,
    stored_key: Option<&str>,
    base_url: Option<&str>,
) -> AppError {
    let secrets = provider_secrets(stored_key, base_url);
    let refs: Vec<&str> = secrets.iter().map(String::as_str).collect();
    strip_secrets_in_place(e, &refs)
}

/// The one error-shaping step both `ai_list_provider_models` and
/// `ai_test_provider_key` run on their result: [`redact_provider_error`] with
/// the secrets from [`provider_secrets`].
pub fn finish_provider_result<T>(
    res: AppResult<T>,
    stored_key: Option<&str>,
    base_url: Option<&str>,
) -> AppResult<T> {
    res.map_err(|e| {
        let secrets = provider_secrets(stored_key, base_url);
        let refs: Vec<&str> = secrets.iter().map(String::as_str).collect();
        redact_provider_error(e, &refs)
    })
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

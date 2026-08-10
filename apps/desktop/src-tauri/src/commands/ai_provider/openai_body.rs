//! The pure `/chat/completions` request-body builders for [`super::OpenAiClient`]
//! — streaming and non-streaming — shared by native OpenAI, every
//! OpenAI-compatible gateway, and Ollama Cloud.
//!
//! Wired via `#[path = "openai_body.rs"] mod body;` in `openai.rs` (the same
//! sibling-file convention as `openai_tests.rs`), so these stay CHILD items of
//! `openai` and keep reaching its private consts. Split out because `openai.rs`
//! is at the R8 LOC cap; no call site or test import moves.

use serde_json::{json, Value};

use super::{
    AiGenerateRequest, ModelCapabilities, SamplingProfile, TokenParam, OPENAI_EFFORT_LEVELS,
};

/// Build the `/chat/completions` streaming request body for a given
/// [`AiGenerateRequest`] + capability matrix + resolved [`SamplingProfile`].
/// Pure + unit-tested — this is the shared body shape for native OpenAI, any
/// OpenAI-compatible gateway, and Ollama Cloud (all backed by
/// [`super::OpenAiClient`]). `sampling` is already merged with the request's
/// explicit numeric overrides (see [`SamplingProfile::resolve`]) — each field
/// is added only when `Some` (never sent as `null`), and the whole block is
/// skipped on reasoning models that reject `temperature`.
pub(super) fn build_chat_stream_body(
    req: &AiGenerateRequest,
    caps: ModelCapabilities,
    sampling: SamplingProfile,
) -> Value {
    let messages = req
        .messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect::<Vec<_>>();

    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    // Real per-call token usage (AI-spend visibility, `crate::spend`): asks for
    // one extra streamed chunk carrying `usage` right before `[DONE]`. Every
    // OpenAI-compatible server this client talks to (native OpenAI, LM
    // Studio/vLLM/OpenRouter/Groq/…, Ollama Cloud) either honors this or
    // silently ignores the unknown field — never a 400.
    body["stream_options"] = json!({ "include_usage": true });
    if caps.supports_temperature {
        if let Some(t) = sampling.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(top_p) = sampling.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(fp) = sampling.frequency_penalty {
            body["frequency_penalty"] = json!(fp);
        }
        if let Some(pp) = sampling.presence_penalty {
            body["presence_penalty"] = json!(pp);
        }
    }
    if let Some(mt) = req.max_tokens {
        let field = match caps.token_param {
            TokenParam::MaxCompletionTokens => "max_completion_tokens",
            _ => "max_tokens",
        };
        body[field] = json!(mt);
    }
    if let Some(effort) = reasoning_effort(req.effort.as_deref(), caps) {
        body["reasoning_effort"] = json!(effort);
    }
    body
}

/// The `reasoning_effort` value a request may send on this model — `None` when
/// it must not be sent at all. Only ever `Some` for one of
/// [`OPENAI_EFFORT_LEVELS`] (see its doc comment) AND when
/// `caps.supports_reasoning` (native OpenAI's o-series/gpt-5.x, or Ollama
/// Cloud's thinking-family models — see
/// [`super::OpenAiClient::supports_reasoning_effort`]): a wrong/guessed value
/// 400s the whole request.
///
/// Shared by BOTH body builders rather than re-written per call site.
/// `complete_structured` is the one non-streaming path that receives the whole
/// [`AiGenerateRequest`], and it silently dropped `effort` for its entire
/// existence while `chat_stream` honored it — a second hand-written copy of
/// this gate is precisely how that happens again.
pub(super) fn reasoning_effort(effort: Option<&str>, caps: ModelCapabilities) -> Option<&str> {
    if !caps.supports_reasoning {
        return None;
    }
    effort
        .map(str::trim)
        .filter(|e| OPENAI_EFFORT_LEVELS.contains(e))
}

/// Build the non-streaming `/chat/completions` body shared by `complete`/
/// `complete_with_usage`/`complete_structured`. Pure + unit-tested —
/// `supports_temperature` is the caller's already-resolved
/// `capabilities(model)` gate (an o-series model rejects the field outright),
/// `reasoning_effort` is the already-gated [`reasoning_effort`] (`None` on the
/// two plain paths, whose signatures carry no request to read `effort` from),
/// and `response_format` is the only structured-output-specific part.
pub(super) fn build_complete_body(
    model: &str,
    system: &str,
    user: &str,
    temperature: Option<f64>,
    supports_temperature: bool,
    reasoning_effort: Option<&str>,
    response_format: Option<Value>,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
        "stream": false,
    });
    if supports_temperature {
        body["temperature"] = json!(temperature.unwrap_or(0.7));
    }
    if let Some(effort) = reasoning_effort {
        body["reasoning_effort"] = json!(effort);
    }
    if let Some(format) = response_format {
        body["response_format"] = format;
    }
    body
}

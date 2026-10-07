//! [`StreamPiece`] — one emittable piece pulled from a provider's stream by
//! its `parse` closure. Split out of `stream.rs` (R8 line-budget split): the
//! wire-agnostic vocabulary is a separate concern from the loop that drives it.

use super::super::{StopReason, Usage};

/// One emittable piece pulled from a provider's stream by its `parse` closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::commands::ai_provider) struct StreamPiece {
    /// Text to forward to the renderer. Empty pieces are skipped (so a parser can
    /// signal `done` without text).
    pub delta: String,
    /// `true` for reasoning/thinking deltas, `false` for normal answer text.
    pub thinking: bool,
    /// `true` when this piece marks the provider's own end-of-stream sentinel
    /// (OpenAI `[DONE]`, Anthropic `message_stop`, Ollama `done:true`). The loop
    /// emits the terminal event and returns after processing this piece.
    pub done: bool,
    /// The provider's REAL token usage, when this piece happens to carry it
    /// (OpenAI's separate `stream_options.include_usage` chunk, Anthropic's
    /// `message_start`/`message_delta` events, Gemini's `usageMetadata`,
    /// Ollama's final `done:true` object). The shared loop remembers the
    /// LATEST non-`None` value it sees across the whole stream and records it
    /// at completion — never estimated, never fabricated when absent.
    pub usage: Option<Usage>,
    /// The provider's own end-of-turn reason, when this piece happens to carry
    /// it: OpenAI/Ollama Cloud's `finish_reason` on a streamed chunk (see
    /// `openai::parse_openai_finish_reason`) and local Ollama's `done_reason` on
    /// its final object (see `ollama::ollama_done_reason`). `None` before one has
    /// been reported, and always `None` for Anthropic/Gemini — NOT because those
    /// have no streamed equivalent (they do: Anthropic's
    /// `message_delta.stop_reason`, Gemini's `candidates[].finishReason`) but
    /// because their frame parsers do not map it yet. Mapping either is
    /// additive: parse the field into [`StopReason`] and the length diagnosis
    /// below starts working there too. Mirrors `usage`'s "latest non-`None`
    /// wins" handling.
    /// What this exists for: telling a `finish_reason: length` empty answer
    /// (the model ran out of budget mid-reasoning, never reached its final
    /// channel) apart from a provider that just silently returned nothing —
    /// see `finish`'s empty-answer branch.
    pub stop_reason: Option<StopReason>,
    /// An in-band upstream error frame (Anthropic `event: error`, OpenAI
    /// `data: {"error":..}`, Gemini `{"error":..}`, Ollama `{"error":".."}`): the
    /// RAW upstream message, unredacted. HTTP-200 streams carry these, so without
    /// it an overloaded/failed call reads as an empty-but-successful one.
    /// `stream::idle::frame_error` redacts and classifies it.
    pub error: Option<String>,
    /// `true` when `delta` is a model REFUSAL (OpenAI `delta.refusal`), not answer
    /// text. Never emitted to the renderer or added to the answer; only quoted in
    /// the empty-answer error.
    pub refusal: bool,
}

impl StreamPiece {
    /// A normal answer-text delta.
    pub fn text(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            thinking: false,
            done: false,
            usage: None,
            stop_reason: None,
            error: None,
            refusal: false,
        }
    }

    /// A reasoning/thinking delta.
    pub fn thinking(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            thinking: true,
            done: false,
            usage: None,
            stop_reason: None,
            error: None,
            refusal: false,
        }
    }

    /// The provider's end-of-stream sentinel, optionally carrying a final delta.
    pub fn done(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            thinking: false,
            done: true,
            usage: None,
            stop_reason: None,
            error: None,
            refusal: false,
        }
    }

    /// A usage-only piece: no visible text, not a completion sentinel — just
    /// reports the provider's real token usage as it becomes known (mid-stream
    /// for OpenAI/Gemini, incrementally for Anthropic, or attached directly to
    /// the `done` piece for Ollama).
    pub fn usage(usage: Usage) -> Self {
        Self {
            delta: String::new(),
            thinking: false,
            done: false,
            usage: Some(usage),
            stop_reason: None,
            error: None,
            refusal: false,
        }
    }

    /// A stop-reason-only piece: no visible text, not a completion sentinel —
    /// OpenAI/Ollama Cloud report `finish_reason` on a regular content chunk
    /// (typically the one right before the `[DONE]` sentinel line), not on the
    /// sentinel itself.
    pub fn stop_reason(reason: StopReason) -> Self {
        Self {
            delta: String::new(),
            thinking: false,
            done: false,
            usage: None,
            stop_reason: Some(reason),
            error: None,
            refusal: false,
        }
    }

    /// An in-band upstream error frame — see [`StreamPiece::error`].
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            error: Some(message.into()),
            ..Self::text("")
        }
    }

    /// An error frame from the provider's own `error` JSON value: a bare string
    /// (Ollama), or an object with `message` (+ optional `type`/`status`).
    pub fn from_error_value(err: &serde_json::Value) -> Self {
        let text = |k: &str| err.get(k).and_then(|v| v.as_str());
        Self::error(match (err.as_str(), text("type"), text("message")) {
            (Some(s), _, _) => s.to_string(),
            (_, Some(t), Some(m)) => format!("{t}: {m}"),
            (_, _, Some(m)) => m.to_string(),
            _ => err.to_string(),
        })
    }

    /// A refusal delta — see [`StreamPiece::refusal`].
    pub fn refusal(delta: impl Into<String>) -> Self {
        Self {
            refusal: true,
            ..Self::text(delta)
        }
    }
}

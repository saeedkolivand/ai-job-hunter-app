//! Gemini `generateContent`/`streamGenerateContent` wire-format parsing:
//! turn/usage extraction and the streamed-JSON-array frame scanner. Split
//! out of `gemini.rs` (R8 LOC cap) — a pure move.

use serde_json::{json, Value};

use super::super::stream::StreamPiece;
use super::super::{AgentTurn, StopReason, ToolCall, Usage};

/// Concatenate every `parts[].text` of the first candidate (non-streaming
/// `generateContent`, incl. grounded responses) into one string. Pure +
/// unit-tested.
pub(super) fn join_parts_text(data: &Value) -> String {
    data.get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Parse a non-streaming `generateContent` response into an [`AgentTurn`]: join
/// the first candidate's `parts[].text` for the visible text, map every
/// `parts[].functionCall` to a [`ToolCall`] (Gemini has no call id, so synthesize
/// `name-index` for our own bookkeeping — `functionResponse` matches by name), and
/// set the stop reason (any functionCall ⇒ ToolUse, else `MAX_TOKENS`→Length /
/// `STOP`→End). `finishReason: "MALFORMED_FUNCTION_CALL"` — Gemini's signal that a
/// tool call was truncated/cut off by the output-token limit — always wins and maps
/// to `Length` too, even if a (possibly half-serialized) functionCall part is
/// present, so those args never reach a tool handler. Pure + unit-tested.
pub(super) fn parse_gemini_turn(data: &Value) -> AgentTurn {
    let text = join_parts_text(data);
    let finish_reason = data
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("finishReason"))
        .and_then(|f| f.as_str());
    let tool_calls: Vec<ToolCall> = data
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .enumerate()
                .filter_map(|(i, part)| {
                    let fc = part.get("functionCall")?;
                    let name = fc.get("name").and_then(|n| n.as_str())?.to_string();
                    let args = fc.get("args").cloned().unwrap_or_else(|| json!({}));
                    Some(ToolCall {
                        id: format!("{name}-{i}"),
                        name,
                        args,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = if finish_reason == Some("MALFORMED_FUNCTION_CALL") {
        StopReason::Length
    } else if !tool_calls.is_empty() {
        // Gemini reports `finishReason: "STOP"` even when returning a functionCall,
        // so the presence of a call is the authoritative "wants tools back" signal.
        StopReason::ToolUse
    } else {
        match finish_reason {
            Some("MAX_TOKENS") => StopReason::Length,
            Some("STOP") => StopReason::End,
            _ => StopReason::Other,
        }
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
        usage: parse_gemini_usage(data).unwrap_or_default(),
    }
}

/// Extract `usageMetadata.{promptTokenCount,candidatesTokenCount}` — present
/// on the non-streaming `generateContent` response and on every
/// `streamGenerateContent` chunk once Gemini starts reporting it (each repeats
/// the running total for the whole response so far, so the LAST one is
/// authoritative — the shared loop already keeps the latest). `None` when
/// absent. Pure + unit-tested.
pub(super) fn parse_gemini_usage(data: &Value) -> Option<Usage> {
    let um = data.get("usageMetadata")?;
    Some(Usage {
        input_tokens: um
            .get("promptTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: um
            .get("candidatesTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        // `usageMetadata.thoughtsTokenCount` — the count that accompanies the
        // `thinkingConfig.includeThoughts` this adapter already requests (see
        // `body`). Absent for non-thinking models, hence `None`, not 0.
        thinking_tokens: um
            .get("thoughtsTokenCount")
            .and_then(|v| v.as_u64())
            // `try_from`, not `as` — see the openai adapter: a wrapping cast
            // turns an absurd count into a plausible one.
            .and_then(|v| u32::try_from(v).ok()),
    })
}

/// Extract real token usage from an `embedContent` response. Gemini's
/// embeddings endpoint does not document a `usageMetadata` field the way
/// `generateContent` does — defensively reads the same key name in case a
/// future/regional variant sends it, but degrades to zero (never fabricated)
/// when absent, exactly like every other "provider reports nothing" case in
/// this module. `output_tokens: 0` always — an embed call has no completion
/// tokens. Pure + unit-tested.
pub(super) fn parse_gemini_embed_usage(data: &Value) -> Usage {
    let input_tokens = data
        .get("usageMetadata")
        .and_then(|u| u.get("promptTokenCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    Usage {
        input_tokens,
        output_tokens: 0,
        thinking_tokens: None,
    }
}

/// Extract a Gemini chunk's streamed parts as `(is_thought, text)` pairs. 2.5
/// thinking models flag reasoning parts with `"thought": true`; the rest are
/// normal answer text. Pure + unit-tested so the streaming loop stays a thin
/// emitter.
pub(super) fn parse_gemini_parts(event: &Value) -> Vec<(bool, &str)> {
    event
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    let text = part.get("text").and_then(|t| t.as_str())?;
                    let thought = part
                        .get("thought")
                        .and_then(|t| t.as_bool())
                        .unwrap_or(false);
                    Some((thought, text))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Brace-depth scanner state carried across chunk boundaries while reading
/// Gemini's streamed JSON **array** for complete top-level objects. `pending`
/// holds the bytes of the current (possibly partial) object accumulated so far;
/// `depth`/`in_string`/`escape` track where we are inside it. Array punctuation
/// (`[`, `,`, `]`, whitespace) seen at depth 0 before an object's opening `{` is
/// dropped, exactly as the original inline scanner did via its `trim_start`
/// `starts_with('{')` guard.
#[derive(Debug, Default)]
pub(super) struct GeminiScanner {
    pub(super) pending: String,
    depth: i32,
    in_string: bool,
    escape: bool,
}

/// Feed the freshly-arrived chunk in `buf` through the [`GeminiScanner`], emitting
/// a [`StreamPiece`] per non-empty `parts[].text` of every complete top-level
/// object. The caller pushes the new chunk into `buf`; this consumes it entirely
/// (leaving `buf` empty) and stashes any partial trailing object in `state.pending`
/// for the next chunk. Gemini has no in-band done sentinel — the stream ends with
/// the HTTP body — so this never yields a `done` piece (the shared loop completes
/// on end-of-body).
///
/// Behavior matches the original inline char scanner: an object is recognized when
/// brace depth returns to 0 *and* the accumulated text trims to something starting
/// with `{`; on a successful parse the accumulator is cleared and scanning
/// continues with the rest of the chunk. Pure + unit-tested; this is Gemini's
/// `parse` closure, so its JSON-array framing lives here only.
pub(super) fn parse_gemini_frames(buf: &mut String, state: &mut GeminiScanner) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    let chunk = std::mem::take(buf);
    for ch in chunk.chars() {
        // Drop the JSON-array framing (`[`, `]`, `,`, whitespace) that appears at
        // depth 0 before an object's `{`; otherwise it pollutes `pending` and the
        // `starts_with('{')` guard never fires for `[{…}` / `,{…}`.
        if !state.in_string
            && state.depth == 0
            && state.pending.is_empty()
            && (matches!(ch, '[' | ']' | ',') || ch.is_whitespace())
        {
            continue;
        }
        if state.escape {
            state.escape = false;
            state.pending.push(ch);
            continue;
        }
        if ch == '\\' && state.in_string {
            state.escape = true;
            state.pending.push(ch);
            continue;
        }
        if ch == '"' {
            state.in_string = !state.in_string;
        }
        if !state.in_string {
            if ch == '{' {
                state.depth += 1;
            } else if ch == '}' {
                state.depth -= 1;
            }
        }
        state.pending.push(ch);

        if state.depth == 0
            && state.pending.trim_start().starts_with('{')
            && !state.pending.trim().is_empty()
        {
            if let Ok(event) = serde_json::from_str::<Value>(state.pending.trim()) {
                for (thought, text) in parse_gemini_parts(&event) {
                    if text.is_empty() {
                        continue;
                    }
                    out.push(if thought {
                        StreamPiece::thinking(text)
                    } else {
                        StreamPiece::text(text)
                    });
                }
                if let Some(usage) = parse_gemini_usage(&event) {
                    out.push(StreamPiece::usage(usage));
                }
            }
            state.pending.clear();
        }
    }
    out
}

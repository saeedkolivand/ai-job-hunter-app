//! Ollama `/api/chat` wire-format parsing: `done_reason` mapping, the
//! non-streaming turn/usage shapes, and the newline-delimited-JSON stream
//! frame drain. Split out of `ollama.rs` (R8 LOC cap) — a pure move.

use serde_json::{json, Value};

use super::super::stream::StreamPiece;
use super::super::{AgentTurn, StopReason, ToolCall, Usage};

/// Ollama's own `done_reason`, from the final `/api/chat` object of EITHER the
/// streaming or the non-streaming path, mapped to a [`StopReason`]. `None` when
/// the field is absent (any frame that isn't the final one).
///
/// Shared by [`parse_ollama_turn`] and [`parse_ollama_frames`] on purpose: the
/// two paths read the same field off the same object shape, and a second
/// hand-written copy of this mapping is exactly the duplicated-heuristic defect
/// this codebase keeps re-learning. Callers layer their own precedence on top
/// (a turn lets `Length` outrank a tool call); this only reports what Ollama
/// said.
pub(super) fn ollama_done_reason(data: &Value) -> Option<StopReason> {
    match data.get("done_reason").and_then(|r| r.as_str())? {
        "length" => Some(StopReason::Length),
        "stop" => Some(StopReason::End),
        // Open-typed on purpose — a future/unknown reason must not be silently
        // reported as a clean end.
        _ => Some(StopReason::Other),
    }
}

/// Parse a non-streaming `/api/chat` response into an [`AgentTurn`]:
/// `message.content` is the text, each `message.tool_calls[]` maps to a
/// [`ToolCall`] (Ollama returns `function.arguments` as an already-decoded JSON
/// object, and no call id — synthesize `name-index`), and `done_reason` maps the
/// stop (`length`→Length even with tool calls present — the arguments may be
/// truncated JSON, so length wins over the tool-call signal; else any tool call ⇒
/// ToolUse, else End). Pure + unit-tested.
pub(super) fn parse_ollama_turn(data: &Value) -> AgentTurn {
    let message = data.get("message");
    let text = message
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or_default()
        .to_string();
    let tool_calls: Vec<ToolCall> = message
        .and_then(|m| m.get("tool_calls"))
        .and_then(|c| c.as_array())
        .map(|calls| {
            calls
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    let func = c.get("function")?;
                    let name = func.get("name").and_then(|n| n.as_str())?.to_string();
                    let args = func.get("arguments").cloned().unwrap_or_else(|| json!({}));
                    Some(ToolCall {
                        id: format!("{name}-{i}"),
                        name,
                        args,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = if ollama_done_reason(data) == Some(StopReason::Length) {
        // A length-truncated turn's tool-call arguments may be truncated /
        // half-serialized JSON — length must win over the tool-call signal.
        StopReason::Length
    } else if !tool_calls.is_empty() {
        StopReason::ToolUse
    } else {
        StopReason::End
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
        usage: parse_ollama_usage(data).unwrap_or_default(),
    }
}

/// Extract `prompt_eval_count`/`eval_count` — Ollama's real input/output token
/// counts, present at the top level of both the non-streaming `/api/chat`
/// response and the final (`done: true`) streamed object. `None` when NEITHER
/// field is present (an absent/malformed response never fabricates a zero
/// that looks like a real reported value). Pure + unit-tested.
pub(super) fn parse_ollama_usage(data: &Value) -> Option<Usage> {
    if data.get("prompt_eval_count").is_none() && data.get("eval_count").is_none() {
        return None;
    }
    Some(Usage {
        input_tokens: data
            .get("prompt_eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: data.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        // Ollama reports no separate thinking count: `eval_count` covers the
        // thinking channel and the answer together. The renderer's live
        // thinking-vs-answer ratio is measured in CHARS off the stream, which
        // is not this unit and is deliberately not laundered into it.
        thinking_tokens: None,
    })
}

/// Drain complete newline-delimited JSON objects from the accumulated stream
/// buffer into [`StreamPiece`]s, leaving any partial trailing line for the next
/// chunk. Each object carries an optional `message.thinking` (structured
/// reasoning from DeepSeek-R1/Qwen3) and `message.content` answer text; the
/// object with `done: true` yields a terminal sentinel carrying the final content
/// delta. Pure + unit-tested; this is Ollama's `parse` closure, so its NDJSON
/// framing lives here only.
pub(super) fn parse_ollama_frames(buf: &mut String) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    // Walk by a `consumed` offset and `drain(..consumed)` once at the end, instead
    // of reallocating the whole tail per line (O(n²) on a big frame).
    let mut consumed = 0;
    while let Some(rel) = buf[consumed..].find('\n') {
        let nl = consumed + rel;
        let line = buf[consumed..nl].trim().to_string();
        consumed = nl + 1;
        if line.is_empty() {
            continue;
        }
        let event: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let message = event.get("message");
        let delta = message
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("");
        let thinking = message
            .and_then(|m| m.get("thinking"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let done = event.get("done").and_then(|d| d.as_bool()).unwrap_or(false);
        if !thinking.is_empty() {
            out.push(StreamPiece::thinking(thinking));
        }
        if done {
            // Final object — carry its content on the terminal sentinel so the
            // shared loop emits it then completes (matches the original framing).
            // Real token usage (`prompt_eval_count`/`eval_count`) lives on this
            // same final object, so it rides along on the sentinel piece too.
            buf.drain(..consumed);
            let mut sentinel = StreamPiece::done(delta);
            sentinel.usage = parse_ollama_usage(&event);
            sentinel.stop_reason = ollama_done_reason(&event);
            // `done_reason` rides on this same final object. Mapping it makes
            // `stream::finish`'s length diagnosis ("ran out of output budget
            // before producing any answer") work for LOCAL Ollama too, instead
            // of only for the openai-compatible providers — a local reasoning
            // model that burns its whole budget thinking is exactly the case
            // that produced an unexplained empty generation.
            out.push(sentinel);
            return out;
        }
        if !delta.is_empty() {
            out.push(StreamPiece::text(delta));
        }
    }
    // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
    buf.drain(..consumed);
    out
}

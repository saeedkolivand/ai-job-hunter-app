//! Anthropic wire-format parsing: turning a `/messages` response (or its SSE
//! stream) into this crate's [`AgentTurn`]/[`StreamPiece`]/[`Usage`] shapes.
//! Split out of `anthropic.rs` (R8 LOC cap — the file was already at the
//! ceiling before the structured-output work in this module added anything).
//! A CHILD module of `anthropic` (`#[path = "anthropic_wire.rs"] mod wire;`),
//! exactly like `anthropic_tests.rs`'s own split, so `use super::*` reaches
//! every private item `anthropic.rs` itself uses, and `anthropic.rs`'s own
//! `use wire::*;` re-export means no caller — production or test — changed.

use serde_json::{json, Value};

use super::*;

/// Concatenate every `type:"text"` block in an Anthropic Messages `content` array
/// into one string (web-search responses interleave `server_tool_use` /
/// `web_search_tool_result` blocks, which have no `text` field and are skipped).
/// Pure + unit-tested.
pub(super) fn join_text_blocks(data: &Value) -> String {
    data.get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Parse a non-streaming Anthropic Messages response into an [`AgentTurn`]:
/// concatenate the `type:"text"` blocks for the visible text, map every
/// `type:"tool_use"` block to a [`ToolCall`] (`id`, `name`, `input`→`args`), and
/// map `stop_reason` (`tool_use`→ToolUse, `end_turn`→End, `max_tokens`→Length,
/// else Other). Pure + unit-tested — this is the error-prone per-vendor shape, so
/// it lives here with no I/O.
pub(super) fn parse_anthropic_turn(data: &Value) -> AgentTurn {
    let text = join_text_blocks(data);
    let tool_calls = data
        .get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                .filter_map(|b| {
                    let name = b.get("name").and_then(|n| n.as_str())?.to_string();
                    Some(ToolCall {
                        id: b
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        name,
                        args: b.get("input").cloned().unwrap_or_else(|| json!({})),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = match data.get("stop_reason").and_then(|s| s.as_str()) {
        Some("tool_use") => StopReason::ToolUse,
        Some("end_turn") => StopReason::End,
        Some("max_tokens") => StopReason::Length,
        _ => StopReason::Other,
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
        usage: parse_anthropic_usage(data),
    }
}

/// Drain complete SSE lines from the accumulated stream buffer into
/// [`StreamPiece`]s. Anthropic emits paired `event:`/`data:` lines; we track the
/// most recent `event:` in `last_event` (carried across chunk boundaries by the
/// caller). `message_stop` (by event name or embedded `type`) yields a terminal
/// sentinel; `thinking_delta` / `text_delta` map to reasoning / answer pieces.
///
/// Real token usage (`crate::spend`) arrives split across two events:
/// `message_start` carries `message.usage.input_tokens` (once, at the top of
/// the stream) and each `message_delta` carries a running `usage.output_tokens`
/// total (the LAST one is authoritative). `usage` is caller-carried mutable
/// state (like `last_event`) so the two halves combine into one [`Usage`]; a
/// [`StreamPiece::usage`] piece is emitted whenever either half updates.
///
/// Pure + unit-tested; this is the OpenAI-style `parse` closure for Anthropic, so
/// its SSE framing lives here only.
pub(super) fn parse_anthropic_frames(
    buf: &mut String,
    last_event: &mut String,
    usage: &mut Usage,
) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    // Walk the buffer by a `consumed` offset and `drain(..consumed)` once at the end,
    // instead of reallocating the whole tail per line (O(n²) on a big frame).
    let mut consumed = 0;
    while let Some(rel) = buf[consumed..].find('\n') {
        let nl = consumed + rel;
        let line = buf[consumed..nl].trim().to_string();
        consumed = nl + 1;

        if let Some(event) = line.strip_prefix("event: ") {
            *last_event = event.trim().to_string();
            continue;
        }
        let data = match line.strip_prefix("data: ") {
            Some(d) => d.trim(),
            None => continue,
        };
        if last_event == "message_stop" || data.contains("\"type\":\"message_stop\"") {
            buf.drain(..consumed);
            out.push(StreamPiece::done(""));
            return out;
        }
        let event: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match last_event.as_str() {
            "message_start" => {
                if let Some(input) = event
                    .get("message")
                    .and_then(|m| m.get("usage"))
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|v| v.as_u64())
                {
                    usage.input_tokens = input as u32;
                    out.push(StreamPiece::usage(*usage));
                }
            }
            "message_delta" => {
                if let Some(output) = event
                    .get("usage")
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|v| v.as_u64())
                {
                    usage.output_tokens = output as u32;
                    out.push(StreamPiece::usage(*usage));
                }
            }
            _ => {}
        }
        let delta_obj = event.get("delta");
        let delta_type = delta_obj
            .and_then(|d| d.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        match delta_type {
            "thinking_delta" => {
                let thinking = delta_obj
                    .and_then(|d| d.get("thinking"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if !thinking.is_empty() {
                    out.push(StreamPiece::thinking(thinking));
                }
            }
            "text_delta" => {
                let text = delta_obj
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if !text.is_empty() {
                    out.push(StreamPiece::text(text));
                }
            }
            _ => {}
        }
    }
    // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
    buf.drain(..consumed);
    out
}

/// Extract `usage.{input_tokens,output_tokens}` from a non-streaming Anthropic
/// Messages response — always present on a successful response. Pure +
/// unit-tested.
pub(super) fn parse_anthropic_usage(data: &Value) -> Usage {
    let usage = data.get("usage");
    Usage {
        input_tokens: usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        // Anthropic does NOT report a separate thinking count — extended /
        // adaptive thinking tokens are billed and counted inside
        // `output_tokens`. `None` says exactly that; a 0 would claim the model
        // did no reasoning.
        thinking_tokens: None,
    }
}

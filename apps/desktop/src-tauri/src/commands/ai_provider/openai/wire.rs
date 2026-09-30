//! OpenAI Chat Completions wire-format parsing: the non-streaming turn/usage
//! shapes and the `data:`-prefixed SSE frame drain. Split out of `openai.rs`
//! (R8 LOC cap) — a pure move.

use serde_json::{json, Value};

use super::super::stream::StreamPiece;
use super::super::{AgentTurn, StopReason, ToolCall, Usage};

/// Concatenate the assistant text from a Responses API result. The `output`
/// array interleaves `web_search_call` items with the final `message`; we take
/// the `output_text` blocks of message items. Pure + unit-tested.
pub(super) fn join_responses_text(data: &Value) -> String {
    data.get("output")
        .and_then(|o| o.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|it| it.get("type").and_then(|t| t.as_str()) == Some("message"))
                .filter_map(|it| it.get("content").and_then(|c| c.as_array()))
                .flatten()
                .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Parse a non-streaming Chat Completions response into an [`AgentTurn`]:
/// `choices[0].message.content` is the text (may be null when tool calls are
/// present), each `choices[0].message.tool_calls[]` maps to a [`ToolCall`] (its
/// `function.arguments` is a JSON *string* — decoded here; malformed → `{}`), and
/// `finish_reason` maps to the stop reason (`tool_calls`→ToolUse, `stop`→End,
/// `length`→Length, else Other). Pure + unit-tested.
pub(super) fn parse_openai_turn(data: &Value) -> AgentTurn {
    let choice = data.get("choices").and_then(|c| c.get(0));
    let message = choice.and_then(|c| c.get("message"));
    let text = message
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string();
    let tool_calls = message
        .and_then(|m| m.get("tool_calls"))
        .and_then(|c| c.as_array())
        .map(|calls| {
            calls
                .iter()
                .filter_map(|c| {
                    let func = c.get("function")?;
                    let name = func.get("name").and_then(|n| n.as_str())?.to_string();
                    let args = func
                        .get("arguments")
                        .and_then(|a| a.as_str())
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .unwrap_or_else(|| json!({}));
                    Some(ToolCall {
                        id: c
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        name,
                        args,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = match choice
        .and_then(|c| c.get("finish_reason"))
        .and_then(|f| f.as_str())
    {
        Some("tool_calls") => StopReason::ToolUse,
        Some("stop") => StopReason::End,
        Some("length") => StopReason::Length,
        _ => StopReason::Other,
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
        usage: parse_openai_usage(data).unwrap_or_default(),
    }
}

/// Extract `usage.{prompt_tokens,completion_tokens}` from an OpenAI Chat
/// Completions response/chunk — always present on the non-streaming response,
/// and (with `stream_options.include_usage: true`, set by
/// `build_chat_stream_body`) on ONE extra streamed chunk carrying no delta,
/// emitted right before `[DONE]`. `None` on every other streamed chunk. Pure +
/// unit-tested.
pub(super) fn parse_openai_usage(data: &Value) -> Option<Usage> {
    let usage = data.get("usage")?;
    Some(Usage {
        input_tokens: usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        // `usage.completion_tokens_details.reasoning_tokens`. Current models
        // send the details object even when they did no reasoning, with a `0`
        // here — so a non-reasoning model records a measured zero, and `None`
        // means the field was genuinely absent (an older model, or an
        // openai-compatible gateway that omits it). Both are honest; neither is
        // invented. It is a SUBSET of `completion_tokens`, so it must not be
        // added to anything.
        thinking_tokens: usage
            .get("completion_tokens_details")
            .and_then(|d| d.get("reasoning_tokens"))
            .and_then(|v| v.as_u64())
            // `try_from`, not `as`: `as` WRAPS, so an absurd or hostile count
            // would land as a small plausible number in the spend ledger. An
            // unrepresentable count is no measurement at all.
            .and_then(|v| u32::try_from(v).ok()),
    })
}

/// Extract real token usage from an OpenAI `/embeddings` response:
/// `usage.prompt_tokens` (falling back to `usage.total_tokens`, which some
/// OpenAI-compatible servers send instead), and `output_tokens: 0` — an
/// embedding call has no completion tokens. Zero on both fields when `usage`
/// is entirely absent (never fabricated). Pure + unit-tested.
pub(super) fn parse_openai_embed_usage(data: &Value) -> Usage {
    let usage = data.get("usage");
    let input_tokens = usage
        .and_then(|u| u.get("prompt_tokens").or_else(|| u.get("total_tokens")))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    Usage {
        input_tokens,
        output_tokens: 0,
        // An embedding call does no reasoning; "not reported" is the truth.
        thinking_tokens: None,
    }
}

pub(super) fn parse_openai_delta(event: &Value) -> (&str, &str) {
    let delta = event
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));
    let reasoning = delta
        .and_then(|d| d.get("reasoning_content").or_else(|| d.get("reasoning")))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let content = delta
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    (reasoning, content)
}

/// Extract a streamed chunk's `finish_reason`, when present and non-null.
/// Most streamed chunks carry `finish_reason: null`; only the terminal
/// content-bearing chunk (typically right before `data: [DONE]`) sets it.
/// Ollama Cloud (routed through this same client, see `ollama_cloud.rs`) uses
/// the identical Chat Completions streaming shape. Reuses the SAME mapping
/// [`parse_openai_turn`] already uses for the non-streaming path, so callers
/// never need a second vocabulary. Pure + unit-tested.
pub(super) fn parse_openai_finish_reason(event: &Value) -> Option<StopReason> {
    let reason = event
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("finish_reason"))
        .and_then(|f| f.as_str())?;
    Some(match reason {
        "tool_calls" => StopReason::ToolUse,
        "stop" => StopReason::End,
        "length" => StopReason::Length,
        _ => StopReason::Other,
    })
}

/// Drain complete `data:`-prefixed SSE lines from the accumulated stream buffer
/// into [`StreamPiece`]s, leaving any partial trailing line for the next chunk.
/// `data: [DONE]` yields a terminal sentinel; other lines split into reasoning +
/// content via [`parse_openai_delta`], plus a `stop_reason` piece whenever a
/// chunk carries a non-null `finish_reason` (see
/// [`parse_openai_finish_reason`]). Pure + unit-tested; this is the `parse`
/// closure handed to `stream_response`, so OpenAI's SSE framing lives here only.
pub(super) fn parse_openai_frames(buf: &mut String) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    // Walk by a `consumed` offset and `drain(..consumed)` once at the end, instead
    // of reallocating the whole tail per line (O(n²) on a big frame).
    let mut consumed = 0;
    while let Some(rel) = buf[consumed..].find('\n') {
        let nl = consumed + rel;
        let line = buf[consumed..nl].trim().to_string();
        consumed = nl + 1;

        let data = match line.strip_prefix("data: ") {
            Some(d) => d.trim(),
            None => continue,
        };
        if data == "[DONE]" {
            buf.drain(..consumed);
            out.push(StreamPiece::done(""));
            return out;
        }
        let event: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(usage) = parse_openai_usage(&event) {
            out.push(StreamPiece::usage(usage));
        }
        if let Some(reason) = parse_openai_finish_reason(&event) {
            out.push(StreamPiece::stop_reason(reason));
        }
        let (reasoning, delta) = parse_openai_delta(&event);
        if !reasoning.is_empty() {
            out.push(StreamPiece::thinking(reasoning));
        }
        if !delta.is_empty() {
            out.push(StreamPiece::text(delta));
        }
    }
    // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
    buf.drain(..consumed);
    out
}

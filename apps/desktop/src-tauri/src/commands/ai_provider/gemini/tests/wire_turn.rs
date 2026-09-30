//! Non-streaming turn/usage parsing: `parse_gemini_turn`,
//! `parse_gemini_usage`, `parse_gemini_embed_usage`, `join_parts_text`.

use serde_json::json;

use super::super::super::{StopReason, ToolCall};
use super::super::wire::{
    join_parts_text, parse_gemini_embed_usage, parse_gemini_turn, parse_gemini_usage,
};

#[test]
fn join_parts_text_concatenates_first_candidate_parts() {
    let data = json!({
        "candidates": [{
            "content": { "parts": [{ "text": "Acme is " }, { "text": "a widget maker." }] },
            "groundingMetadata": { "webSearchQueries": ["Acme"] }
        }]
    });
    assert_eq!(join_parts_text(&data), "Acme is a widget maker.");
    assert_eq!(join_parts_text(&json!({})), "");
    assert_eq!(join_parts_text(&json!({ "candidates": [] })), "");
}

#[test]
fn parse_usage_reads_prompt_and_candidates_token_counts() {
    let data = json!({ "usageMetadata": { "promptTokenCount": 55, "candidatesTokenCount": 22, "totalTokenCount": 77 } });
    let usage = parse_gemini_usage(&data).expect("usage present");
    assert_eq!(usage.input_tokens, 55);
    assert_eq!(usage.output_tokens, 22);
}

#[test]
fn parse_usage_is_none_when_absent() {
    assert!(parse_gemini_usage(&json!({})).is_none());
}

/// Sibling of the OpenAI adapter's guard: a `thoughtsTokenCount` too large for
/// `u32` is no measurement, not a small one — `as u32` would wrap it into a
/// plausible ledger entry.
///
/// Mutation check (executed): restore `.map(|v| v as u32)` and the oversized
/// case records `Some(8)`.
#[test]
fn an_unrepresentable_thoughts_count_is_not_recorded() {
    let with_thoughts = |v: serde_json::Value| {
        json!({
            "usageMetadata": {
                "promptTokenCount": 1,
                "candidatesTokenCount": 2,
                "thoughtsTokenCount": v,
            }
        })
    };

    let over =
        parse_gemini_usage(&with_thoughts(json!(u32::MAX as u64 + 9))).expect("usage present");
    assert_eq!(over.thinking_tokens, None);

    let ok = parse_gemini_usage(&with_thoughts(json!(2_048))).expect("usage present");
    assert_eq!(ok.thinking_tokens, Some(2_048));
}

#[test]
fn parse_embed_usage_reads_prompt_token_count_when_present() {
    let data = json!({ "usageMetadata": { "promptTokenCount": 7 } });
    let usage = parse_gemini_embed_usage(&data);
    assert_eq!(usage.input_tokens, 7);
    assert_eq!(usage.output_tokens, 0);
}

#[test]
fn parse_embed_usage_zero_when_absent() {
    // Gemini's embedContent response typically carries no usageMetadata —
    // must degrade to zero, never fabricate a token count.
    let usage = parse_gemini_embed_usage(&json!({ "embedding": { "values": [0.1] } }));
    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
}

#[test]
fn parse_turn_extracts_function_calls_alongside_text() {
    // Gemini reports finishReason "STOP" even when it emits a functionCall — the
    // call's presence, not the finishReason, is the "wants tools back" signal.
    let data = json!({
        "candidates": [{
            "content": { "parts": [
                { "text": "Looking up the company." },
                { "functionCall": { "name": "research_company", "args": { "company": "Acme" } } }
            ] },
            "finishReason": "STOP"
        }]
    });
    let turn = parse_gemini_turn(&data);
    assert_eq!(turn.text, "Looking up the company.");
    assert_eq!(turn.stop, StopReason::ToolUse);
    assert_eq!(
        turn.tool_calls,
        vec![ToolCall {
            id: "research_company-1".to_string(),
            name: "research_company".to_string(),
            args: json!({ "company": "Acme" }),
        }]
    );
}

#[test]
fn parse_turn_plain_answer_maps_stop_reason() {
    let data = json!({
        "candidates": [{
            "content": { "parts": [{ "text": "Final answer." }] },
            "finishReason": "STOP"
        }]
    });
    let turn = parse_gemini_turn(&data);
    assert_eq!(turn.text, "Final answer.");
    assert!(turn.tool_calls.is_empty());
    assert_eq!(turn.stop, StopReason::End);

    let truncated = json!({
        "candidates": [{ "content": { "parts": [{ "text": "..." }] }, "finishReason": "MAX_TOKENS" }]
    });
    assert_eq!(parse_gemini_turn(&truncated).stop, StopReason::Length);
}

#[test]
fn parse_turn_malformed_function_call_maps_to_length_not_tool_use() {
    // A tool call truncated by the output-token limit comes back with
    // `finishReason: "MALFORMED_FUNCTION_CALL"` (NOT `MAX_TOKENS`) — it must
    // route through the same non-executable/truncated path as `MAX_TOKENS`, so
    // the (possibly half-serialized) args never reach a tool handler.
    let data = json!({
        "candidates": [{
            "content": { "parts": [
                { "functionCall": { "name": "research_company", "args": { "company": "Ac" } } }
            ] },
            "finishReason": "MALFORMED_FUNCTION_CALL"
        }]
    });
    let turn = parse_gemini_turn(&data);
    assert_eq!(turn.stop, StopReason::Length);
}

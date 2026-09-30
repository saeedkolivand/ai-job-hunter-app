//! Non-streaming turn/usage parsing: `parse_openai_turn`,
//! `parse_openai_usage`, `parse_openai_embed_usage`, `join_responses_text`.

use serde_json::json;

use super::super::super::{StopReason, ToolCall};
use super::super::wire::{
    join_responses_text, parse_openai_embed_usage, parse_openai_turn, parse_openai_usage,
};

#[test]
fn parse_usage_extracts_real_token_counts() {
    let data = json!({ "usage": { "prompt_tokens": 42, "completion_tokens": 17 } });
    let usage = parse_openai_usage(&data).expect("usage present");
    assert_eq!(usage.input_tokens, 42);
    assert_eq!(usage.output_tokens, 17);
}

/// A reasoning count that cannot fit in `u32` is NO measurement, not a small
/// one. `as u32` wraps, so `2^32 + 7` would be recorded as `7` — a plausible
/// number in the spend ledger, sourced from a provider response.
///
/// Mutation check (executed): restore `.map(|v| v as u32)` and the oversized
/// case records `Some(7)`.
#[test]
fn an_unrepresentable_reasoning_count_is_not_recorded() {
    let with_reasoning = |v: serde_json::Value| {
        json!({
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 2,
                "completion_tokens_details": { "reasoning_tokens": v },
            }
        })
    };

    let over =
        parse_openai_usage(&with_reasoning(json!(u32::MAX as u64 + 8))).expect("usage present");
    assert_eq!(
        over.thinking_tokens, None,
        "an oversized count must not wrap into a plausible one"
    );

    // The ordinary counts still land, including the measured zero.
    let ok = parse_openai_usage(&with_reasoning(json!(1_024))).expect("usage present");
    assert_eq!(ok.thinking_tokens, Some(1_024));
    let zero = parse_openai_usage(&with_reasoning(json!(0))).expect("usage present");
    assert_eq!(zero.thinking_tokens, Some(0));
}

#[test]
fn parse_usage_is_none_when_absent() {
    // Every streamed chunk except the final one has no `usage` field.
    assert!(parse_openai_usage(&json!({ "choices": [] })).is_none());
    assert!(parse_openai_usage(&json!({})).is_none());
}

#[test]
fn parse_embed_usage_prefers_prompt_tokens() {
    let data = json!({ "usage": { "prompt_tokens": 12, "total_tokens": 12 } });
    let usage = parse_openai_embed_usage(&data);
    assert_eq!(usage.input_tokens, 12);
    assert_eq!(usage.output_tokens, 0, "an embed call has no output tokens");
}

#[test]
fn parse_embed_usage_falls_back_to_total_tokens() {
    // Some OpenAI-compatible embed servers send only `total_tokens`.
    let data = json!({ "usage": { "total_tokens": 9 } });
    assert_eq!(parse_openai_embed_usage(&data).input_tokens, 9);
}

#[test]
fn parse_embed_usage_zero_when_absent() {
    let usage = parse_openai_embed_usage(&json!({}));
    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
}

#[test]
fn join_responses_text_takes_message_items_only() {
    // The Responses `output` array interleaves the web_search_call with the
    // final assistant message.
    let data = json!({
        "output": [
            { "type": "web_search_call", "id": "ws_1", "status": "completed" },
            { "type": "message", "role": "assistant", "content": [
                { "type": "output_text", "text": "Acme is a ", "annotations": [] },
                { "type": "output_text", "text": "widget maker.", "annotations": [] }
            ]}
        ]
    });
    assert_eq!(join_responses_text(&data), "Acme is a widget maker.");
    assert_eq!(join_responses_text(&json!({})), "");
    assert_eq!(join_responses_text(&json!({ "output": [] })), "");
}

#[test]
fn parse_turn_decodes_tool_calls_with_stringified_arguments() {
    // Chat Completions puts function args in a JSON *string* — it must be decoded.
    let data = json!({
        "choices": [{
            "message": {
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": { "name": "match_resume", "arguments": "{\"resumeId\":\"r1\",\"jobId\":\"j1\"}" }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    });
    let turn = parse_openai_turn(&data);
    assert_eq!(turn.text, "");
    assert_eq!(turn.stop, StopReason::ToolUse);
    assert_eq!(
        turn.tool_calls,
        vec![ToolCall {
            id: "call_1".to_string(),
            name: "match_resume".to_string(),
            args: json!({ "resumeId": "r1", "jobId": "j1" }),
        }]
    );
}

#[test]
fn parse_turn_plain_answer_has_no_tool_calls() {
    let data = json!({
        "choices": [{ "message": { "content": "Here is the answer." }, "finish_reason": "stop" }]
    });
    let turn = parse_openai_turn(&data);
    assert_eq!(turn.text, "Here is the answer.");
    assert!(turn.tool_calls.is_empty());
    assert_eq!(turn.stop, StopReason::End);
}

#[test]
fn parse_turn_malformed_arguments_degrade_to_empty_object() {
    // A truncated/invalid arguments string must not error the whole turn.
    let data = json!({
        "choices": [{
            "message": { "tool_calls": [{ "id": "c", "function": { "name": "f", "arguments": "{not json" } }] },
            "finish_reason": "tool_calls"
        }]
    });
    let turn = parse_openai_turn(&data);
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].args, json!({}));
}

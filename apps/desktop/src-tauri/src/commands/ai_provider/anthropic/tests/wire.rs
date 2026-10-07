//! Anthropic Messages wire-format parsing: SSE frame draining, usage
//! extraction, and non-streaming turn parsing.

use serde_json::json;

use super::super::super::stream::StreamPiece;
use super::super::super::{StopReason, ToolCall, Usage};
use super::super::wire::{
    join_text_blocks, parse_anthropic_frames, parse_anthropic_turn, parse_anthropic_usage,
};

#[test]
fn parse_frames_splits_thinking_and_text_deltas() {
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from(
        "event: content_block_delta\n\
         data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hmm\"}}\n\
         event: content_block_delta\n\
         data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n",
    );
    let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
    assert_eq!(
        pieces,
        vec![StreamPiece::thinking("hmm"), StreamPiece::text("hi")]
    );
}

#[test]
fn parse_frames_done_on_message_stop_event() {
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from("event: message_stop\ndata: {\"type\":\"message_stop\"}\n");
    assert_eq!(
        parse_anthropic_frames(&mut buf, &mut last, &mut usage),
        vec![StreamPiece::done("")]
    );
}

#[test]
fn parse_frames_done_when_event_line_split_across_chunks() {
    // The `event:` line arrives in one chunk, the `data:` in the next — the
    // caller carries `last_event`, so message_stop is still detected.
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from("event: message_stop\n");
    assert!(parse_anthropic_frames(&mut buf, &mut last, &mut usage).is_empty());
    assert_eq!(last, "message_stop");
    buf.push_str("data: {}\n");
    assert_eq!(
        parse_anthropic_frames(&mut buf, &mut last, &mut usage),
        vec![StreamPiece::done("")]
    );
}

#[test]
fn parse_frames_leaves_partial_trailing_line_buffered() {
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from("data: {\"type\":\"content_block_de");
    assert!(parse_anthropic_frames(&mut buf, &mut last, &mut usage).is_empty());
    assert_eq!(buf, "data: {\"type\":\"content_block_de");
}

#[test]
fn parse_frames_drains_consumed_lines_keeping_partial_tail() {
    // The in-place `drain(..consumed)` must drop exactly the fully-parsed lines
    // (incl. a multi-byte char before the newline) and keep the partial tail —
    // the offset arithmetic stays on char boundaries.
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from(
        "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"café\"}}\n\
         data: {\"type\":\"content_block_de",
    );
    let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
    assert_eq!(pieces, vec![StreamPiece::text("café")]);
    // Only the unterminated trailing line survives the drain.
    assert_eq!(buf, "data: {\"type\":\"content_block_de");
}

#[test]
fn parse_frames_combines_message_start_input_and_message_delta_output_tokens() {
    let mut last = String::new();
    let mut usage = Usage::default();
    let mut buf = String::from(
        "event: message_start\n\
         data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25,\"output_tokens\":1}}}\n",
    );
    let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
    assert_eq!(
        pieces,
        vec![StreamPiece::usage(Usage {
            input_tokens: 25,
            output_tokens: 0,
            thinking_tokens: None,
        })]
    );

    buf.push_str(
        "event: message_delta\n\
         data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":91}}\n",
    );
    let pieces = parse_anthropic_frames(&mut buf, &mut last, &mut usage);
    assert_eq!(
        pieces[..1],
        vec![StreamPiece::usage(Usage {
            input_tokens: 25,
            output_tokens: 91,
            thinking_tokens: None,
        })]
    );
    // The same `message_delta` also reports why the turn ended (terminal proof).
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[1], StreamPiece::stop_reason(StopReason::End));
}

#[test]
fn parse_usage_reads_a_non_streaming_response() {
    let data = json!({ "usage": { "input_tokens": 12, "output_tokens": 34 } });
    let usage = parse_anthropic_usage(&data);
    assert_eq!(usage.input_tokens, 12);
    assert_eq!(usage.output_tokens, 34);
}

#[test]
fn parse_usage_defaults_to_zero_when_absent() {
    let usage = parse_anthropic_usage(&json!({}));
    assert_eq!(usage, Usage::default());
}

#[test]
fn join_text_blocks_concatenates_only_text_blocks() {
    // Web-search responses interleave tool blocks among the text blocks.
    let data = json!({
        "content": [
            { "type": "text", "text": "Acme is a " },
            { "type": "server_tool_use", "name": "web_search", "input": { "query": "Acme" } },
            { "type": "web_search_tool_result", "content": [{ "url": "x", "title": "y" }] },
            { "type": "text", "text": "widget maker." }
        ]
    });
    assert_eq!(join_text_blocks(&data), "Acme is a widget maker.");
}

#[test]
fn join_text_blocks_empty_on_missing_or_error() {
    assert_eq!(join_text_blocks(&json!({})), "");
    assert_eq!(join_text_blocks(&json!({ "content": [] })), "");
}

#[test]
fn parse_turn_extracts_text_and_tool_use_blocks() {
    // Assistant text interleaved with a `tool_use` block; stop_reason=tool_use.
    let data = json!({
        "content": [
            { "type": "text", "text": "Let me look that up." },
            { "type": "tool_use", "id": "toolu_1", "name": "research_company",
              "input": { "company": "Acme", "jobAd": "..." } }
        ],
        "stop_reason": "tool_use"
    });
    let turn = parse_anthropic_turn(&data);
    assert_eq!(turn.text, "Let me look that up.");
    assert_eq!(turn.stop, StopReason::ToolUse);
    assert_eq!(
        turn.tool_calls,
        vec![ToolCall {
            id: "toolu_1".to_string(),
            name: "research_company".to_string(),
            args: json!({ "company": "Acme", "jobAd": "..." }),
        }]
    );
}

#[test]
fn parse_turn_no_tool_calls_is_a_plain_end_turn() {
    let data = json!({
        "content": [{ "type": "text", "text": "All done." }],
        "stop_reason": "end_turn"
    });
    let turn = parse_anthropic_turn(&data);
    assert_eq!(turn.text, "All done.");
    assert!(turn.tool_calls.is_empty());
    assert_eq!(turn.stop, StopReason::End);
}

#[test]
fn parse_turn_maps_max_tokens_and_missing_input() {
    // `max_tokens` → Length; a tool_use with no `input` still parses (args = {}).
    let data = json!({
        "content": [{ "type": "tool_use", "id": "t", "name": "match_resume" }],
        "stop_reason": "max_tokens"
    });
    let turn = parse_anthropic_turn(&data);
    assert_eq!(turn.stop, StopReason::Length);
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].args, json!({}));
}

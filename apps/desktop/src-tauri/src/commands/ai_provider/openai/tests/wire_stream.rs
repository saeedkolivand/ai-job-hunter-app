//! Streaming SSE parsing: `parse_openai_delta`, `parse_openai_finish_reason`,
//! `parse_openai_frames`.

use serde_json::json;

use super::super::super::stream::StreamPiece;
use super::super::super::StopReason;
use super::super::wire::{parse_openai_delta, parse_openai_finish_reason, parse_openai_frames};

#[test]
fn parse_delta_splits_reasoning_from_content() {
    // DeepSeek-R1 / vLLM style: reasoning on `reasoning_content`.
    let ev = json!({ "choices": [{ "delta": { "reasoning_content": "let me think" } }] });
    assert_eq!(parse_openai_delta(&ev), ("let me think", ""));

    // OpenRouter style: reasoning on `reasoning`.
    let ev = json!({ "choices": [{ "delta": { "reasoning": "pondering" } }] });
    assert_eq!(parse_openai_delta(&ev), ("pondering", ""));

    // Normal answer content.
    let ev = json!({ "choices": [{ "delta": { "content": "the answer" } }] });
    assert_eq!(parse_openai_delta(&ev), ("", "the answer"));
}

#[test]
fn parse_delta_empty_when_no_choices_or_fields() {
    assert_eq!(parse_openai_delta(&json!({})), ("", ""));
    assert_eq!(
        parse_openai_delta(&json!({ "choices": [{ "delta": {} }] })),
        ("", "")
    );
}

#[test]
fn parse_frames_splits_sse_lines_into_pieces() {
    // Two complete data lines (reasoning then content) + a partial trailing line.
    let mut buf = String::from(
        "data: {\"choices\":[{\"delta\":{\"reasoning\":\"think\"}}]}\n\
         data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\
         data: {\"choices\":[{\"delta\":{\"con",
    );
    let pieces = parse_openai_frames(&mut buf);
    assert_eq!(
        pieces,
        vec![StreamPiece::thinking("think"), StreamPiece::text("hello")]
    );
    // The incomplete final line is left buffered for the next chunk.
    assert!(buf.starts_with("data: {\"choices\""));
    assert!(!buf.contains('\n'));
}

#[test]
fn parse_frames_emits_done_sentinel_on_done_marker() {
    let mut buf = String::from(
        "data: {\"choices\":[{\"delta\":{\"content\":\"last\"}}]}\n\
         data: [DONE]\n",
    );
    let pieces = parse_openai_frames(&mut buf);
    assert_eq!(
        pieces,
        vec![StreamPiece::text("last"), StreamPiece::done("")]
    );
}

#[test]
fn parse_frames_skips_non_data_and_unparseable_lines() {
    // Comment/keepalive lines and malformed JSON are ignored, not errors.
    let mut buf = String::from(": keepalive\ndata: not-json\n\n");
    assert!(parse_openai_frames(&mut buf).is_empty());
}

// ── finish_reason (streaming) — HIGH: distinguishes a truncated-mid-reasoning
// empty answer (finish_reason: length) from a provider that silently returned
// nothing, so `finish`'s empty-answer branch can report the right one. ───────

#[test]
fn parse_finish_reason_maps_every_known_value_like_the_non_streaming_turn_parser() {
    let ev = |reason: &str| json!({ "choices": [{ "delta": {}, "finish_reason": reason }] });
    assert_eq!(
        parse_openai_finish_reason(&ev("length")),
        Some(StopReason::Length)
    );
    assert_eq!(
        parse_openai_finish_reason(&ev("stop")),
        Some(StopReason::End)
    );
    assert_eq!(
        parse_openai_finish_reason(&ev("tool_calls")),
        Some(StopReason::ToolUse)
    );
    assert_eq!(
        parse_openai_finish_reason(&ev("content_filter")),
        Some(StopReason::Other)
    );
}

#[test]
fn parse_finish_reason_is_none_for_a_null_or_absent_value() {
    // The common case: every streamed chunk except (usually) the last one.
    assert_eq!(
        parse_openai_finish_reason(&json!({ "choices": [{ "delta": {}, "finish_reason": null }] })),
        None
    );
    assert_eq!(
        parse_openai_finish_reason(&json!({ "choices": [{ "delta": {} }] })),
        None
    );
    assert_eq!(parse_openai_finish_reason(&json!({})), None);
}

#[test]
fn parse_frames_emits_a_stop_reason_piece_when_a_chunk_carries_finish_reason() {
    let mut buf = String::from(
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\
         data: [DONE]\n",
    );
    let pieces = parse_openai_frames(&mut buf);
    assert_eq!(
        pieces,
        vec![
            StreamPiece::stop_reason(StopReason::Length),
            StreamPiece::done(""),
        ]
    );
}

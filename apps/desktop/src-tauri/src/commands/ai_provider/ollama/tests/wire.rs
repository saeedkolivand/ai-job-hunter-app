//! Ollama `/api/chat` wire-format parsing: the NDJSON stream frame drain,
//! usage extraction, and non-streaming turn parsing.

use serde_json::json;

use super::super::super::stream::StreamPiece;
use super::super::super::{StopReason, ToolCall};
use super::super::wire::{parse_ollama_frames, parse_ollama_turn, parse_ollama_usage};

#[test]
fn parse_ollama_frames_splits_thinking_and_content() {
    let mut buf = String::from(
        "{\"message\":{\"thinking\":\"hmm\"},\"done\":false}\n\
         {\"message\":{\"content\":\"hello\"},\"done\":false}\n",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(
        pieces,
        vec![StreamPiece::thinking("hmm"), StreamPiece::text("hello")]
    );
    assert!(buf.is_empty());
}

#[test]
fn parse_ollama_frames_done_carries_final_content() {
    // The `done:true` object becomes a sentinel carrying its final content.
    let mut buf = String::from("{\"message\":{\"content\":\"end\"},\"done\":true}\n");
    assert_eq!(
        parse_ollama_frames(&mut buf),
        vec![StreamPiece::done("end")]
    );
}

#[test]
fn parse_ollama_frames_done_carries_real_token_usage() {
    let mut buf = String::from(
        "{\"message\":{\"content\":\"end\"},\"done\":true,\"prompt_eval_count\":123,\"eval_count\":45}\n",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces.len(), 1);
    let usage = pieces[0].usage.expect("done piece must carry usage");
    assert_eq!(usage.input_tokens, 123);
    assert_eq!(usage.output_tokens, 45);
}

/// The streamed sentinel must carry `done_reason` too, not just usage. Without
/// it `stream::finish` can't tell "the local model burned its whole budget
/// reasoning and never answered" from "the provider silently returned nothing" —
/// the exact ambiguity that made an empty generation unexplainable.
#[test]
fn parse_ollama_frames_done_carries_the_stop_reason() {
    let mut buf = String::from(
        "{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"length\"}
",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].stop_reason, Some(StopReason::Length));
}

#[test]
fn parse_ollama_frames_done_reports_a_clean_stop_as_end_not_length() {
    // The differential: a normal completion must NOT be reported as truncated,
    // or every finished generation would claim it ran out of budget.
    let mut buf = String::from(
        "{\"message\":{\"content\":\"hi\"},\"done\":true,\"done_reason\":\"stop\"}
",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces[0].stop_reason, Some(StopReason::End));
}

#[test]
fn parse_ollama_frames_done_without_a_reason_reports_none() {
    // Older/leaner Ollama builds omit the field; absent must stay `None` rather
    // than being invented as a clean end.
    let mut buf = String::from(
        "{\"message\":{\"content\":\"hi\"},\"done\":true}
",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces[0].stop_reason, None);
}

#[test]
fn parse_usage_reads_prompt_eval_and_eval_counts() {
    let data = json!({ "prompt_eval_count": 10, "eval_count": 20 });
    let usage = parse_ollama_usage(&data).expect("usage present");
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 20);
}

#[test]
fn parse_usage_is_none_when_absent() {
    assert!(parse_ollama_usage(&json!({})).is_none());
}

#[test]
fn parse_ollama_frames_buffers_partial_line() {
    // A partial trailing JSON line is left for the next chunk.
    let mut buf = String::from("{\"message\":{\"content\":\"hi\"},\"done\":false}\n{\"mess");
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces, vec![StreamPiece::text("hi")]);
    assert_eq!(buf, "{\"mess");
}

#[test]
fn parse_ollama_frames_skips_blank_and_unparseable_lines() {
    let mut buf = String::from("\nnot-json\n");
    assert!(parse_ollama_frames(&mut buf).is_empty());
}

#[test]
fn parse_turn_reads_object_arguments_and_content() {
    // Ollama returns arguments as an already-decoded object (NOT a JSON string).
    let data = json!({
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{ "function": { "name": "match_resume", "arguments": { "resumeId": "r1", "jobId": "j1" } } }]
        },
        "done": true,
        "done_reason": "stop"
    });
    let turn = parse_ollama_turn(&data);
    assert_eq!(turn.stop, StopReason::ToolUse);
    assert_eq!(
        turn.tool_calls,
        vec![ToolCall {
            id: "match_resume-0".to_string(),
            name: "match_resume".to_string(),
            args: json!({ "resumeId": "r1", "jobId": "j1" }),
        }]
    );
}

#[test]
fn parse_turn_plain_answer_has_no_tool_calls() {
    let data = json!({
        "message": { "role": "assistant", "content": "The answer." },
        "done": true,
        "done_reason": "stop"
    });
    let turn = parse_ollama_turn(&data);
    assert_eq!(turn.text, "The answer.");
    assert!(turn.tool_calls.is_empty());
    assert_eq!(turn.stop, StopReason::End);
}

#[test]
fn parse_turn_tool_calls_with_length_done_reason_maps_to_length_not_tool_use() {
    // `done_reason: "length"` means the arguments may be truncated JSON — this
    // must win over the tool-call signal, never `ToolUse`.
    let data = json!({
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{ "function": { "name": "match_resume", "arguments": { "resumeId": "r1" } } }]
        },
        "done": true,
        "done_reason": "length"
    });
    let turn = parse_ollama_turn(&data);
    assert_eq!(turn.stop, StopReason::Length);
}

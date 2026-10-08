//! `stream::collect` driven through the REAL `parse_openai_frames` (#1353).

use super::super::super::stream::collect_canned;
use super::super::super::Usage;
use super::super::wire::parse_openai_frames;
use crate::error::AppError;

const HELLO: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"a\\\":1}\"}}]}\n";
const STOP: &str = "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n";
const LENGTH: &str = "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n";
const USAGE: &str =
    "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":3}}\n";
const DONE: &str = "data: [DONE]\n";

#[tokio::test]
async fn assembles_text_and_usage_up_to_done() {
    let (text, usage) = collect_canned(&[HELLO, STOP, USAGE, DONE], parse_openai_frames, true)
        .await
        .unwrap();
    assert_eq!(text, "{\"a\":1}");
    assert_eq!((usage.input_tokens, usage.output_tokens), (7, 3));
}

#[tokio::test]
async fn a_null_usage_chunk_is_not_a_measured_zero() {
    let null = "data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}],\"usage\":null}\n";
    let (_, usage) = collect_canned(&[null, STOP, DONE], parse_openai_frames, false)
        .await
        .unwrap();
    assert_eq!(usage, Usage::default());
}

#[tokio::test]
async fn an_error_frame_fails_with_the_upstream_message() {
    let frame = "data: {\"error\":{\"message\":\"maximum context length is 8192 tokens\",\"type\":\"invalid_request_error\"}}\n";
    let err = collect_canned(&[HELLO, frame], parse_openai_frames, true)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("maximum context length")),
        "{err:?}"
    );
}

#[tokio::test]
async fn eof_without_a_terminal_piece_fails() {
    let err = collect_canned(&[HELLO], parse_openai_frames, true)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Network(m) if m.contains("ended before")),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_empty_answer_fails_and_a_refusal_is_quoted() {
    let refusal = "data: {\"choices\":[{\"delta\":{\"refusal\":\"I cannot help with that\"}}]}\n";
    let err = collect_canned(&[refusal, STOP, DONE], parse_openai_frames, true)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Refusal(m) if m.contains("I cannot help with that")),
        "{err:?}"
    );
    let err = collect_canned(&[STOP, DONE], parse_openai_frames, false)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("unexpected response shape")),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_length_stop_fails_json_calls_but_plain_text_keeps_the_partial() {
    let chunks = [HELLO, LENGTH, DONE];
    let err = collect_canned(&chunks, parse_openai_frames, true)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::OutputLimit(m) if m.contains("cut off")),
        "{err:?}"
    );
    let (text, _) = collect_canned(&chunks, parse_openai_frames, false)
        .await
        .unwrap();
    assert_eq!(text, "{\"a\":1}");
}

#[tokio::test]
async fn an_unbounded_line_is_capped() {
    let huge = "x".repeat(crate::net::http::DEFAULT_MAX_BODY_BYTES + 1);
    let err = collect_canned(&[&huge], parse_openai_frames, false)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("size limit")),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_oversized_answer_is_capped() {
    let big = "y".repeat(crate::net::http::DEFAULT_MAX_BODY_BYTES / 2 + 1);
    let frame = format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{big}\"}}}}]}}\n");
    let err = collect_canned(&[&frame, &frame], parse_openai_frames, false)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("size limit")),
        "{err:?}"
    );
}

/// Refusal text accumulates outside the answer, so it must count toward the cap too.
#[tokio::test]
async fn an_oversized_refusal_is_capped() {
    let big = "y".repeat(crate::net::http::DEFAULT_MAX_BODY_BYTES / 2 + 1);
    let frame = format!("data: {{\"choices\":[{{\"delta\":{{\"refusal\":\"{big}\"}}}}]}}\n");
    let err = collect_canned(&[&frame, &frame], parse_openai_frames, false)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("size limit")),
        "{err:?}"
    );
}

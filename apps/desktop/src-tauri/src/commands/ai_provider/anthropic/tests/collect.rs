//! `stream::collect` driven through the REAL `parse_anthropic_frames` (#1353).

use super::super::super::stream::collect_canned;
use super::super::super::Usage;
use super::super::wire::parse_anthropic_frames;
use crate::error::{AppError, AppResult};

const START: &str = "event: message_start\ndata: {\"message\":{\"usage\":{\"input_tokens\":11}}}\n";
const TEXT: &str = "event: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\"{\\\"a\\\":1}\"}}\n";
const END: &str = "event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":4}}\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n";
const CUT: &str = "event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"max_tokens\"},\"usage\":{\"output_tokens\":4}}\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n";

async fn run(chunks: &[&str], json: bool) -> AppResult<(String, Usage)> {
    let (mut event, mut usage) = (String::new(), Usage::default());
    collect_canned(
        chunks,
        move |b| parse_anthropic_frames(b, &mut event, &mut usage),
        json,
    )
    .await
}

#[tokio::test]
async fn assembles_text_and_both_usage_halves() {
    let (text, usage) = run(&[START, TEXT, END], true).await.unwrap();
    assert_eq!(text, "{\"a\":1}");
    assert_eq!((usage.input_tokens, usage.output_tokens), (11, 4));
}

#[tokio::test]
async fn an_overloaded_error_event_fails_as_retriable_with_the_upstream_text() {
    let frame = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n";
    let err = run(&[START, TEXT, frame], true).await.unwrap_err();
    assert!(
        matches!(&err, AppError::Network(m) if m.contains("Overloaded")),
        "{err:?}"
    );
}

#[tokio::test]
async fn eof_without_message_stop_fails() {
    let err = run(&[START, TEXT], true).await.unwrap_err();
    assert!(
        matches!(&err, AppError::Network(m) if m.contains("ended before")),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_empty_answer_fails() {
    let err = run(&[START, END], false).await.unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("unexpected response shape")),
        "{err:?}"
    );
}

#[tokio::test]
async fn max_tokens_fails_json_calls_only() {
    let err = run(&[START, TEXT, CUT], true).await.unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("cut off")),
        "{err:?}"
    );
    assert!(run(&[START, TEXT, CUT], false).await.is_ok());
}

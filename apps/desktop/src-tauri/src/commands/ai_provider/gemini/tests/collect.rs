//! `stream::collect` driven through the REAL `parse_gemini_frames` (#1353).

use super::super::super::stream::collect_canned;
use super::super::super::Usage;
use super::super::wire::{parse_gemini_frames, GeminiScanner};
use crate::error::{AppError, AppResult};

const TEXT: &str = "[{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"{\\\"a\\\":1}\"}]}}]}\n";
const FINISH: &str = ",{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":9,\"candidatesTokenCount\":2}}]";
const CUT: &str = ",{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"\"}]},\"finishReason\":\"MAX_TOKENS\"}]}]";

async fn run(chunks: &[&str], json: bool) -> AppResult<(String, Usage)> {
    let mut state = GeminiScanner::default();
    collect_canned(chunks, move |b| parse_gemini_frames(b, &mut state), json).await
}

#[tokio::test]
async fn assembles_text_and_usage_once_a_finish_reason_arrives() {
    let (text, usage) = run(&[TEXT, FINISH], true).await.unwrap();
    assert_eq!(text, "{\"a\":1}");
    assert_eq!((usage.input_tokens, usage.output_tokens), (9, 2));
}

#[tokio::test]
async fn an_error_element_fails_with_the_upstream_message() {
    let frame = ",{\"error\":{\"code\":503,\"message\":\"The model is overloaded.\",\"status\":\"UNAVAILABLE\"}}]";
    let err = run(&[TEXT, frame], true).await.unwrap_err();
    assert!(
        matches!(&err, AppError::Network(m) if m.contains("overloaded")),
        "{err:?}"
    );
}

#[tokio::test]
async fn eof_without_a_finish_reason_fails() {
    let err = run(&[TEXT], true).await.unwrap_err();
    assert!(
        matches!(&err, AppError::Network(m) if m.contains("ended before")),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_empty_answer_fails() {
    let err = run(&[FINISH], false).await.unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("unexpected response shape")),
        "{err:?}"
    );
}

#[tokio::test]
async fn max_tokens_fails_json_calls_only() {
    let err = run(&[TEXT, CUT], true).await.unwrap_err();
    assert!(
        matches!(&err, AppError::OutputLimit(m) if m.contains("cut off")),
        "{err:?}"
    );
    assert!(run(&[TEXT, CUT], false).await.is_ok());
}

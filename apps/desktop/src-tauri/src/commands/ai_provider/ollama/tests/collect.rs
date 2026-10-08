//! `stream::collect` driven through the REAL `parse_ollama_frames` (#1353).

use super::super::super::stream::collect_canned;
use super::super::wire::parse_ollama_frames;
use crate::error::AppError;

const PART: &str = "{\"message\":{\"content\":\"{\\\"a\\\":1}\"},\"done\":false}\n";
const THINK: &str = "{\"message\":{\"thinking\":\"hmm\"},\"done\":false}\n";
const DONE: &str = "{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"stop\",\"prompt_eval_count\":5,\"eval_count\":2}\n";
const CUT: &str = "{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"length\"}\n";

#[tokio::test]
async fn assembles_answer_without_thinking_and_takes_usage_from_the_done_object() {
    let (text, usage) = collect_canned(&[THINK, PART, DONE], parse_ollama_frames, true)
        .await
        .unwrap();
    assert_eq!(text, "{\"a\":1}");
    assert_eq!((usage.input_tokens, usage.output_tokens), (5, 2));
}

#[tokio::test]
async fn an_error_line_fails_with_the_upstream_message() {
    let line = "{\"error\":\"model requires more system memory\"}\n";
    let err = collect_canned(&[PART, line], parse_ollama_frames, true)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("more system memory")),
        "{err:?}"
    );
}

#[tokio::test]
async fn eof_without_done_fails() {
    let err = collect_canned(&[PART], parse_ollama_frames, true)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Network(m) if m.contains("ended before")),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_empty_answer_fails() {
    let err = collect_canned(&[THINK, DONE], parse_ollama_frames, false)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Provider(m) if m.contains("unexpected response shape")),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_length_stop_fails_json_calls() {
    let err = collect_canned(&[PART, CUT], parse_ollama_frames, true)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::OutputLimit(m) if m.contains("cut off")),
        "{err:?}"
    );
    assert!(collect_canned(&[PART, CUT], parse_ollama_frames, false)
        .await
        .is_ok());
}

/// Ollama's own timing fields ride the final `done` object (nanoseconds) and
/// reach the caller as whole milliseconds, through the real parser + `collect`.
#[tokio::test]
async fn the_done_object_timings_reach_usage_in_milliseconds() {
    let done = "{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"stop\",\
                \"prompt_eval_count\":5,\"eval_count\":40,\"load_duration\":2500000000,\
                \"prompt_eval_duration\":150000000,\"eval_duration\":5700000000}\n";
    let (_, usage) = collect_canned(&[PART, done], parse_ollama_frames, true)
        .await
        .unwrap();
    let t = usage
        .timings
        .expect("a done object with counts carries timings");
    assert_eq!(
        (t.load_ms, t.prompt_eval_ms, t.eval_ms, t.eval_count),
        (Some(2500), Some(150), Some(5700), Some(40))
    );
}

/// A done object that omits a timing field leaves it `None` — never a zero.
#[tokio::test]
async fn an_absent_timing_field_stays_none() {
    let (_, usage) = collect_canned(&[PART, DONE], parse_ollama_frames, true)
        .await
        .unwrap();
    let t = usage.timings.expect("counts present");
    assert_eq!((t.load_ms, t.prompt_eval_ms, t.eval_ms), (None, None, None));
}

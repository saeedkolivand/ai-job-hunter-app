//! The source-level redaction in `friendly_api_error` (#1346) must not break
//! the context-length halving retry, which reads the error MESSAGE.

use super::super::*;
use crate::commands::ai_provider::{friendly_api_error, ProviderId};

/// A fake provider that rejects input over `ok_at_or_below` chars with the
/// error `friendly_api_error` builds from a REAL wire body, which also echoes
/// a credential (so the redactor demonstrably runs on the message the retry
/// reads).
struct RejectViaFriendlyError {
    ok_at_or_below: usize,
    body: String,
    calls: std::sync::atomic::AtomicUsize,
}

#[async_trait]
impl EmbedAttempt for RejectViaFriendlyError {
    async fn attempt(&self, text: &str) -> AppResult<(Vec<f64>, Usage)> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if text.chars().count() <= self.ok_at_or_below {
            return Ok((vec![0.1], Usage::default()));
        }
        Err(friendly_api_error(
            ProviderId::OpenAi,
            reqwest::StatusCode::BAD_REQUEST,
            &self.body,
        ))
    }
}

#[test]
fn context_length_wording_survives_source_redaction() {
    let body = r#"{"error":{"message":"This model's maximum context length is 8192 tokens, however you requested 9000 tokens. Authorization: Bearer sk-TESTKEY123456 rejected","code":"context_length_exceeded"}}"#;
    let msg =
        friendly_api_error(ProviderId::OpenAi, reqwest::StatusCode::BAD_REQUEST, body).to_string();
    // Precondition: redaction really ran on this message...
    assert!(!msg.contains("TESTKEY"), "key survived: {msg}");
    assert!(msg.contains("<credential-redacted>"), "{msg}");
    // ...and the keyword the retry matches on is still there.
    assert!(
        is_context_length_error(&msg),
        "matcher lost the wording: {msg}"
    );
    // Ollama's wording, as `ollama/{chat,embed,tools}.rs` now build it.
    let ollama = format!(
        "Ollama 500 Internal Server Error: {}",
        crate::commands::ai_provider::redact_upstream_text(
            "{\"error\":\"the input length exceeds the context length\"}"
        )
    );
    assert!(is_context_length_error(&ollama), "{ollama}");
}

#[tokio::test]
async fn embed_adaptive_still_halves_on_a_redacted_context_length_error() {
    let body = r#"{"error":{"message":"maximum context length is 8192 tokens. Authorization: Bearer sk-TESTKEY123456"}}"#;
    let attempt = RejectViaFriendlyError {
        ok_at_or_below: 3000,
        body: body.to_string(),
        calls: std::sync::atomic::AtomicUsize::new(0),
    };
    let text = "a".repeat(8000);
    let result = embed_adaptive(&attempt, &text, 8000, &mut Usage::default()).await;
    // 8000 (fail) -> 4000 (fail) -> 2000 (ok) then the sticky cap covers the
    // rest: the retry fired. A non-retried error would be `Err` after 1 call.
    assert!(result.is_ok(), "retry did not fire: {result:?}");
    assert!(attempt.calls.load(std::sync::atomic::Ordering::SeqCst) > 2);
}

#[test]
fn context_length_wording_past_200_chars_is_not_cut_off_at_the_source() {
    // The source must NOT apply the 200-char `sanitize_reason` cap: a provider
    // that leads with a long preamble would lose the keyword the retry reads.
    let preamble = "request details ".repeat(20); // 320 chars
    let body =
        format!(r#"{{"error":{{"message":"{preamble}maximum context length is 8192 tokens"}}}}"#);
    let msg =
        friendly_api_error(ProviderId::OpenAi, reqwest::StatusCode::BAD_REQUEST, &body).to_string();
    assert!(msg.chars().count() > 250, "precondition: {}", msg.len());
    assert!(is_context_length_error(&msg), "wording was cut off: {msg}");
}

//! Every keyword `is_context_length_error` reads must survive the shape
//! redactor (including the #1348 header/JSON rules) in a realistic provider
//! error, even when the same message echoes a credential header.

use super::super::*;
use crate::commands::ai_provider::{friendly_api_error, redact_upstream_text, ProviderId};

const KEYWORDS: [&str; 10] = [
    "context length",
    "context_length_exceeded",
    "maximum context",
    "token limit",
    "too many tokens",
    "input length exceeds",
    "too long",
    "exceeds the maximum number of tokens",
    "request too large",
    "payload too large",
];

#[test]
fn every_matcher_keyword_survives_realistic_provider_error_shapes() {
    for kw in KEYWORDS {
        // Precondition: the bare keyword is what the matcher fires on.
        assert!(is_context_length_error(kw), "{kw}");
        let echo = r#"x-goog-api-key: AIzaSyTESTKEYabcdefghijklmnopqrstu Authorization: Bearer gsk_TESTKEYabcdefghijklmnopqrstuvwx"#;
        let bodies = [
            // OpenAI
            format!(
                r#"{{"error": {{"message": "This model: {kw}. {echo}", "type": "invalid_request_error", "code": "context_length_exceeded"}}}}"#
            ),
            // Anthropic
            format!(
                r#"{{"type": "error", "error": {{"type": "invalid_request_error", "message": "{kw}: 250000 tokens > 200000. x-api-key: sk-ant-TESTKEYabcdefghijkl"}}}}"#
            ),
            // Gemini
            format!(
                r#"{{"error": {{"code": 400, "message": "{kw} ({echo})", "status": "INVALID_ARGUMENT"}}}}"#
            ),
            // Ollama
            format!(r#"{{"error": "{kw} {echo}"}}"#),
            // Cohere
            format!(r#"{{"message": "{kw}. api_token: gsk_TESTKEYabcdefghijklmnopqrstuvwx"}}"#),
        ];
        for body in bodies {
            let msg =
                friendly_api_error(ProviderId::OpenAi, reqwest::StatusCode::BAD_REQUEST, &body)
                    .to_string();
            assert!(!msg.contains("TESTKEY"), "{kw}: key survived: {msg}");
            assert!(
                is_context_length_error(&msg),
                "{kw}: matcher lost it: {msg}"
            );
            let ollama = format!("Ollama 500: {}", redact_upstream_text(&body));
            assert!(
                is_context_length_error(&ollama),
                "{kw}: ollama shape: {ollama}"
            );
        }
    }
}

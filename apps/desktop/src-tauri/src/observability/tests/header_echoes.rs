//! Header-style and pretty-JSON credential echoes (#1348) in `redact_tokens`.

use super::super::{redact_tokens, sanitize_reason};

const GEMINI: &str = "AIzaSyTESTKEYabcdefghijklmnopqrstu";
const GROQ: &str = "gsk_TESTKEYabcdefghijklmnopqrstuvwx";

#[test]
fn each_header_and_json_echo_form_is_redacted() {
    for (label, line, secret) in [
        (
            "x-goog-api-key",
            format!("sent x-goog-api-key: {GEMINI} to upstream"),
            GEMINI,
        ),
        (
            "X-API-KEY case",
            format!("X-Api-Key: {GROQ} rejected"),
            GROQ,
        ),
        ("x-api-key", format!("headers x-api-key: {GROQ}"), GROQ),
        ("any *-key", format!("x-custom-key: {GROQ} nope"), GROQ),
        ("any *-token", format!("x-auth-token: {GROQ} nope"), GROQ),
        (
            "authorization Token",
            format!("Authorization: Token {GROQ} bad"),
            GROQ,
        ),
        (
            "authorization Key",
            format!("authorization: Key {GROQ} bad"),
            GROQ,
        ),
        (
            "authorization Basic",
            "authorization: Basic dXNlcjpwYXNzd29yZA== bad".into(),
            "dXNlcjpwYXNz",
        ),
        (
            "bare Bearer",
            format!("rejected Bearer {GROQ} at gateway"),
            GROQ,
        ),
        (
            "json x-goog-api-key",
            format!(r#"{{"x-goog-api-key": "{GEMINI}"}}"#),
            GEMINI,
        ),
        (
            "json Authorization",
            format!(r#"{{"Authorization": "Bearer {GROQ}"}}"#),
            GROQ,
        ),
        (
            "json api_key",
            format!(r#"{{"api_key": "{GEMINI}", "n": 1}}"#),
            GEMINI,
        ),
        ("json token", format!(r#"{{"token": "{GROQ}"}}"#), GROQ),
    ] {
        let out = redact_tokens(&line);
        assert!(!out.contains(secret), "{label} survived: {out}");
        assert!(out.contains("<credential-redacted>"), "{label}: {out}");
        assert!(!sanitize_reason(&line).contains(secret), "{label}");
    }
}

#[test]
fn ordinary_messages_pass_byte_identical() {
    for msg in [
        "429 Too Many Requests",
        "Bearer token missing from the request",
        "Bearer authentication failed",
        "Basic auth failed",
        "Authorization failed for this request",
        "invalid token: expired",
        "no key: configured",
        "max-tokens exceeded, please retry",
        r#"{"max_tokens": 4096, "message": "too many tokens"}"#,
        "gemini: model or endpoint not found — model not found",
    ] {
        assert_eq!(redact_tokens(msg), msg, "over-redacted: {msg}");
    }
}

#[test]
fn a_header_name_at_the_end_of_the_text_does_not_panic_or_redact() {
    for msg in [
        "x-api-key:",
        "Authorization:",
        "Authorization: Bearer",
        "Bearer",
    ] {
        assert_eq!(redact_tokens(msg), msg);
    }
}

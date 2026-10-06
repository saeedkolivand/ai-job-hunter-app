//! The verbatim key strip the provider-call seams (`Completer`, `embed_text`)
//! apply (#1348): the stored key / base-URL secrets leave the error text while
//! its VARIANT and (when it holds no secret) its exact bytes are untouched.
//!
//! The crate has no `tauri::test` mock app, so the `Completer`/`embed_text`
//! methods themselves cannot be called; these drive the same shaping function
//! they call (`strip_provider_secrets`) on errors built by `friendly_api_error`
//! from an upstream-shaped body (no real socket: a round trip through the
//! process-global pooled client left keep-alive connections that flaked later
//! tests), and `wiring_guard` pins that the seams call it.

use std::mem::discriminant;

use crate::commands::ai_provider::stream::{
    empty_answer_error_for_test, is_empty_answer_length_cut,
};
use crate::commands::ai_provider::{
    friendly_api_error, strip_provider_secrets, ProviderId, StopReason,
};
use crate::error::AppError;

const GEMINI_KEY: &str = "AIzaSyTESTKEYabcdefghijklmnopqrstu";
const GROQ_KEY: &str = "gsk_TESTKEYabcdefghijklmnopqrstuvwx";

/// The error every adapter builds for an upstream `status` whose body echoes `key`.
fn upstream_error(status: u16, key: &str) -> AppError {
    let body = format!(r#"{{"error":{{"message":"bad request, credential {key} rejected"}}}}"#);
    friendly_api_error(
        ProviderId::Gemini,
        reqwest::StatusCode::from_u16(status).unwrap(),
        &body,
    )
}

#[test]
fn a_bare_key_echoed_by_the_upstream_is_stripped_and_the_variant_kept() {
    for key in [GEMINI_KEY, GROQ_KEY] {
        // 400/404/418 echo the body; 500 maps to a fixed message (nothing to strip).
        for (status, echoed) in [(400, true), (404, true), (418, true), (500, false)] {
            let err = upstream_error(status, key);
            assert_eq!(
                err.to_string().contains(key),
                echoed,
                "precondition for {status}: {err}"
            );
            let kind = discriminant(&err);
            let retriable = err.retriable();
            let stripped = strip_provider_secrets(err, Some(key), None);
            assert!(!stripped.to_string().contains(key), "{status}: {stripped}");
            assert_eq!(discriminant(&stripped), kind, "variant changed at {status}");
            assert_eq!(stripped.retriable(), retriable);
        }
    }
}

#[test]
fn the_trimmed_key_is_stripped_when_the_stored_value_has_whitespace() {
    let stored = format!("  {GROQ_KEY}\n");
    let err = AppError::Provider(format!("groq: echoed {GROQ_KEY} back"));
    let out = strip_provider_secrets(err, Some(&stored), None).to_string();
    assert!(!out.contains(GROQ_KEY), "{out}");
}

#[test]
fn base_url_userinfo_password_and_query_values_are_stripped() {
    let url = "https://user:hunter2hunter2@gw.example.com/v1?api_key=QUERYSECRET123&x=1";
    let err = AppError::Network("unreachable: hunter2hunter2 / QUERYSECRET123".into());
    let out = strip_provider_secrets(err, None, Some(url)).to_string();
    assert!(
        !out.contains("hunter2hunter2") && !out.contains("QUERYSECRET123"),
        "{out}"
    );
}

#[test]
fn a_timeout_stays_a_timeout() {
    let err = AppError::Timeout(format!("gemini: no response within 30s ({GEMINI_KEY})"));
    let out = strip_provider_secrets(err, Some(GEMINI_KEY), None);
    assert!(
        matches!(&out, AppError::Timeout(m) if !m.contains(GEMINI_KEY)),
        "{out:?}"
    );
    assert!(matches!(
        strip_provider_secrets(AppError::Cancelled, Some(GEMINI_KEY), None),
        AppError::Cancelled
    ));
}

#[test]
fn the_empty_answer_length_cut_text_still_compares_equal() {
    for provider in [ProviderId::OpenAi, ProviderId::Ollama] {
        let err = empty_answer_error_for_test(Some(StopReason::Length), provider);
        assert!(is_empty_answer_length_cut(&err), "precondition");
        let out = strip_provider_secrets(err, Some(GEMINI_KEY), Some("https://h/v1?k=abcdefgh12"));
        assert!(is_empty_answer_length_cut(&out), "{out:?}");
    }
}

#[test]
fn a_message_without_a_secret_is_byte_identical_and_short_secrets_are_ignored() {
    let msg = "gemini: model or endpoint not found — model not found   (note  spacing)";
    let out = strip_provider_secrets(AppError::Provider(msg.into()), Some("short"), None);
    assert!(matches!(&out, AppError::Provider(m) if m == msg), "{out:?}");
    let long = "y".repeat(20_000);
    let out = strip_provider_secrets(AppError::Provider(long.clone()), Some(GROQ_KEY), None);
    assert!(
        matches!(&out, AppError::Provider(m) if *m == long),
        "uncapped"
    );
}

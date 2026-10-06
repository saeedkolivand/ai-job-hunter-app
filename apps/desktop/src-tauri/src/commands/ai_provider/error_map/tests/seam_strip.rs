//! The verbatim key strip the provider-call seams (`Completer`, `embed_text`)
//! apply (#1348): the stored key / base-URL secrets leave the error text while
//! its VARIANT and (when it holds no secret) its exact bytes are untouched.
//!
//! The crate has no `tauri::test` mock app, so the `Completer`/`embed_text`
//! methods themselves cannot be called; these drive the same shaping function
//! they call (`strip_provider_secrets`) on errors built from a REAL wiremock
//! upstream response, and `call_sites_are_wrapped` pins that the seams call it.

use std::mem::discriminant;

use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::commands::ai_provider::stream::{
    empty_answer_error_for_test, is_empty_answer_length_cut,
};
use crate::commands::ai_provider::{
    friendly_api_error, strip_provider_secrets, ProviderId, StopReason,
};
use crate::error::AppError;

const GEMINI_KEY: &str = "AIzaSyTESTKEYabcdefghijklmnopqrstu";
const GROQ_KEY: &str = "gsk_TESTKEYabcdefghijklmnopqrstuvwx";

/// A real HTTP round trip to a mock upstream that answers `status` with a body
/// echoing `key`, mapped the way every adapter maps it.
async fn upstream_error(status: u16, key: &str) -> AppError {
    let server = MockServer::start().await;
    let body = format!(r#"{{"error":{{"message":"bad request, credential {key} rejected"}}}}"#);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(status).set_body_string(body))
        .mount(&server)
        .await;
    let resp = crate::net::http::shared()
        .post(server.uri())
        .send()
        .await
        .expect("mock upstream reachable");
    let code = resp.status();
    let text = resp.text().await.expect("body");
    friendly_api_error(ProviderId::Gemini, code, &text)
}

#[tokio::test]
async fn a_bare_key_echoed_by_the_upstream_is_stripped_and_the_variant_kept() {
    for key in [GEMINI_KEY, GROQ_KEY] {
        // 400/404/418 echo the body; 500 maps to a fixed message (nothing to strip).
        for (status, echoed) in [(400, true), (404, true), (418, true), (500, false)] {
            let err = upstream_error(status, key).await;
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

/// Deleting the strip at a seam fails nothing else (no mock app), so pin the
/// call sites textually: every provider-call in `Completer`'s methods and the
/// `embed_text` result both pass through the strip.
#[test]
fn call_sites_are_wrapped() {
    let completion = include_str!("../../../../pipeline/completion.rs");
    let completer = include_str!("../../../../pipeline/completer.rs");
    for (src, calls) in [
        (
            completion,
            &[
                "chat_stream(",
                "complete_with_usage(",
                "chat_with_tools(",
                "complete_structured(",
            ][..],
        ),
        (
            completer,
            &[
                "fetch_company_brief(",
                "research_salary(",
                "research_answer(",
                "searched_research_salary(",
                "searched_research_answer(",
            ][..],
        ),
    ] {
        for call in calls {
            let hits: Vec<_> = src
                .match_indices(call)
                .filter(|(i, _)| src[..*i].ends_with(".") || src[..*i].ends_with("::"))
                .collect();
            assert!(!hits.is_empty(), "no call site for {call}");
            for (i, _) in hits {
                let window = &src[i.saturating_sub(220)..i];
                assert!(
                    window.contains("strip_secrets("),
                    "{call} not wrapped near byte {i}"
                );
            }
        }
    }
    let embeddings = include_str!("../../embeddings.rs");
    let adaptive = embeddings
        .find("embed_adaptive(&metered")
        .expect("embed_adaptive call");
    assert!(embeddings[adaptive..].contains("strip_provider_secrets("));
}

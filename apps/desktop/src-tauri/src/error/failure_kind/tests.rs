use super::*;
use crate::commands::ai_provider::{friendly_api_error, ProviderId};
use reqwest::StatusCode;

fn kind(e: AppError) -> Option<&'static str> {
    FailureKind::of(&e).map(FailureKind::as_str)
}

/// Status and body as a provider sent them, through the one mapper that has both.
fn via_mapper(provider: ProviderId, status: u16, body: &str) -> Option<&'static str> {
    kind(friendly_api_error(
        provider,
        StatusCode::from_u16(status).unwrap(),
        body,
    ))
}

// Real response bodies. OpenAI reports an out-of-credit account as a 429, which no text re-parse
// of the mapper's rate-limit sentence can tell from a rate limit.
const OPENAI_INSUFFICIENT_QUOTA: &str = r#"{"error":{"message":"You exceeded your current quota, please check your plan and billing details. For more information on this error, read the docs: https://platform.openai.com/docs/guides/error-codes/api-errors.","type":"insufficient_quota","param":null,"code":"insufficient_quota"}}"#;
const OPENAI_REAL_RATE_LIMIT: &str = r#"{"error":{"message":"Rate limit reached for gpt-4o in organization org-x on requests per min (RPM): Limit 3, Used 3, Requested 1. Please try again in 20s.","type":"requests","param":null,"code":"rate_limit_exceeded"}}"#;
const ANTHROPIC_CREDIT_BALANCE: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits."}}"#;
const ANTHROPIC_OVERLOADED: &str =
    r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;

/// H1: the reported misclassification. Mutation check: delete the `is_billing_text` branch in
/// `friendly_api_error` and the first assertion reads `rate_limited`.
#[test]
fn an_openai_out_of_credit_429_is_billing_and_a_real_rate_limit_is_not() {
    assert_eq!(
        via_mapper(ProviderId::OpenAi, 429, OPENAI_INSUFFICIENT_QUOTA),
        Some("billing")
    );
    assert_eq!(
        via_mapper(ProviderId::OpenAi, 429, OPENAI_REAL_RATE_LIMIT),
        Some("rate_limited")
    );
}

#[test]
fn anthropic_credit_balance_is_billing_and_overloaded_is_unavailable() {
    assert_eq!(
        via_mapper(ProviderId::Anthropic, 400, ANTHROPIC_CREDIT_BALANCE),
        Some("billing")
    );
    assert_eq!(
        via_mapper(ProviderId::Anthropic, 529, ANTHROPIC_OVERLOADED),
        Some("unavailable")
    );
    // An ordinary bad request is not billing, and not guessed at.
    assert_eq!(
        via_mapper(
            ProviderId::Anthropic,
            400,
            r#"{"error":{"message":"max_tokens: must be positive"}}"#
        ),
        None
    );
}

#[test]
fn keys_and_statuses_map_through_the_mapper() {
    assert_eq!(via_mapper(ProviderId::OpenAi, 401, "{}"), Some("auth"));
    assert_eq!(via_mapper(ProviderId::OpenAi, 403, "{}"), Some("auth"));
    assert_eq!(via_mapper(ProviderId::OpenAi, 402, "{}"), Some("billing"));
    assert_eq!(
        via_mapper(ProviderId::OpenAi, 503, "{}"),
        Some("unavailable")
    );
}

/// A proxy's 407 is none of the five: unclassified is `None`, never a default guess.
#[test]
fn a_proxy_407_is_unclassified() {
    assert_eq!(
        via_mapper(ProviderId::OpenAi, 407, "Proxy Authentication Required"),
        None
    );
}

#[test]
fn an_out_of_credit_search_transport_text_is_billing() {
    // The web-search transport has only a string: the text fallback, which needs a status.
    let e = AppError::Network("ollama web_search 402 Payment Required: out of credit".to_string());
    assert_eq!(kind(e), Some("billing"));
    let e = AppError::Network("ollama web_search 401 Unauthorized".to_string());
    assert_eq!(kind(e), Some("auth"));
}

#[test]
fn bad_output_unreachable_and_loose_words_are_handled_precisely() {
    assert_eq!(
        kind(AppError::Parse("no requirements".into())),
        Some("model_output")
    );
    assert_eq!(kind(AppError::Refusal("no".into())), Some("model_output"));
    assert_eq!(
        kind(AppError::Timeout("no response".into())),
        Some("unavailable")
    );
    assert_eq!(
        kind(AppError::RateLimited("daily".into())),
        Some("rate_limited")
    );
    assert_eq!(
        kind(AppError::Network("x unreachable: dns".into())),
        Some("unavailable")
    );
    // Loose words alone are not evidence.
    for loose in [
        "authentication step failed",
        "forbidden word in prompt",
        "billing address",
    ] {
        assert_eq!(kind(AppError::Provider(loose.into())), None, "{loose}");
    }
    assert_eq!(
        kind(AppError::Provider("request of 4020 tokens failed".into())),
        None
    );
    assert_eq!(kind(AppError::Validation("job id required".into())), None);
    assert_eq!(kind(AppError::Config("no model selected".into())), None);
    assert_eq!(kind(AppError::Cancelled), None);
}

#[test]
fn the_wire_token_round_trips_and_unknown_tokens_are_dropped() {
    for k in [
        FailureKind::Billing,
        FailureKind::Auth,
        FailureKind::RateLimited,
        FailureKind::Unavailable,
        FailureKind::ModelOutput,
    ] {
        assert_eq!(FailureKind::parse(k.as_str()), Some(k));
        assert!(!k.summary().is_empty());
    }
    assert_eq!(FailureKind::parse("run: call-irreversible"), None);
}

/// Mutation check: drop `is_billing_text` from `from_text` and this reads `rate_limited`.
#[test]
fn an_ollama_search_429_with_a_credit_body_is_billing() {
    let e = AppError::Network(
        "ollama web_search 429 Too Many Requests: Your credit balance is too low".to_string(),
    );
    assert_eq!(kind(e), Some("billing"));
    let e = AppError::Network("ollama web_search 429 Too Many Requests: slow down".to_string());
    assert_eq!(kind(e), Some("rate_limited"));
}

/// A transport failure has no status to read. Mutation check: revert the `.or(Some(Unavailable))`
/// on the `Network` arm and this reads `None`. An unknown provider error stays unclassified.
#[test]
fn a_transport_failure_is_unavailable_and_an_unknown_provider_error_is_not() {
    let e = AppError::Network("Stream error: connection reset by peer".to_string());
    assert_eq!(kind(e), Some("unavailable"));
    assert_eq!(kind(AppError::Provider("mystery".to_string())), None);
}

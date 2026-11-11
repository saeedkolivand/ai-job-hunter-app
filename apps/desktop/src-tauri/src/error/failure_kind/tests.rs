use super::*;

fn kind(e: AppError) -> Option<&'static str> {
    FailureKind::of(&e).map(FailureKind::as_str)
}

#[test]
fn an_out_of_credit_provider_is_billing_not_unavailable() {
    // The shape `friendly_api_error`'s default arm gives a 402 (#1496).
    let e = AppError::Provider(
        "ollama-cloud 402: Add usage credits or subscribe to keep going".to_string(),
    );
    assert_eq!(kind(e), Some("billing"));
    // The web-search transport's own shape (#1500), under the Network variant.
    let e = AppError::Network("ollama web_search 402 Payment Required: out of credit".to_string());
    assert_eq!(kind(e), Some("billing"));
}

#[test]
fn keys_limits_outages_and_bad_output_each_get_their_own_kind() {
    let auth = "openai: invalid or unauthorized API key.".to_string();
    assert_eq!(kind(AppError::Config(auth)), Some("auth"));
    assert_eq!(
        kind(AppError::Network(
            "ollama web_search 401 Unauthorized".into()
        )),
        Some("auth")
    );
    assert_eq!(
        kind(AppError::Network("x: rate limit or quota reached.".into())),
        Some("rate_limited")
    );
    assert_eq!(
        kind(AppError::RateLimited("daily".into())),
        Some("rate_limited")
    );
    assert_eq!(
        kind(AppError::Network(
            "x: service error (503). Try again shortly.".into()
        )),
        Some("unavailable")
    );
    assert_eq!(
        kind(AppError::Timeout("no response".into())),
        Some("unavailable")
    );
    assert_eq!(
        kind(AppError::Parse("no requirements".into())),
        Some("model_output")
    );
    assert_eq!(kind(AppError::Refusal("no".into())), Some("model_output"));
}

#[test]
fn non_provider_errors_and_number_fragments_are_not_classified() {
    assert_eq!(kind(AppError::Validation("job id required".into())), None);
    assert_eq!(kind(AppError::Storage("disk".into())), None);
    assert_eq!(kind(AppError::Cancelled), None);
    // A misconfiguration that is not about a key stays unclassified.
    assert_eq!(kind(AppError::Config("no model selected".into())), None);
    // "4020" must not read as a 402.
    assert_eq!(
        kind(AppError::Provider("request of 4020 tokens failed".into())),
        Some("unavailable")
    );
}

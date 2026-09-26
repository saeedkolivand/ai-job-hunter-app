//! `check_ai_assist_gate` + `to_draft_failed` (wire-error sentinel collapse
//! — HIGH finding).

use crate::error::AppError;

use super::super::errors::{
    check_ai_assist_gate, to_draft_failed, DRAFT_CONFIG_FAILED_MESSAGE, DRAFT_FAILED_MESSAGE,
};

// ── check_ai_assist_gate ──────────────────────────────────────────────

#[test]
fn check_ai_assist_gate_refuses_when_opt_in_off() {
    let err = check_ai_assist_gate(false).unwrap_err();
    assert!(err.to_string().contains("AI answer drafting is off"));
}

#[test]
fn check_ai_assist_gate_allows_when_opt_in_on() {
    assert!(check_ai_assist_gate(true).is_ok());
}

// ── to_draft_failed ─────────────────────────────────────────────────────

#[test]
fn to_draft_failed_collapses_a_rate_limit_error_to_the_generic_sentinel() {
    let dynamic = AppError::RateLimited(
        "Daily request limit reached for provider 'openai' (max 4000/day). Resets at UTC midnight."
            .to_string(),
    );
    let mapped = to_draft_failed("daily budget exceeded before compose", dynamic);
    assert_eq!(mapped.to_string(), DRAFT_FAILED_MESSAGE);
    assert!(!mapped.to_string().contains("openai"));
}

#[test]
fn to_draft_failed_collapses_a_provider_error_carrying_an_endpoint_to_the_generic_sentinel() {
    let dynamic = AppError::Provider(
        "POST https://api.example.com/v1/chat/completions failed: 500 internal error".to_string(),
    );
    let mapped = to_draft_failed("compose failed", dynamic);
    assert_eq!(mapped.to_string(), DRAFT_FAILED_MESSAGE);
    assert!(!mapped.to_string().contains("https://"));
}

// #1217: a 401/403 becomes `AppError::Config` (see
// `commands::ai_provider::friendly_api_error`). Retrying it fails identically
// every time AND charges the daily budget again, so it must NOT collapse into
// the generic "Please retry." sentinel.
#[test]
fn to_draft_failed_maps_a_provider_auth_error_to_the_config_sentinel() {
    let dynamic = AppError::Config("openai: invalid or unauthorized API key.".to_string());
    let mapped = to_draft_failed("compose failed", dynamic);
    assert_eq!(mapped.to_string(), DRAFT_CONFIG_FAILED_MESSAGE);
    assert_ne!(mapped.to_string(), DRAFT_FAILED_MESSAGE);
    // Still a fixed string — the provider's own text never reaches the wire.
    assert!(!mapped.to_string().contains("openai"));
}

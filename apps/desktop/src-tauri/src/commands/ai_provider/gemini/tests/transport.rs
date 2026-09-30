//! Key validation + `/v1beta/models` page parsing (pure, no HTTP — see
//! `list_models` for the wiremock transport tests).

use serde_json::{json, Value};

use super::super::transport::{parse_model_page, validate_gemini_key};
use crate::error::AppError;

#[test]
fn blank_or_missing_key_is_rejected_with_unauthorized() {
    // A missing key, an empty string, and whitespace-only must all fail fast with
    // the same unauthorized message `friendly_api_error` maps a real 401/403 to —
    // never sending an empty `x-goog-api-key` header for a wasted round-trip.
    for stored in [None, Some(String::new()), Some("   \n\t".to_string())] {
        match validate_gemini_key(stored) {
            Err(AppError::Config(msg)) => {
                assert_eq!(msg, "gemini: invalid or unauthorized API key.")
            }
            other => panic!("expected unauthorized Config error, got {other:?}"),
        }
    }
}

#[test]
fn present_key_is_returned_unchanged_when_already_clean() {
    assert_eq!(
        validate_gemini_key(Some("AIza-secret".to_string())).unwrap(),
        "AIza-secret"
    );
}

#[test]
fn validate_gemini_key_trims_the_returned_key_not_just_the_checked_one() {
    // A pasted key with a trailing space/newline must reach the
    // `x-goog-api-key` header TRIMMED — checking `k.trim().is_empty()` but
    // returning the padded `k` is the exact bug: a trailing space just
    // 401s, an embedded `\n` makes the header value invalid and the request
    // never builds at all.
    assert_eq!(
        validate_gemini_key(Some(" AIza-secret \n".to_string())).unwrap(),
        "AIza-secret"
    );
}

#[test]
fn parse_model_page_strips_the_models_prefix() {
    let body = json!({
        "models": [
            { "name": "models/gemini-3-pro" },
            { "name": "models/gemini-2.5-flash" },
            { "name": "tunedModels/not-a-real-model" },
        ]
    });
    let (page, cursor) = parse_model_page(&body).unwrap();
    let names: Vec<String> = page
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["gemini-3-pro", "gemini-2.5-flash"]);
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_does_not_filter_out_preview_ids() {
    // Regression guard for the `/v1` -> `/v1beta` switch: `/v1beta` is the ONLY
    // version that lists `-preview`/experimental models (e.g. the curated
    // Pro-tier default in `provider-meta.ts` is a `-preview` id) — the parser
    // must never re-introduce a filter that drops them.
    let body = json!({
        "models": [{ "name": "models/gemini-3.1-pro-preview" }]
    });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(page, vec![json!({ "name": "gemini-3.1-pro-preview" })]);
}

#[test]
fn parse_model_page_populates_display_name_and_context_length_when_present() {
    let body = json!({
        "models": [{
            "name": "models/gemini-3-pro",
            "displayName": "Gemini 3 Pro",
            "inputTokenLimit": 1_000_000,
        }]
    });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(
        page,
        vec![json!({
            "name": "gemini-3-pro",
            "displayName": "Gemini 3 Pro",
            "contextLength": 1_000_000,
        })]
    );
}

#[test]
fn parse_model_page_never_populates_created_at_gemini_does_not_return_one() {
    // Gemini's `/v1beta/models` reports no creation timestamp at all —
    // `createdAt` must be ABSENT from the entry, never defaulted to some
    // sentinel, even when every other optional field IS present.
    let body = json!({
        "models": [{
            "name": "models/gemini-3-pro",
            "displayName": "Gemini 3 Pro",
            "inputTokenLimit": 1_000_000,
        }]
    });
    let (page, _) = parse_model_page(&body).unwrap();
    assert!(page[0].get("createdAt").is_none());
}

#[test]
fn parse_model_page_omits_optional_fields_the_provider_does_not_return_and_keeps_name_unchanged() {
    // `name` must stay byte-identical to the pre-widening shape — a stored
    // model preference matches against it.
    let body = json!({ "models": [{ "name": "models/gemini-2.5-flash" }] });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(page, vec![json!({ "name": "gemini-2.5-flash" })]);
}

#[test]
fn parse_model_page_ok_empty_on_genuinely_empty_catalogue() {
    let body = json!({ "models": [] });
    let (page, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(page, Vec::<Value>::new());
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_errors_when_models_field_is_missing() {
    let body = json!({ "unexpected": "shape" });
    assert!(matches!(
        parse_model_page(&body),
        Err(AppError::Provider(_))
    ));
}

#[test]
fn parse_model_page_carries_a_non_empty_next_page_token() {
    let body = json!({
        "models": [{ "name": "models/gemini-2.5-flash" }],
        "nextPageToken": "page-2-token",
    });
    let (_, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(cursor, Some("page-2-token".to_string()));
}

#[test]
fn parse_model_page_treats_an_empty_next_page_token_as_the_last_page() {
    let body = json!({
        "models": [{ "name": "models/gemini-2.5-flash" }],
        "nextPageToken": "",
    });
    let (_, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(cursor, None);
}

// `advance_cursor`/`PaginationStep`/`pagination_step` are shared, generic
// helpers now — see `ai_provider::mod`'s test module for their coverage.
// Duplicating them here per-adapter is exactly the "a rule implemented
// twice that silently stops agreeing" defect class this codebase keeps
// paying for; one copy, one set of tests.

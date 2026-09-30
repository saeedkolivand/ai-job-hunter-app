//! Key resolution + `/v1/models` page parsing (pure, no HTTP — see
//! `list_models` for the wiremock transport tests).

use serde_json::{json, Value};

use super::super::super::AppError;
use super::super::transport::{parse_model_page, require_anthropic_key};

#[test]
fn require_anthropic_key_errors_on_missing_or_blank_key() {
    assert!(matches!(
        require_anthropic_key(None),
        Err(AppError::Config(_))
    ));
    assert!(matches!(
        require_anthropic_key(Some("   ".to_string())),
        Err(AppError::Config(_))
    ));
}

#[test]
fn require_anthropic_key_accepts_a_real_key() {
    assert_eq!(
        require_anthropic_key(Some("sk-ant-real".to_string())).unwrap(),
        "sk-ant-real"
    );
}

#[test]
fn require_anthropic_key_trims_the_returned_key_not_just_the_checked_one() {
    // A pasted key with a trailing space/newline must reach the `x-api-key`
    // header TRIMMED — checking `k.trim().is_empty()` but returning the
    // padded `k` is the exact bug: a trailing space just 401s, an embedded
    // `\n` makes the header value invalid and the request never builds at all.
    assert_eq!(
        require_anthropic_key(Some(" sk-ant-real \n".to_string())).unwrap(),
        "sk-ant-real"
    );
}

#[test]
fn parse_model_page_keeps_only_claude_ids() {
    let body = json!({
        "data": [
            { "id": "claude-sonnet-5" },
            { "id": "claude-opus-4-5" },
            { "id": "some-other-model" },
        ]
    });
    let (page, cursor) = parse_model_page(&body).unwrap();
    let names: Vec<String> = page
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["claude-sonnet-5", "claude-opus-4-5"]);
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_populates_display_name_created_at_and_context_length_when_present() {
    let body = json!({
        "data": [{
            "type": "model",
            "id": "claude-sonnet-5-20251101",
            "display_name": "Claude Sonnet 5",
            "created_at": "2024-01-01T00:00:00Z",
            "max_input_tokens": 200_000,
        }]
    });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(
        page,
        vec![json!({
            "name": "claude-sonnet-5-20251101",
            "displayName": "Claude Sonnet 5",
            "createdAt": 1_704_067_200_000i64,
            "contextLength": 200_000,
        })]
    );
}

#[test]
fn parse_model_page_omits_optional_fields_the_provider_does_not_return_and_keeps_name_unchanged() {
    // `name` must stay byte-identical to the pre-widening shape — a stored
    // model preference matches against it.
    let body = json!({ "data": [{ "id": "claude-sonnet-5" }] });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(page, vec![json!({ "name": "claude-sonnet-5" })]);
}

#[test]
fn parse_model_page_ok_empty_on_genuinely_empty_catalogue() {
    let body = json!({ "data": [] });
    let (page, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(page, Vec::<Value>::new());
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_errors_when_data_field_is_missing() {
    let body = json!({ "unexpected": "shape" });
    assert!(matches!(
        parse_model_page(&body),
        Err(AppError::Provider(_))
    ));
}

#[test]
fn parse_model_page_carries_the_cursor_only_when_has_more_is_true() {
    let body = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": true,
        "last_id": "claude-sonnet-5",
    });
    let (_, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(cursor, Some("claude-sonnet-5".to_string()));
}

#[test]
fn parse_model_page_omits_the_cursor_when_has_more_is_false_even_with_a_last_id() {
    // A `last_id` can still be present on the final page — the cursor must be
    // driven by `has_more`, never by `last_id`'s mere presence, or pagination
    // would loop forever re-requesting the same last page.
    let body = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": false,
        "last_id": "claude-sonnet-5",
    });
    let (_, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_errors_when_has_more_is_true_but_last_id_is_missing() {
    // `has_more: true` with no cursor to continue from is a malformed
    // response, not a clean end-of-pages — silently stopping would return a
    // truncated catalogue as `Ok`, exactly the bug pagination exists to fix.
    let body = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": true,
    });
    assert!(matches!(
        parse_model_page(&body),
        Err(AppError::Provider(_))
    ));
}

#[test]
fn parse_model_page_errors_when_has_more_is_true_but_last_id_is_blank() {
    let body = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": true,
        "last_id": "   ",
    });
    assert!(matches!(
        parse_model_page(&body),
        Err(AppError::Provider(_))
    ));
}

// `advance_cursor`/`PaginationStep`/`pagination_step` are shared, generic
// helpers now — see `ai_provider::mod`'s test module for their coverage.
// Duplicating them here per-adapter is exactly the "a rule implemented
// twice that silently stops agreeing" defect class this codebase keeps
// paying for; one copy, one set of tests.

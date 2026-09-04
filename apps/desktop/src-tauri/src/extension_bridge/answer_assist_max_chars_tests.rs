//! Unit tests for [`parse_max_chars`] only, split out of `answer_assist.rs`
//! into this sibling file for the same R8 line-budget reason as
//! `answer_assist_tests.rs` (the sibling file's rewrite has since landed, so
//! this no longer needs to stay inline).

use super::*;

/// A draft-mode payload carrying whatever `maxChars` value is under test.
fn draft_with(max_chars: Value) -> Value {
    json!({ "question": "Why this role?", "maxChars": max_chars })
}

#[test]
fn reads_a_plain_positive_limit() {
    assert_eq!(
        parse_max_chars(&draft_with(json!(300)), AssistMode::Draft),
        Some(300)
    );
}

#[test]
fn accepts_the_draft_cap_itself_unchanged() {
    assert_eq!(
        parse_max_chars(&draft_with(json!(DRAFT_CAP)), AssistMode::Draft),
        Some(DRAFT_CAP)
    );
}

#[test]
fn accepts_a_limit_of_one() {
    // The smallest legal value: `0` is the "no limit" boundary, not `1`.
    assert_eq!(
        parse_max_chars(&draft_with(json!(1)), AssistMode::Draft),
        Some(1)
    );
}

#[test]
fn clamps_an_over_large_limit_to_the_draft_cap() {
    // The wire deliberately ACCEPTS this value (see the shared schema's
    // doc) — the reduction happens here, and only here.
    assert_eq!(
        parse_max_chars(&draft_with(json!(DRAFT_CAP + 1)), AssistMode::Draft),
        Some(DRAFT_CAP)
    );
    assert_eq!(
        parse_max_chars(&draft_with(json!(1_000_000)), AssistMode::Draft),
        Some(DRAFT_CAP)
    );
}

#[test]
fn clamps_the_largest_representable_integer_rather_than_overflowing() {
    // `u64::MAX` exceeds `usize` on a 32-bit target; the conversion must
    // fall back to the cap instead of panicking or wrapping.
    assert_eq!(
        parse_max_chars(&draft_with(json!(u64::MAX)), AssistMode::Draft),
        Some(DRAFT_CAP)
    );
}

#[test]
fn reads_zero_as_no_limit() {
    // Zero is not "an answer of length zero" — it is a client bug or a
    // field with an empty maxlength attribute. Degrade, never refuse.
    assert_eq!(
        parse_max_chars(&draft_with(json!(0)), AssistMode::Draft),
        None
    );
}

#[test]
fn rejects_a_negative_limit() {
    assert_eq!(
        parse_max_chars(&draft_with(json!(-1)), AssistMode::Draft),
        None
    );
    assert_eq!(
        parse_max_chars(&draft_with(json!(-300)), AssistMode::Draft),
        None
    );
}

#[test]
fn rejects_a_non_integer_limit() {
    for value in [json!(12.5), json!(300.0), json!(-0.5)] {
        assert_eq!(
            parse_max_chars(&draft_with(value.clone()), AssistMode::Draft),
            None,
            "a non-integer maxChars ({value}) must read as no limit"
        );
    }
}

#[test]
fn rejects_a_limit_that_is_not_a_number_at_all() {
    for value in [
        json!("300"),
        json!(true),
        json!(null),
        json!([300]),
        json!({}),
    ] {
        assert_eq!(
            parse_max_chars(&draft_with(value.clone()), AssistMode::Draft),
            None,
            "a non-numeric maxChars ({value}) must read as no limit"
        );
    }
}

#[test]
fn reads_an_absent_key_as_no_limit() {
    // An extension older than the field, or a field with no maxlength.
    assert_eq!(
        parse_max_chars(&json!({ "question": "Why this role?" }), AssistMode::Draft),
        None
    );
}

#[test]
fn ignores_the_field_entirely_in_rewrite_mode() {
    // Same payload, same valid value, opposite answer: rewrite carries its
    // own instruction and its output is never measured against a limit.
    let payload = draft_with(json!(300));
    assert_eq!(parse_max_chars(&payload, AssistMode::Draft), Some(300));
    assert_eq!(parse_max_chars(&payload, AssistMode::Rewrite), None);
}

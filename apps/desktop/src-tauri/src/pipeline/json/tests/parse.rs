//! [`parse`]'s error classification (`NotFound`/`Truncated`/`Syntax`/`Shape`)
//! and its content-free `Display`/`Debug` (ADR-027).

use super::super::*;
use super::support::Result;

#[test]
fn parses_a_fenced_prefixed_response() {
    let raw = "Here you go:\n```json\n{\"score\": 72, \"notes\": \"ok\"}\n```\nDone.";
    assert_eq!(
        parse::<Result>(raw).expect("parses"),
        Result {
            score: 72,
            notes: "ok".to_string()
        }
    );
}

#[test]
fn parses_only_after_repair_when_the_model_hand_wrote_the_json() {
    let raw = "{\u{201c}score\u{201d}: 72, \u{201c}notes\u{201d}: \u{201c}ok\u{201d},}";
    assert_eq!(
        parse::<Result>(raw).expect("repairs then parses"),
        Result {
            score: 72,
            notes: "ok".to_string()
        }
    );
}

#[test]
fn reports_not_found_for_a_prose_only_answer() {
    assert_eq!(
        parse::<Result>("I'm sorry, I can't do that.").unwrap_err(),
        JsonParseError::NotFound
    );
}

#[test]
fn reports_truncated_for_a_response_cut_off_mid_value() {
    // Distinct from NotFound on purpose: the fix is a shorter request, not
    // a re-ask with the same prompt.
    assert_eq!(
        parse::<Result>("{\"score\": 72, \"notes\": \"ok").unwrap_err(),
        JsonParseError::Truncated
    );
}

#[test]
fn reports_shape_not_syntax_when_the_json_is_valid_but_wrong() {
    // The schema was ignored, not the JSON format — different follow-up.
    let err = parse::<Result>(r#"{"score": "seventy", "notes": "ok"}"#).unwrap_err();
    assert!(
        matches!(err, JsonParseError::Shape(_)),
        "expected a shape error, got {err:?}"
    );
    let missing = parse::<Result>(r#"{"score": 72}"#).unwrap_err();
    assert!(
        matches!(missing, JsonParseError::Shape(_)),
        "a missing key is a shape error, got {missing:?}"
    );
}

#[test]
fn reports_syntax_for_json_repair_cannot_rescue() {
    let err = parse::<Result>(r#"{"score": 72 "notes": "ok"}"#).unwrap_err();
    assert!(
        matches!(err, JsonParseError::Syntax(_)),
        "expected a syntax error, got {err:?}"
    );
}

#[test]
fn display_never_leaks_model_content_but_detail_keeps_it_for_the_re_ask() {
    // ADR-027: `{e}` in a log line must not carry the model's own text.
    let err = parse::<Result>(r#"{"score": "SECRET-VALUE", "notes": "ok"}"#).unwrap_err();
    assert!(
        !err.to_string().contains("SECRET-VALUE"),
        "Display leaked model content: {err}"
    );
    assert_eq!(err.to_string(), err.reason());
    assert!(
        err.detail().contains("SECRET-VALUE"),
        "detail must keep the specifics for a re-ask: {}",
        err.detail()
    );
}

#[test]
fn debug_never_leaks_model_content_either() {
    // HIGH-2: `Display` was content-free but `#[derive(Debug)]` printed the
    // withheld fragment verbatim — and Debug is what `tracing::error!(error
    // = ?e)`, any `{e:?}`, and an `.expect()` panic message (which reaches
    // the crash reporter) actually print. Mutation check: restore
    // `#[derive(Debug)]` on `JsonParseError` and this fails — on the
    // equality assertion now rather than the leak one, because the
    // `RawDetail` payload is content-free under Debug too (defense in
    // depth: the derive prints `Shape(RawDetail(<withheld>))`).
    let err = parse::<Result>(r#"{"score": "SECRET-VALUE", "notes": "ok"}"#).unwrap_err();
    let debug = format!("{err:?}");
    assert!(!debug.contains("SECRET-VALUE"), "Debug leaked: {debug}");
    assert_eq!(debug, format!("Shape({:?})", err.reason()));
}

#[test]
fn the_payload_a_caller_can_reach_by_pattern_matching_is_content_free_too() {
    // MEDIUM: the two content-carrying variants held a bare `String`, and
    // a variant's fields inherit the ENUM's visibility — so any caller
    // could sidestep the content-free `Display`/`Debug` entirely with
    // `if let JsonParseError::Shape(detail) = &e { … }` and log the model's
    // own output. The payload is now a newtype whose field is private, so
    // the only way through is the `pub(crate)` `detail()` (or the fenced
    // `reask_detail()`), and what a caller CAN reach formats content-free.
    // Mutation check: `#[derive(Debug)]` on `RawDetail` and this fails.
    let err = parse::<Result>(r#"{"score": "SECRET-VALUE", "notes": "ok"}"#).unwrap_err();
    let (JsonParseError::Shape(raw) | JsonParseError::Syntax(raw)) = &err else {
        panic!("expected a content-carrying variant, got {err:?}");
    };
    let debug = format!("{raw:?}");
    assert!(
        !debug.contains("SECRET-VALUE"),
        "payload Debug leaked: {debug}"
    );
}

#[test]
fn the_raw_detail_payload_field_stays_private() {
    // The one property of this fix no runtime assertion can observe:
    // `RawDetail`'s field must stay PRIVATE, because that — not the
    // hand-written `Debug` — is what makes reading the raw message
    // impossible outside this module. A source pin is the cheapest guard
    // that fails on the mutation (same `include_str!` compile-time-pin
    // convention as `extension_bridge::answer_rewrite`'s translation
    // parity test); `include_str!` also makes rustc track the file, so the
    // test can never read a stale copy.
    const SRC: &str = include_str!("../../json.rs");
    // Assembled at runtime, never written out as one literal: an inline
    // needle would appear in THIS line and satisfy its own scan (it did,
    // on the first run — the test passed before the newtype existed).
    let private_field = format!("pub struct RawDetail({}String);", "");
    assert!(
        SRC.contains(&private_field),
        "RawDetail's field must stay private — `pub`/`pub(crate)` on it \
         re-opens the pattern-match leak the newtype exists to close"
    );
}

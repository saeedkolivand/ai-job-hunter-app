//! [`candidates`]'s extraction order (via the [`extract_json`] test helper),
//! and `repair_json`'s hand-written-JSON fixups.

use super::super::*;
use super::support::extract_json;

// ── the first candidate ───────────────────────────────────────────────

#[test]
fn extracts_a_bare_object() {
    assert_eq!(extract_json(r#"{"a":1}"#), Some(r#"{"a":1}"#));
}

#[test]
fn extracts_from_a_json_fence_and_a_bare_fence() {
    assert_eq!(extract_json("```json\n{\"a\":1}\n```"), Some("{\"a\":1}"));
    assert_eq!(extract_json("```\n[1,2]\n```"), Some("[1,2]"));
}

#[test]
fn prefers_the_fenced_body_over_prose_braces_before_it() {
    // The prose `{score, notes}` is balanced but is not JSON — searching
    // the fence first is what keeps it from winning.
    let raw = "I'll return {score, notes}:\n```json\n{\"a\":1}\n```";
    assert_eq!(extract_json(raw), Some("{\"a\":1}"));
}

#[test]
fn strips_prose_before_and_after_an_unfenced_object() {
    let raw = "Sure! Here it is:\n{\"a\":1}\nHope that helps.";
    assert_eq!(extract_json(raw), Some("{\"a\":1}"));
}

#[test]
fn is_not_confused_by_braces_and_brackets_inside_strings() {
    // The regression the two parsers this replaces both had: a `}` inside
    // a string value ended the extraction early.
    let raw = r#"{"note":"use {} and [] here","a":1}"#;
    assert_eq!(extract_json(raw), Some(raw));
    // …including an escaped quote right before the decoy brace.
    let escaped = r#"{"note":"a \" then }","a":1}"#;
    assert_eq!(extract_json(escaped), Some(escaped));
}

#[test]
fn keeps_nesting_balanced_across_both_delimiter_kinds() {
    let raw = r#"{"items":[{"id":"x"}],"n":1}"#;
    assert_eq!(extract_json(raw), Some(raw));
}

#[test]
fn is_none_for_prose_and_for_a_truncated_value() {
    assert_eq!(extract_json("I cannot help with that."), None);
    assert_eq!(extract_json("```json\n{\"a\":1"), None);
}

// ── repair_json ───────────────────────────────────────────────────────

#[test]
fn repair_leaves_valid_json_untouched() {
    let valid = "{\"a\":1,\"b\":[1,2],\"c\":\"x\"}";
    assert_eq!(repair_json(valid), valid);
    // Idempotent — a second pass changes nothing either.
    assert_eq!(repair_json(&repair_json(valid)), valid);
}

#[test]
fn repair_drops_trailing_commas_in_objects_and_arrays() {
    assert_eq!(repair_json("{\"a\":1,}"), "{\"a\":1}");
    assert_eq!(repair_json("[1,2, ]"), "[1,2]");
    // Nested, and with the whitespace between the comma and the closer:
    // the comma AND that whitespace go (whitespace is not significant).
    assert_eq!(repair_json("{\"a\":[1,2,],\n}"), "{\"a\":[1,2]}");
}

#[test]
fn repair_rewrites_smart_and_single_quotes_used_as_delimiters() {
    assert_eq!(
        repair_json("{\u{201c}a\u{201d}: \u{201c}b\u{201d}}"),
        "{\"a\": \"b\"}"
    );
    assert_eq!(repair_json("{'a': 'b'}"), "{\"a\": \"b\"}");
}

#[test]
fn repair_preserves_a_curly_quote_that_is_real_string_content() {
    // The whole point of being string-aware: an apostrophe or curly quote
    // INSIDE a value is content, not a delimiter.
    let raw = "{\"a\": \"it\u{2019}s \u{201c}fine\u{201d}\"}";
    assert_eq!(repair_json(raw), raw);
}

#[test]
fn repair_strips_a_bom_and_normalizes_nbsp_outside_strings() {
    assert_eq!(repair_json("\u{feff}{\u{a0}\"a\":1}"), "{ \"a\":1}");
    // …but an NBSP inside a string is legal content and stays.
    assert_eq!(repair_json("{\"a\":\"x\u{a0}y\"}"), "{\"a\":\"x\u{a0}y\"}");
}

#[test]
fn repair_escapes_a_raw_newline_inside_a_string() {
    assert_eq!(repair_json("{\"a\":\"one\ntwo\"}"), "{\"a\":\"one\\ntwo\"}");
}

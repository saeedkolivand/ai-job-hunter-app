//! Cursor validation tests: malformed/wrong-scope/wrong-filter cursors, absent/null defaults,
//! colon-bearing ids.

use super::super::*;
use super::support::*;

#[test]
fn found_jobs_rejects_a_non_numeric_cursor_rather_than_silently_resetting() {
    let err = parse_found_jobs_cursor(&json!({ "cursor": "not-a-number" }), "ap-1").unwrap_err();
    assert_eq!(err.to_string(), MALFORMED_CURSOR_MESSAGE);
}

/// A value that HAS a colon but is not a cursor (its head is not the
/// requested scope and its tail is not an offset) reads as malformed,
/// never as "another scope issued this" — the shape is checked before
/// the issuer for exactly this reason.
#[test]
fn found_jobs_reads_a_colon_bearing_non_cursor_as_malformed_not_as_another_scopes() {
    let err = parse_found_jobs_cursor(&json!({ "cursor": "https://jobs.example/x" }), "ap-1")
        .unwrap_err();
    assert_eq!(err.to_string(), MALFORMED_CURSOR_MESSAGE);
}

/// HIGH fix, pre-PR review round 2 — `{"cursor": 100}` (a JSON NUMBER,
/// not a string) used to collapse silently to offset 0 via
/// `.and_then(Value::as_str)` returning `None` for a non-string just
/// like it does for an absent key. Must now be a clean rejection, never
/// a silent restart of the traversal.
#[test]
fn found_jobs_rejects_a_numeric_cursor_rather_than_silently_resetting() {
    let err = parse_found_jobs_cursor(&json!({ "cursor": 100 }), "ap-1").unwrap_err();
    assert_eq!(err.to_string(), MALFORMED_CURSOR_MESSAGE);
}

#[test]
fn found_jobs_cursor_defaults_to_zero_when_absent() {
    assert_eq!(parse_found_jobs_cursor(&json!({}), "ap-1").unwrap(), 0);
}

/// An explicit JSON `null` is absent-like, not a type error — mirrors
/// `mcp.rs`'s `tool_argv` treating a `null` `cursor` argument the same
/// way rather than forwarding the literal string `"null"`.
#[test]
fn found_jobs_cursor_null_is_treated_like_absent() {
    assert_eq!(
        parse_found_jobs_cursor(&json!({ "cursor": null }), "ap-1").unwrap(),
        0
    );
}

/// The issue #1130 repro, still valid under #1168's optional
/// `autopilotId`: a cursor a LONG list issued, replayed against a
/// SHORT one, used to be read as a valid deep offset into the wrong
/// list. The cursor is taken from a real `resolve_found_jobs` reply,
/// never hand-built, so this fails if the two halves of the format
/// ever stop agreeing.
#[test]
fn found_jobs_rejects_a_cursor_issued_for_a_different_autopilot() {
    let long = autopilot_with_jobs("ap-1", (0..30).map(numbered_job).collect());
    let short = autopilot_with_jobs("ap-2", (0..3).map(numbered_job).collect());
    let records = vec![long, short];
    let issued = resolve_found_jobs(&records, Some("ap-1"), &no_filters(), &no_applied(), 0, 10)
        .expect("page 1")["nextCursor"]
        .as_str()
        .expect("ap-1 has more pages")
        .to_string();

    let err = parse_found_jobs_cursor(&json!({ "cursor": issued }), "ap-2").unwrap_err();
    // MEDIUM fix, review round 4 — the two refusals carry DIFFERENT fixed
    // texts: this one still has a list it pages, the malformed one does
    // not. Neither ever echoes the caller's value.
    assert_eq!(err.to_string(), WRONG_AUTOPILOT_CURSOR_MESSAGE);
    assert_ne!(WRONG_AUTOPILOT_CURSOR_MESSAGE, MALFORMED_CURSOR_MESSAGE);
    for message in [WRONG_AUTOPILOT_CURSOR_MESSAGE, MALFORMED_CURSOR_MESSAGE] {
        assert!(
            !message.contains("ap-1") && !message.contains("ap-2"),
            "a refusal never echoes the cursor or the id it named: {message}"
        );
    }
}

/// The pre-#1130 wire shape. Rejected, NOT accepted for compatibility —
/// accepting a bare offset would leave the cross-autopilot hole open for
/// exactly the callers most likely to still be mid-traversal.
#[test]
fn found_jobs_rejects_a_bare_numeric_offset_cursor() {
    let err = parse_found_jobs_cursor(&json!({ "cursor": "10" }), "ap-1").unwrap_err();
    assert_eq!(err.to_string(), MALFORMED_CURSOR_MESSAGE);
}

/// An id containing `:` still round-trips — the reason the parser splits
/// from the RIGHT. Pins the property, not today's UUID id format.
#[test]
fn found_jobs_cursor_round_trips_an_id_containing_a_colon() {
    let records = vec![autopilot_with_jobs(
        "ns:ap:1",
        (0..5).map(numbered_job).collect(),
    )];
    let issued = resolve_found_jobs(
        &records,
        Some("ns:ap:1"),
        &no_filters(),
        &no_applied(),
        0,
        2,
    )
    .expect("page 1")["nextCursor"]
        .as_str()
        .expect("more pages")
        .to_string();
    assert_eq!(
        parse_found_jobs_cursor(&json!({ "cursor": issued }), &issuer(Some("ns:ap:1"))).unwrap(),
        2
    );
}

// ── issue #1168: autopilotId optional, spans every autopilot ──────────

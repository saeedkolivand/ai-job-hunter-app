use serde_json::json;

use super::*;

// ── clamp_limit ───────────────────────────────────────────────────────

/// Both bounds AND every "no usable limit" shape in one place. The
/// zero/negative/string/absent cases all land on `default` and NEVER on
/// "unbounded" — the one interpretation that would defeat paging
/// entirely (`agent-cli-standards`: an empty variable must never widen a
/// selector).
#[test]
fn clamp_limit_defaults_every_unusable_shape_and_caps_at_the_max() {
    for payload in [
        json!({}),
        json!({ "limit": 0 }),
        json!({ "limit": -5 }),
        json!({ "limit": "20" }),
        json!({ "limit": null }),
    ] {
        assert_eq!(
            clamp_limit(&payload, 20, 100),
            20,
            "must fall back to the default, never to unbounded: {payload}"
        );
    }
    assert_eq!(clamp_limit(&json!({ "limit": 7 }), 20, 100), 7);
    assert_eq!(clamp_limit(&json!({ "limit": 100 }), 20, 100), 100);
    assert_eq!(
        clamp_limit(&json!({ "limit": 10_000 }), 20, 100),
        100,
        "an over-max limit is capped, not honoured"
    );
}

// ── parse_offset_cursor ──────────────────────────────────────────

#[test]
fn parse_offset_cursor_accepts_an_absent_null_or_digit_string_cursor() {
    assert_eq!(parse_offset_cursor(&json!({})), Some(0));
    assert_eq!(parse_offset_cursor(&json!({ "cursor": null })), Some(0));
    assert_eq!(parse_offset_cursor(&json!({ "cursor": "0" })), Some(0));
    assert_eq!(parse_offset_cursor(&json!({ "cursor": "40" })), Some(40));
}

/// The rejection side, INCLUDING the JSON-number case that used to
/// collapse silently to page 1 (this fn's own doc). A rejected cursor is
/// `None`, never `Some(0)` — restarting a traversal while looking like
/// forward progress is how a paging loop turns into an infinite one.
#[test]
fn parse_offset_cursor_rejects_anything_that_is_not_a_non_negative_integer_string() {
    for payload in [
        json!({ "cursor": 100 }),
        json!({ "cursor": -1 }),
        json!({ "cursor": "-1" }),
        json!({ "cursor": "12.5" }),
        json!({ "cursor": "abc" }),
        json!({ "cursor": "" }),
        json!({ "cursor": true }),
        json!({ "cursor": ["40"] }),
        json!({ "cursor": { "offset": 40 } }),
    ] {
        assert_eq!(
            parse_offset_cursor(&payload),
            None,
            "must refuse rather than silently restart at 0: {payload}"
        );
    }
}

// ── fingerprint ──────────────────────────────────────────────────

#[test]
fn fingerprint_is_deterministic_and_distinguishes_different_inputs() {
    assert_eq!(
        fingerprint(&["ap-1", "70", "germany"]),
        fingerprint(&["ap-1", "70", "germany"]),
        "the same parts must always produce the same fingerprint"
    );
    assert_ne!(
        fingerprint(&["ap-1", "70", "germany"]),
        fingerprint(&["ap-1", "90", "germany"]),
        "a changed part must change the fingerprint"
    );
    // Different part BOUNDARIES, same concatenated bytes — the
    // separator is what keeps these apart.
    assert_ne!(
        fingerprint(&["ab", "c"]),
        fingerprint(&["a", "bc"]),
        "part boundaries must matter, not just the concatenated bytes"
    );
    assert_ne!(
        fingerprint(&[""]),
        fingerprint(&[]),
        "an empty part must differ from no parts at all"
    );
}

// ── trim_to_byte_budget ──────────────────────────────────────────

/// The forward-progress guarantee: a single row larger than the WHOLE
/// budget still survives, because a page of zero rows whose `nextCursor`
/// never advanced would hang every traversal built on this forever. The
/// second half pins that this is the only case where the budget is
/// exceeded — rows past the first are still dropped.
#[test]
fn trim_to_byte_budget_keeps_at_least_one_row_and_drops_the_rest() {
    let huge = json!({ "text": "x".repeat(500) });
    let trimmed = trim_to_byte_budget(vec![huge.clone(), huge.clone(), huge], 0, 100);
    assert_eq!(
        trimmed.len(),
        1,
        "exactly one row survives an impossible budget"
    );

    // An empty input stays empty — "at least one" is never "invent one".
    assert!(trim_to_byte_budget(Vec::new(), 0, 100).is_empty());

    // Under budget: nothing is dropped.
    let small = vec![json!({ "id": 1 }), json!({ "id": 2 })];
    assert_eq!(trim_to_byte_budget(small.clone(), 0, 10_000), small);

    // `base_cost` really is subtracted from the same budget — the same
    // rows fit with no envelope cost and stop fitting with a large one.
    let rows: Vec<Value> = (0..20).map(|i| json!({ "id": i })).collect();
    let with_no_base = trim_to_byte_budget(rows.clone(), 0, 200).len();
    let with_big_base = trim_to_byte_budget(rows, 190, 200).len();
    assert!(
        with_big_base < with_no_base,
        "a larger base_cost must leave less room for rows ({with_big_base} !< {with_no_base})"
    );
}

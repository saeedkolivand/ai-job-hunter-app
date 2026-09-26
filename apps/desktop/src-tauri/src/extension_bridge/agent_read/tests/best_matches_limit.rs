//! Tests for `best-matches`' `limit` clamp (`best_matches.rs`).

use super::super::*;
use super::best_matches_projection::full_best_match_row_json;

#[test]
fn best_matches_limit_clamps_to_the_server_max() {
    let payload = json!({ "resource": "best-matches", "limit": 5_000 });
    assert_eq!(
        best_matches::clamp_best_matches_limit(&payload),
        best_matches::MAX_BEST_MATCHES_LIMIT
    );
}

#[test]
fn best_matches_limit_defaults_when_absent() {
    let payload = json!({ "resource": "best-matches" });
    assert_eq!(
        best_matches::clamp_best_matches_limit(&payload),
        best_matches::DEFAULT_BEST_MATCHES_LIMIT
    );
}

/// Regression for the hand-rolled clamp this now-shared one replaced: a
/// `limit: 0` used to read as `Some(0)` off `Value::as_u64` and slip past
/// `.unwrap_or`, returning 0 rows per page forever — a page whose
/// `nextCursor` never advances hangs any paging loop. `0` must fall back to
/// the default, same as an absent limit.
#[test]
fn best_matches_limit_zero_falls_back_to_the_default_not_to_zero() {
    let payload = json!({ "resource": "best-matches", "limit": 0 });
    assert_eq!(
        best_matches::clamp_best_matches_limit(&payload),
        best_matches::DEFAULT_BEST_MATCHES_LIMIT
    );
}

/// B3-r3-F2 — `MAX_BEST_MATCHES_LIMIT` must reach the full row set
/// `commands::autopilot::best_matches::BEST_MATCHES_CAP` (100) allows
/// through, in ONE page: that command's clustering pass is real CPU work
/// (its own doc — 3.03s at 2000 found-jobs, 12.3s at 4000), and the
/// 30s-refill throttle bucket is sized for exactly one call per traversal.
/// Before this fix `MAX_BEST_MATCHES_LIMIT` was half the cap, so a max-limit
/// page never reached the end in one call — this fails against that value
/// (both on the length assertion and on `nextCursor` staying non-null).
#[test]
fn max_best_matches_limit_covers_the_full_capped_row_set_in_one_page() {
    // Mirrors `commands::autopilot::best_matches::BEST_MATCHES_CAP` — that
    // const is private to a sibling module this file doesn't own, so this is
    // a literal pin, not an import; the two must be kept in sync by hand.
    const BEST_MATCHES_CAP: usize = 100;
    assert_eq!(
        best_matches::MAX_BEST_MATCHES_LIMIT,
        BEST_MATCHES_CAP,
        "a max-limit page must cover the whole capped row set in one call"
    );

    let rows: Vec<Value> = (0..BEST_MATCHES_CAP)
        .map(|i| {
            let mut row = full_best_match_row_json();
            row["url"] = json!(format!("https://boards.example.com/jobs/{i}"));
            row
        })
        .collect();
    let out =
        best_matches::resolve_best_matches(&rows, 0, best_matches::MAX_BEST_MATCHES_LIMIT, None);
    assert_eq!(
        out["matches"].as_array().unwrap().len(),
        BEST_MATCHES_CAP,
        "every row of the capped set must fit in one max-limit page"
    );
    assert!(
        out["nextCursor"].is_null(),
        "a single max-limit page must reach the true end, not need a second call"
    );
}

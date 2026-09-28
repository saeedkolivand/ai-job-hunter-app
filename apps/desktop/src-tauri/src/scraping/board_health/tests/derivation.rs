//! Pure-`fold` derivation tests: streak open/close, `skipped` neutrality,
//! and the run-id correlation stamp. See [`super::flaky_window`] for the
//! decayed flaky-rate window.

use super::super::fold::{fold, MAX_ERROR_LEN};
use super::super::*;
use super::support::{failed, fold_at, ok, skipped, DAY, T0};

// ── derivation (pure `fold`) ────────────────────────────────────────────────

#[test]
fn a_board_that_fails_then_succeeds_clears_its_streak() {
    // Three consecutive failures, one per day.
    let mut h = fold_at(None, &failed("wwr", "HTTP 500"), T0);
    h = fold_at(Some(h), &failed("wwr", "HTTP 500"), T0 + DAY);
    h = fold_at(Some(h), &failed("wwr", "HTTP 429"), T0 + 2 * DAY);

    assert_eq!(h.consecutive_failures, 3);
    assert_eq!(h.status, BoardHealthStatus::Failing);
    // "Failing SINCE" is the FIRST failure of the streak, not the latest.
    assert_eq!(h.failing_since, Some(T0));
    assert_eq!(h.last_verified_at, Some(T0 + 2 * DAY));
    assert_eq!(h.last_success_at, None);
    // The remembered reason tracks the LATEST failure, so the chip explains the
    // current breakage rather than a stale first one.
    assert_eq!(h.last_error.as_deref(), Some("HTTP 429"));

    // Then it works again.
    let h = fold_at(Some(h), &ok("wwr", 12), T0 + 3 * DAY);
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.status, BoardHealthStatus::Healthy);
    assert_eq!(h.failing_since, None, "a success closes the streak window");
    assert_eq!(h.last_error, None, "a success clears the stale reason");
    assert_eq!(h.last_success_at, Some(T0 + 3 * DAY));
    assert_eq!(h.last_verified_at, Some(T0 + 3 * DAY));
}

#[test]
fn a_successful_run_with_zero_results_is_still_a_success() {
    // The whole point of the feature: "found nothing" is NOT "broken".
    let h = fold_at(None, &ok("remotive", 0), T0);
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.last_success_at, Some(T0));
    assert_eq!(h.status, BoardHealthStatus::Healthy);
    assert!(!h.is_noteworthy(), "a healthy board must not badge");
}

#[test]
fn a_board_that_has_never_succeeded_reports_no_last_success() {
    let h = fold_at(None, &failed("linkedin", "blocked"), T0);
    assert_eq!(h.consecutive_failures, 1);
    assert_eq!(h.last_success_at, None);
    assert_eq!(h.failing_since, Some(T0));
    assert_eq!(h.last_verified_at, Some(T0));
    assert_eq!(h.status, BoardHealthStatus::Failing);
    assert!(h.is_noteworthy());
}

#[test]
fn a_skipped_board_is_not_a_failure_and_is_not_a_success() {
    // A board that is only ever skipped verifies NOTHING — it must not be
    // reported as broken, and it must not be reported as working.
    let h = fold_at(None, &skipped("greenhouse", "needs-company"), T0);
    assert_eq!(h.consecutive_failures, 0, "a skip is not a failure");
    assert_eq!(h.last_success_at, None, "a skip is not a success");
    assert_eq!(h.last_verified_at, None, "a skip verifies nothing");
    assert_eq!(h.failing_since, None);
    assert_eq!(h.status, BoardHealthStatus::Unknown);
    assert!(!h.is_noteworthy(), "an unverified board must not badge");
}

#[test]
fn a_skip_neither_extends_nor_clears_an_existing_failure_streak() {
    // Broken on day 0 and day 1, then skipped (session expired) for two days.
    let mut h = fold_at(None, &failed("linkedin", "HTTP 999"), T0);
    h = fold_at(Some(h), &failed("linkedin", "HTTP 999"), T0 + DAY);
    h = fold_at(Some(h), &skipped("linkedin", "needs-login"), T0 + 2 * DAY);
    h = fold_at(Some(h), &skipped("linkedin", "needs-login"), T0 + 3 * DAY);

    assert_eq!(
        h.consecutive_failures, 2,
        "skips must not extend the streak"
    );
    assert_eq!(
        h.failing_since,
        Some(T0),
        "still broken since the FIRST failure"
    );
    assert_eq!(
        h.last_verified_at,
        Some(T0 + DAY),
        "the last time we actually contacted the board was the last real attempt"
    );
    assert_eq!(h.status, BoardHealthStatus::Failing);
    assert_eq!(
        h.last_error.as_deref(),
        Some("HTTP 999"),
        "a skip must not erase why the board is unhealthy"
    );
}

#[test]
fn a_board_only_skipped_since_its_last_success_goes_stale() {
    let h = fold_at(None, &ok("xing", 3), T0);
    assert_eq!(h.status, BoardHealthStatus::Healthy);

    // 15 days later, still nothing but skips: the success is no longer evidence.
    let h = fold_at(Some(h), &skipped("xing", "needs-login"), T0 + 15 * DAY);
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.last_success_at, Some(T0), "the old success is retained");
    assert_eq!(h.status, BoardHealthStatus::Stale);
    assert!(h.is_noteworthy(), "a stale board must badge");

    // One day earlier it is still inside the fortnight window and stays healthy.
    let fresh = fold_at(
        Some(fold_at(None, &ok("xing", 3), T0)),
        &skipped("xing", "needs-login"),
        T0 + 13 * DAY,
    );
    assert_eq!(fresh.status, BoardHealthStatus::Healthy);
}

#[test]
fn an_error_outranks_a_simultaneous_skip_on_a_tampered_record() {
    // The engine never sets both; a hand-edited persisted record could.
    let mut s = failed("wwr", "HTTP 500");
    s.skipped = Some("needs-login".to_string());
    let h = fold_at(None, &s, T0);
    assert_eq!(h.consecutive_failures, 1);
    assert_eq!(h.status, BoardHealthStatus::Failing);
}

#[test]
fn a_partial_harvest_counts_as_a_working_board() {
    // `truncated` means the board answered and returned rows before a later page
    // failed — it is reachable, which is what "does this source work?" asks.
    let mut s = ok("aggregator", 40);
    s.truncated = Some("page 3 of 5 failed: HTTP 429".to_string());
    let h = fold_at(None, &s, T0);
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.last_success_at, Some(T0));
    assert_eq!(h.status, BoardHealthStatus::Healthy);
}

#[test]
fn a_long_failure_reason_is_capped_without_splitting_a_codepoint() {
    // Non-ASCII on purpose: a byte-range slice would panic mid-codepoint.
    let long = "ü".repeat(MAX_ERROR_LEN + 50);
    let h = fold_at(None, &failed("wwr", &long), T0);
    let stored = h.last_error.expect("a failure records its reason");
    assert_eq!(
        stored.chars().count(),
        MAX_ERROR_LEN + 1,
        "cap + one ellipsis"
    );
    assert!(stored.ends_with('…'));
}

#[test]
fn the_run_id_names_only_a_run_that_actually_contacted_the_board() {
    // Run A fails; run B skips the board entirely (session expired). `last_run_id`
    // means "the run that PRODUCED this state" — B produced nothing, and grepping
    // logs for B would find no fetch of this board at all.
    let h = fold(None, &failed("linkedin", "HTTP 999"), "job-a", T0);
    assert_eq!(h.last_run_id.as_deref(), Some("job-a"));

    let h = fold(
        Some(h),
        &skipped("linkedin", "needs-login"),
        "job-b",
        T0 + DAY,
    );
    assert_eq!(
        h.last_run_id.as_deref(),
        Some("job-a"),
        "a skip must not re-attribute the state to a run that never fetched it"
    );
    // The state it points at is unchanged too.
    assert_eq!(h.failing_since, Some(T0));
    assert_eq!(h.last_error.as_deref(), Some("HTTP 999"));

    // A run that DOES contact the board takes ownership again.
    let h = fold(Some(h), &ok("linkedin", 4), "job-c", T0 + 2 * DAY);
    assert_eq!(h.last_run_id.as_deref(), Some("job-c"));
}

#[test]
fn a_flapping_board_is_reported_even_though_its_streak_keeps_resetting() {
    // ok/fail/ok/fail… — `consecutive_failures` is 0 on every other run, so a
    // streak counter alone would call this board healthy forever.
    let mut h = None;
    let mut at = T0;
    // Ends on a SUCCESS (i == 11 is odd), so the streak really is empty.
    for i in 0..12 {
        let s = if i % 2 == 0 {
            failed("wwr", "HTTP 502")
        } else {
            ok("wwr", 3)
        };
        h = Some(fold_at(h, &s, at));
        at += DAY;
    }
    let h = h.unwrap();

    // The run ended on a SUCCESS, so the streak is genuinely empty…
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.verified_runs, 12);
    assert_eq!(h.failed_runs, 6);
    // …but half its runs failed, which is what the user needs told.
    assert_eq!(h.status, BoardHealthStatus::Flaky);
    assert!(h.is_noteworthy(), "a flapping board must badge");
}

#[test]
fn a_board_with_one_bad_run_in_a_long_healthy_life_is_not_called_flaky() {
    // 1 failure in 15 verified runs = 6.7%, under FLAKY_FAIL_PERCENT, and
    // under FLAKY_WINDOW_CAP so decay_tallies never touches it — this test is
    // about the RATE gate, not the decay window (see the `decay_tallies`
    // section below for that).
    let mut h = Some(fold_at(None, &failed("wwr", "HTTP 502"), T0));
    let mut at = T0 + DAY;
    for _ in 0..14 {
        h = Some(fold_at(h, &ok("wwr", 3), at));
        at += DAY;
    }
    let h = h.unwrap();
    assert_eq!(h.verified_runs, 15);
    assert_eq!(h.failed_runs, 1);
    assert_eq!(h.status, BoardHealthStatus::Healthy);
}

#[test]
fn a_failure_rate_below_the_minimum_sample_is_not_yet_a_verdict() {
    // 3 of 6 runs failed — a 50% rate, but on too small a sample to brand a
    // board. `FLAKY_MIN_RUNS` is what stops a two-run install badging everything.
    let mut h = None;
    let mut at = T0;
    for i in 0..6 {
        let s = if i % 2 == 0 {
            failed("wwr", "HTTP 502")
        } else {
            ok("wwr", 3)
        };
        h = Some(fold_at(h, &s, at));
        at += DAY;
    }
    let h = h.unwrap();
    assert_eq!(h.verified_runs, 6);
    assert_eq!(h.failed_runs, 3);
    assert_eq!(h.status, BoardHealthStatus::Healthy);

    // Prove it is the SAMPLE SIZE holding the verdict back and not the rate:
    // keep the identical 50/50 alternation running and the same board flips.
    let mut h = Some(h);
    let mut at = T0 + 6 * DAY;
    for i in 6..14 {
        let s = if i % 2 == 0 {
            failed("wwr", "HTTP 502")
        } else {
            ok("wwr", 3)
        };
        h = Some(fold_at(h, &s, at));
        at += DAY;
    }
    let h = h.unwrap();
    assert_eq!(h.verified_runs, 14);
    assert_eq!(h.failed_runs, 7);
    assert_eq!(h.status, BoardHealthStatus::Flaky);
}

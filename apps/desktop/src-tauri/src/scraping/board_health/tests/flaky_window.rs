//! Flaky-rate + decayed rolling-window (`decay_tallies`) tests — a
//! consecutive-failure streak alone cannot see an alternating ok/fail
//! pattern; see [`super::super::fold`] module docs for the rationale.

use super::super::fold::{decay_tallies, is_flaky, FLAKY_WINDOW_CAP, STALE_AFTER_MS};
use super::super::*;
use super::support::{failed, fold_at, ok, skipped, DAY, T0};

// ── decay_tallies (MEDIUM 1: bounded rolling window, not lifetime totals) ───

#[test]
fn a_recovered_board_stops_reading_flaky_within_the_decay_window() {
    // A 10-run outage…
    let mut h = None;
    let mut at = T0;
    for _ in 0..10 {
        h = Some(fold_at(h, &failed("wwr", "HTTP 500"), at));
        at += DAY;
    }
    // …then the board is fixed and works PERFECTLY every run afterwards.
    // Before this fix the LIFETIME ratio (10 failed / N verified) kept the
    // board reading `unreliable · failed 10 of N runs` for 31 consecutive
    // flawless runs, clearing only once `verified_runs` reached 41. The
    // decayed window bounds that to FLAKY_WINDOW_CAP.
    for _ in 0..15 {
        h = Some(fold_at(h, &ok("wwr", 3), at));
        at += DAY;
    }
    assert_eq!(
        h.clone().unwrap().status,
        BoardHealthStatus::Flaky,
        "15 flawless runs after a 10-run outage: still inside the window"
    );

    // The 16th flawless run must clear it — pin both sides of the boundary so
    // a regression back to unbounded tallies (or a wrong cap) is caught.
    let recovered = fold_at(h, &ok("wwr", 3), at);
    assert_eq!(recovered.status, BoardHealthStatus::Healthy);
    assert_eq!(recovered.verified_runs, 8);
    assert_eq!(recovered.failed_runs, 1);
}

#[test]
fn an_alternating_failure_pattern_is_flagged_within_the_decay_window() {
    // 100 clean runs establish the "long, boring history" precondition.
    let mut h = None;
    let mut at = T0;
    for _ in 0..100 {
        h = Some(fold_at(h, &ok("wwr", 3), at));
        at += DAY;
    }

    // Then the board starts failing every OTHER run. Every FAIL run correctly
    // reads `Failing` on its own (that part was never broken) — the gap is
    // that every OK run in between resets `consecutive_failures` to 0, so only
    // the RATE can catch the alternation on a run that itself succeeded, and
    // `derive_status` only consults the rate once the streak is empty. Before
    // this fix that rate took 100 more runs / 50 more failures (measured
    // against the undecayed `is_flaky` before this fix) to cross
    // FLAKY_FAIL_PERCENT on a recovered run at all. The decayed window
    // catches it in 14 more runs / 7 more failures.
    for _ in 0..6 {
        h = Some(fold_at(h, &failed("wwr", "HTTP 502"), at));
        at += DAY;
        h = Some(fold_at(h, &ok("wwr", 3), at));
        at += DAY;
        assert_ne!(
            h.clone().unwrap().status,
            BoardHealthStatus::Flaky,
            "not yet — still inside the tolerance"
        );
    }
    // The 7th failure (14th run of the alternation, itself an OK run) crosses
    // the threshold.
    h = Some(fold_at(h, &failed("wwr", "HTTP 502"), at));
    at += DAY;
    let flagged = fold_at(h, &ok("wwr", 3), at);
    assert_eq!(flagged.status, BoardHealthStatus::Flaky);
    assert_eq!(flagged.verified_runs, 15);
    assert_eq!(flagged.failed_runs, 4);
}

#[test]
fn decay_never_manufactures_a_flaky_verdict_out_of_pure_rounding() {
    // v=17, f=4 is 23.5% — correctly NOT flaky. Naive INDEPENDENT halving
    // (`f / 2`) rounds `f` down less than `v` (odd) rounds down, inflating the
    // ratio to exactly 25% and flipping the verdict on what was, from the
    // board's point of view, a plain success. Proportional rescaling can only
    // round the ratio DOWN, never up.
    let (v, f) = decay_tallies(17, 4);
    let h = BoardHealth {
        verified_runs: v,
        failed_runs: f,
        ..BoardHealth::empty()
    };
    assert!(
        !is_flaky(&h),
        "decay alone must never manufacture a Flaky verdict; got v={v} f={f}"
    );
}

#[test]
fn decay_collapses_an_oversized_legacy_tally_in_one_fold_call() {
    // A row written before this fix could carry an arbitrarily large lifetime
    // `verified_runs`. `decay_tallies` must not need dozens of future runs to
    // bring it back under the cap — one fold call must do it (hence `while`,
    // not `if`, inside `decay_tallies`).
    let legacy = BoardHealth {
        verified_runs: 1000,
        failed_runs: 500,
        ..BoardHealth::empty()
    };
    let h = fold_at(Some(legacy), &ok("wwr", 1), T0);
    assert!(
        h.verified_runs <= FLAKY_WINDOW_CAP,
        "one fold call must collapse an oversized legacy tally; got {}",
        h.verified_runs
    );
}

#[test]
fn decay_preserves_the_failure_rate_of_a_huge_tally_not_just_the_cap() {
    // The sibling test above pins only that the CAP is respected, so it stays
    // green while the ratio is destroyed. That is exactly what happened: in
    // `u32`, `failed * next_verified` overflows past ~4.29e9 and
    // `saturating_mul` clamps BEFORE the divide, so a board that failed 75% of
    // 200_000 runs decayed to 2-in-12 (17%) and read HEALTHY.
    //
    // Anchored to an absolute expectation (a rate near 75%), not to whatever
    // the function happens to return, so a regression cannot move both sides.
    let (verified, failed) = decay_tallies(200_000, 150_000);
    assert!(
        verified <= FLAKY_WINDOW_CAP,
        "cap must still hold; got {verified}"
    );
    // 200_000 takes 14 halvings to reach the cap, and floor-scaling can only
    // round DOWN, so 75% legitimately erodes to ~67%. The absolute floor that
    // matters is that a board failing three runs in four must not come out the
    // other side looking better than a coin flip. The overflow bug produced
    // 17%, which fails this; a correct rescale cannot.
    let pct = failed * 100 / verified;
    assert!(
        pct >= 50,
        "a 75% failure rate must not decay below a coin flip; got {failed}/{verified} = {pct}%"
    );
    assert!(
        is_flaky(&BoardHealth {
            verified_runs: verified,
            failed_runs: failed,
            ..BoardHealth::empty()
        }),
        "a board failing three runs in four must still read flaky after decay"
    );
}

#[test]
fn skips_do_not_count_toward_either_run_tally() {
    let mut h = Some(fold_at(None, &ok("wwr", 3), T0));
    for i in 1..=5 {
        h = Some(fold_at(h, &skipped("wwr", "needs-login"), T0 + i * DAY));
    }
    let h = h.unwrap();
    assert_eq!(h.verified_runs, 1, "only the contacting run is counted");
    assert_eq!(h.failed_runs, 0);
}

#[test]
fn the_stale_boundary_is_strictly_greater_than_the_window() {
    // Pin BOTH sides of the comparison, so a `>` → `>=` flip is caught.
    let base = fold_at(None, &ok("xing", 3), T0);
    let exactly_at = fold_at(
        Some(base.clone()),
        &skipped("xing", "needs-login"),
        T0 + STALE_AFTER_MS,
    );
    assert_eq!(
        exactly_at.status,
        BoardHealthStatus::Healthy,
        "exactly at the window is still inside it"
    );
    let one_ms_past = fold_at(
        Some(base),
        &skipped("xing", "needs-login"),
        T0 + STALE_AFTER_MS + 1,
    );
    assert_eq!(one_ms_past.status, BoardHealthStatus::Stale);
}

#[test]
fn a_stored_reason_is_redacted_before_it_ever_reaches_the_row() {
    // The reason originates from an upstream `e.to_string()`. Persisting it raw
    // would put a filesystem path / URL / credential on disk for as long as the
    // streak lasts — indefinitely for a permanently-broken board.
    let raw = "GET https://api.example.com/v1?app_key=sk-abc123 failed for C:\\Users\\me\\ajh";
    let h = fold_at(None, &failed("wwr", raw), T0);
    let stored = h.last_error.expect("a failure records a reason");
    assert_eq!(
        stored, "GET <url-redacted> failed for <path-redacted>",
        "the reason must be redacted at the STORE, not only at display time"
    );
}

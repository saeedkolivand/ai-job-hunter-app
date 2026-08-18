//! Derivation + persistence tests for the per-board reliability history.
//!
//! Every assertion is anchored to an ABSOLUTE expected value (a literal
//! timestamp, a literal streak length, a literal status) rather than to a second
//! derived value — a regression that broke both sides of a `derived == derived`
//! comparison would keep such a test green.

use super::*;
use tempfile::TempDir;

/// A fixed epoch-ms base so every expected timestamp in this file is a literal.
/// 2026-01-01T00:00:00Z.
const T0: u64 = 1_767_225_600_000;
const DAY: u64 = 24 * 60 * 60 * 1000;

/// `fold` with a fixed run id — the derivation tests below are about the
/// counters and timestamps; run-id stamping has its own tests.
fn fold_at(prev: Option<BoardHealth>, summary: &BoardScrapeSummary, now: u64) -> BoardHealth {
    fold(prev, summary, "job-test", now)
}

fn ok(board: &str, count: usize) -> BoardScrapeSummary {
    BoardScrapeSummary {
        board: board.to_string(),
        count,
        error: None,
        skipped: None,
        truncated: None,
        note: None,
        health: None,
    }
}

fn failed(board: &str, reason: &str) -> BoardScrapeSummary {
    BoardScrapeSummary {
        error: Some(reason.to_string()),
        ..ok(board, 0)
    }
}

fn skipped(board: &str, reason: &str) -> BoardScrapeSummary {
    BoardScrapeSummary {
        skipped: Some(reason.to_string()),
        ..ok(board, 0)
    }
}

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

// ── persistence ────────────────────────────────────────────────────────────

fn open_store() -> (TempDir, BoardHealthStore) {
    let dir = TempDir::new().unwrap();
    let store = BoardHealthStore::open(dir.path()).unwrap();
    (dir, store)
}

/// A recovery must be written as NULLs, not merely computed as `None`.
///
/// The pure-`fold` recovery tests above can all pass while the UPSERT quietly
/// preserves the old `failing_since` / `last_error` (e.g. a `COALESCE(old, new)`
/// on either column), because nothing else in this file ever persists a
/// fail→success transition for the SAME board. The consequence is silent at
/// first — `derive_status` reads neither column, so the board still reports
/// `Healthy` — and then surfaces months later as "failing since <an outage from
/// three months ago>" the first time `fold`'s `get_or_insert` meets the stale
/// value on a one-run blip.
#[test]
fn a_recovery_clears_the_streak_columns_on_disk_not_just_in_memory() {
    let (_dir, store) = open_store();
    store
        .record_run("job-1", &[failed("wwr", "HTTP 500")])
        .unwrap();
    store
        .record_run("job-2", &[failed("wwr", "HTTP 429")])
        .unwrap();
    store.record_run("job-3", &[ok("wwr", 7)]).unwrap();

    // Read BACK from SQLite — an in-memory `fold` result would not catch a
    // write that failed to null the columns out.
    let h = store.health_for("wwr").expect("the board has history");
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.failing_since, None, "the streak window must be NULLed");
    assert_eq!(h.last_error, None, "the stale reason must be NULLed");
    assert_eq!(h.status, BoardHealthStatus::Healthy);
    assert!(
        h.last_success_at.is_some(),
        "the recovery run must be recorded as a success"
    );

    // And the next single failure opens a FRESH window at that failure, not at
    // the pre-recovery outage — the user-visible symptom of a leaked column.
    store
        .record_run("job-4", &[failed("wwr", "HTTP 503")])
        .unwrap();
    let after = store.health_for("wwr").unwrap();
    assert_eq!(after.consecutive_failures, 1);
    assert!(
        after.failing_since >= h.last_success_at,
        "a fresh streak must start at or after the recovery, not before it; \
         failing_since={:?} last_success_at={:?}",
        after.failing_since,
        h.last_success_at
    );
    assert_eq!(after.last_error.as_deref(), Some("HTTP 503"));
}

#[test]
fn a_streak_survives_across_runs_and_reopens() {
    let dir = TempDir::new().unwrap();
    {
        let store = BoardHealthStore::open(dir.path()).unwrap();
        store
            .record_run("job-1", &[failed("wwr", "HTTP 500")])
            .unwrap();
        store
            .record_run("job-2", &[failed("wwr", "HTTP 500")])
            .unwrap();
    }
    // Reopening re-runs the (idempotent) migration and keeps the history.
    let store = BoardHealthStore::open(dir.path()).unwrap();
    let h = store.health_for("wwr").expect("history survives a reopen");
    assert_eq!(h.consecutive_failures, 2);
    assert_eq!(h.status, BoardHealthStatus::Failing);
    assert_eq!(h.last_run_id.as_deref(), Some("job-2"));
    assert_eq!(h.last_error.as_deref(), Some("HTTP 500"));
}

#[test]
fn record_run_returns_health_positionally_and_keeps_boards_independent() {
    let (_dir, store) = open_store();
    store.record_run("job-1", &[failed("wwr", "boom")]).unwrap();

    let out = store
        .record_run(
            "job-2",
            &[
                failed("wwr", "boom"),
                ok("remotive", 5),
                skipped("greenhouse", "needs-company"),
            ],
        )
        .unwrap();

    assert_eq!(out.len(), 3, "one health per input summary, in input order");
    assert_eq!(out[0].consecutive_failures, 2);
    assert_eq!(out[0].status, BoardHealthStatus::Failing);
    assert_eq!(out[1].consecutive_failures, 0);
    assert_eq!(out[1].status, BoardHealthStatus::Healthy);
    assert_eq!(out[2].status, BoardHealthStatus::Unknown);
    assert_eq!(out[2].last_verified_at, None);
    // A board's failure must not bleed into its neighbours.
    assert_eq!(
        store.health_for("remotive").unwrap().consecutive_failures,
        0
    );
}

#[test]
fn the_same_board_twice_in_one_run_double_counts_it_todays_contract() {
    // `record_run` reads-folds-writes INSIDE the loop (one row per summary in
    // ONE transaction), so a duplicated board id in `summaries` folds the
    // second occurrence on top of the write the first one just made — one
    // run counts as two verified runs and the streak advances by two. The
    // engine's caller (`scrape_boards_with_resolver_and_overrides`) dedupes
    // `boards` before resolving, so this path is unreachable from the engine
    // today — but nothing in `record_run` itself enforces that, and the
    // module doc only obligates the caller to pre-filter to resolvable ids,
    // not to deduplicate them. Pinned here so a future direct caller (or an
    // engine change that drops the pre-dedupe) double-counts LOUDLY, in a
    // failing test, rather than silently.
    let (_dir, store) = open_store();
    let out = store
        .record_run("job-1", &[failed("wwr", "boom"), failed("wwr", "boom")])
        .unwrap();

    assert_eq!(
        out.len(),
        2,
        "one health per input summary, even duplicated"
    );
    assert_eq!(
        store.health_for("wwr").unwrap().consecutive_failures,
        2,
        "today's contract: an in-run duplicate is NOT deduped by record_run"
    );
    assert_eq!(store.health_for("wwr").unwrap().verified_runs, 2);
}

#[test]
fn the_table_holds_exactly_one_row_per_board_however_many_runs() {
    // The retention bound: rows are bounded by the board registry, not by time.
    let (_dir, store) = open_store();
    for i in 0..50 {
        store
            .record_run(
                &format!("job-{i}"),
                &[ok("wwr", i), failed("remotive", "x")],
            )
            .unwrap();
    }
    let conn = store.conn.lock();
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM board_health", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 2, "50 runs over 2 boards must leave exactly 2 rows");
}

#[test]
fn an_unseen_board_has_no_health() {
    let (_dir, store) = open_store();
    assert!(store.health_for("never-run").is_none());
}

#[test]
fn clear_all_wipes_every_board() {
    let (_dir, store) = open_store();
    store.record_run("job-1", &[failed("wwr", "boom")]).unwrap();
    assert!(store.health_for("wwr").is_some());
    store.clear_all();
    assert!(store.health_for("wwr").is_none());
}

#[test]
fn health_is_serialized_camel_case_for_the_renderer() {
    let h = fold_at(None, &failed("wwr", "HTTP 500"), T0);
    let json = serde_json::to_value(&h).unwrap();
    assert_eq!(json["status"], "failing");
    assert_eq!(json["consecutiveFailures"], 1);
    assert_eq!(json["failingSince"], T0);
    assert_eq!(json["lastError"], "HTTP 500");
    assert!(
        json.get("lastSuccessAt").is_none(),
        "absent optionals are omitted, not null"
    );
}

#[test]
fn a_summary_persisted_before_this_feature_still_deserializes() {
    // `lastRunSummaries` in an existing autopilot record / backup has no
    // `health` key at all — it must not fail the import.
    let legacy = serde_json::json!({ "board": "wwr", "count": 3 });
    let s: BoardScrapeSummary = serde_json::from_value(legacy).unwrap();
    assert_eq!(s.board, "wwr");
    assert_eq!(s.count, 3);
    assert!(s.health.is_none());
}

#[test]
fn an_existing_v1_database_gains_the_run_tallies_without_losing_its_rows() {
    // Migrations here are POSITION-indexed off `PRAGMA user_version`, so the
    // tallies had to be APPENDED as migration 2 rather than folded into the
    // CREATE. This builds a real v1 database by hand and opens the store on it.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("board_health.db");
    {
        let conn = crate::db::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE board_health (
                board                TEXT PRIMARY KEY,
                last_success_at      INTEGER,
                last_verified_at     INTEGER,
                failing_since        INTEGER,
                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                last_error           TEXT,
                last_run_id          TEXT,
                updated_at           INTEGER NOT NULL
            );
            INSERT INTO board_health
                (board, last_verified_at, failing_since, consecutive_failures,
                 last_error, last_run_id, updated_at)
            VALUES ('wwr', 1000, 1000, 3, 'HTTP 500', 'job-old', 1000);
            PRAGMA user_version = 1;",
        )
        .unwrap();
    }

    let store = BoardHealthStore::open(dir.path()).unwrap();
    let h = store.health_for("wwr").expect("the v1 row must survive");
    assert_eq!(h.consecutive_failures, 3, "pre-existing state is preserved");
    assert_eq!(h.failing_since, Some(1000));
    assert_eq!(h.last_run_id.as_deref(), Some("job-old"));
    // The new columns default to 0 — the board simply needs FLAKY_MIN_RUNS more
    // runs before a failure RATE can mean anything for it.
    assert_eq!(h.verified_runs, 0);
    assert_eq!(h.failed_runs, 0);

    // And the upgraded row still writes.
    store.record_run("job-new", &[ok("wwr", 5)]).unwrap();
    let h = store.health_for("wwr").unwrap();
    assert_eq!(h.verified_runs, 1);
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.last_run_id.as_deref(), Some("job-new"));
}

#[test]
fn a_negative_tally_on_a_hand_edited_row_reads_as_zero_not_four_billion() {
    let (_dir, store) = open_store();
    store
        .record_run("job-1", &[failed("wwr", "HTTP 500")])
        .unwrap();
    {
        let conn = store.conn.lock();
        conn.execute(
            "UPDATE board_health SET consecutive_failures = -1, verified_runs = -7",
            [],
        )
        .unwrap();
    }
    let h = store.health_for("wwr").unwrap();
    assert_eq!(h.consecutive_failures, 0, "never u32::MAX");
    assert_eq!(h.verified_runs, 0);
    assert_eq!(
        h.status,
        BoardHealthStatus::Stale,
        "verified but no success on record reads as stale, not as a 4-billion streak"
    );
}

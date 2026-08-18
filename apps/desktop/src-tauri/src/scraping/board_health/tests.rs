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
    let mut h = fold(None, &failed("wwr", "HTTP 500"), T0);
    h = fold(Some(h), &failed("wwr", "HTTP 500"), T0 + DAY);
    h = fold(Some(h), &failed("wwr", "HTTP 429"), T0 + 2 * DAY);

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
    let h = fold(Some(h), &ok("wwr", 12), T0 + 3 * DAY);
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
    let h = fold(None, &ok("remotive", 0), T0);
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.last_success_at, Some(T0));
    assert_eq!(h.status, BoardHealthStatus::Healthy);
    assert!(!h.is_noteworthy(), "a healthy board must not badge");
}

#[test]
fn a_board_that_has_never_succeeded_reports_no_last_success() {
    let h = fold(None, &failed("linkedin", "blocked"), T0);
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
    let h = fold(None, &skipped("greenhouse", "needs-company"), T0);
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
    let mut h = fold(None, &failed("linkedin", "HTTP 999"), T0);
    h = fold(Some(h), &failed("linkedin", "HTTP 999"), T0 + DAY);
    h = fold(Some(h), &skipped("linkedin", "needs-login"), T0 + 2 * DAY);
    h = fold(Some(h), &skipped("linkedin", "needs-login"), T0 + 3 * DAY);

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
    let h = fold(None, &ok("xing", 3), T0);
    assert_eq!(h.status, BoardHealthStatus::Healthy);

    // 15 days later, still nothing but skips: the success is no longer evidence.
    let h = fold(Some(h), &skipped("xing", "needs-login"), T0 + 15 * DAY);
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.last_success_at, Some(T0), "the old success is retained");
    assert_eq!(h.status, BoardHealthStatus::Stale);
    assert!(h.is_noteworthy(), "a stale board must badge");

    // One day earlier it is still inside the fortnight window and stays healthy.
    let fresh = fold(
        Some(fold(None, &ok("xing", 3), T0)),
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
    let h = fold(None, &s, T0);
    assert_eq!(h.consecutive_failures, 1);
    assert_eq!(h.status, BoardHealthStatus::Failing);
}

#[test]
fn a_partial_harvest_counts_as_a_working_board() {
    // `truncated` means the board answered and returned rows before a later page
    // failed — it is reachable, which is what "does this source work?" asks.
    let mut s = ok("aggregator", 40);
    s.truncated = Some("page 3 of 5 failed: HTTP 429".to_string());
    let h = fold(None, &s, T0);
    assert_eq!(h.consecutive_failures, 0);
    assert_eq!(h.last_success_at, Some(T0));
    assert_eq!(h.status, BoardHealthStatus::Healthy);
}

#[test]
fn a_long_failure_reason_is_capped_without_splitting_a_codepoint() {
    // Non-ASCII on purpose: a byte-range slice would panic mid-codepoint.
    let long = "ü".repeat(MAX_ERROR_LEN + 50);
    let h = fold(None, &failed("wwr", &long), T0);
    let stored = h.last_error.expect("a failure records its reason");
    assert_eq!(
        stored.chars().count(),
        MAX_ERROR_LEN + 1,
        "cap + one ellipsis"
    );
    assert!(stored.ends_with('…'));
}

// ── persistence ────────────────────────────────────────────────────────────

fn open_store() -> (TempDir, BoardHealthStore) {
    let dir = TempDir::new().unwrap();
    let store = BoardHealthStore::open(dir.path()).unwrap();
    (dir, store)
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
    let h = fold(None, &failed("wwr", "HTTP 500"), T0);
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

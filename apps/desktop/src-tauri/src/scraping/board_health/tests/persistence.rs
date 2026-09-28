//! `BoardHealthStore` persistence tests: on-disk streak recovery, migration
//! upgrade, and the retention/row-count invariants from the module doc.

use super::super::*;
use super::support::{failed, fold_at, ok, skipped, T0};
use crate::scraping::engine::BoardScrapeSummary;
use tempfile::TempDir;

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

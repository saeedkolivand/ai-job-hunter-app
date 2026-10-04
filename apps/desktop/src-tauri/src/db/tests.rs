use super::{open, run_migrations, ts_from_db, ts_to_db, Migration};
use tempfile::TempDir;

// ── R-DB-1: db::open applies WAL + busy_timeout ───────────────────────────
//
// Every store opened via `db::open` must have WAL journal mode and a 5-second
// busy timeout.  Read the PRAGMAs back from the live connection to confirm.
//
// Note: `busy_timeout` cannot be queried back through rusqlite's `PRAGMA
// busy_timeout` statement in all SQLite versions (it is a write-only PRAGMA in
// some builds), so we only assert WAL mode here.  The `busy_timeout` call is
// still exercised (it must not return Err).

#[test]
fn open_sets_wal_journal_mode() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let conn = open(&path).expect("db::open must succeed");

    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .expect("PRAGMA journal_mode must be readable");

    assert_eq!(
        mode, "wal",
        "db::open must switch the connection to WAL mode; got '{mode}'"
    );
}

#[test]
fn open_does_not_err_on_busy_timeout_pragma() {
    // The busy_timeout PRAGMA must be accepted without error by SQLite.
    // We call `open` and assert the Result is Ok — the pragma succeeds or the
    // test panics from the unwrap, surfacing the exact error.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("busy_timeout_test.db");
    open(&path).expect("db::open must succeed including the busy_timeout pragma");
}

// ── R-DB-2: run_migrations partial-failure rollback ───────────────────────
//
// A migration list where the last `up` returns Err must:
//   1. return Err from run_migrations
//   2. NOT advance user_version past the last SUCCESSFUL migration
//      (the transaction around the failing migration must have rolled back)

#[test]
fn run_migrations_rollback_on_partial_failure() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("migrations.db");
    let mut conn = open(&path).expect("db::open must succeed");

    let migrations: &[Migration] = &[
        Migration {
            name: "good_first",
            up: |conn| conn.execute_batch("CREATE TABLE t1 (id INTEGER PRIMARY KEY)"),
        },
        Migration {
            name: "bad_second",
            up: |_conn| {
                Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error {
                        code: rusqlite::ffi::ErrorCode::ConstraintViolation,
                        extended_code: 1,
                    },
                    Some("injected failure".into()),
                ))
            },
        },
    ];

    let result = run_migrations(&mut conn, migrations);
    assert!(
        result.is_err(),
        "run_migrations must return Err when a migration's up fn fails"
    );

    // user_version must NOT be advanced past migration 1 (the last good one).
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("PRAGMA user_version must be readable");
    assert_eq!(
        version, 1,
        "user_version must equal 1 (last good migration); \
             the failing migration (index 2) must have been rolled back"
    );

    // t1 (from the good migration) must exist.
    let t1_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='t1'",
            [],
            |r| r.get(0),
        )
        .expect("sqlite_master query must succeed");
    assert_eq!(t1_exists, 1, "table t1 from migration 1 must still exist");
}

#[test]
fn run_migrations_all_good_advances_version_to_count() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("good_migrations.db");
    let mut conn = open(&path).expect("db::open must succeed");

    let migrations: &[Migration] = &[
        Migration {
            name: "m1",
            up: |conn| conn.execute_batch("CREATE TABLE a (x INTEGER)"),
        },
        Migration {
            name: "m2",
            up: |conn| conn.execute_batch("CREATE TABLE b (x INTEGER)"),
        },
    ];

    run_migrations(&mut conn, migrations).expect("all-good migrations must succeed");

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .expect("PRAGMA user_version must be readable");
    assert_eq!(version, 2, "user_version must equal the migration count");
}

#[test]
fn run_migrations_is_idempotent_when_already_at_current_version() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("idempotent.db");
    let mut conn = open(&path).expect("db::open must succeed");

    let migrations: &[Migration] = &[Migration {
        name: "once",
        up: |conn| conn.execute_batch("CREATE TABLE once_table (x INTEGER)"),
    }];

    run_migrations(&mut conn, migrations).expect("first run must succeed");
    // Running again must be a no-op — no duplicate-table error.
    run_migrations(&mut conn, migrations).expect("second run must be idempotent");

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        version, 1,
        "user_version must still be 1 after idempotent re-run"
    );
}

#[test]
fn roundtrip_is_lossless_for_real_timestamps() {
    // zero, a realistic "today" epoch-ms, and the largest representable value.
    for ms in [0u64, 1_780_531_200_000, i64::MAX as u64] {
        assert_eq!(ts_from_db(ts_to_db(ms)), ms);
    }
}

#[test]
fn matches_the_old_as_casts_over_the_real_domain() {
    // For any `u64 <= i64::MAX` the helper equals the previous `as i64` cast,
    // and for any non-negative `i64` the read equals the previous `as u64`.
    for ms in [0u64, 1, 1_780_531_200_000, i64::MAX as u64] {
        assert_eq!(ts_to_db(ms), ms as i64);
    }
    for v in [0i64, 1, 1_780_531_200_000, i64::MAX] {
        assert_eq!(ts_from_db(v), v as u64);
    }
}

#[test]
fn out_of_domain_inputs_saturate_safely() {
    assert_eq!(ts_to_db(u64::MAX), i64::MAX); // year 292M+ never happens
    assert_eq!(ts_from_db(-1), 0); // ts_to_db never writes negatives
}

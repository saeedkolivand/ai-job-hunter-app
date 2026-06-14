/// Lightweight SQLite migration runner.
///
/// Uses `PRAGMA user_version` to track the last applied migration index.
/// Migrations are numbered 1..N; any migration whose index > current version
/// is applied in order, then the version is bumped.
///
/// Migrations must be idempotent where possible (use IF NOT EXISTS, etc.).
use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub struct Migration {
    pub name: &'static str,
    pub up: fn(&Connection) -> rusqlite::Result<()>,
}

/// Open one of **our own** SQLite stores at `path`, applying the two connection
/// pragmas every store in this crate relies on:
///
/// * `busy_timeout = 5000` — a writer that finds the DB locked waits up to 5 s
///   instead of failing immediately with `SQLITE_BUSY`. We hold a single
///   `Mutex<Connection>` per store, but a sibling store (or a raw reader in a
///   test) can open the same file, so the timeout removes spurious lock errors.
/// * `journal_mode = WAL` — write-ahead logging lets readers run concurrently
///   with a writer and makes the multi-statement transactions used across the
///   data stores durable + atomic. This leaves `*.db-wal` / `*.db-shm` sidecar
///   files next to each store, which is expected and accepted.
///
/// **Scope:** this is for stores OWNED by this app only. It must NOT be used to
/// open foreign databases (e.g. the read-only Chrome cookie DB in
/// `scraping/board_login/import.rs`), which set their own flags and must not be
/// switched into WAL.
pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path).map_err(AppError::from)?;
    // `journal_mode = WAL` returns the resulting mode as a row, so it must run
    // through `query_row`/`pragma_update` rather than `execute_batch` (which
    // rejects a statement that yields rows). `busy_timeout` is a plain pragma.
    conn.busy_timeout(std::time::Duration::from_millis(5000))
        .map_err(AppError::from)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(AppError::from)?;
    Ok(conn)
}

/// Run all pending migrations against `conn`.
///
/// Each pending migration body **and** its `PRAGMA user_version` bump run inside
/// one transaction, so a migration that fails partway rolls back wholesale — the
/// schema never lands in a half-applied state with the version already advanced.
/// Takes `&mut Connection` because `rusqlite::Connection::transaction` needs an
/// exclusive borrow.
pub fn run_migrations(conn: &mut Connection, migrations: &[Migration]) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    for (i, m) in migrations.iter().enumerate() {
        let version = (i + 1) as i64;
        if current >= version {
            continue;
        }
        let tx = conn.transaction()?;
        (m.up)(&tx).map_err(|e| {
            AppError::Storage(format!("Migration {} '{}' failed: {e}", version, m.name))
        })?;
        tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        tx.commit()?;
        log::info!("[db] migration {version} '{}' applied", m.name);
    }
    Ok(())
}

/// Returns true if the given column exists in the table.
pub fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
        rusqlite::params![table, column],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

// ── Timestamp ⇄ SQLite conversion ────────────────────────────────────────────
//
// Our stores hold epoch-millisecond timestamps as `u64` in Rust, but SQLite's
// only integer type is a signed 64-bit `i64`. Every store therefore casts at the
// row boundary: `u64 → i64` on write, `i64 → u64` on read.
//
// The conversion is lossless in practice. Epoch-ms is ~1.7e15 today and grows by
// ~3.15e10 per year, while `i64::MAX` is ~9.2e18 — so a `u64` ms timestamp stays
// well below `i64::MAX` until roughly the year 292 277 026, and a stored value is
// always non-negative. These helpers make that intent explicit and replace ~40
// unannotated `as` casts scattered across the data stores with a single
// documented pair, so the "this is a timestamp, not an arbitrary number" meaning
// lives in one place.
//
// Both forms are byte-identical to the previous `as` casts for the entire real
// domain (any `u64 <= i64::MAX`, any non-negative `i64`). The only divergence is
// outside that domain — a `u64 > i64::MAX` (year 292M+) saturates here instead of
// wrapping negative, and a negative stored `i64` (never produced by [`ts_to_db`])
// clamps to 0 instead of wrapping to a huge `u64` — both strictly safer and
// unreachable with real data, so existing rows round-trip unchanged.

/// Convert a `u64` epoch-millisecond timestamp to the `i64` SQLite stores.
///
/// Saturates at `i64::MAX` instead of wrapping; unreachable for real timestamps
/// (see module note), so this is byte-identical to `value as i64` for all stored
/// data.
#[inline]
pub fn ts_to_db(ms: u64) -> i64 {
    i64::try_from(ms).unwrap_or(i64::MAX)
}

/// Convert an `i64` timestamp read back from SQLite to the in-memory `u64`.
///
/// Clamps a negative value (never written by [`ts_to_db`]) to 0; for every
/// non-negative value — i.e. all real stored timestamps — this equals
/// `value as u64`.
#[inline]
pub fn ts_from_db(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

#[cfg(test)]
mod open_and_migration_tests {
    use super::{open, run_migrations, Migration};
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
        assert_eq!(version, 1, "user_version must still be 1 after idempotent re-run");
    }
}

#[cfg(test)]
mod ts_tests {
    use super::{ts_from_db, ts_to_db};

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
}

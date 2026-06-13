/// Lightweight SQLite migration runner.
///
/// Uses `PRAGMA user_version` to track the last applied migration index.
/// Migrations are numbered 1..N; any migration whose index > current version
/// is applied in order, then the version is bumped.
///
/// Migrations must be idempotent where possible (use IF NOT EXISTS, etc.).
use rusqlite::Connection;

use crate::error::AppResult;

pub struct Migration {
    pub name: &'static str,
    pub up: fn(&Connection) -> rusqlite::Result<()>,
}

/// Run all pending migrations against `conn`.
pub fn run_migrations(conn: &Connection, migrations: &[Migration]) -> AppResult<()> {
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    for (i, m) in migrations.iter().enumerate() {
        let version = (i + 1) as i64;
        if current >= version {
            continue;
        }
        (m.up)(conn).map_err(|e| format!("Migration {} '{}' failed: {e}", version, m.name))?;
        conn.execute_batch(&format!("PRAGMA user_version = {version}"))
            .map_err(|e| e.to_string())?;
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

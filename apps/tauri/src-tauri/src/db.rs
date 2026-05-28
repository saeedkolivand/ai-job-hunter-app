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

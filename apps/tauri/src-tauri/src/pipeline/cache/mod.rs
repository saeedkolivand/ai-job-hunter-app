//! Reusable namespaced TTL cache, backed by SQLite. Any enrichment stage can
//! cache results under its own namespace (e.g. `"company_brief"`) without owning
//! a bespoke table — generalizes the old per-feature `CompanyBriefCache`.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection};

use crate::db::{run_migrations, Migration};
use crate::error::AppResult;

pub struct KvCache {
    conn: Mutex<Connection>,
}

impl KvCache {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_kv_cache",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS kv_cache (
                    namespace   TEXT NOT NULL,
                    key         TEXT NOT NULL COLLATE NOCASE,
                    value       TEXT NOT NULL,
                    created_at  INTEGER NOT NULL,
                    PRIMARY KEY (namespace, key)
                );",
            )
        },
    }];

    pub fn open(data_dir: &Path) -> AppResult<Self> {
        let path = data_dir.join("pipeline_cache.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Returns the cached value if present and younger than `ttl_secs`.
    pub fn get(&self, namespace: &str, key: &str, ttl_secs: i64) -> Option<String> {
        let conn = self.conn.lock();
        let cutoff = now_secs() - ttl_secs;
        conn.query_row(
            "SELECT value FROM kv_cache WHERE namespace = ?1 AND key = ?2 AND created_at > ?3",
            params![namespace, key, cutoff],
            |row| row.get(0),
        )
        .ok()
    }

    pub fn set(&self, namespace: &str, key: &str, value: &str) {
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO kv_cache (namespace, key, value, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![namespace, key, value, now_secs()],
        );
    }

    /// Drop every cached entry (e.g. company briefs, OCR results). Factory reset.
    pub fn clear(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute("DELETE FROM kv_cache", []);
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod test;

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

    /// Bound the cache: expire entries older than `ttl_secs` and cap the table to
    /// the newest `max_rows`. `None` for a knob disables that bound (today's
    /// unbounded behavior). Best-effort — a failed prune never blocks the caller.
    /// Mirrors `DocumentStore::prune_caches`, so the same performance-tier knobs
    /// reclaim the `KvCache` (company briefs / OCR results) alongside the result
    /// caches. `created_at` is epoch-SECONDS here (unlike the ms-based stores).
    pub fn prune(&self, ttl_secs: Option<i64>, max_rows: Option<i64>) {
        // Negative knobs would invert the bounds (delete all / delete all-but-newest);
        // drop them defensively even though all current callers clamp non-negative.
        let ttl_secs = ttl_secs.filter(|&t| t >= 0);
        let max_rows = max_rows.filter(|&n| n >= 0);
        let conn = self.conn.lock();
        if let Some(ttl) = ttl_secs {
            let cutoff = now_secs().saturating_sub(ttl);
            let _ = conn.execute(
                "DELETE FROM kv_cache WHERE created_at < ?1",
                params![cutoff],
            );
        }
        if let Some(n) = max_rows {
            if n == 0 {
                let _ = conn.execute("DELETE FROM kv_cache", []);
            } else {
                // Keep the n newest rows: the cutoff is the created_at of the
                // n-th newest row (OFFSET n-1). ≤ n rows → subquery is NULL →
                // deletes nothing. Ties on created_at may retain slightly more
                // than n — fine for a cache bound.
                let _ = conn.execute(
                    "DELETE FROM kv_cache WHERE created_at < \
                     (SELECT created_at FROM kv_cache ORDER BY created_at DESC LIMIT 1 OFFSET ?1)",
                    params![n - 1],
                );
            }
        }
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

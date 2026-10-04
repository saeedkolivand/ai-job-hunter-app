//! Discovered-companies store (ADR-030 §b): passively harvested ATS company
//! slugs.
//!
//! Every aggregator/board posting leaks its ATS company slug in the apply/redirect
//! URL. [`crate::scraping::ats_ref::extract_ats_ref`] pulls `(ats, slug)` out of a
//! posting URL (parse-only, zero network); [`harvest_ats_refs`] batches those into
//! this store so the slug typeahead and watched-company autopilot targets populate
//! with no user effort. Starred rows are the user's "watched companies".
//!
//! Wired like every other L1 store: opened via `db::open` + a transactional
//! migration (ADR-022), backed up/restored via [`crate::data_store::DataStore`]
//! (`discoveredCompanies` section), and wiped on factory reset via `Resettable`
//! (registered in `commands::privacy`).

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::data_store::DataStore;
use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::{AppError, AppResult};

mod harvest;
pub use harvest::harvest_ats_refs;
mod queries;
pub mod vendored;

/// Per-field byte cap on any stored string — the same ~200-byte convention as
/// `job_preferences`/`dedup` clamp untrusted renderer/scrape input at the write
/// boundary. A real ATS slug/company sits well under this.
const MAX_FIELD_BYTES: usize = 200;

/// Upper bound on rows returned by the watched-company queries (CWE-770), in the
/// same query-discipline spirit as `search`'s `limit.clamp(1, 100)`. Generous over
/// any real starred set (a user watching hundreds of companies is already
/// implausible), while capping an unbounded read + the per-run autopilot fan-out.
const WATCHED_LIMIT: i64 = 500;

/// The renderer-facing row shape — matches `DiscoveredCompany` in
/// `packages/shared`. `search`/`watched` never expose the raw timestamps.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredCompany {
    pub ats_kind: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub seen_count: u64,
    pub starred: bool,
    pub source: String,
}

/// One persisted row, for the backup bundle (carries the timestamps `search`
/// omits). `camelCase` on the wire so a bundle is human-readable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveredRow {
    ats_kind: String,
    slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    first_seen_at: u64,
    last_seen_at: u64,
    seen_count: u64,
    source: String,
    starred: bool,
}

pub struct DiscoveredCompanyStore {
    conn: Mutex<Connection>,
}

/// Clamp `s` to at most [`MAX_FIELD_BYTES`], cutting on a UTF-8 char boundary
/// (same discipline as `dedup`/`job_preferences`). Trims first.
fn clamp_field(s: &str) -> String {
    let s = s.trim();
    if s.len() <= MAX_FIELD_BYTES {
        return s.to_string();
    }
    let mut end = MAX_FIELD_BYTES;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

impl DiscoveredCompanyStore {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_discovered_companies",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS discovered_companies (
                    id            INTEGER PRIMARY KEY,
                    ats_kind      TEXT NOT NULL,
                    slug          TEXT NOT NULL,
                    display_name  TEXT,
                    first_seen_at INTEGER NOT NULL,
                    last_seen_at  INTEGER NOT NULL,
                    seen_count    INTEGER NOT NULL DEFAULT 1,
                    source        TEXT NOT NULL,
                    starred       INTEGER NOT NULL DEFAULT 0,
                    UNIQUE(ats_kind, slug)
                );",
            )
        },
    }];

    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("discovered.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Upsert a batch of `(ats, slug, display_name, source)` refs in ONE
    /// transaction. A first sighting inserts with `seen_count = 1`; a re-sighting
    /// bumps `last_seen_at` + `seen_count` and BACKFILLS an empty `display_name`
    /// (a non-empty one is never overwritten). Empty ats/slug entries are skipped;
    /// every string is byte-clamped at this boundary (CWE-770). Errors map to
    /// [`AppError::Storage`].
    pub fn upsert_batch(&self, refs: &[(String, String, Option<String>, String)]) -> AppResult<()> {
        let now = ts_to_db(now_ms());
        let mut guard = self.conn.lock();
        let tx = guard
            .transaction()
            .map_err(|e| AppError::Storage(e.to_string()))?;
        for (ats, slug, display_name, source) in refs {
            let ats = clamp_field(ats);
            let slug = clamp_field(slug);
            if ats.is_empty() || slug.is_empty() {
                continue; // nothing to key on
            }
            let display = display_name
                .as_deref()
                .map(clamp_field)
                .filter(|s| !s.is_empty());
            let source = clamp_field(source);
            tx.execute(
                "INSERT INTO discovered_companies
                    (ats_kind, slug, display_name, first_seen_at, last_seen_at, seen_count, source, starred)
                 VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5, 0)
                 ON CONFLICT(ats_kind, slug) DO UPDATE SET
                    last_seen_at = excluded.last_seen_at,
                    seen_count   = seen_count + 1,
                    display_name = COALESCE(NULLIF(display_name, ''), excluded.display_name)",
                params![ats, slug, display, now, source],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        }
        tx.commit().map_err(|e| AppError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Star / unstar a company. When no row exists yet (starring a curated seed
    /// that has never been organically seen) a `source='seed'` row is
    /// materialized so the star persists. Unstarring a missing row is a no-op.
    pub fn set_starred(&self, ats: &str, slug: &str, starred: bool) -> AppResult<()> {
        let ats = clamp_field(ats);
        let slug = clamp_field(slug);
        if ats.is_empty() || slug.is_empty() {
            return Ok(()); // nothing to key on — no-op
        }
        let mut guard = self.conn.lock();
        let tx = guard
            .transaction()
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let updated = tx
            .execute(
                "UPDATE discovered_companies SET starred = ?3 WHERE ats_kind = ?1 AND slug = ?2",
                params![ats, slug, starred as i64],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        // Materialize a curated-seed row only when STARRING a missing company —
        // unstarring a company we've never seen is meaningless.
        if updated == 0 && starred {
            let now = ts_to_db(now_ms());
            tx.execute(
                "INSERT OR IGNORE INTO discovered_companies
                    (ats_kind, slug, display_name, first_seen_at, last_seen_at, seen_count, source, starred)
                 VALUES (?1, ?2, NULL, ?3, ?3, 0, 'seed', 1)",
                params![ats, slug, now],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        }
        tx.commit().map_err(|e| AppError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Wipe every discovered company (factory reset).
    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM discovered_companies", []).ok();
    }

    /// Snapshot all rows (deterministic order) for export.
    fn rows(&self) -> Vec<DiscoveredRow> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT ats_kind, slug, display_name, first_seen_at, last_seen_at,
                    seen_count, source, starred
             FROM discovered_companies ORDER BY ats_kind, slug",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(DiscoveredRow {
                    ats_kind: row.get(0)?,
                    slug: row.get(1)?,
                    display_name: row.get::<_, Option<String>>(2)?,
                    first_seen_at: ts_from_db(row.get::<_, i64>(3)?),
                    last_seen_at: ts_from_db(row.get::<_, i64>(4)?),
                    // Plain count cast (not a timestamp) — clamp a stray negative to 0.
                    seen_count: u64::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
                    source: row.get(6)?,
                    starred: row.get::<_, i64>(7)? != 0,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(Result::ok).collect())
        })
        .unwrap_or_default()
    }
}

impl DataStore for DiscoveredCompanyStore {
    fn key(&self) -> &'static str {
        "discoveredCompanies"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::json!(self.rows())
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data.as_array().ok_or_else(|| {
            AppError::Validation("discoveredCompanies: expected an array".to_string())
        })?;
        // Deserialize EVERY row before mutating, so a malformed row aborts the
        // import without having cleared the table (mirrors the other stores).
        let rows: Vec<DiscoveredRow> = items
            .iter()
            .map(|item| serde_json::from_value(item.clone()).map_err(AppError::from))
            .collect::<AppResult<_>>()?;

        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM discovered_companies", [])?;
        for row in &rows {
            let ats = clamp_field(&row.ats_kind);
            let slug = clamp_field(&row.slug);
            if ats.is_empty() || slug.is_empty() {
                continue;
            }
            let display = row
                .display_name
                .as_deref()
                .map(clamp_field)
                .filter(|s| !s.is_empty());
            tx.execute(
                "INSERT OR IGNORE INTO discovered_companies
                    (ats_kind, slug, display_name, first_seen_at, last_seen_at, seen_count, source, starred)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    ats,
                    slug,
                    display,
                    ts_to_db(row.first_seen_at),
                    ts_to_db(row.last_seen_at),
                    // Plain count cast (not a timestamp) — saturate at i64::MAX.
                    i64::try_from(row.seen_count).unwrap_or(i64::MAX),
                    clamp_field(&row.source),
                    row.starred as i64,
                ],
            )?;
        }
        tx.commit()?;
        Ok(rows.len())
    }
}

#[cfg(test)]
mod tests;

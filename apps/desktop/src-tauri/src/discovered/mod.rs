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

    /// Search over slug + display_name (case-insensitive substring). Starred rows
    /// rank first, then by `seen_count` desc. An empty query returns the top
    /// `limit` rows overall. `query`'s LIKE metacharacters are escaped so a `%`/`_`
    /// in a slug matches literally.
    pub fn search(&self, query: &str, limit: u32) -> Vec<DiscoveredCompany> {
        let pattern = format!("%{}%", like_escape(query.trim()));
        let limit = limit.clamp(1, 100) as i64;
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT ats_kind, slug, display_name, seen_count, starred, source
             FROM discovered_companies
             WHERE slug LIKE ?1 ESCAPE '\\' OR display_name LIKE ?1 ESCAPE '\\'
             ORDER BY starred DESC, seen_count DESC, slug ASC
             LIMIT ?2",
        ) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[discovered] search prepare failed ({e}); returning empty");
                return Vec::new();
            }
        };
        let rows = stmt.query_map(params![pattern, limit], Self::row_to_company);
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(e) => {
                log::warn!("[discovered] search query failed ({e}); returning empty");
                Vec::new()
            }
        }
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

    /// Every watched (starred) company as `(ats_kind, slug)` — the runtime-resolved
    /// input for a `watchedCompaniesOnly` autopilot run (ADR-030 §e). Ranked stably
    /// (most-seen first) so a per-board company cap keeps the most-relevant slugs.
    /// Bounded to [`WATCHED_LIMIT`] (CWE-770), same query discipline as `search`.
    pub fn watched(&self) -> Vec<(String, String)> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT ats_kind, slug FROM discovered_companies
             WHERE starred = 1 ORDER BY seen_count DESC, slug ASC LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[discovered] watched prepare failed ({e}); returning empty");
                return Vec::new();
            }
        };
        let rows = stmt.query_map(params![WATCHED_LIMIT], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        });
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(e) => {
                log::warn!("[discovered] watched query failed ({e}); returning empty");
                Vec::new()
            }
        }
    }

    /// Every watched (starred) company as full renderer rows, ranked most-seen
    /// first. Unlike surfacing the starred prefix of `search("")`, this has NO
    /// search-cap coupling — it returns the whole starred set up to
    /// [`WATCHED_LIMIT`] (CWE-770). Backs the `discovery.watched()` IPC read; the
    /// autopilot resolver uses the lighter [`Self::watched`] `(ats, slug)` pairs.
    pub fn watched_companies(&self) -> Vec<DiscoveredCompany> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT ats_kind, slug, display_name, seen_count, starred, source
             FROM discovered_companies WHERE starred = 1
             ORDER BY seen_count DESC, slug ASC LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[discovered] watched_companies prepare failed ({e}); returning empty");
                return Vec::new();
            }
        };
        let rows = stmt.query_map(params![WATCHED_LIMIT], Self::row_to_company);
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(e) => {
                log::warn!("[discovered] watched_companies query failed ({e}); returning empty");
                Vec::new()
            }
        }
    }

    /// Wipe every discovered company (factory reset).
    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM discovered_companies", []).ok();
    }

    fn row_to_company(row: &rusqlite::Row) -> rusqlite::Result<DiscoveredCompany> {
        Ok(DiscoveredCompany {
            ats_kind: row.get(0)?,
            slug: row.get(1)?,
            display_name: row.get::<_, Option<String>>(2)?,
            // `seen_count` is a plain count, not a timestamp — clamp a stray
            // negative to 0 (never written) without borrowing the epoch-ms helpers.
            seen_count: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
            starred: row.get::<_, i64>(4)? != 0,
            source: row.get(5)?,
        })
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

/// Escape LIKE metacharacters so a user query matches literally (paired with
/// `ESCAPE '\'` in the statement).
fn like_escape(q: &str) -> String {
    q.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
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
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn open() -> (TempDir, DiscoveredCompanyStore) {
        let dir = TempDir::new().unwrap();
        let store = DiscoveredCompanyStore::open(dir.path()).unwrap();
        (dir, store)
    }

    fn r(
        ats: &str,
        slug: &str,
        name: Option<&str>,
        source: &str,
    ) -> (String, String, Option<String>, String) {
        (
            ats.to_string(),
            slug.to_string(),
            name.map(str::to_string),
            source.to_string(),
        )
    }

    #[test]
    fn upsert_insert_then_resighting_bumps_seen_count() {
        let (_dir, store) = open();
        store
            .upsert_batch(&[r("greenhouse", "stripe", None, "scrape")])
            .unwrap();
        // A fresh insert starts at seen_count 1 (not bumped).
        let first = store.search("stripe", 10);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].seen_count, 1, "a first sighting is seen_count 1");

        // Re-sighting the same (ats, slug) bumps to 2.
        store
            .upsert_batch(&[r("greenhouse", "stripe", None, "scrape")])
            .unwrap();
        let again = store.search("stripe", 10);
        assert_eq!(again.len(), 1, "unique(ats,slug) keeps it one row");
        assert_eq!(again[0].seen_count, 2, "a re-sighting bumps seen_count");
    }

    #[test]
    fn display_name_backfills_but_never_overwrites() {
        let (_dir, store) = open();
        // First sighting has no display name.
        store
            .upsert_batch(&[r("lever", "spotify", None, "scrape")])
            .unwrap();
        assert_eq!(store.search("spotify", 10)[0].display_name, None);

        // A later sighting WITH a name backfills the empty one.
        store
            .upsert_batch(&[r("lever", "spotify", Some("Spotify"), "scrape")])
            .unwrap();
        assert_eq!(
            store.search("spotify", 10)[0].display_name.as_deref(),
            Some("Spotify"),
            "an empty display_name must be backfilled"
        );

        // A still-later sighting with a DIFFERENT name must NOT overwrite it.
        store
            .upsert_batch(&[r("lever", "spotify", Some("Spotify AB"), "scrape")])
            .unwrap();
        assert_eq!(
            store.search("spotify", 10)[0].display_name.as_deref(),
            Some("Spotify"),
            "a non-empty display_name is never overwritten"
        );
    }

    #[test]
    fn set_starred_materializes_a_seed_row_when_missing() {
        let (_dir, store) = open();
        // Starring a company that was never harvested materializes a seed row.
        store.set_starred("ashby", "Linear", true).unwrap();
        let watched = store.watched();
        assert_eq!(watched, vec![("ashby".to_string(), "Linear".to_string())]);
        // The materialized row is source='seed'.
        assert_eq!(store.search("Linear", 10)[0].source, "seed");

        // Unstarring removes it from the watched set.
        store.set_starred("ashby", "Linear", false).unwrap();
        assert!(store.watched().is_empty(), "unstarred → not watched");
    }

    #[test]
    fn unstarring_a_missing_company_is_a_noop() {
        let (_dir, store) = open();
        store.set_starred("greenhouse", "ghost", false).unwrap();
        // No junk row was materialized.
        assert!(store.search("ghost", 10).is_empty());
        assert!(store.watched().is_empty());
    }

    #[test]
    fn search_ranks_starred_first_then_by_seen_count() {
        let (_dir, store) = open();
        // seen twice, unstarred
        store
            .upsert_batch(&[r("greenhouse", "acme", None, "scrape")])
            .unwrap();
        store
            .upsert_batch(&[r("greenhouse", "acme", None, "scrape")])
            .unwrap();
        // seen once, unstarred
        store
            .upsert_batch(&[r("greenhouse", "acorn", None, "scrape")])
            .unwrap();
        // seen once, but STARRED → must rank first despite the lower count
        store
            .upsert_batch(&[r("greenhouse", "aced", None, "scrape")])
            .unwrap();
        store.set_starred("greenhouse", "aced", true).unwrap();

        let results = store.search("ac", 10);
        let slugs: Vec<&str> = results.iter().map(|c| c.slug.as_str()).collect();
        assert_eq!(
            slugs,
            vec!["aced", "acme", "acorn"],
            "starred first, then seen_count desc"
        );
    }

    #[test]
    fn search_escapes_like_wildcards() {
        let (_dir, store) = open();
        store
            .upsert_batch(&[r("greenhouse", "acme", None, "scrape")])
            .unwrap();
        // A `%` query must not match everything — it's escaped to a literal.
        assert!(
            store.search("%", 10).is_empty(),
            "a literal % must not act as a wildcard"
        );
        // Empty query returns everything (top-N).
        assert_eq!(store.search("", 10).len(), 1);
    }

    #[test]
    fn export_import_round_trips() {
        let (_dir, store) = open();
        store
            .upsert_batch(&[
                r("greenhouse", "stripe", Some("Stripe"), "scrape"),
                r("lever", "spotify", None, "extension"),
            ])
            .unwrap();
        store.set_starred("greenhouse", "stripe", true).unwrap();
        let bundle = store.export();

        let (_dir2, store2) = open();
        let restored = store2.import(&bundle).unwrap();
        assert_eq!(restored, 2);
        // Watched state survives the round-trip.
        assert_eq!(
            store2.watched(),
            vec![("greenhouse".to_string(), "stripe".to_string())]
        );
        // Display name + source survive.
        let stripe = &store2.search("stripe", 10)[0];
        assert_eq!(stripe.display_name.as_deref(), Some("Stripe"));
        assert_eq!(stripe.source, "scrape");
    }

    #[test]
    fn import_with_a_malformed_row_errors_and_preserves_existing_rows() {
        let (_dir, store) = open();
        store
            .upsert_batch(&[r("greenhouse", "keep", None, "scrape")])
            .unwrap();

        // One malformed row (missing required `slug`) must fail the whole import
        // BEFORE any DELETE runs — deserialize-all-before-mutate.
        let bundle = serde_json::json!([
            { "atsKind": "lever", "slug": "ok", "firstSeenAt": 1, "lastSeenAt": 1, "seenCount": 1, "source": "scrape", "starred": false },
            { "atsKind": "lever", "firstSeenAt": 2, "lastSeenAt": 2, "seenCount": 1, "source": "scrape", "starred": false }
        ]);
        assert!(
            store.import(&bundle).is_err(),
            "malformed row must fail import"
        );
        // The pre-existing row survives untouched.
        assert_eq!(store.search("keep", 10).len(), 1);
        assert!(store.search("ok", 10).is_empty(), "no partial insert");
    }

    #[test]
    fn watched_companies_returns_full_starred_rows_only() {
        let (_dir, store) = open();
        store
            .upsert_batch(&[
                r("greenhouse", "stripe", Some("Stripe"), "scrape"),
                r("greenhouse", "stripe", None, "scrape"), // bump to seen_count 2
                r("ashby", "Linear", None, "scrape"),
            ])
            .unwrap();
        store.set_starred("greenhouse", "stripe", true).unwrap();

        let watched = store.watched_companies();
        assert_eq!(watched.len(), 1, "only the starred row is returned");
        assert_eq!(watched[0].slug, "stripe");
        assert_eq!(watched[0].display_name.as_deref(), Some("Stripe"));
        assert_eq!(watched[0].seen_count, 2, "full row carries the seen_count");
        assert!(watched[0].starred);
        // The unstarred ashby row is excluded.
        assert!(watched.iter().all(|c| c.slug != "Linear"));
    }

    #[test]
    fn watched_queries_are_bounded_to_the_limit() {
        let (_dir, store) = open();
        // Star WATCHED_LIMIT + 1 companies (a pathological/hostile set) — both
        // watched queries must cap at WATCHED_LIMIT (CWE-770), never read them all.
        let over = (WATCHED_LIMIT + 1) as usize;
        for i in 0..over {
            store
                .set_starred("greenhouse", &format!("co-{i}"), true)
                .unwrap();
        }
        assert_eq!(
            store.watched().len() as i64,
            WATCHED_LIMIT,
            "watched() (pairs) must be bounded to WATCHED_LIMIT"
        );
        assert_eq!(
            store.watched_companies().len() as i64,
            WATCHED_LIMIT,
            "watched_companies() (full rows) must be bounded to WATCHED_LIMIT"
        );
    }

    #[test]
    fn clear_all_empties_the_store() {
        let (_dir, store) = open();
        store
            .upsert_batch(&[r("greenhouse", "stripe", None, "scrape")])
            .unwrap();
        assert!(!store.search("stripe", 10).is_empty());
        store.clear_all();
        assert!(store.search("stripe", 10).is_empty());
    }

    #[test]
    fn reopening_the_same_db_is_migration_idempotent() {
        let dir = TempDir::new().unwrap();
        {
            let store = DiscoveredCompanyStore::open(dir.path()).unwrap();
            store
                .upsert_batch(&[r("greenhouse", "stripe", None, "scrape")])
                .unwrap();
        }
        // Second open re-runs run_migrations (no-op) and keeps the data.
        let store = DiscoveredCompanyStore::open(dir.path()).unwrap();
        assert_eq!(store.search("stripe", 10).len(), 1);
    }

    #[test]
    fn upsert_skips_empty_and_byte_clamps() {
        let (_dir, store) = open();
        let big = "z".repeat(500);
        store
            .upsert_batch(&[
                r("", "slug", None, "scrape"),         // empty ats → skipped
                r("greenhouse", "  ", None, "scrape"), // whitespace slug → skipped
                r("greenhouse", &big, None, "scrape"), // over-cap slug → clamped
            ])
            .unwrap();
        let all = store.search("", 100);
        assert_eq!(all.len(), 1, "only the clamped valid row survives");
        assert!(all[0].slug.len() <= MAX_FIELD_BYTES, "slug byte-clamped");
    }
}

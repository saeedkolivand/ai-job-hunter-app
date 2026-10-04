//! Read side of [`DiscoveredCompanyStore`]: the typeahead `search`, the two
//! watched-company reads and the row mapper they share, plus the LIKE escaping
//! `search` needs.

use rusqlite::params;

use super::{DiscoveredCompany, DiscoveredCompanyStore, WATCHED_LIMIT};
use crate::observability::sanitize_reason;

impl DiscoveredCompanyStore {
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
                log::warn!(
                    "[discovered] search prepare failed ({}); returning empty",
                    sanitize_reason(&e.to_string())
                );
                return Vec::new();
            }
        };
        let rows = stmt.query_map(params![pattern, limit], Self::row_to_company);
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(e) => {
                log::warn!(
                    "[discovered] search query failed ({}); returning empty",
                    sanitize_reason(&e.to_string())
                );
                Vec::new()
            }
        }
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
                log::warn!(
                    "[discovered] watched prepare failed ({}); returning empty",
                    sanitize_reason(&e.to_string())
                );
                return Vec::new();
            }
        };
        let rows = stmt.query_map(params![WATCHED_LIMIT], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        });
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(e) => {
                log::warn!(
                    "[discovered] watched query failed ({}); returning empty",
                    sanitize_reason(&e.to_string())
                );
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
                log::warn!(
                    "[discovered] watched_companies prepare failed ({}); returning empty",
                    sanitize_reason(&e.to_string())
                );
                return Vec::new();
            }
        };
        let rows = stmt.query_map(params![WATCHED_LIMIT], Self::row_to_company);
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(e) => {
                log::warn!(
                    "[discovered] watched_companies query failed ({}); returning empty",
                    sanitize_reason(&e.to_string())
                );
                Vec::new()
            }
        }
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
}

/// Escape LIKE metacharacters so a user query matches literally (paired with
/// `ESCAPE '\'` in the statement).
fn like_escape(q: &str) -> String {
    q.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

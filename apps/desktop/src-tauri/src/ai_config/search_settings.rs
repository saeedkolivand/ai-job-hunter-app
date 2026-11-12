//! The web-search backend settings that live beside the provider config: today only the
//! SearXNG base URL. A search backend is not an AI provider (`commands::ai_provider::search`),
//! so it has no `ai_provider_config` row; it shares this store only so backup, restore, sync
//! and factory reset (which already sweep this store) cover it without a second one.
//!
//! The URL is non-secret (the keychain holds keys only) and is validated at WRITE time and
//! again where it is read for egress, because a restored backup can also fill it.

use rusqlite::{params, Connection};

use super::AiConfigStore;
use crate::commands::ai_provider::search::normalize_searxng_url;
use crate::error::AppResult;

/// Migration: the single-row `search_settings` table.
pub(super) fn create_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS search_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1), searxng_url TEXT);
        INSERT OR IGNORE INTO search_settings (id, searxng_url) VALUES (1, NULL);",
    )
}

impl AiConfigStore {
    /// The stored SearXNG base URL, if one is set.
    pub fn searxng_url(&self) -> Option<String> {
        Self::searxng_url_conn(&self.conn.lock())
    }

    /// Set (or, for `None`/blank, clear) the SearXNG base URL. Rejects anything that is not a
    /// plain http/https URL with a host.
    pub fn set_searxng_url(&self, url: Option<&str>) -> AppResult<()> {
        let url = url.map(normalize_searxng_url).transpose()?.flatten();
        Self::write_searxng_url_conn(&self.conn.lock(), url.as_deref())
    }

    pub(super) fn searxng_url_conn(conn: &Connection) -> Option<String> {
        conn.query_row(
            "SELECT searxng_url FROM search_settings WHERE id = 1",
            [],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        // Re-validated on every read: a restored backup, or a rule tightened since the value
        // was written, must not hand an invalid URL to the egress path or `ai_search_settings`.
        .and_then(|u| normalize_searxng_url(&u).ok().flatten())
    }

    /// Whether a URL is configured: the seed gate counts it, so a first-run seed carrying an
    /// older snapshot (no `searxngUrl`) cannot clear a URL set on a fresh store.
    pub(super) fn has_searxng_url_conn(conn: &Connection) -> bool {
        Self::searxng_url_conn(conn).is_some()
    }

    pub(super) fn write_searxng_url_conn(conn: &Connection, url: Option<&str>) -> AppResult<()> {
        conn.execute(
            "UPDATE search_settings SET searxng_url = ?1 WHERE id = 1",
            params![url],
        )?;
        Ok(())
    }

    /// Restore from an untrusted snapshot: a URL that fails validation is dropped, never stored.
    pub(super) fn apply_searxng_url_conn(conn: &Connection, url: Option<&str>) -> AppResult<()> {
        let valid = url.and_then(|u| normalize_searxng_url(u).ok().flatten());
        Self::write_searxng_url_conn(conn, valid.as_deref())
    }
}

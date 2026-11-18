//! Posting history store: per ghost key (normalized company + title), when it
//! was first and last seen and how often it came back (ghost-job signals).
//!
//! Recorded at ingest (`commands::autopilot::run`, `commands::scrape::run`);
//! the derived [`GhostSignals`] ride on `autopilot::FoundJob` as advisory
//! fields — a posting is never filtered or hidden because of them.
//!
//! Unlike the dedup verdicts this IS observation history: it cannot be rebuilt
//! from a re-scrape, so it travels in the backup bundle (`postingHistory`
//! section), syncs per record (ADR-051), and is wiped by factory reset
//! (`Resettable`, registered in `commands::privacy`).
//!
//! A repost means "a scrape that could have seen the posting ran and did not".
//! "Could have seen" is tracked per SOURCE (a board plus the normalized search,
//! [`source_id`]): each key remembers the source that last saw it, and each
//! source remembers when it last ingested successfully (device-local markers,
//! never backed up or synced). Only a return from the SAME source, after that
//! source ingested without the key, and after the gap, counts. A source cut off
//! at its item cap is not "ingested without the key" (its marker does not
//! advance). A restore clears the markers; a key with no recorded source (an old
//! row, or one written by sync) never counts.
//! ponytail: a posting that moves between sources (another board, an edited
//! query) never counts as a repost; that costs recall, never precision.
//!
//! Rows arriving by import or sync are peer-supplied, so they are clamped on the
//! way in ([`insert_conn`]): counts to [`MAX_COUNT`], `last_seen_at` to now (a
//! future value would dodge the retention prune), `first_seen_at` to at most
//! `last_seen_at`. A peer can still claim an OLD `first_seen_at` (a posting can
//! genuinely be years old, so no lower bound exists) or replace a row with a
//! later one, so a sync can make signals louder as well as quieter; they stay
//! advisory and never filter or hide a posting.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::data_store::DataStore;
use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::{AppError, AppResult};

mod key;
mod sync_section;

use key::clamp_id;
pub use key::{ghost_key, source_id};

const DAY_MS: u64 = 86_400_000;
/// A key absent from scrapes for at least this long and then seen again is a
/// repost. Concurrent listings (multi-city, many urls) are never absent, so
/// they never count.
pub const REPOST_GAP_DAYS: u64 = 7;
/// Keys unseen for this long are forgotten (pruned at ingest).
pub const POSTING_HISTORY_RETENTION_DAYS: u64 = 180;
/// `open_long` once the key has been seen for at least this many days.
pub const OPEN_LONG_DAYS: u32 = 45;
/// `reposted` once the key has come back at least this many times.
pub const REPOSTED_MIN: u32 = 2;
/// Cap on `seen_count` / `repost_count` accepted from a backup or sync peer. Far
/// above any real value, small enough that `+ 1` can never overflow.
const MAX_COUNT: u64 = 100_000;

/// Advisory ghost-job signals for one posting; machine-readable on purpose
/// (counts and booleans, no prose) so agents and the UI read the same fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GhostSignals {
    /// When this company + title was first seen (epoch ms).
    pub first_seen_at: u64,
    /// Whole days from `first_seen_at` to the scrape that produced these signals.
    pub open_days: u32,
    /// Times the posting vanished for [`REPOST_GAP_DAYS`] or more and came back.
    pub repost_count: u32,
    /// `open_days >= OPEN_LONG_DAYS`.
    pub open_long: bool,
    /// `repost_count >= REPOSTED_MIN`.
    pub reposted: bool,
}

/// Derive the signals from a history row as of `now`.
pub fn signals_for(first_seen_at: u64, repost_count: u32, now: u64) -> GhostSignals {
    let open_days = u32::try_from(now.saturating_sub(first_seen_at) / DAY_MS).unwrap_or(u32::MAX);
    GhostSignals {
        first_seen_at,
        open_days,
        repost_count,
        open_long: open_days >= OPEN_LONG_DAYS,
        reposted: repost_count >= REPOSTED_MIN,
    }
}

/// One persisted row, for the backup bundle and sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryRow {
    ghost_key: String,
    first_seen_at: u64,
    last_seen_at: u64,
    seen_count: u64,
    #[serde(default)]
    repost_count: u64,
    #[serde(default)]
    last_url: String,
    /// The [`source_id`] that last saw the key; empty on a row from before sources.
    #[serde(default)]
    last_source: String,
}

pub struct PostingHistoryStore {
    conn: Mutex<Connection>,
}

impl PostingHistoryStore {
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
            name: "create_posting_history",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS posting_history (
                    ghost_key     TEXT PRIMARY KEY,
                    first_seen_at INTEGER NOT NULL,
                    last_seen_at  INTEGER NOT NULL,
                    seen_count    INTEGER NOT NULL DEFAULT 1,
                    repost_count  INTEGER NOT NULL DEFAULT 0,
                    last_url      TEXT NOT NULL DEFAULT '',
                    last_source   TEXT NOT NULL DEFAULT ''
                );",
                )
            },
        },
        // Device-local: when each source last ingested successfully. Not in
        // backup/sync (a restore or another device says nothing about THIS
        // device's scrapes).
        Migration {
            name: "create_posting_history_ingest_meta",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS posting_history_meta (
                    source         TEXT PRIMARY KEY,
                    last_ingest_at INTEGER NOT NULL
                );",
                )
            },
        },
    ];

    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let mut conn = crate::db::open(&data_dir.join("posting_history.db"))?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Record one scrape in ONE transaction and return each key's signals as of
    /// `seen_at`. `seen` is `(company, title, url, source)` per posting, `source`
    /// being the posting's [`source_id`]; `ok_sources` are the sources that ran
    /// to completion (no error, skip or partial harvest), whose markers advance
    /// even when they returned nothing: an empty success is evidence of absence.
    ///
    /// Postings sharing a key within the batch (multi-city, several urls) count
    /// once. A repost needs ALL of: the key's `last_source` equals this posting's
    /// source (never empty), that source ingested successfully after
    /// `last_seen_at` (so it ran and did not list the key), and `last_seen_at` is
    /// at least [`REPOST_GAP_DAYS`] old. Keys unseen for
    /// [`POSTING_HISTORY_RETENTION_DAYS`] are pruned first.
    ///
    /// Callers must not record a cancelled or failed run.
    pub fn record_scrape<'a>(
        &self,
        seen: impl IntoIterator<Item = (&'a str, &'a str, &'a str, &'a str)>,
        ok_sources: &[String],
        seen_at: u64,
    ) -> AppResult<HashMap<String, GhostSignals>> {
        let mut batch: BTreeMap<String, (String, String)> = BTreeMap::new();
        for (company, title, url, source) in seen {
            if let Some(key) = ghost_key(company, title) {
                batch
                    .entry(key)
                    .or_insert_with(|| (key::clamp(url), clamp_id(source)));
            }
        }
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        let cutoff = seen_at.saturating_sub(POSTING_HISTORY_RETENTION_DAYS * DAY_MS);
        tx.execute(
            "DELETE FROM posting_history WHERE last_seen_at < ?1",
            params![ts_to_db(cutoff)],
        )?;
        let marker = |source: &str| -> AppResult<u64> {
            Ok(tx
                .query_row(
                    "SELECT last_ingest_at FROM posting_history_meta WHERE source = ?1",
                    params![source],
                    |r| r.get(0).map(ts_from_db),
                )
                .optional()?
                .unwrap_or(0))
        };
        let mut out = HashMap::with_capacity(batch.len());
        for (key, (url, source)) in batch {
            let prior: Option<(i64, i64, i64, i64, String)> = tx
                .query_row(
                    "SELECT first_seen_at, last_seen_at, seen_count, repost_count, last_source
                     FROM posting_history WHERE ghost_key = ?1",
                    params![key],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()?;
            let (first, last, count, reposts, came_back) = match &prior {
                None => (seen_at, seen_at, 0, 0, false),
                Some((first, last, count, reposts, last_source)) => {
                    let last = ts_from_db(*last);
                    let came_back = !source.is_empty()
                        && *last_source == source
                        && marker(&source)? > last
                        && seen_at.saturating_sub(last) >= REPOST_GAP_DAYS * DAY_MS;
                    (ts_from_db(*first), last, *count, *reposts, came_back)
                }
            };
            let reposts = reposts.saturating_add(i64::from(came_back));
            tx.execute(
                "INSERT OR REPLACE INTO posting_history
                    (ghost_key, first_seen_at, last_seen_at, seen_count, repost_count,
                     last_url, last_source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    key,
                    ts_to_db(first),
                    ts_to_db(last.max(seen_at)),
                    count.saturating_add(1),
                    reposts,
                    url,
                    source
                ],
            )?;
            let reposts = u32::try_from(reposts).unwrap_or(u32::MAX);
            out.insert(key, signals_for(first, reposts, seen_at));
        }
        for source in ok_sources {
            let source = clamp_id(source);
            tx.execute(
                "INSERT INTO posting_history_meta (source, last_ingest_at) VALUES (?1, ?2)
                 ON CONFLICT(source) DO UPDATE SET
                    last_ingest_at = MAX(last_ingest_at, excluded.last_ingest_at)",
                params![source, ts_to_db(seen_at)],
            )?;
        }
        tx.commit()?;
        Ok(out)
    }

    /// Wipe all history and the ingest marker (factory reset), in one
    /// transaction. `Resettable` has no error channel, so a failure is logged
    /// by code only, like the other stores' resets.
    pub fn clear_all(&self) {
        if let Err(e) = self.try_clear_all() {
            log::warn!("[posting_history] reset failed: {}", e.code());
        }
    }

    fn try_clear_all(&self) -> AppResult<()> {
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM posting_history", [])?;
        tx.execute("DELETE FROM posting_history_meta", [])?;
        Ok(tx.commit()?)
    }

    /// Snapshot all rows (deterministic order) for export. Fails the whole read
    /// rather than skip a row: a partial snapshot would restore as a wipe.
    fn rows(&self) -> AppResult<Vec<HistoryRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT ghost_key, first_seen_at, last_seen_at, seen_count, repost_count, last_url,
                    last_source
             FROM posting_history ORDER BY ghost_key",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(HistoryRow {
                ghost_key: r.get(0)?,
                first_seen_at: ts_from_db(r.get(1)?),
                last_seen_at: ts_from_db(r.get(2)?),
                // Plain counts, not timestamps: clamp a stray negative to 0.
                seen_count: u64::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
                repost_count: u64::try_from(r.get::<_, i64>(4)?).unwrap_or(0),
                last_url: r.get(5)?,
                last_source: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// The export section, or an error when the read failed (so the backup can
    /// omit the section instead of exporting an empty one).
    pub fn try_export(&self) -> AppResult<serde_json::Value> {
        Ok(serde_json::json!(self.rows()?))
    }
}

/// Write one row (replacing any with the same key). Shared by the backup import
/// and the per-record sync upsert.
fn insert_conn(conn: &Connection, row: &HistoryRow) -> AppResult<()> {
    if row.ghost_key.trim().is_empty() {
        return Ok(());
    }
    // Bend `last_seen_at` down to now (the clock is the one trusted value), then
    // `first_seen_at` down to it, so first <= last always holds.
    let last = row.last_seen_at.min(now_ms());
    let first = row.first_seen_at.min(last);
    conn.execute(
        "INSERT OR REPLACE INTO posting_history
            (ghost_key, first_seen_at, last_seen_at, seen_count, repost_count, last_url,
             last_source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            clamp_id(&row.ghost_key),
            ts_to_db(first),
            ts_to_db(last),
            i64::try_from(row.seen_count.min(MAX_COUNT)).unwrap_or(i64::MAX),
            i64::try_from(row.repost_count.min(MAX_COUNT)).unwrap_or(i64::MAX),
            key::clamp(&row.last_url),
            clamp_id(&row.last_source),
        ],
    )?;
    Ok(())
}

impl DataStore for PostingHistoryStore {
    fn key(&self) -> &'static str {
        "postingHistory"
    }

    /// Prefer [`PostingHistoryStore::try_export`]: this trait method has no
    /// error channel, so a failed read is logged and exported as `null`, which
    /// `data_import`'s shape pre-pass rejects instead of wiping the history.
    fn export(&self) -> serde_json::Value {
        self.try_export().unwrap_or_else(|e| {
            log::warn!("[posting_history] export failed: {}", e.code());
            serde_json::Value::Null
        })
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data
            .as_array()
            .ok_or_else(|| AppError::Validation("postingHistory: expected an array".to_string()))?;
        // Deserialize EVERY row before mutating, so a malformed row aborts the
        // import without having cleared the table.
        let rows: Vec<HistoryRow> = items
            .iter()
            .map(|item| serde_json::from_value(item.clone()).map_err(AppError::from))
            .collect::<AppResult<_>>()?;

        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM posting_history", [])?;
        // A restore brings back old `last_seen_at` values: markers earned by this
        // device's scrapes since would turn them into false reposts.
        tx.execute("DELETE FROM posting_history_meta", [])?;
        for row in &rows {
            insert_conn(&tx, row)?;
        }
        tx.commit()?;
        Ok(rows.len())
    }
}

#[cfg(test)]
mod tests;

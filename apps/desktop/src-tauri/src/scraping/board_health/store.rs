//! SQLite persistence for the per-board reliability history
//! (`<dataDir>/board_health.db`) — see [`super`] for the shape/retention
//! rationale and [`super::fold`] for the pure derivation this wraps.

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension as _};

use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::{AppError, AppResult};
use crate::observability::sanitize_reason;
use crate::scraping::engine::BoardScrapeSummary;

use super::fold::{derive_status, fold};
use super::types::{BoardHealth, BoardHealthEntry, BoardHealthStatus};

/// Per-board reliability store (`<dataDir>/board_health.db`).
pub struct BoardHealthStore {
    pub(super) conn: Mutex<Connection>,
}

impl BoardHealthStore {
    /// Position-indexed migrations (ADR-022) — **append only**, never edit or
    /// insert: `run_migrations` keys off `PRAGMA user_version`, so reordering
    /// would silently skip a migration on an already-migrated install.
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
            name: "create_board_health",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS board_health (
                        board                TEXT PRIMARY KEY,
                        last_success_at      INTEGER,
                        last_verified_at     INTEGER,
                        failing_since        INTEGER,
                        consecutive_failures INTEGER NOT NULL DEFAULT 0,
                        last_error           TEXT,
                        last_run_id          TEXT,
                        updated_at           INTEGER NOT NULL
                    );",
                )
            },
        },
        // Lifetime verified/failed tallies — the flapping signal a
        // consecutive-failure counter cannot express. `DEFAULT 0` makes the
        // upgrade forward-safe: an existing row starts both tallies at zero and
        // simply needs `FLAKY_MIN_RUNS` more runs before a rate means anything.
        Migration {
            name: "add_board_health_run_tallies",
            up: |conn| {
                conn.execute_batch(
                    "ALTER TABLE board_health
                        ADD COLUMN verified_runs INTEGER NOT NULL DEFAULT 0;
                     ALTER TABLE board_health
                        ADD COLUMN failed_runs INTEGER NOT NULL DEFAULT 0;",
                )
            },
        },
    ];

    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("board_health.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Fold one scrape run's summaries into the per-board state and return the
    /// resulting health for each board in the SAME order as `summaries`.
    ///
    /// One transaction for the whole run, so a partial write can't leave half the
    /// boards advanced. A storage failure is reported to the caller, which
    /// degrades to "no health badges this run" rather than failing the scrape —
    /// diagnostics must never break the thing they diagnose.
    ///
    /// `pub(crate)`, not `pub`: the "bounded by the registry" invariant (see the
    /// module doc) depends on every caller pre-filtering to
    /// `resolvable_boards` — `ScraperEngine::record_health` is the only one
    /// that does, and narrowing visibility keeps it that way rather than
    /// leaving it a convention an out-of-crate caller could skip.
    pub(crate) fn record_run(
        &self,
        run_id: &str,
        summaries: &[BoardScrapeSummary],
    ) -> AppResult<Vec<BoardHealth>> {
        let now = now_ms();
        let mut guard = self.conn.lock();
        let tx = guard
            .transaction()
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let mut out = Vec::with_capacity(summaries.len());
        for summary in summaries {
            let prev =
                read_row(&tx, &summary.board).map_err(|e| AppError::Storage(e.to_string()))?;
            // `run_id` goes through `fold`, which stamps it ONLY on an arm that
            // actually contacted the board — a skip must not claim to have been
            // produced by a run that never fetched it.
            let next = fold(prev, summary, run_id, now);
            // Every column is written from `next` UNCONDITIONALLY (no COALESCE):
            // a recovery must NULL `failing_since`/`last_error` on disk, not just
            // in memory, or a later blip re-opens a window dated to an outage
            // months back. Guarded by
            // `a_recovery_clears_the_streak_columns_on_disk_not_just_in_memory`.
            tx.execute(
                "INSERT INTO board_health
                    (board, last_success_at, last_verified_at, failing_since,
                     consecutive_failures, last_error, last_run_id, updated_at,
                     verified_runs, failed_runs)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(board) DO UPDATE SET
                    last_success_at      = excluded.last_success_at,
                    last_verified_at     = excluded.last_verified_at,
                    failing_since        = excluded.failing_since,
                    consecutive_failures = excluded.consecutive_failures,
                    last_error           = excluded.last_error,
                    last_run_id          = excluded.last_run_id,
                    updated_at           = excluded.updated_at,
                    verified_runs        = excluded.verified_runs,
                    failed_runs          = excluded.failed_runs",
                params![
                    summary.board,
                    next.last_success_at.map(ts_to_db),
                    next.last_verified_at.map(ts_to_db),
                    next.failing_since.map(ts_to_db),
                    i64::from(next.consecutive_failures),
                    next.last_error,
                    next.last_run_id,
                    ts_to_db(now),
                    i64::from(next.verified_runs),
                    i64::from(next.failed_runs),
                ],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
            out.push(next);
        }
        tx.commit().map_err(|e| AppError::Storage(e.to_string()))?;
        Ok(out)
    }

    /// Every board with stored history, verdicts re-derived against *now*.
    ///
    /// The LIVE read behind `boards_health`: a persisted run snapshot must never
    /// carry the verdict (it is cross-run state, and it would go stale inside an
    /// immutable record and ride into the backup bundle), so the Autopilot card
    /// asks for the current answer instead. Bounded by the row count, which is
    /// bounded by the scraper registry.
    pub fn all(&self) -> Vec<BoardHealthEntry> {
        let now = now_ms();
        let conn = self.conn.lock();
        let Ok(mut stmt) = conn.prepare(&format!("{SELECT_COLUMNS} ORDER BY board")) else {
            log::warn!("[board-health] failed to prepare the health read; reporting none");
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, health_from(row)?)))
        else {
            log::warn!("[board-health] failed to query health; reporting none");
            return Vec::new();
        };
        // A row whose columns fail to decode is dropped rather than failing the
        // whole read (one corrupt row must not blank every OTHER board's
        // badge) — but silently dropping it is indistinguishable from "this
        // board is healthy" (no badge either way), so count and warn once
        // instead of losing the signal entirely.
        let mut dropped = 0usize;
        let out: Vec<BoardHealthEntry> = rows
            .filter_map(|row| match row {
                Ok(row) => Some(row),
                Err(_) => {
                    dropped += 1;
                    None
                }
            })
            .map(|(board, mut health)| {
                health.status = derive_status(&health, now);
                BoardHealthEntry { board, health }
            })
            .collect();
        if dropped > 0 {
            log::warn!("[board-health] dropped {dropped} row(s) that failed to decode");
        }
        out
    }

    /// Current health of one board, with the verdict re-derived against *now*.
    /// `None` when the board has no stored history.
    ///
    /// Test-only: production reads the whole (tiny) table via [`Self::all`], so
    /// a per-board query would be a second way to say the same thing.
    #[cfg(test)]
    pub(crate) fn health_for(&self, board: &str) -> Option<BoardHealth> {
        self.all()
            .into_iter()
            .find(|e| e.board == board)
            .map(|e| e.health)
    }

    /// Wipe every board's history (factory reset).
    ///
    /// `Resettable::reset` is infallible (returns `()`), so a `DELETE` that
    /// fails has nowhere to surface but a log line — matching
    /// `PipelineRunStore::clear_all`'s pattern (see that fn and
    /// `commands::privacy`'s `impl Resettable for PipelineRunStore` doc):
    /// that is the trait's contract for every store here, not a shortcut
    /// unique to one of them. Silently swallowing it (as this used to) would
    /// let `board_health.db` survive a reset the UI reports as successful.
    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        if let Err(e) = conn.execute("DELETE FROM board_health", []) {
            log::warn!(
                "[board-health] factory reset failed to clear board_health: {}",
                sanitize_reason(&e.to_string())
            );
        }
    }

    /// How many boards have stored history — i.e. the row count.
    ///
    /// Test seam for the guard that a renderer-supplied board id can never
    /// create a row: `board` is this table's PRIMARY KEY, so "no unexpected
    /// row appeared" is a claim only a count can make (asserting
    /// `health_for(id).is_none()` for one id you happened to think of does not).
    #[cfg(test)]
    pub(crate) fn tracked_boards(&self) -> usize {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM board_health", [], |r| {
            r.get::<_, i64>(0)
        })
        .map(|n| usize::try_from(n).unwrap_or(0))
        .unwrap_or(0)
    }
}

/// A stored tally as a `u32`. A negative value is never written by this store;
/// if a hand-edited row carries one, it reads as 0 ("no runs counted") rather
/// than saturating to `u32::MAX` — "down for 4294967295 runs" is a worse lie
/// than "nothing recorded".
fn count_from_db(v: i64) -> u32 {
    u32::try_from(v).unwrap_or(0)
}

/// Column list shared by the single-row and whole-table reads, so both map the
/// same indices. `board` is column 0; [`health_from`] reads from column 1.
const SELECT_COLUMNS: &str = "SELECT board, last_success_at, last_verified_at, failing_since,
                                     consecutive_failures, last_error, last_run_id,
                                     verified_runs, failed_runs
                              FROM board_health";

/// Map a [`SELECT_COLUMNS`] row to the stored facts. `status` is left `Unknown`
/// — the DB stores facts, never the verdict, which depends on the current time
/// and is filled by `derive_status` at every read.
fn health_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<BoardHealth> {
    Ok(BoardHealth {
        status: BoardHealthStatus::Unknown,
        consecutive_failures: count_from_db(row.get::<_, i64>(4)?),
        last_success_at: row.get::<_, Option<i64>>(1)?.map(ts_from_db),
        last_verified_at: row.get::<_, Option<i64>>(2)?.map(ts_from_db),
        failing_since: row.get::<_, Option<i64>>(3)?.map(ts_from_db),
        last_error: row.get::<_, Option<String>>(5)?,
        last_run_id: row.get::<_, Option<String>>(6)?,
        verified_runs: count_from_db(row.get::<_, i64>(7)?),
        failed_runs: count_from_db(row.get::<_, i64>(8)?),
    })
}

/// Read one board's stored row. Used by the transactional write path to fold
/// against the previous state.
fn read_row(conn: &Connection, board: &str) -> rusqlite::Result<Option<BoardHealth>> {
    conn.query_row(
        &format!("{SELECT_COLUMNS} WHERE board = ?1"),
        params![board],
        health_from,
    )
    .optional()
}

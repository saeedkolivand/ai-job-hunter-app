//! Per-board reliability history (Track B1).
//!
//! Every scrape already computes a [`BoardScrapeSummary`] per board and then
//! throws it away everywhere except the last autopilot run — so a board that has
//! been failing for a week is indistinguishable from one that simply found
//! nothing today. This store keeps the *derived* answer to "is this board
//! working?" across runs, and the engine folds each run's summaries into it.
//!
//! ## Shape: aggregate state, not a run log
//!
//! The table holds **exactly one row per board**, upserted on every run — it is
//! bounded by the size of the scraper registry (~24 rows), not by time or by the
//! autopilot cadence, so there is no growth axis to prune and no retention
//! window to age out. That is a deliberate departure from a per-run history
//! table: everything the badge (and the "why did this job source fail?"
//! diagnostic) needs is a fold over the runs, and a bounded ring of raw rows
//! would additionally *lie* once the last success aged out of the window
//! (`last_success_at` would read `NULL` = "never worked" for a board that worked
//! fine last month).
//!
//! ## `skipped` is not `error`
//!
//! A skipped board (`needs-login` / `needs-company` / `needs-keys`) was never
//! contacted, so it verifies nothing: [`fold`] leaves `last_verified_at`,
//! `last_success_at`, `consecutive_failures` and `failing_since` **untouched**
//! for a skip. A skip neither counts as a failure nor clears an existing failure
//! streak — a board that broke on Tuesday and has been skipped since Thursday is
//! still reported as broken since Tuesday.
//!
//! ## Correlation id
//!
//! No new id is minted: the scrape's existing `job_id` (`db::new_job_id`, the
//! same id `jobs_cancel` and the progress events use) is stored as
//! `last_run_id`, which is what turns a chip into something greppable in the
//! logs.
//!
//! Wired like the other L1 stores: `db::open` + a transactional, position-indexed
//! migration (ADR-022), wiped on factory reset via `Resettable` (registered in
//! `commands::privacy`). Deliberately **not** part of the backup bundle
//! (`DataStore`), for the same reason `email_watch` isn't: it is machine-local
//! bookkeeping about *this* install's network luck, and restoring it onto another
//! machine would assert a history that machine never had.

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension as _};
use serde::{Deserialize, Serialize};

use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::{AppError, AppResult};

use super::engine::BoardScrapeSummary;

/// A board whose last verified run succeeded, but whose last success is older
/// than this, is reported [`BoardHealthStatus::Stale`] — we have not actually
/// confirmed it works in a fortnight (it has only been skipped since). Two weeks
/// is comfortably longer than any built-in autopilot cadence, so a board that is
/// genuinely being exercised never trips it.
const STALE_AFTER_MS: u64 = 14 * 24 * 60 * 60 * 1000;

/// Max stored length of the remembered failure reason. The string is capped, not
/// sanitized, here: the identical raw `BoardScrapeSummary::error` is already
/// persisted verbatim by the autopilot store, and every consumer sanitizes at
/// display time (`BoardSummaryChips::sanitizeReason`), so this adds no exposure
/// the app did not already have — it only bounds the row.
const MAX_ERROR_LEN: usize = 200;

/// What a board's history says about it right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoardHealthStatus {
    /// The board has never actually been run — only skipped (or this is its
    /// first-ever appearance). We know nothing; say nothing.
    Unknown,
    /// The last run that actually contacted the board succeeded, recently.
    Healthy,
    /// The board's current failure streak is non-empty.
    Failing,
    /// Not failing, but the last confirmed success is older than
    /// [`STALE_AFTER_MS`] — in practice a board that has only been skipped for
    /// a fortnight, so its "0 results" is not evidence of anything.
    Stale,
}

/// One board's derived reliability, as shipped to the renderer on each
/// [`BoardScrapeSummary`].
///
/// Timestamps are epoch-ms. `#[serde(default)]` on every optional field so a
/// record persisted before this struct existed still deserializes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardHealth {
    pub status: BoardHealthStatus,
    /// Length of the current failure streak (0 when the board is not failing).
    /// Skipped runs are transparent — they neither extend nor break a streak.
    pub consecutive_failures: u32,
    /// Last run that actually returned results (or an empty-but-successful
    /// answer). `None` = the board has never succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<u64>,
    /// Last run that actually contacted the board at all (success OR error).
    /// `None` = only ever skipped, so nothing about it has been verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verified_at: Option<u64>,
    /// Start of the CURRENT failure streak — the "broken since Tuesday"
    /// timestamp. `None` when the board is not currently failing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failing_since: Option<u64>,
    /// The reason the current streak started failing, capped (see
    /// [`MAX_ERROR_LEN`]). `None` when not failing. Present so a board that is
    /// merely *skipped* this run can still explain why it is unhealthy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// The scrape `job_id` of the run that produced this state — the per-board
    /// correlation id for the logs. Reuses the existing id; none is minted here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_id: Option<String>,
}

impl BoardHealth {
    /// The state of a board with no stored history at all.
    fn empty() -> Self {
        Self {
            status: BoardHealthStatus::Unknown,
            consecutive_failures: 0,
            last_success_at: None,
            last_verified_at: None,
            failing_since: None,
            last_error: None,
            last_run_id: None,
        }
    }

    /// Whether this health is worth showing the user. A healthy or
    /// never-verified board adds nothing to its chip.
    pub fn is_noteworthy(&self) -> bool {
        matches!(
            self.status,
            BoardHealthStatus::Failing | BoardHealthStatus::Stale
        )
    }
}

/// The three mutually-exclusive things a run can say about a board.
///
/// Derived from a [`BoardScrapeSummary`] by [`outcome_of`]; existing as a type
/// (rather than two booleans) is what makes "a skip is not a failure"
/// unrepresentable-otherwise rather than a convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunOutcome {
    /// The board answered. A partial (`truncated`) harvest counts: the board was
    /// reachable and returned rows, which is what "does this source work?" asks.
    Ok,
    /// The board was contacted and failed.
    Error,
    /// The board was never contacted (needs-login / needs-company / needs-keys).
    Skipped,
}

/// Classify one run's summary. `error` wins over `skipped` — the engine never
/// sets both, but a persisted/tampered record could, and an error is the more
/// alarming reading.
fn outcome_of(summary: &BoardScrapeSummary) -> RunOutcome {
    if summary.error.is_some() {
        RunOutcome::Error
    } else if summary.skipped.is_some() {
        RunOutcome::Skipped
    } else {
        RunOutcome::Ok
    }
}

/// Fold one run's summary into a board's stored health. **Pure** — `now` is
/// injected, so the whole derivation is testable without a clock or a DB.
///
/// * `Ok`    → clears the streak, advances `last_success_at` + `last_verified_at`.
/// * `Error` → extends the streak (opening `failing_since` on the 0→1 edge so the
///   "since" is the FIRST failure, not the latest), advances `last_verified_at`.
/// * `Skipped` → advances nothing but the correlation id: the board was not
///   contacted, so it neither succeeded nor failed.
pub fn fold(prev: Option<BoardHealth>, summary: &BoardScrapeSummary, now: u64) -> BoardHealth {
    let mut next = prev.unwrap_or_else(BoardHealth::empty);
    match outcome_of(summary) {
        RunOutcome::Ok => {
            next.consecutive_failures = 0;
            next.failing_since = None;
            next.last_error = None;
            next.last_success_at = Some(now);
            next.last_verified_at = Some(now);
        }
        RunOutcome::Error => {
            // Saturating so a pathological run count can never wrap the streak
            // back to "healthy".
            next.consecutive_failures = next.consecutive_failures.saturating_add(1);
            // Only the 0→1 edge opens the window; a continuing streak keeps its
            // original start so the UI can say "failing since <first failure>".
            next.failing_since.get_or_insert(now);
            next.last_error = summary.error.as_deref().map(cap_error);
            next.last_verified_at = Some(now);
        }
        RunOutcome::Skipped => {}
    }
    next.status = derive_status(&next, now);
    next
}

/// Cap a reason to [`MAX_ERROR_LEN`] **characters** (not bytes — slicing a byte
/// range would panic mid-codepoint on a non-ASCII message).
fn cap_error(raw: &str) -> String {
    if raw.chars().count() <= MAX_ERROR_LEN {
        return raw.to_string();
    }
    let mut out: String = raw.chars().take(MAX_ERROR_LEN).collect();
    out.push('…');
    out
}

/// Status from the folded counters. Kept separate from [`fold`] so it can also
/// re-derive a row read back from disk (whose `Stale` verdict depends on *now*,
/// not on when the row was written).
fn derive_status(h: &BoardHealth, now: u64) -> BoardHealthStatus {
    if h.consecutive_failures > 0 {
        return BoardHealthStatus::Failing;
    }
    let Some(_verified) = h.last_verified_at else {
        // Only ever skipped — nothing has been confirmed either way.
        return BoardHealthStatus::Unknown;
    };
    match h.last_success_at {
        // Verified, no failure streak, but the confirmation has aged out: only
        // skips since. `saturating_sub` so a clock that moved backwards reads as
        // "recent", never as a giant staleness.
        Some(at) if now.saturating_sub(at) > STALE_AFTER_MS => BoardHealthStatus::Stale,
        Some(_) => BoardHealthStatus::Healthy,
        // Verified but never successful with an empty streak is unreachable via
        // `fold`; a hand-edited row could still produce it. Treat "tried, never
        // worked" as stale rather than claiming health.
        None => BoardHealthStatus::Stale,
    }
}

/// Per-board reliability store (`<dataDir>/board_health.db`).
pub struct BoardHealthStore {
    conn: Mutex<Connection>,
}

impl BoardHealthStore {
    /// Position-indexed migrations (ADR-022) — **append only**, never edit or
    /// insert: `run_migrations` keys off `PRAGMA user_version`, so reordering
    /// would silently skip a migration on an already-migrated install.
    const MIGRATIONS: &'static [Migration] = &[Migration {
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
    }];

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
    pub fn record_run(
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
            let mut next = fold(prev, summary, now);
            next.last_run_id = Some(run_id.to_string());
            tx.execute(
                "INSERT INTO board_health
                    (board, last_success_at, last_verified_at, failing_since,
                     consecutive_failures, last_error, last_run_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(board) DO UPDATE SET
                    last_success_at      = excluded.last_success_at,
                    last_verified_at     = excluded.last_verified_at,
                    failing_since        = excluded.failing_since,
                    consecutive_failures = excluded.consecutive_failures,
                    last_error           = excluded.last_error,
                    last_run_id          = excluded.last_run_id,
                    updated_at           = excluded.updated_at",
                params![
                    summary.board,
                    next.last_success_at.map(ts_to_db),
                    next.last_verified_at.map(ts_to_db),
                    next.failing_since.map(ts_to_db),
                    i64::from(next.consecutive_failures),
                    next.last_error,
                    next.last_run_id,
                    ts_to_db(now),
                ],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
            out.push(next);
        }
        tx.commit().map_err(|e| AppError::Storage(e.to_string()))?;
        Ok(out)
    }

    /// Current health of one board, with `Stale` re-evaluated against *now*.
    /// `None` when the board has no stored history.
    pub fn health_for(&self, board: &str) -> Option<BoardHealth> {
        let conn = self.conn.lock();
        let mut health = read_row(&conn, board).ok().flatten()?;
        health.status = derive_status(&health, now_ms());
        Some(health)
    }

    /// Wipe every board's history (factory reset).
    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM board_health", []).ok();
    }
}

/// Read one board's stored row. Shared by the transactional write path and the
/// read path, so both interpret the columns identically.
fn read_row(conn: &Connection, board: &str) -> rusqlite::Result<Option<BoardHealth>> {
    conn.query_row(
        "SELECT last_success_at, last_verified_at, failing_since,
                consecutive_failures, last_error, last_run_id
         FROM board_health WHERE board = ?1",
        params![board],
        |row| {
            Ok(BoardHealth {
                // Filled by `derive_status` at every read; the DB stores the
                // facts, never the verdict (which depends on the current time).
                status: BoardHealthStatus::Unknown,
                consecutive_failures: u32::try_from(row.get::<_, i64>(3)?).unwrap_or(u32::MAX),
                last_success_at: row.get::<_, Option<i64>>(0)?.map(ts_from_db),
                last_verified_at: row.get::<_, Option<i64>>(1)?.map(ts_from_db),
                failing_since: row.get::<_, Option<i64>>(2)?.map(ts_from_db),
                last_error: row.get::<_, Option<String>>(4)?,
                last_run_id: row.get::<_, Option<String>>(5)?,
            })
        },
    )
    .optional()
}

#[cfg(test)]
mod tests;

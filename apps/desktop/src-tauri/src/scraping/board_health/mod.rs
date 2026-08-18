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
//! The table holds **exactly one row per board**, upserted on every run — so it
//! is bounded by the number of DISTINCT board keys, not by time or by the
//! autopilot cadence, and there is no growth axis to prune and no retention
//! window to age out. That is a deliberate departure from a per-run history
//! table: everything the badge (and the "why did this job source fail?"
//! diagnostic) needs is a fold over the runs, and a bounded ring of raw rows
//! would additionally *lie* once the last success aged out of the window
//! (`last_success_at` would read `NULL` = "never worked" for a board that worked
//! fine last month).
//!
//! **That bound is only real because of a filter the CALLER applies.** `board`
//! is this table's PRIMARY KEY and it originates as a renderer-supplied string
//! (`commands::scrape` clones `req.boards` into the engine; the generated
//! `ScrapeBoardsRequest.boards` is an unvalidated `Vec<String>`), and the engine
//! deliberately lets an unknown id through to an ordinary error summary rather
//! than dropping it. So [`BoardHealthStore::record_run`] must only ever be
//! handed summaries whose board id the scraper RESOLVER recognised —
//! `ScraperEngine::record_health` filters on `resolvable_boards` for exactly
//! this reason. Without that filter a looping or XSS'd renderer could create
//! unbounded rows here, which is the threat `commands::scrape`'s limiter
//! already exists to stop. Guarded by
//! `engine::test::an_unresolvable_board_id_never_creates_a_health_row`.
//!
//! ## `skipped` is not `error`
//!
//! A skipped board (`needs-login` / `needs-company` / `needs-keys`) was never
//! contacted, so it verifies nothing: [`fold`] leaves **every** field untouched
//! for a skip — including `last_run_id`, which names the run that PRODUCED the
//! state and must therefore never name a run in which the board was not fetched.
//! A skip neither counts as a failure nor clears an existing failure streak — a
//! board that broke on Tuesday and has been skipped since Thursday is still
//! reported as broken since Tuesday, by Tuesday's run id.
//!
//! ## Correlation id
//!
//! No new id is minted: the scrape's existing `job_id` (`db::new_job_id`, the
//! same id `jobs_cancel` and the progress events use) is stored as
//! `last_run_id`, which is what turns a chip into something greppable in the
//! logs. It is stamped inside [`fold`]'s `Ok`/`Error` arms only, so it always
//! names a run that actually contacted the board.
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
use crate::observability::sanitize_reason;

use super::engine::BoardScrapeSummary;

/// A board whose last verified run succeeded, but whose last success is older
/// than this, is reported [`BoardHealthStatus::Stale`] — we have not actually
/// confirmed it works in a fortnight (it has only been skipped since). Two weeks
/// is comfortably longer than any built-in autopilot cadence, so a board that is
/// genuinely being exercised never trips it.
const STALE_AFTER_MS: u64 = 14 * 24 * 60 * 60 * 1000;

/// Minimum verified runs before a failure RATE means anything — below this a
/// single unlucky run would brand a board unreliable.
const FLAKY_MIN_RUNS: u32 = 8;

/// Share of verified runs that must have failed for a currently-working board to
/// report [`BoardHealthStatus::Flaky`].
const FLAKY_FAIL_PERCENT: u32 = 25;

/// Cap on `verified_runs` before [`decay_tallies`] halves both tallies.
///
/// Without a cap, `verified_runs`/`failed_runs` are LIFETIME counts, which
/// gives [`is_flaky`] unbounded inertia in both directions: a board that had a
/// 10-run outage and has since worked flawlessly stays badged `unreliable` for
/// dozens more clean runs (the old failures are diluted, not forgotten), while
/// a board with a long clean history that starts genuinely flapping needs just
/// as many runs before the rate crosses the threshold — precisely the
/// alternating-failure pattern [`BoardHealthStatus::Flaky`] exists to catch,
/// which `consecutive_failures` structurally cannot see.
///
/// Twice [`FLAKY_MIN_RUNS`] so a decay can never drop `verified_runs` below the
/// minimum sample [`is_flaky`] itself requires.
///
/// This is a window of RUNS, not of time, and the two only coincide at a steady
/// cadence. On the daily autopilot it spans roughly one to two weeks, which
/// lines up with [`STALE_AFTER_MS`]; for someone scraping manually several
/// times an afternoon the same 16 runs can span hours, so the verdict forgets
/// this morning's trouble by evening. Deliberate — the counter has no clock and
/// a run is the only evidence a board ever produces — but do not read
/// "recent reliability" here as a calendar claim.
const FLAKY_WINDOW_CAP: u32 = FLAKY_MIN_RUNS * 2;

/// Max stored length of the remembered failure reason.
///
/// The reason is REDACTED (`observability::sanitize_reason`) before it is stored,
/// not merely capped. "The autopilot store already persists the raw string" does
/// not license storing it here: that sink holds one autopilot's latest run and is
/// overwritten every run, whereas this row covers every scrape including manual
/// ones and is retained until the failure streak clears — indefinitely for a
/// permanently-broken board, and deliberately preserved across skips. That is new
/// retention and new scope, so the redaction is enforced here rather than left as
/// an invariant every current and future board has to remember.
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
    /// Working right now, but failing an unacceptable SHARE of its verified runs
    /// (see [`FLAKY_MIN_RUNS`]/[`FLAKY_FAIL_PERCENT`]).
    ///
    /// A consecutive-failure counter alone cannot see this: a board that
    /// alternates ok/fail every run reads `Healthy` on every other run and
    /// "down for 1 run" in between, so it never badges — indistinguishable from
    /// a board that failed exactly once. The tallies close that gap without
    /// reintroducing a growth axis (two more columns in the same row) — and
    /// [`decay_tallies`] bounds them to a rolling window so the verdict tracks
    /// RECENT reliability, not the board's entire history.
    Flaky,
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
    /// Only ever set by a run that actually CONTACTED the board (see [`fold`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_id: Option<String>,
    /// Count of runs that actually contacted the board (ok + error) within the
    /// decayed rolling window [`decay_tallies`] maintains (bounded by
    /// [`FLAKY_WINDOW_CAP`], not the board's entire history). Skips are
    /// excluded — they verify nothing.
    #[serde(default)]
    pub verified_runs: u32,
    /// Count of those windowed runs that failed. `failed_runs / verified_runs`
    /// is the flapping signal a consecutive-failure counter structurally
    /// cannot see.
    #[serde(default)]
    pub failed_runs: u32,
}

/// One row of [`BoardHealthStore::all`] — a board id and its current verdict.
/// The board id stays a sibling field rather than being flattened in, so the
/// renderer can key a lookup map off it directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardHealthEntry {
    pub board: String,
    pub health: BoardHealth,
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
            verified_runs: 0,
            failed_runs: 0,
        }
    }

    /// Whether this health is worth showing the user. A healthy or
    /// never-verified board adds nothing to its chip.
    pub fn is_noteworthy(&self) -> bool {
        matches!(
            self.status,
            BoardHealthStatus::Failing | BoardHealthStatus::Stale | BoardHealthStatus::Flaky
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
pub fn fold(
    prev: Option<BoardHealth>,
    summary: &BoardScrapeSummary,
    run_id: &str,
    now: u64,
) -> BoardHealth {
    let mut next = prev.unwrap_or_else(BoardHealth::empty);
    match outcome_of(summary) {
        RunOutcome::Ok => {
            next.consecutive_failures = 0;
            next.failing_since = None;
            next.last_error = None;
            next.last_success_at = Some(now);
            next.last_verified_at = Some(now);
            next.verified_runs = next.verified_runs.saturating_add(1);
            (next.verified_runs, next.failed_runs) =
                decay_tallies(next.verified_runs, next.failed_runs);
            next.last_run_id = Some(run_id.to_string());
        }
        RunOutcome::Error => {
            // Saturating so a pathological run count can never wrap the streak
            // back to "healthy".
            next.consecutive_failures = next.consecutive_failures.saturating_add(1);
            // Only the 0→1 edge opens the window; a continuing streak keeps its
            // original start so the UI can say "failing since <first failure>".
            next.failing_since.get_or_insert(now);
            next.last_error = summary.error.as_deref().map(clean_error);
            next.last_verified_at = Some(now);
            next.verified_runs = next.verified_runs.saturating_add(1);
            next.failed_runs = next.failed_runs.saturating_add(1);
            (next.verified_runs, next.failed_runs) =
                decay_tallies(next.verified_runs, next.failed_runs);
            next.last_run_id = Some(run_id.to_string());
        }
        // A skip advances NOTHING — not even `last_run_id`. That field names the
        // run which produced this state, and a skipped board was never contacted
        // by this run, so stamping it would attribute a Tuesday outage to a
        // Thursday run containing no fetch of that board at all.
        RunOutcome::Skipped => {}
    }
    next.status = derive_status(&next, now);
    next
}

/// Redact a reason (`observability::sanitize_reason`, the same scrubbing the
/// autopilot step log uses) and cap it to [`MAX_ERROR_LEN`] **characters** — not
/// bytes, since slicing a byte range would panic mid-codepoint on a non-ASCII
/// message. `sanitize_reason` already caps at its own ceiling; the cap is
/// re-applied here so this store's bound holds regardless of that constant.
fn clean_error(raw: &str) -> String {
    let redacted = crate::observability::sanitize_reason(raw);
    if redacted.chars().count() <= MAX_ERROR_LEN {
        return redacted;
    }
    let mut out: String = redacted.chars().take(MAX_ERROR_LEN).collect();
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
        // Working right now, but it has been failing a meaningful SHARE of the
        // runs that reached it — the state a streak counter cannot see.
        Some(_) if is_flaky(h) => BoardHealthStatus::Flaky,
        Some(_) => BoardHealthStatus::Healthy,
        // Verified but never successful with an empty streak is unreachable via
        // `fold`; a hand-edited row could still produce it. Treat "tried, never
        // worked" as stale rather than claiming health.
        None => BoardHealthStatus::Stale,
    }
}

/// Whether the windowed failure RATE (see [`decay_tallies`]) crosses the
/// flapping threshold. Integer math (no float rounding), and a hard minimum
/// sample so one bad run out of two never brands a board.
fn is_flaky(h: &BoardHealth) -> bool {
    h.verified_runs >= FLAKY_MIN_RUNS
        && h.failed_runs.saturating_mul(100) >= h.verified_runs.saturating_mul(FLAKY_FAIL_PERCENT)
}

/// Bound `verified`/`failed` to a rolling window of at most [`FLAKY_WINDOW_CAP`]
/// runs, called from [`fold`] every time a run actually contacts the board.
/// `while` (not `if`) so a row written before this fix — whose `verified_runs`
/// may already be arbitrarily large — collapses into the window in ONE fold
/// call instead of trickling down one halving per run.
///
/// `failed` is rescaled BY THE SAME RATIO the halving applies to `verified`
/// (`failed * new_verified / verified`), not halved independently
/// (`failed / 2`). Independent halving rounds `failed` down LESS than
/// `verified` whenever `verified` is odd, which can *inflate* the ratio and
/// manufacture a Flaky verdict out of nothing but rounding — measured:
/// `verified=17, failed=4` (23.5%, correctly not flaky) independently halves to
/// `8, 2` (25.0%, now flaky) on a run that was a plain success. Proportional
/// scaling can only round the ratio DOWN (floor), so a decay step alone can
/// never flip a board from not-flaky to flaky.
/// The rescale is done in `u64`. In `u32`, `failed * next_verified` overflows
/// once the product passes ~4.29e9 and `saturating_mul` clamps it BEFORE the
/// divide, which destroys the very proportionality this function exists to
/// preserve: `(200_000, 150_000)` — a 75% failure rate — decayed to `(12, 2)`,
/// i.e. 17%, reading not-flaky. Unreachable through [`fold`], which caps
/// `verified` at [`FLAKY_WINDOW_CAP`] after every call, but reachable from a
/// hand-edited row — the same threat model this module already defends against
/// for a negative tally, so it is defended here too rather than left to the
/// next caller to rediscover.
fn decay_tallies(mut verified: u32, mut failed: u32) -> (u32, u32) {
    while verified > FLAKY_WINDOW_CAP {
        let next_verified = verified / 2;
        // `failed <= verified` is an invariant of `fold`, and floor-scaling
        // preserves it, so the result is always <= next_verified and the
        // narrowing cast cannot truncate.
        failed = (u64::from(failed) * u64::from(next_verified) / u64::from(verified)) as u32;
        verified = next_verified;
    }
    (verified, failed)
}

/// Per-board reliability store (`<dataDir>/board_health.db`).
pub struct BoardHealthStore {
    conn: Mutex<Connection>,
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

#[cfg(test)]
mod tests;

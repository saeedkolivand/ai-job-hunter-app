//! Reliability types stored/derived by [`super::store`] and [`super::fold`]: the
//! per-board status enum, the health snapshot shipped to the renderer, and one
//! table row.
//!
//! Split out of `board_health` (issue #1280) — see the module doc on
//! `super` for the store's shape/retention rationale.

use serde::{Deserialize, Serialize};

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
    pub(super) fn empty() -> Self {
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

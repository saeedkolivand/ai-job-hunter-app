//! Outcome insights (what happened to the applications the user sent) and the
//! volume nudge (a gentle "slow down and tailor" signal), both derived on read
//! and never stored (ADR-042).
//!
//! `mod.rs` holds the wire types and the named thresholds; the math is split by
//! responsibility: `outcome` (classify one application), `group` (count and rate
//! per dimension), `nudge` (the trailing-24h check) and `build` (join the stores'
//! rows into the pure inputs). Nothing here touches SQLite, the clock or Tauri:
//! `now` is always a parameter. Pure L2, like the other derived-view modules.

use serde::Serialize;

mod build;
mod group;
mod nudge;
mod outcome;
#[cfg(test)]
mod tests;

pub use build::{build_inputs, AppInput, GenerationFacts};
pub use outcome::{classify, effective_status, ever_reached_positive};

/// An application counts as "no reply" once it has sat at `applied` longer than this.
pub const NO_REPLY_AFTER_DAYS: u64 = 21;
/// Fewer decided (non-pending) applications than this and a group reports counts
/// only, never a rate: a rate over three applications is noise dressed as a fact.
pub const MIN_GROUP_SAMPLE: u32 = 5;
/// Volume nudge: applications that became `applied` within the trailing window
/// must reach this count before the nudge can fire at all.
pub const NUDGE_MIN_APPLICATIONS: u32 = 10;
/// Volume nudge: the trailing window, in hours (`applied_at`-based).
pub const NUDGE_WINDOW_HOURS: u64 = 24;
/// Volume nudge: a share strictly above this (of the window's applications for
/// "untailored", of its letters for "near-copy") is what trips a condition.
pub const NUDGE_MAJORITY_SHARE: f64 = 0.5;
/// Two letters are near-copies at this content-word Jaccard or above. High on
/// purpose: a letter legitimately reuses a paragraph or two, a copy reuses nearly
/// all of it (same family as `voice.restated_point`, a higher bar).
pub const NEAR_COPY_JACCARD: f64 = 0.8;
/// Upper bound on letters compared pairwise inside the window (the comparison is
/// quadratic); beyond it only the most recent letters are considered.
pub const MAX_NUDGE_LETTERS: usize = 200;
/// Byte cap on any group key: board and language are stored or synced text, and the key
/// reaches agents in the reply, so it is bounded.
pub const MAX_GROUP_KEY_BYTES: usize = 64;
/// Group key for a row whose value is not known (no generation, no template yet).
pub const UNKNOWN_KEY: &str = "unknown";

pub(crate) const DAY_MS: u64 = 24 * 60 * 60 * 1000;
pub(crate) const HOUR_MS: u64 = 60 * 60 * 1000;

/// What became of one applied application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// Ever reached screening, interviewing, offer or accepted, whatever the
    /// status is now (derived from the status history, not just the current status).
    PositiveReply,
    /// Closed as rejected without ever reaching a positive stage.
    Rejected,
    /// Ghosted, or still `applied` after [`NO_REPLY_AFTER_DAYS`].
    NoReply,
    /// Still `applied`, inside the window: too early to call.
    Pending,
}

/// The axis a group slices the applications by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Dimension {
    /// The job board the application came from.
    Board,
    /// Whether an AI generation exists for the job (`tailored` / `untailored`).
    Tailored,
    /// Whether that generation carries a cover letter (`included` / `none`).
    CoverLetter,
    /// The generation's target language code.
    TargetLanguage,
    /// The month the application was sent (UTC, `YYYY-MM`).
    Month,
    /// The template last used to export the generation.
    Template,
}

/// How many applications landed in each outcome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeCounts {
    pub positive_reply: u32,
    pub rejected: u32,
    pub no_reply: u32,
    pub pending: u32,
}

/// Fractions in `0..=1` over the decided (non-pending) applications.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeRates {
    pub positive_reply: f64,
    pub rejected: f64,
    pub no_reply: f64,
}

/// One slice of one dimension.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeGroup {
    pub dimension: Dimension,
    pub key: String,
    pub counts: OutcomeCounts,
    /// `None` while fewer than [`MIN_GROUP_SAMPLE`] applications are decided.
    pub rates: Option<OutcomeRates>,
}

/// Which condition of the volume nudge held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NudgeTrigger {
    /// More than [`NUDGE_MAJORITY_SHARE`] of the window's applications have no generation.
    Untailored,
    /// More than [`NUDGE_MAJORITY_SHARE`] of the window's letters near-copy another letter.
    NearCopyLetters,
}

/// The volume nudge: counts over the trailing window and the conditions that held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeNudge {
    pub window_hours: u32,
    /// Applications that became `applied` inside the window.
    pub applications: u32,
    /// Of those, how many have no generation.
    pub untailored: u32,
    /// Of those, how many carry a non-empty cover letter.
    pub letters: u32,
    /// Of those letters, how many near-copy another letter in the window.
    pub near_copy_letters: u32,
    pub triggers: Vec<NudgeTrigger>,
}

/// The whole report: every group, the nudge when it fires, and when it was computed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightsReport {
    pub groups: Vec<OutcomeGroup>,
    pub nudge: Option<VolumeNudge>,
    pub generated_at: u64,
}

/// Compute the report from prepared inputs at `now` (ms since epoch).
pub fn report(inputs: &[AppInput], now: u64) -> InsightsReport {
    InsightsReport {
        groups: group::groups(inputs),
        nudge: nudge::volume_nudge(inputs, now),
        generated_at: now,
    }
}

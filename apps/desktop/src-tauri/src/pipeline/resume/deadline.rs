//! The run's wall clock: [`RunDeadline`], its effort-scaled limit
//! ([`run_deadline`]), and the boundary check that refuses the next provider
//! call once time is up ([`guard_deadline`]).

use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::pipeline::budget::{Budget, StoppedReason};

use super::RunLedger;

/// The wall-clock a run is allowed, given its reasoning effort — the trigger
/// for [`StoppedReason::RunTimeout`].
///
/// `Budget::run_timeout` is the FLOOR, not the answer: the budget constant is
/// effort-blind (it is a compile-time ceiling for the flow) while half of a
/// run's real cost scales with effort. Taking the larger of the two means a
/// deliberately-raised budget still wins and a high-effort run still gets its
/// scaled allowance. The effort-scaled half is computed by the caller (L3,
/// which owns `commands::ai_provider::timeouts`) and passed in.
pub fn run_deadline(budget: Budget, effort_scaled: Duration) -> Duration {
    budget.run_timeout.max(effort_scaled)
}

/// The run's wall clock: when it started and how long it is allowed.
///
/// A VALUE, not a hook, because two very different places have to ask the same
/// question. `StageHooks::before` checks it at every stage boundary — the
/// cheapest place to stop, since nothing is in flight — but a boundary check
/// alone cannot bound the LAST stage, and `repair` is both the last stage and
/// the only one that fans out (up to `max_repair_attempts ×
/// MAX_SECTIONS_PER_ROUND` provider calls). Before this existed, a repair loop
/// could run for ~2400 s past a deadline nothing would check again, and the
/// renderer's own client timeout — which can only say "it timed out" — fired
/// first. Copyable and Tauri-free so the stage can hold one without reaching
/// into L3.
#[derive(Debug, Clone, Copy)]
pub struct RunDeadline {
    started: Instant,
    limit: Duration,
}

impl RunDeadline {
    /// Start the clock now, with `limit` of wall time.
    pub fn starting_now(limit: Duration) -> Self {
        Self {
            started: Instant::now(),
            limit,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn limit(&self) -> Duration {
        self.limit
    }

    /// Whether the run has used up its allowance.
    pub fn passed(&self) -> bool {
        self.elapsed() >= self.limit
    }
}

/// The error a run stopped by its own deadline reports.
///
/// ONE string, shared by the boundary check
/// (`commands::resume_pipeline::hooks::apply_stop`) and by [`guard_deadline`],
/// so which of the deadline's enforcement points happened to see the clock
/// first is not something the user can tell from the message.
pub fn run_timeout_error(limit: Duration) -> AppError {
    AppError::Message(format!(
        "This generation ran past its {}-minute limit and was stopped. \
         Try a lower reasoning effort, or a faster model.",
        limit.as_secs() / 60
    ))
}

/// Refuse the NEXT provider round-trip when the run is already out of time,
/// recording WHY on the way out.
///
/// The boundary check cannot cover a stage that makes more than one call — the
/// lesson the repair loop's per-section check already carries. The other
/// multi-call shape is a JSON stage: [`Completer::complete_json`] is allowed one
/// re-ask, which is a second full provider call decided on inside the stage, so
/// a run whose deadline expired during the first call would pay for a second
/// (up to `timeouts::ollama_completion_deadline`) that nothing would look at.
///
/// **Hard error rather than the repair loop's "stop and keep".** A JSON stage
/// has no partial result to keep: the first response failed to parse, so there
/// is no artifact, and every downstream stage reads it. Recording
/// [`StoppedReason::RunTimeout`] and erroring is exactly what the boundary check
/// does one instant later — the terminal state then depends on whether a
/// document was already persisted, which is
/// `commands::resume_pipeline::hooks::terminal_state`'s decision, not this one's.
pub fn guard_deadline(ledger: &RunLedger, deadline: RunDeadline) -> AppResult<()> {
    if deadline.passed() {
        ledger.stop(StoppedReason::RunTimeout);
        return Err(run_timeout_error(deadline.limit()));
    }
    Ok(())
}

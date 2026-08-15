//! Run budgets — the per-flow ceilings a multi-step run is allowed to spend,
//! and [`StoppedReason`], the vocabulary for why one stopped early.
//!
//! Both used to live in the (now-deleted) agentic controller as four loose
//! `const`s. They moved here in Phase 3 because the agent loop was never the
//! only budgeted, cancellable, multi-step run in the app: the résumé pipeline
//! is one too, and a second copy of "how many steps / how many tokens / how
//! long per step" is exactly the drift this codebase keeps re-discovering.
//! `Budget::AGENT_PREP`/`Budget::AGENT_IMPROVE`, the two agent-flow budgets,
//! were deleted alongside the agent module (PR-5 step 2) —
//! [`Budget::RESUME_QUALITY`] is the sole shipped budget now.
//!
//! **Budgets are NEVER renderer-supplied.** They are compile-time constants
//! picked by the backend from the flow it is running, exactly like generation
//! ROUTING is picked from the backend-owned store rather than the request
//! (task #25 / [`crate::pipeline::Completer::from_active`]). A
//! renderer-supplied `maxSteps` or `maxTokens` would be an unbounded-spend
//! knob on a paid API: the anti-abuse limiter caps how OFTEN a run starts, not
//! how much ONE run may spend. The lock test in this module's `test` child
//! pins that no IPC request struct carries a budget field.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The ceilings for ONE run of a multi-step flow.
///
/// Every field is a HARD stop, not a target: hitting one ends the run with the
/// matching [`StoppedReason`] and keeps whatever progress was already made,
/// rather than discarding it (see the enum's variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Provider round-trips (agent turns / pipeline stages) per run.
    pub max_steps: usize,
    /// Tool invocations per run. Distinct from [`Self::max_steps`]: one turn can
    /// request several tool calls, and a loop that keeps calling tools without
    /// converging burns real money per call even while the step count crawls.
    ///
    /// **Currently unenforced.** The agentic controller that counted executed
    /// calls and ended a run at [`StoppedReason::MaxToolCalls`] once this was
    /// spent was deleted along with the rest of `agent/` (PR-5 step 2) —
    /// [`Self::RESUME_QUALITY`], the sole shipped budget, calls no tools and
    /// pins this field at `0`. Kept as a documented ceiling (not a live bound)
    /// for a future tool-calling flow, which would need its own enforcement
    /// point the way the deleted controller once was one.
    ///
    /// What bounds spend alongside it: `max_steps`, `max_tokens` (which counts
    /// the whole tool-schema payload every turn), the per-call
    /// [`Self::step_timeout`] race, and
    /// [`crate::limits::Limiter::charge_provider_daily`] on every provider
    /// round-trip a tool makes.
    pub max_tool_calls: usize,
    /// Accumulated token estimate (~chars/4) across prompts + completions.
    pub max_tokens: usize,
    /// Sections a single document run may produce. Bounds a section-wise
    /// generator (the résumé pipeline) whose section list comes from MODEL
    /// output: without it, a model that emits 400 "sections" turns one run into
    /// 400 provider calls.
    pub max_sections: usize,
    /// Re-asks allowed for ONE rejected artifact (a failed content validation, a
    /// response that would not parse). Distinct from a retry: the re-ask carries
    /// the rejection reason back to the model. Bounded because a model that
    /// cannot satisfy the check in two corrections will not satisfy it in ten.
    pub max_repair_attempts: usize,
    /// Wall clock for ONE provider turn or ONE tool call.
    ///
    /// **Currently unenforced anywhere in the crate.** The agentic controller
    /// that raced each turn and each tool call against it in a `select!` was
    /// deleted along with `agent/` (PR-5 step 2).
    /// [`crate::pipeline::Pipeline::run_hooked`] never enforced it either — a
    /// staged run's per-call bounds are the HTTP timeouts of the calls a stage
    /// makes (`timeouts::stream_deadline` / `timeouts::OLLAMA_COMPLETION`), and
    /// its whole-run bound is [`Self::run_timeout`], checked at every stage
    /// boundary AND inside the one stage that fans out (`stages::repair`).
    /// Documented rather than fixed: wrapping every stage in
    /// `tokio::time::timeout` would add a second timing mechanism above bounds
    /// that already fire with a specific, actionable error, and a stage that
    /// legitimately makes several calls (the repair loop) has no single "step"
    /// for this to bound. Do not read a value here as a guarantee anything
    /// will be interrupted.
    pub step_timeout: Duration,
    /// Wall clock for the WHOLE run — the backstop for a run that never trips a
    /// per-step timeout but crawls forever (many slow-but-answering steps).
    pub run_timeout: Duration,
    /// How long a suspended human-in-the-loop confirmation waits for an answer
    /// before defaulting to NOT acting.
    pub confirm_timeout: Duration,
}

impl Budget {
    /// The staged résumé generation + validation pipeline.
    ///
    /// **`max_steps` = 20, and it is UNENFORCED here** — `Pipeline::run_hooked`
    /// counts no steps, so `StoppedReason::MaxSteps` has no producer on this
    /// pipeline. It is a documented ceiling, not a live bound. The pipeline
    /// runs 8 stages (`analyze_job`, `match_evidence`, `strategy`, `draft`,
    /// `cover_letter`, `validate`, `repair`, `humanize` — see
    /// [`pipeline::resume::QUALITY_STAGES`](crate::pipeline::resume::QUALITY_STAGES)),
    /// comfortably under 20 with room for a future addition; the live per-run
    /// ceiling is [`Self::run_timeout`] plus the per-provider daily spend cap.
    ///
    /// **`max_tool_calls` = 0.** The pipeline calls no tools — it is stages, not
    /// an agentic loop. Zero is the honest bound, not an oversight: a stage that
    /// starts calling tools should have to justify raising this.
    ///
    /// **`max_tokens` = 200_000, over-provisioned by design.** The Phase-3
    /// derivation: 3 JSON stages (~3k + ~6k + ~5k tokens, each doubled by the
    /// one allowed re-ask ⇒ ~28k) + the draft (~7.5k) + ≤2 repair rounds × ≤4
    /// sections × ~6k (~37k) + (PR-2) the letter (~7.5k, the same order as the
    /// draft) + `humanize`'s ≤2 flagged-document rewrites (~6k each ⇒ ~12k) ≈
    /// **92k**. The 200k ceiling over-provisions this by ~2.2× — a since-removed
    /// second, section-wise depth was what it was originally sized for, and it
    /// is left as is rather than tightened for no live effect: it is NOT the
    /// live bound, the per-provider daily ceiling and the run deadline are, and
    /// this figure has no enforcement point in the Phase-3 stages
    /// ([`StoppedReason::MaxTokens`] stays unreachable here).
    ///
    /// **`step_timeout` = 360s**, but read its field doc first: it is INERT
    /// for this flow — `Pipeline::run_hooked` does not enforce it.
    ///
    /// **`run_timeout` = 90 min**, raised from an unvalidated 30 (via a wrong
    /// 45, then 75) and now DERIVED from the fan-out that actually runs: it is
    /// the effort-blind FLOOR that must agree with
    /// `timeouts::quality_run_deadline(None)`, which is
    /// `fixed + baseline × passes × 1.0` = 4800 s + 600 s = 5400 s. The fixed
    /// term is every call whose per-call bound is FLAT — 3 JSON stages × 2
    /// round-trips (1800 s), the repair fan-out (`max_repair_attempts` (2)
    /// rounds × `MAX_SECTIONS_PER_ROUND` (4) sections, 2400 s), and `humanize`'s
    /// worst case (≤2 flagged documents, 600 s), all at the 300 s
    /// `OLLAMA_COMPLETION` bound — and the scaled term is TWO streamed calls,
    /// the draft and the cover letter (`cover_letter` stage, PR-2). The
    /// 75-minute version counted only ONE streamed pass and no `humanize` term,
    /// so raising either without raising this floor reopens the inversion the
    /// 45-minute bug already taught: the renderer's own client timeout firing
    /// before the backend's, instead of after it — the backend must give up
    /// first because it is the side that knows WHY. Pinned by
    /// `quality_run_deadline_agrees_with_the_budget_floor_at_the_bottom_tier`
    /// and by `quality_run_deadline_clears_the_inner_per_call_bounds`, which
    /// computes those inner bounds from the fan-out constants themselves; the
    /// effort-scaled deadline above this floor is picked by
    /// `pipeline::resume::run_deadline`.
    ///
    /// **`confirm_timeout`** carries the app-wide value even though the pipeline
    /// suspends on nothing today — so a future confirm inherits the same wait as
    /// the agent's rather than a second, drifting number.
    pub const RESUME_QUALITY: Self = Self {
        max_steps: 20,
        max_tool_calls: 0,
        max_tokens: 200_000,
        max_sections: DEFAULT_MAX_SECTIONS,
        max_repair_attempts: DEFAULT_MAX_REPAIR_ATTEMPTS,
        step_timeout: Duration::from_secs(360),
        run_timeout: Duration::from_secs(90 * 60),
        confirm_timeout: Duration::from_secs(300),
    };
}

// ── Compile-time budget relations ────────────────────────────────────────────
//
// `RESUME_QUALITY` must still fit a full section list. The relation is between
// compile-time constants, so it is asserted at COMPILE time: a budget shrunk
// past what the pipeline needs fails `cargo build`, not merely `cargo test`. It
// lives HERE rather than in the `test` child for exactly that reason —
// `#[cfg(test)]` code is never compiled by a release build, so the same assert
// under `mod test` would have made the "fails the build" claim false. (A
// runtime `assert!` on two consts is also what clippy's
// `assertions_on_constants` correctly objects to.) The two agent-flow budgets
// (`AGENT_PREP`, `AGENT_IMPROVE`) and their own compile-time relations were
// deleted along with `agent/` (PR-5 step 2) — see git history for the
// prep/improve step-and-tool-call arithmetic if it is ever needed again.

/// The résumé pipeline's fixed non-section stages: plan, header, assemble,
/// validate. Named so the `max_steps` relation below asserts the arithmetic
/// [`Budget::RESUME_QUALITY`]'s doc actually states (12 + 4 = 16) rather than
/// merely "more than the section count" — which 13 would have satisfied,
/// leaving a run to die at [`StoppedReason::MaxSteps`] three stages from the end.
const RESUME_FRAMING_STAGES: usize = 4;

const _: () = assert!(
    Budget::RESUME_QUALITY.max_steps >= DEFAULT_MAX_SECTIONS + RESUME_FRAMING_STAGES,
    "RESUME_QUALITY.max_steps must fit one step per section PLUS the four framing stages"
);

/// Sections one document run may produce.
///
/// A résumé has a conventional shape — summary, experience, education, skills,
/// plus optional certifications/projects/languages/publications/awards — which
/// is well under a dozen in every market
/// (`@ajh/prompts`'s `resumeConventions`). Twelve accepts every real document
/// while still bounding a model that decides each bullet is its own section.
pub const DEFAULT_MAX_SECTIONS: usize = 12;

/// Re-asks allowed for ONE rejected artifact.
///
/// Two, because the failure modes are two: the model misread the instruction
/// (a second, more specific ask fixes it) or the model cannot do it (a third
/// ask is money spent on the same answer). The observed shape on the
/// structured-output path is the same — see
/// [`crate::pipeline::Completer::complete_json`], which allows exactly ONE
/// re-ask for a parse failure because a parse failure has no gradient at all.
pub const DEFAULT_MAX_REPAIR_ATTEMPTS: usize = 2;

/// Why a budgeted run stopped.
///
/// **Wire-compatible with the pre-move agentic-controller `StoppedReason`**
/// (the seven original variants, before the now-deleted agent module's
/// `StoppedReason` was replaced by this one and re-exported at its old path):
/// the `snake_case` rename is unchanged, so a job result's `stoppedReason`
/// field — e.g. the résumé pipeline's — still serializes to the exact same
/// strings the renderer's `STOPPED_SUFFIX` map keys on. The round-trip test in
/// this module's `test` child pins every wire string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoppedReason {
    /// The model returned a final answer with no tool calls / the last stage
    /// completed. The only non-early stop.
    Done,
    /// Hit [`Budget::max_steps`].
    MaxSteps,
    /// Hit [`Budget::max_tokens`].
    MaxTokens,
    /// The cancellation token fired.
    Cancelled,
    /// A turn hit the provider's output-length limit (`StopReason::Length`)
    /// WHILE requesting tool calls — its arguments may be truncated/
    /// half-serialized JSON, so the calls are never executed; the run stops here
    /// instead of guessing at malformed args.
    Truncated,
    /// A turn was refused by [`crate::limits::Limiter::charge_provider_daily`]
    /// (`AppError::RateLimited`) mid-run — stop gracefully and keep whatever
    /// progress was already accumulated instead of discarding it.
    Budgeted,
    /// One provider turn or tool call exceeded [`Budget::step_timeout`] — a
    /// hung/misconfigured endpoint must not block the run forever with no
    /// terminal event. Maps to a job FAILURE, never a silent success.
    Timeout,
    /// The WHOLE run exceeded [`Budget::run_timeout`] — every individual step
    /// answered in time, but the run as a whole never converged. Distinct from
    /// [`Self::Timeout`] because the remedy is different: one names a broken
    /// endpoint, this one names a run that needs a smaller job.
    RunTimeout,
    /// Hit [`Budget::max_tool_calls`] — the loop kept invoking tools without
    /// converging. Distinct from [`Self::MaxSteps`]: the step count can be well
    /// under budget while the per-call spend is not.
    MaxToolCalls,
    /// An artifact still failed its check after [`Budget::max_repair_attempts`]
    /// re-asks. The run stops rather than shipping the last rejected draft:
    /// output that failed validation must never be persisted as a success.
    MaxRepairs,
}

#[cfg(test)]
mod test;

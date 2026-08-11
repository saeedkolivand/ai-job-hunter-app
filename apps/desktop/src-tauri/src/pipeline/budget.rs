//! Run budgets — the per-flow ceilings a multi-step run is allowed to spend,
//! and [`StoppedReason`], the vocabulary for why one stopped early.
//!
//! Both used to live in `agent::controller` as four loose `const`s. They moved
//! here because the agent loop is no longer the only budgeted, cancellable,
//! multi-step run in the app: the résumé pipeline is one too, and a second copy
//! of "how many steps / how many tokens / how long per step" is exactly the
//! drift this codebase keeps re-discovering. One struct, one budget per flow.
//!
//! **Budgets are NEVER renderer-supplied.** They are compile-time constants
//! ([`Budget::AGENT_PREP`], [`Budget::RESUME_QUALITY`]) picked by the backend
//! from the flow it is running, exactly like generation ROUTING is picked from
//! the backend-owned store rather than the request (task #25 /
//! [`crate::pipeline::Completer::from_active`]). A renderer-supplied `maxSteps`
//! or `maxTokens` would be an unbounded-spend knob on a paid API: the anti-abuse
//! limiter caps how OFTEN a run starts, not how much ONE run may spend. The lock
//! test in this module's `test` child pins that no IPC request struct carries a
//! budget field, mirroring the routing lock in `commands::agent`.

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
    pub step_timeout: Duration,
    /// Wall clock for the WHOLE run — the backstop for a run that never trips a
    /// per-step timeout but crawls forever (many slow-but-answering steps).
    pub run_timeout: Duration,
    /// How long a suspended human-in-the-loop confirmation waits for an answer
    /// before defaulting to NOT acting.
    pub confirm_timeout: Duration,
}

impl Budget {
    /// The "prep this application" agentic flow
    /// ([`crate::agent::flows::PREP_APPLICATION_SYSTEM`]).
    ///
    /// **`max_steps` = 14.** Sized for today's longest FIXED sequence: 8 tool
    /// turns (`research_company`, `match_resume`, `draft_cover_letter`,
    /// `draft_resume`, `validate_resume`, `suggest_interview_questions`,
    /// `save_cover_letter`, `save_resume`) plus a planning turn and a
    /// closing-summary turn — a 10-turn floor. The whitelist carries 11 tools,
    /// so the three remaining résumé-quality Read tools
    /// ([`crate::agent::tools_quality::quality_tools`]) are reachable too; the
    /// prompt NAMES and RATIONS them at one optional call plus one
    /// `validate_resume` re-check after a fix, so the worst case is 10 + 2 = 12
    /// turns and 14 leaves two turns of slack for a model that splits a step or
    /// retries a declined confirm. The prompt-side half of that arithmetic is
    /// asserted by `agent::flows::tests::prep_application_sequence_fits_the_step_budget`,
    /// so a new numbered step fails a test instead of stranding a real run at
    /// [`StoppedReason::MaxSteps`] between the drafting spend and the saves.
    ///
    /// **`max_tool_calls` = 12.** The same arithmetic counted in CALLS rather
    /// than turns: 8 fixed + 1 rationed optional + 1 re-check = 10, plus 2 of
    /// slack. Deliberately below `max_steps` — a run that has spent 12 tool
    /// calls still has turns left to write its summary.
    ///
    /// **`max_tokens` = 120_000.** The drafted résumé is echoed through the
    /// accumulator TWICE — once as the `draft_resume` tool result, once as the
    /// `save_resume` args turn — on top of the cover letter, match result,
    /// company research, and every fenced input. At
    /// [`crate::agent::tools::SAVED_RESUME_CAP`] (40k chars, ~10k tokens) that
    /// is ~20k tokens from the résumé echoes alone; 120k leaves clear headroom
    /// for the rest of the transcript, so a large résumé cannot truncate the run
    /// before the final save/summary. Each extra turn also re-sends the whole
    /// tool-schema payload (all 11 tools), counted once per turn — which is why
    /// the optional calls are rationed in the PROMPT rather than by raising
    /// these ceilings again.
    ///
    /// **`max_sections`/`max_repair_attempts`** are inert for this flow (the
    /// agent produces no section list and runs no repair loop); they carry the
    /// app-wide defaults so a future agent-side repair inherits a sane bound
    /// instead of an unbounded one.
    ///
    /// **`step_timeout` = 360s.** Set comfortably above the longest single-call
    /// HTTP timeout we ship (`commands::ai_provider::timeouts::OLLAMA_COMPLETION`
    /// = 300s) so that timeout's own specific network error surfaces first in
    /// the common case; this is the backstop for whatever slips past it (a
    /// custom `base_url` whose connect/read hangs outside the per-request client
    /// timeout). Before it existed, the loop raced only against `cancel`, so a
    /// hung endpoint blocked the run for minutes with no terminal event and the
    /// run looked stuck at pending forever.
    ///
    /// **`run_timeout` = 45 min.** `max_steps × step_timeout` is 84 minutes,
    /// which is a bound but not a ceiling anyone would want to sit through. A
    /// realistic worst case is a 12-turn run against a slow local model at ~2
    /// min/turn ≈ 24 min, so 45 leaves real headroom while still ending a run
    /// that answers every step just slowly enough to never trip `step_timeout`.
    pub const AGENT_PREP: Self = Self {
        max_steps: 14,
        max_tool_calls: 12,
        max_tokens: 120_000,
        max_sections: DEFAULT_MAX_SECTIONS,
        max_repair_attempts: DEFAULT_MAX_REPAIR_ATTEMPTS,
        step_timeout: Duration::from_secs(360),
        run_timeout: Duration::from_secs(45 * 60),
        confirm_timeout: Duration::from_secs(300),
    };

    /// The section-wise résumé generation + validation pipeline.
    ///
    /// **`max_steps` = 20.** A section-wise run is one step per section plus the
    /// fixed framing stages (plan, header, assemble, validate); at
    /// [`DEFAULT_MAX_SECTIONS`] sections that is 12 + 4 = 16, with slack.
    ///
    /// **`max_tool_calls` = 0.** The pipeline calls no tools — it is stages, not
    /// an agentic loop. Zero is the honest bound, not an oversight: a stage that
    /// starts calling tools should have to justify raising this.
    ///
    /// **`max_tokens` = 200_000.** Each section re-sends the grounding context
    /// (the source résumé slice + the job ad): ~4k prompt + ~1k output per
    /// section × 12 ≈ 60k, and a section that fails validation is re-asked up to
    /// [`DEFAULT_MAX_REPAIR_ATTEMPTS`] times, so the worst case is roughly 3×
    /// that ≈ 180k. 200k covers it without licensing an unbounded loop.
    ///
    /// **`step_timeout`** matches [`Self::AGENT_PREP`] for the same reason (it
    /// is a backstop above the longest HTTP timeout, not a per-call target),
    /// while `run_timeout` is 30 min: a per-section call is far smaller than a
    /// whole-résumé call, so 20 slow steps should not add up to three quarters
    /// of an hour.
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
        run_timeout: Duration::from_secs(30 * 60),
        confirm_timeout: Duration::from_secs(300),
    };
}

// ── Compile-time budget relations ────────────────────────────────────────────
//
// `AGENT_PREP` must still fit the prep flow's fixed sequence, and
// `RESUME_QUALITY` must still fit a full section list. Both relations are
// between compile-time constants, so they are asserted at COMPILE time: a budget
// shrunk past what its flow needs fails `cargo build`, not merely `cargo test`.
// They live HERE rather than in the `test` child for exactly that reason —
// `#[cfg(test)]` code is never compiled by a release build, so the same asserts
// under `mod test` would have made the "fails the build" claim false. (A runtime
// `assert!` on two consts is also what clippy's `assertions_on_constants`
// correctly objects to.) The prompt-side half of the agent arithmetic lives in
// `agent::flows`; this is the budget-side half.

/// 8 fixed tool turns + 1 rationed optional quality call + 1 `validate_resume`
/// re-check after a fix.
const PREP_WORST_CASE_TOOL_CALLS: usize = 10;
/// ...plus a planning turn and a closing-summary turn.
const PREP_WORST_CASE_TURNS: usize = PREP_WORST_CASE_TOOL_CALLS + 2;

const _: () = assert!(
    Budget::AGENT_PREP.max_steps > PREP_WORST_CASE_TURNS,
    "AGENT_PREP.max_steps must leave slack above the 12-turn prep worst case"
);
const _: () = assert!(
    Budget::AGENT_PREP.max_tool_calls >= PREP_WORST_CASE_TOOL_CALLS,
    "AGENT_PREP.max_tool_calls must admit the 10-call prep worst case"
);
const _: () = assert!(
    Budget::AGENT_PREP.max_tool_calls < Budget::AGENT_PREP.max_steps,
    "a run that exhausts its tool calls must still have turns left to summarize"
);
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
/// **Wire-compatible with the pre-move `agent::controller::StoppedReason`**: the
/// variant set is a superset and the `snake_case` rename is unchanged, so the
/// `stoppedReason` field of an `agent.run` job result still serializes to the
/// exact same strings the renderer's `STOPPED_SUFFIX` map keys on. The round-trip
/// test in this module's `test` child pins every wire string.
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
    /// terminal event. Maps to a job FAILURE in `commands::agent::agent_run`
    /// (never a silent success).
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

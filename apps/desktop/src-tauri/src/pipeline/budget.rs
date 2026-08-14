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
    ///
    /// **ENFORCED since Phase 7** by [`crate::agent::controller`], which counts
    /// every executed call (including a refused unknown-tool name) and ends the
    /// run at [`StoppedReason::MaxToolCalls`] once the count is spent — the
    /// variant's first producer in the crate. Pinned by
    /// `agent::controller::test::the_tool_call_ceiling_stops_a_runaway_tool_loop`.
    ///
    /// **It was a dead constant for two flows' worth of history, and the second
    /// flow is what made that a hole.** `run_quality_pipeline` is a 75-minute
    /// call reachable from `improve_resume`, the prompt's "spend AT MOST ONE of
    /// them" is prose a prompt-injected posting can argue the model out of
    /// (OWASP LLM01), and nothing else in the loop counts calls — so the
    /// reachable spend was the whole allowance times the most expensive tool
    /// the app has.
    ///
    /// **Cost of enforcing it this way, stated rather than discovered.** A hard
    /// stop spends none of the summary turns the ration reserves
    /// ([`Self::AGENT_PREP`] rations 12 calls under 14 steps precisely so a run
    /// that runs out of calls "still has turns left to write its summary"), and
    /// it makes [`StoppedReason::MaxSteps`] unreachable for a tool-calling run
    /// under any budget where `max_tool_calls < max_steps` — which is every
    /// shipped one. MaxSteps still binds a loop that answers without calling
    /// tools, and
    /// `the_step_ceiling_is_still_reachable_when_tool_calls_are_not_the_binding_ceiling`
    /// pins that it is not dead code. The better fix remains to stop OFFERING
    /// tools once the count is spent (an empty
    /// [`crate::commands::ai_provider::ToolSpec`] list for the remaining
    /// turns), which the loop still cannot do: the spec list is built once in
    /// `LiveAgentEnv` and `AgentEnv::turn` takes only messages, so it is a
    /// signature change across the trait and both fakes. A stop that is crude
    /// beats a ceiling that is fiction.
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
    /// **Enforced only by [`crate::agent::controller`]**, which races each turn
    /// and each tool call against it in a `select!`. [`crate::pipeline::Pipeline::run_hooked`]
    /// does NOT — a staged run's per-call bounds are the HTTP timeouts of the
    /// calls a stage makes (`timeouts::stream_deadline` /
    /// `timeouts::OLLAMA_COMPLETION`), and its whole-run bound is
    /// [`Self::run_timeout`], checked at every stage boundary AND inside the one
    /// stage that fans out (`stages::repair`). Documented rather than fixed:
    /// wrapping every stage in `tokio::time::timeout` would add a second
    /// timing mechanism above bounds that already fire with a specific,
    /// actionable error, and a stage that legitimately makes several calls (the
    /// repair loop) has no single "step" for this to bound. Do not read a value
    /// here as a guarantee a pipeline stage will be interrupted.
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
    /// closing-summary turn — a 10-turn floor. The whitelist carries 13 tools,
    /// so the three remaining résumé-quality Read tools
    /// ([`crate::agent::tools_quality::quality_tools`]) and the two cheap
    /// pipeline tools ([`crate::agent::tools_pipeline`] — `analyze_job`,
    /// `get_quality_report`) are reachable too; the prompt NAMES and RATIONS
    /// them at one optional call plus one
    /// `validate_resume` re-check after a fix, so the worst case is 10 + 2 = 12
    /// turns and 14 leaves two turns of slack for a model that splits a step or
    /// retries a declined confirm. The prompt-side half of that arithmetic is
    /// asserted by `agent::flows::tests::prep_application_sequence_fits_the_step_budget`,
    /// so a new numbered step fails a test instead of stranding a real run at
    /// [`StoppedReason::MaxSteps`] between the drafting spend and the saves.
    ///
    /// **Phase 3 added tools and this budget did NOT move — the arithmetic
    /// says so, rather than the omission being an oversight.** The plan's
    /// "re-sized for the larger toolset" note was paid in Phase 1, when the
    /// four quality tools took `max_steps` to 14 and `max_tool_calls` to 12.
    /// Phase 3's two additions ride the SAME single optional ration the prompt
    /// already grants — the ration is per RUN, not per tool — so the worst case
    /// is still 12 turns and 10 calls no matter how many optional entries the
    /// list grows. Only the per-turn TOKEN cost scales with the tool count
    /// (every turn re-sends the whole schema payload), and two more
    /// no-argument schemas are ~300 chars ≈ 75 tokens × ≤14 turns ≈ 1k against
    /// `max_tokens` = 120 000. Widening the ration is what would need a
    /// re-size, and the compile-time relations below would fail first.
    ///
    /// **`max_tool_calls` = 12.** The same arithmetic counted in CALLS rather
    /// than turns: 8 fixed + 1 rationed optional + 1 re-check = 10, plus 2 of
    /// slack. Below `max_steps` so the ceiling a runaway tool loop hits is the
    /// one that NAMES it ([`StoppedReason::MaxToolCalls`]) rather than a generic
    /// step exhaustion — enforced since Phase 7, and the summary turns the gap
    /// once reserved are not reachable through it; see the field doc on
    /// [`Budget::max_tool_calls`] for what that stop does and does not buy.
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

    /// The "improve this résumé" agentic flow
    /// ([`crate::agent::flows::IMPROVE_RESUME_SYSTEM`]) — a REVIEW pass over a
    /// generation that already exists, not a second way to write one.
    ///
    /// **`step_timeout` = 105 min, and it is DERIVED, not chosen.** This flow is
    /// the only home of `run_quality_pipeline`
    /// ([`crate::agent::tools_pipeline`]), and
    /// [`crate::agent::controller`] races EVERY tool call against this field:
    /// one call of that tool is a whole quality run, bounded by
    /// `tools_pipeline::quality_tool_deadline()` =
    /// `pipeline::resume::run_deadline(RESUME_QUALITY, timeouts::quality_run_deadline(None))`
    /// = **5 400 s** (PR-2 raised this floor from 4 500 s — see
    /// [`Self::RESUME_QUALITY::run_timeout`](Self::RESUME_QUALITY)'s own doc for
    /// why: the `cover_letter` stage adds a second streamed pass and `humanize`
    /// adds a flat allowance), and the pipeline's `guard_deadline` refuses only
    /// the NEXT call — a call admitted one second inside the deadline still runs
    /// its own full [`Self::RESUME_QUALITY`]-flow bound
    /// (`timeouts::OLLAMA_COMPLETION`, 300 s). So the floor is
    /// 5 400 + 300 + a 10 s two-clock margin = 5 710 s, and 6 300 leaves the
    /// SAME ~590 s of headroom the pre-PR-2 arithmetic had for the non-provider
    /// work inside the same call (loading the inputs, assembling, compaction,
    /// the JSON summary). The arithmetic is asserted at COMPILE time next to the
    /// constants it reads, in `agent::tools_pipeline` — the same place, and the
    /// same discipline, as `AGENT_STAGE_DEADLINE`'s own relation.
    ///
    /// **What the long clock does and does not weaken.** It is ONE clock for
    /// every turn and every tool call, so the AGENT-side backstop on this flow
    /// is 105 minutes rather than 6. That matters less than it reads, because
    /// the backstop is not what bounds an ordinary turn: every provider call
    /// goes through `commands::ai_provider::retry::send_with_retry_capped`,
    /// which applies its caller's timeout to the whole retry SEQUENCE, at all
    /// four adapters (Ollama 300 s; OpenAI/Anthropic/Gemini 120 s). A hung
    /// endpoint on a normal turn still fails in 2–5 minutes with its own
    /// specific network error. This clock is the sole bound only for a tool
    /// that owns an internal deadline instead of a single HTTP call — today
    /// exactly one, `run_quality_pipeline`, bounded at 5 400 s by
    /// `quality_tool_deadline`, which is the reason the number is what it is.
    ///
    /// **Trigger for revisiting it** (a per-TOOL timeout in the controller,
    /// which is a controller change, not a budget one): **the first tool that
    /// has NEITHER its own internal deadline NOR a per-call HTTP bound joining
    /// a long-clock flow.** Such a tool would be raced only by this 105-minute
    /// value, and 105 minutes is not a bound on anything that can hang. Until
    /// then a second timing mechanism would sit above bounds that already fire
    /// with actionable errors.
    ///
    /// **Reachable worst case, honestly.** `max_tool_calls` (8) × a 105-minute
    /// call is ~14 h if a run somehow spent every call on the pipeline tool —
    /// that is the ceiling the ENFORCED bounds admit, and `run_timeout` below
    /// does not cut it down (nothing in the agent loop reads it).
    ///
    /// The product is only a bound because the count is checked BEFORE EACH
    /// CALL, not once per turn: providers may return several tool calls in one
    /// turn, and each executed call races `step_timeout` on its own, so a
    /// turn-boundary check would have admitted `max_tool_calls - 1 + K` calls —
    /// an unbounded tail at 105 minutes apiece (HIGH, Phase-7 delta review; see
    /// `agent::controller`'s per-call refusal and
    /// `a_parallel_tool_turn_cannot_spend_past_the_tool_call_ceiling`).
    ///
    /// What keeps a real run far under the ceiling: the prompt rations that
    /// tool to at most one use, its own 5 400 s deadline, the per-provider
    /// daily ceiling, and the user's Stop.
    ///
    /// **`max_steps` = 10.** The prompt's fixed sequence is 7 turns (a plan
    /// turn, 5 tool turns — `get_quality_report`, `validate_resume`,
    /// `search_candidate_evidence`, the post-fix `validate_resume` re-check,
    /// `save_resume` — and a closing summary), plus AT MOST 1 rationed optional
    /// call (`get_trim_suggestions` or `run_quality_pipeline`): 8 worst case,
    /// with 2 turns of slack for a model that splits a step or retries a
    /// declined confirm. Fewer than [`Self::AGENT_PREP`]'s 14 because this flow
    /// drafts nothing: it reads a document that exists, reasons, and asks once
    /// to save. The prompt-side half is asserted by
    /// `agent::flows::tests::improve_resume_sequence_fits_the_step_budget`, the
    /// budget-side half by the compile-time relations below.
    ///
    /// **`max_tool_calls` = 8.** The same arithmetic in CALLS: 5 fixed + 1
    /// rationed optional = 6, plus 2 of slack, and below `max_steps` for the
    /// reason `AGENT_PREP` gives. Enforced (Phase 7), and on this flow that is
    /// load-bearing rather than tidy: it is the only ENFORCED bound on how many
    /// times a steered run may re-enter the 90-minute pipeline tool.
    ///
    /// **`max_tokens` = 80_000.** The résumé under review crosses the
    /// transcript up to four times: as the fenced generation in the seed
    /// message, as `validate_resume`'s `draft` twice (the check and the
    /// post-fix re-check), and as `save_resume`'s args. At
    /// [`crate::agent::tools::SAVED_RESUME_CAP`] (40k chars ≈ 10k tokens) for
    /// the save and [`crate::agent::tools::RESUME_CAP`] (8k chars ≈ 2k tokens)
    /// for each fenced/`draft` copy that is ~16k tokens, plus the compacted
    /// report/evidence/trim summaries (each bounded by `tools_quality`'s
    /// `SUMMARY_CAP`), plus `run_quality_pipeline`'s returned draft, plus the
    /// 6-tool schema payload re-sent every turn. ~50k realistic worst case;
    /// 80k keeps a large document from truncating the run before the save, at
    /// two thirds of the prep ceiling because this flow carries no cover
    /// letter, no company research and no match result.
    ///
    /// **`run_timeout` = 135 min — the run this flow PLANS, not a bound anything
    /// applies.** [`crate::agent::controller`] counts steps, tokens and tool
    /// calls and races `step_timeout`; it has no whole-run clock, so
    /// [`StoppedReason::RunTimeout`] has no producer on the agent path and this
    /// number stops nothing. It describes the intended run — one 105-minute
    /// pipeline call plus nine ordinary turns at ~2 min ≈ 123 min — while the
    /// ceiling the enforced bounds actually admit is the ~14 h computed above.
    /// Both figures are stated because the gap between them is the honest state
    /// of this flow, and because `run_timeout >= step_timeout` (which the
    /// consistency test checks) holds either way.
    ///
    /// **`max_sections`/`max_repair_attempts`** are inert here for the reason
    /// [`Self::AGENT_PREP`] states; they carry the app-wide defaults.
    ///
    /// **`confirm_timeout`** is the app-wide 300 s: the same wait the prep
    /// flow's saves get, because it is the same human answering the same
    /// question about the same kind of document.
    pub const AGENT_IMPROVE: Self = Self {
        max_steps: 10,
        max_tool_calls: 8,
        max_tokens: 80_000,
        max_sections: DEFAULT_MAX_SECTIONS,
        max_repair_attempts: DEFAULT_MAX_REPAIR_ATTEMPTS,
        step_timeout: Duration::from_secs(105 * 60),
        run_timeout: Duration::from_secs(135 * 60),
        confirm_timeout: Duration::from_secs(300),
    };

    /// The staged résumé generation + validation pipeline (quality depth).
    ///
    /// **`max_steps` = 20, and it is UNENFORCED here** — the same caveat
    /// [`Self::RESUME_MAX`]'s own copy of this field states: `Pipeline::run_hooked`
    /// counts no steps, so `StoppedReason::MaxSteps` has no producer on this
    /// pipeline. It is a documented ceiling, not a live bound. Quality depth
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
    /// **`max_tokens` = 200_000.** Each section re-sends the grounding context
    /// (the source résumé slice + the job ad): ~4k prompt + ~1k output per
    /// section × 12 ≈ 60k, and a section that fails validation is re-asked up to
    /// [`DEFAULT_MAX_REPAIR_ATTEMPTS`] times, so the worst case is roughly 3×
    /// that ≈ 180k. 200k covers it without licensing an unbounded loop.
    ///
    /// *Re-derived for QUALITY depth (Phase 3), which is not section-wise:* 3
    /// JSON stages (~3k + ~6k + ~5k tokens, each doubled by the one allowed
    /// re-ask ⇒ ~28k) + the draft (~7.5k) + ≤2 repair rounds × ≤4 sections ×
    /// ~6k (~37k) + (PR-2) the letter (~7.5k, the same order as the draft) +
    /// `humanize`'s ≤2 flagged-document rewrites (~6k each ⇒ ~12k) ≈ **92k**.
    /// The 200k ceiling over-provisions quality depth by ~2.2× and is sized for
    /// the max-depth fan-out it was written for, so it is left as is — but it
    /// is NOT the live bound at quality depth: the per-provider daily ceiling
    /// and the run deadline are, and this figure has no enforcement point in
    /// the Phase-3 stages ([`StoppedReason::MaxTokens`] stays unreachable
    /// here).
    ///
    /// **`step_timeout`** matches [`Self::AGENT_PREP`]'s value, but read its
    /// field doc first: it is INERT for this flow — `Pipeline::run_hooked` does
    /// not enforce it.
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

    /// MAX depth: the same pipeline with the whole-body draft replaced by one
    /// structured call per section.
    ///
    /// **`max_steps` = 24, and it is UNENFORCED in this flow.** One step per
    /// section ([`DEFAULT_MAX_SECTIONS`] = 12) plus the six framing stages
    /// around the fan-out (analyze, evidence, strategy, assemble, validate,
    /// repair) is 18; 24 leaves room for the judge and for a stage split. But
    /// `Pipeline::run_hooked` counts no steps and [`StoppedReason::MaxSteps`] is
    /// unreachable from here — it is the AGENT loop's stop reason. The number is
    /// a documented ceiling that the compile-time assertion against
    /// `pipeline::resume::MAX_STAGES` keeps HONEST (adding a stage without
    /// raising it fails the build), not a bound anything checks at run time; the
    /// live bounds are the run deadline, `step_timeout`'s per-call cousins, and
    /// the per-provider daily ceiling. Said plainly rather than left to be
    /// inferred, because "a budget field exists" reads as "a budget field is
    /// enforced".
    ///
    /// **`step_timeout` = 360 s, also unenforced as a STAGE bound.** No stage
    /// here is wrapped in it; what actually bounds a call is
    /// `timeouts::OLLAMA_COMPLETION`/`COMPLETION` inside the provider layer. Its
    /// one real use in this flow is the per-entry REGENERATE
    /// (`commands::resume_pipeline::max::regenerate_entry`), which has no run
    /// clock to share and takes this as the click's own deadline.
    ///
    /// **`max_tool_calls` = 0**, for the reason [`Self::RESUME_QUALITY`] gives:
    /// this is stages, not an agentic loop.
    ///
    /// **`max_tokens` = 200_000.** Unchanged, and this is the flow the figure
    /// was originally DERIVED for (see `RESUME_QUALITY`'s doc, which notes it
    /// over-provisions quality depth by ~2.7× because it was sized for the max
    /// fan-out): ~4k prompt + ~1k output per section × 12 ≈ 60k, tripled for the
    /// re-ask and repair worst case ≈ 180k. Still unenforced — the live bounds
    /// are the per-provider daily ceiling and the run deadline.
    ///
    /// **`run_timeout` = 125 min.** The effort-blind FLOOR, DERIVED like its
    /// quality sibling and required to equal `timeouts::max_run_deadline(None)`
    /// (`max_run_deadline_agrees_with_the_budget_floor_at_the_bottom_tier`).
    /// Max depth streams nothing, so every call is bounded by the flat
    /// `OLLAMA_COMPLETION` (300 s): 4 single-call stages (analyze, evidence,
    /// strategy, judge) + 12 sections + 2 repair rounds × 4 sections = 24 calls
    /// = 7 200 s, plus one effort-scaled whole-document pass (300 s at the
    /// bottom tier) = 7 500 s.
    ///
    /// The judge was missing from this count while the stage was already in the
    /// pipeline, which put the fixed term at 6 900 s and made the total exactly
    /// equal the 24 calls a run plans — a backstop with zero slack, and a pin
    /// (`max_run_deadline_clears_the_inner_per_call_bounds`) that could not see
    /// the problem because it transcribed "3 JSON stages" instead of reading
    /// the pipeline.
    ///
    /// **The one re-ask per JSON call is deliberately NOT counted, which is
    /// where this derivation departs from `RESUME_QUALITY`'s.** Counting it
    /// doubles the fan-out term to 12 000 s — 3 h 20 m, a "backstop" no user
    /// would sit through and one that makes [`StoppedReason::RunTimeout`]
    /// unreachable in practice. A re-ask happens only on a PARSE failure, it is
    /// refused by the same `guard_deadline` the rest of the run is bounded by,
    /// and — the part that only max depth can say — a run stopped mid-fan-out
    /// KEEPS the sections it already assembled, because the deadline is checked
    /// between section calls and `assemble` renders whatever the fan-out
    /// produced. Stopping a max run early costs the tail of a document; stopping
    /// a quality run early costs the whole draft, which is why that one had to
    /// buy the worst case and this one does not. Cost of the choice, stated: a
    /// run whose every call takes the full 300 s stops with the sections it has.
    pub const RESUME_MAX: Self = Self {
        max_steps: 24,
        max_tool_calls: 0,
        max_tokens: 200_000,
        max_sections: DEFAULT_MAX_SECTIONS,
        max_repair_attempts: DEFAULT_MAX_REPAIR_ATTEMPTS,
        step_timeout: Duration::from_secs(360),
        run_timeout: Duration::from_secs(125 * 60),
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
// STRICTLY above the worst case, not equal to it. Equality was fine while the
// ceiling was dead; with the count enforced it means the run STOPS the instant
// the last planned call returns — for the prep flow, the moment `save_resume`
// comes back, so the user gets `MaxToolCalls` and the pre-save text as the
// proposal instead of the summary the prompt's last step writes. A ceiling has
// to leave room for the call it is counting to be USED.
const _: () = assert!(
    Budget::AGENT_PREP.max_tool_calls > PREP_WORST_CASE_TOOL_CALLS,
    "AGENT_PREP.max_tool_calls must leave slack above the 10-call prep worst case"
);
// With the count enforced (Phase 7), this relation decides WHICH stop a runaway
// tool loop gets: keeping `max_tool_calls` below `max_steps` means such a run
// ends at `MaxToolCalls`, which names the problem, instead of at `MaxSteps`,
// which does not. It is also still the precondition for the better fix
// (suppressing tools rather than stopping — see [`Budget::max_tool_calls`]), so
// inverting it would both blur the diagnosis and rule that fix out. What it no
// longer buys is the summary turns it once reserved: a hard stop spends none.
const _: () = assert!(
    Budget::AGENT_PREP.max_tool_calls < Budget::AGENT_PREP.max_steps,
    "a run that exhausts its tool calls must still have turns left to summarize"
);

/// 5 fixed tool turns (`get_quality_report`, `validate_resume`,
/// `search_candidate_evidence`, the post-fix `validate_resume` re-check,
/// `save_resume`) + 1 rationed optional call.
const IMPROVE_WORST_CASE_TOOL_CALLS: usize = 6;
/// ...plus a planning turn and a closing-summary turn.
const IMPROVE_WORST_CASE_TURNS: usize = IMPROVE_WORST_CASE_TOOL_CALLS + 2;

// The same three relations `AGENT_PREP` carries, for the same reasons, on the
// review flow: a budget shrunk past what its prompt's own sequence needs must
// fail `cargo build` rather than strand a real run at `StoppedReason::MaxSteps`
// — here, between the validation spend and the one save the whole flow exists
// to offer.
const _: () = assert!(
    Budget::AGENT_IMPROVE.max_steps > IMPROVE_WORST_CASE_TURNS,
    "AGENT_IMPROVE.max_steps must leave slack above the 8-turn improve worst case"
);
// Strict for the reason its prep sibling above states, and the review flow is
// where it bites hardest: its last planned call IS `save_resume`, so an
// exactly-equal ceiling would end the run the moment the save returns — with
// the PRE-save text as the proposal, on the one flow whose whole purpose is the
// corrected version. 8 > 6 holds with two calls of slack.
const _: () = assert!(
    Budget::AGENT_IMPROVE.max_tool_calls > IMPROVE_WORST_CASE_TOOL_CALLS,
    "AGENT_IMPROVE.max_tool_calls must leave slack above the 6-call improve worst case"
);
const _: () = assert!(
    Budget::AGENT_IMPROVE.max_tool_calls < Budget::AGENT_IMPROVE.max_steps,
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

//! The staged résumé generation + validation pipeline.
//!
//! One run is a [`Pipeline`](crate::pipeline::Pipeline) of stages over one
//! [`QualityCtx`]:
//!
//! | stage            | calls  | what it produces                              |
//! | ---------------- | ------ | ---------------------------------------------- |
//! | `analyze_job`    | 1      | [`JobAnalysis`] — what the posting asks         |
//! | `match_evidence` | 1      | [`EvidenceMap`] — what the RÉSUMÉ backs         |
//! | `strategy`       | 1      | [`ResumeStrategy`] — how to present it          |
//! | `draft`          | 1      | the résumé body, streamed for display           |
//! | `cover_letter`   | 0 or 1 | the letter body, streamed — 0 unless `includeCoverLetter` |
//! | `validate`       | 0      | the deterministic [`ContentReport`]             |
//! | `repair`         | ≤2×N   | section-scoped corrections, re-checked          |
//! | `humanize`       | ≤2     | `voice.*`-flagged lines rewritten, re-checked   |
//!
//! There used to be a second, `max`, depth here — one structured call per
//! section instead of the single streamed `draft`, plus a Warning-only
//! `llm_judge` review pass. The owner ruled it wasted tokens for no acted-on
//! value (`max` alone cost 12+ calls a run) and it was removed; every run is
//! this one pipeline now. [`types_max::ProjectOut`] and
//! [`assemble::render_project`] survive — they are also how the
//! deterministic Projects normalization (`projects.rs`, PR #990) renders an
//! entry, unrelated to depth.
//!
//! ## The rule the stage split exists to enforce
//!
//! The model decides HOW to present verified evidence, never WHAT the candidate
//! has done. Each stage re-anchors to the SOURCE résumé rather than to the
//! previous stage's output: `match_evidence` drops a quote that is not
//! literally in the source, `strategy` has its company identities re-seeded
//! from the parsed source after the model answers, and every Critical in the
//! report comes from a deterministic comparison against the source — never from
//! a model. A chain where each step trusted the last is exactly how a single
//! early fabrication becomes a confident finished document.
//!
//! ## Tauri-free (L2)
//!
//! Nothing here holds an `AppHandle`, emits an event, or touches the run store.
//! The `KvCache` and the resolved `Completer` are INJECTED by the L3 command,
//! which also owns the [`StageHooks`](crate::pipeline::StageHooks)
//! implementation and therefore every `pipeline:stage` emit. What the stages
//! need to tell that hook travels through the shared [`RunLedger`].

pub mod assemble;
pub mod cache;
pub mod projects;
pub mod prompt_blocks;
pub mod prompts;
pub mod source;
pub mod stages;
pub mod types;
pub mod types_max;

#[cfg(test)]
mod test;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::pipeline::budget::{Budget, StoppedReason};
use crate::pipeline::cache::KvCache;
use crate::pipeline::{Completer, Pipeline};
use crate::validate::content::ContentReport;

use self::cache::{StageCacheKey, StageIdentity};
use self::types::{EvidenceMap, JobAnalysis, ResumeStrategy};

/// The completers a run resolved for the stages the user explicitly overrode,
/// keyed by stage name. Built by L3 ([`Completer::for_stages`]) before the
/// first stage runs; a stage with no entry uses the run's default completer.
pub type StageCompleters = HashMap<String, Completer>;

/// Which entry of a per-stage map applies to `stage`, falling back to the
/// run's default.
///
/// A free generic function rather than a method so the rule it encodes —
/// **override wins for its own stage, default for every other, and an absent
/// map changes nothing** — is testable without an `AppHandle` (a `Completer`
/// needs one; a `String` does not). It is also the single lookup BOTH
/// [`QualityCtx::completer_for`] and [`QualityCtx::stage_cache_key`] go
/// through, so the two cannot answer differently.
pub(crate) fn pick<'m, T>(
    per_stage: Option<&'m HashMap<String, T>>,
    default: &'m T,
    stage: &str,
) -> &'m T {
    per_stage.and_then(|map| map.get(stage)).unwrap_or(default)
}

/// The letter text a downstream stage reads: `stage_letter` (the `cover_letter`
/// stage's own output) when it produced one, else `request_letter` (the
/// renderer-supplied, validate-only legacy text). A free function, not just a
/// `QualityCtx` method, so the decision is a test on two `&str`s rather than a
/// claim about a type this crate cannot construct in a test — see
/// [`QualityCtx::letter_text`], the only production caller.
pub(crate) fn effective_letter_text<'a>(stage_letter: &'a str, request_letter: &'a str) -> &'a str {
    if stage_letter.trim().is_empty() {
        request_letter
    } else {
        stage_letter
    }
}

/// The BINDING: pick the routing for `stage`, then derive that stage's cache
/// key from the routing that was picked.
///
/// Generic over the routed value, and taking `identity` as a parameter, purely
/// so the binding is reachable from a test — a `Completer` needs an
/// `AppHandle`, a [`StageIdentity`] does not.
/// [`QualityCtx::stage_cache_key`] instantiates it at `Completer` with
/// `StageIdentity::of`; `the_stage_cache_key_binding_follows_the_override`
/// instantiates it at `StageIdentity` with the identity function. Both run THIS
/// body, so "the key is derived from the completer the stage will actually
/// call" is a test rather than a claim.
///
/// **What this does NOT pin** (needs a `Completer`, so needs an `AppHandle`):
/// the two field arguments `QualityCtx::stage_cache_key` passes in —
/// `self.stage_completers` and `self.default_completer`. Swapping either for a
/// wrong value there is invisible to every test in this crate.
pub(crate) fn stage_cache_key_for<'m, T>(
    base: &StageCacheKey,
    per_stage: Option<&'m HashMap<String, T>>,
    default: &'m T,
    stage: &str,
    identity: impl Fn(&'m T) -> StageIdentity<'m>,
) -> StageCacheKey {
    base.rebound(identity(pick(per_stage, default, stage)))
}

/// Everything one run is run AGAINST — all of it resolved server-side before
/// the pipeline starts. Borrowed: a run reads these repeatedly and copying a
/// 200 KB résumé per stage would be silly.
#[derive(Debug, Clone, Copy)]
pub struct QualityInput<'a> {
    /// The candidate's own résumé — the ONLY source of factual truth.
    pub source_resume: &'a str,
    pub job_ad: &'a str,
    /// The language the document must be written in.
    pub target_language: &'a str,
    pub top_requirements: &'a [String],
    /// An already-generated cover letter to validate alongside the résumé;
    /// empty when no letter is in scope. Legacy/validate-only: the `cover_letter`
    /// stage's OWN output (`QualityCtx::letter`) takes precedence over this once
    /// it has one — see [`QualityCtx::letter_text`].
    pub cover_letter: &'a str,
    /// Whether the `cover_letter` stage should generate a letter. `false` is a
    /// complete no-op (the stage finishes instantly, zero cost) — the default
    /// for every caller that predates it, so this field is the ONLY thing that
    /// changes behavior.
    pub include_cover_letter: bool,
    /// The cross-provider reasoning-effort token, threaded to the draft's
    /// stream request and to the run deadline.
    pub effort: Option<&'a str>,
    /// The run's umbrella job id — the draft and cover_letter stages stream
    /// under it.
    pub job_id: &'a str,
}

/// The shared, stage-writable half of a run: what the L3 hook needs to see, and
/// what the command reads back as metrics.
///
/// Behind an `Arc<Mutex<…>>` because [`StageHooks::after`] receives no context —
/// it is handed a [`StageInfo`](crate::pipeline::StageInfo) and an outcome, by
/// design (an observer must not be able to reach into the run). A stage that
/// wants its summary in the `pipeline:stage` event and in the persisted trail
/// therefore leaves it here.
#[derive(Debug, Default)]
pub struct RunLedger {
    state: Mutex<LedgerState>,
}

#[derive(Debug, Default)]
struct LedgerState {
    artifacts: HashMap<&'static str, Value>,
    stopped: Option<StoppedReason>,
    calls: u32,
    cached: u32,
    repair_rounds: u32,
    reverted: bool,
}

impl RunLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one stage's content-free summary. Counts, codes and section keys
    /// only — never generated text (ADR-027); this value is emitted on
    /// `pipeline:stage` AND persisted as the event's `artifact_json`.
    pub fn record(&self, stage: &'static str, artifact: Value) {
        self.state.lock().artifacts.insert(stage, artifact);
    }

    /// The summary for `stage`, if it left one.
    pub fn artifact(&self, stage: &str) -> Option<Value> {
        self.state.lock().artifacts.get(stage).cloned()
    }

    /// Count one provider round-trip, and whether it was served from the stage
    /// cache instead.
    pub fn count_call(&self, cached: bool) {
        let mut state = self.state.lock();
        if cached {
            state.cached += 1;
        } else {
            state.calls += 1;
        }
    }

    /// Record why the run stopped early. FIRST writer wins: the earliest cause
    /// is the true one, and a later stage's own stop must not relabel a run
    /// that was already cancelled or already out of time.
    pub fn stop(&self, reason: StoppedReason) {
        let mut state = self.state.lock();
        if state.stopped.is_none() {
            state.stopped = Some(reason);
        }
    }

    pub fn stopped(&self) -> Option<StoppedReason> {
        self.state.lock().stopped
    }

    pub fn note_repair(&self, rounds: u32, reverted: bool) {
        let mut state = self.state.lock();
        state.repair_rounds = rounds;
        state.reverted = reverted;
    }

    /// The run's content-free metrics blob, for `pipeline_runs.metrics_json`.
    pub fn metrics(&self) -> Value {
        let state = self.state.lock();
        json!({
            "calls": state.calls,
            "cached": state.cached,
            "repairRounds": state.repair_rounds,
            "reverted": state.reverted,
        })
    }
}

/// The mutable context one run threads through its stages.
pub struct QualityCtx<'a> {
    pub input: QualityInput<'a>,
    /// The run's DEFAULT resolved provider — routing is backend-owned, so a
    /// stage never chooses one.
    ///
    /// Named `default_` rather than `completer` deliberately: a stage must ask
    /// [`completer_for`](Self::completer_for), and Rust cannot stop a sibling
    /// module from reaching a private field of its own ancestor, so the next
    /// best guard is a name that makes the bypass read as one.
    pub(crate) default_completer: &'a Completer,
    /// The stages the user explicitly overrode, resolved ONCE by L3 before the
    /// run started ([`Completer::for_stages`]). `None` for every caller that
    /// has no overrides to inject — including every test — which is the same
    /// thing as an empty map and is why the default path is untouched.
    stage_completers: Option<&'a StageCompleters>,
    /// `None` when the app has no `KvCache` managed (tests, an early failure at
    /// setup): every stage then simply runs.
    pub cache: Option<&'a KvCache>,
    pub budget: Budget,
    /// The whole run's wall clock. Read by the stage that fans out (`repair`)
    /// so the deadline is enforced INSIDE it, not only at the boundaries around
    /// it — see [`RunDeadline`].
    pub deadline: RunDeadline,
    pub ledger: Arc<RunLedger>,
    /// The rolling cache identity — each stage extends it with the artifact it
    /// produced, so a later stage's key depends on everything upstream.
    pub cache_key: StageCacheKey,

    pub analysis: JobAnalysis,
    pub evidence: EvidenceMap,
    pub strategy: ResumeStrategy,
    /// The résumé body. Written by `draft`, spliced by `repair`, corrected by
    /// `humanize`.
    pub draft: String,
    pub report: Option<ContentReport>,
    /// The letter `cover_letter` generated. Empty when
    /// [`QualityInput::include_cover_letter`] is false (the stage no-ops).
    /// Never read directly by a downstream stage — see [`Self::letter_text`].
    pub letter: String,
    /// The letter's own report — present only when a letter was in scope.
    pub letter_report: Option<ContentReport>,
}

impl<'a> QualityCtx<'a> {
    pub fn new(
        input: QualityInput<'a>,
        completer: &'a Completer,
        cache: Option<&'a KvCache>,
        deadline: RunDeadline,
        ledger: Arc<RunLedger>,
    ) -> Self {
        // The seed binds the cache chain to the run's own inputs. The résumé and
        // the posting go in whole: they ARE the question, and a key built from
        // an id instead would serve an analysis of a posting the user has since
        // re-scraped.
        let seed = format!(
            "{}\u{1f}{}\u{1f}{}",
            input.source_resume, input.job_ad, input.target_language
        );
        let cache_key = StageCacheKey::new(StageIdentity::of(completer), &seed);
        Self {
            input,
            default_completer: completer,
            stage_completers: None,
            cache,
            budget: Budget::RESUME_QUALITY,
            deadline,
            ledger,
            cache_key,
            analysis: JobAnalysis::default(),
            evidence: EvidenceMap::default(),
            strategy: ResumeStrategy::default(),
            draft: String::new(),
            report: None,
            letter: String::new(),
            letter_report: None,
        }
    }

    /// Inject the per-stage completers L3 resolved for this run.
    ///
    /// A builder so the existing constructor and every caller that has no
    /// overrides stay untouched.
    pub fn with_stage_completers(mut self, completers: &'a StageCompleters) -> Self {
        self.stage_completers = Some(completers);
        self
    }

    /// The completer `stage` runs on: its own override if the user set one,
    /// otherwise the run's default. **The only way a stage should reach a
    /// provider** — `ctx.default_completer` would ignore the override.
    ///
    /// Returns a `&'a Completer`, not a borrow of `self`: both fields it reads
    /// are already `'a` references, so a stage can hold the result across the
    /// `.await` it makes and still write back into `ctx` afterwards.
    pub fn completer_for(&self, stage: &str) -> &'a Completer {
        pick(self.stage_completers, self.default_completer, stage)
    }

    /// The cache key for `stage`, bound to the routing THAT stage will actually
    /// call — provider, model AND context window.
    ///
    /// Goes through the same [`pick`] as [`completer_for`](Self::completer_for),
    /// inside the shared [`stage_cache_key_for`] binding, so the routing a
    /// stage's answer is FILED under and the routing that PRODUCED it cannot
    /// disagree. A single run-wide key would let an overridden stage's artifact
    /// be served back to a run using the default model (and vice versa), which
    /// is the one failure a cache key exists to prevent.
    pub fn stage_cache_key(&self, stage: &str) -> StageCacheKey {
        stage_cache_key_for(
            &self.cache_key,
            self.stage_completers,
            self.default_completer,
            stage,
            StageIdentity::of,
        )
    }

    /// The run's deadline guard, ready to hand to
    /// [`Completer::complete_json`](crate::pipeline::Completer::complete_json) —
    /// see [`guard_deadline`]. Owned (an `Arc` clone plus a `Copy` clock), so it
    /// does not borrow the context across the call it guards.
    pub fn deadline_guard(&self) -> impl Fn() -> AppResult<()> + 'static {
        let ledger = Arc::clone(&self.ledger);
        let deadline = self.deadline;
        move || guard_deadline(&ledger, deadline)
    }

    /// The letter text every downstream reader (validate, repair, persist, the
    /// report) must use: the `cover_letter` stage's OWN output when it produced
    /// one, falling back to [`QualityInput::cover_letter`] — the
    /// renderer-supplied, validate-only legacy text — otherwise.
    ///
    /// **The one rule that keeps two callers from disagreeing about "the
    /// letter".** Before the `cover_letter` stage existed, every reader took
    /// `ctx.input.cover_letter` directly; a run that generates its own letter
    /// must not leave any of them still reading the (now stale, usually empty)
    /// request text instead. Preferring [`Self::letter`] when it is non-empty
    /// and falling back otherwise is exactly what preserves the PRE-PR-2
    /// behavior for a run where the stage skipped (`include_cover_letter:
    /// false`).
    ///
    /// Delegates to [`effective_letter_text`], a free function for the same
    /// reason [`pick`] is one: a `QualityCtx` needs a live `Completer` to
    /// construct (which needs an `AppHandle`, which this crate's tests cannot
    /// build), while the DECISION here needs only two `&str`s.
    pub fn letter_text(&self) -> &str {
        effective_letter_text(&self.letter, self.input.cover_letter)
    }

    /// How many Criticals the current report carries. `0` when nothing has been
    /// validated yet — callers must not read that as "clean" without also
    /// checking that a report exists.
    pub fn critical_count(&self) -> usize {
        self.report.as_ref().map_or(0, |report| {
            report
                .issues
                .iter()
                .filter(|issue| issue.severity == crate::validate::Severity::Critical)
                .count()
        })
    }
}

/// The quality-depth stage list, in order.
///
/// A free function rather than a `Pipeline` constant because `Pipeline` owns
/// boxed stages and the context carries a lifetime; building it per run costs
/// six allocations and keeps the stage list in one readable place.
pub fn quality_pipeline<'a>() -> Pipeline<QualityCtx<'a>> {
    Pipeline::new("resume_quality")
        .add(stages::AnalyzeJob)
        .add(stages::MatchEvidence)
        .add(stages::Strategy)
        .add(stages::Draft)
        .add(stages::CoverLetter)
        .add(stages::Validate)
        .add(stages::Repair)
        .add(stages::Humanize)
}

/// The stage names, in pipeline order — the vocabulary a `pipeline:stage`
/// event's `stage` field can carry at quality depth. Pinned by a test so the
/// renderer's timeline can key on them.
pub const QUALITY_STAGES: &[&str] = &[
    "analyze_job",
    "match_evidence",
    "strategy",
    "draft",
    "cover_letter",
    "validate",
    "repair",
    "humanize",
];

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
/// (up to `OLLAMA_COMPLETION`) that nothing would look at.
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

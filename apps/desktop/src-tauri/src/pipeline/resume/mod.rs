//! The staged résumé pipeline — quality and max depth.
//!
//! One run is a [`Pipeline`](crate::pipeline::Pipeline) of stages over one
//! [`QualityCtx`]. The two depths share every stage but the one that writes the
//! document:
//!
//! | stage            | quality | max  | what it produces                       |
//! | ---------------- | ------- | ---- | -------------------------------------- |
//! | `analyze_job`    | 1 call  | same | [`JobAnalysis`] — what the posting asks |
//! | `match_evidence` | 1 call  | same | [`EvidenceMap`] — what the RÉSUMÉ backs |
//! | `strategy`       | 1 call  | same | [`ResumeStrategy`] — how to present it  |
//! | `draft`          | 1 call  | –    | the résumé body, streamed for display   |
//! | `sections`       | –       | ≤12  | one typed answer per section            |
//! | `assemble`       | –       | 0    | those sections as the same body         |
//! | `validate`       | 0       | 0    | the deterministic [`ContentReport`]     |
//! | `repair`         | ≤2×N    | same | section-scoped corrections, re-checked  |
//!
//! The three shared stages are the SAME values with the SAME cache keys at both
//! depths, so a max run of a posting a quality run already analyzed pays for
//! neither the analysis nor the evidence again.
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
pub mod prompt_blocks;
pub mod prompts;
pub mod section_prompts;
pub mod source;
pub mod stages;
pub mod types;
pub mod types_max;

#[cfg(test)]
mod max_test;
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

use self::cache::StageCacheKey;
use self::types::{EvidenceMap, GenerationDepth, JobAnalysis, ResumeStrategy};
use self::types_max::SectionResult;

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
    /// empty when no letter is in scope.
    pub cover_letter: &'a str,
    /// The cross-provider reasoning-effort token, threaded to the draft's
    /// stream request and to the run deadline.
    pub effort: Option<&'a str>,
    /// The run's umbrella job id — the draft stage streams under it.
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
    details: HashMap<&'static str, Value>,
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

    /// Record one stage's FULL artifact, for the DB event row only.
    ///
    /// **The log boundary and the DB boundary are not the same boundary.**
    /// ADR-027 governs the LOG: a log line carries codes, ids, hashes, counts
    /// and durations, never résumé text — and [`record`](Self::record) is what
    /// feeds it, so those lines stay counts-only at every depth. The database
    /// already stores far more than this (`ai_generations.resume_text`, the
    /// quality report's evidence spans), and a max run needs its `strategy` and
    /// `match_evidence` artifacts back HOURS later, when the user clicks
    /// "regenerate this role" — the KvCache cannot answer that (its key chain
    /// is not reconstructible outside a run) and re-deriving them would mean
    /// two fresh provider calls per click.
    ///
    /// So the detail rides in the persisted row beside the counts, and nowhere
    /// else: it is not emitted on `pipeline:stage` and never reaches a `Span`.
    /// It is also CLAMPED by the store like every other artifact — an oversized
    /// one becomes unparseable by design, which the regenerate path treats as a
    /// soft miss.
    pub fn record_detail(&self, stage: &'static str, detail: Value) {
        self.state.lock().details.insert(stage, detail);
    }

    /// The full artifact for `stage`, if it left one.
    pub fn detail(&self, stage: &str) -> Option<Value> {
        self.state.lock().details.get(stage).cloned()
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

/// What a section-wise run tells the shell WHILE it is running.
///
/// Declared here (L2) and implemented only by the L3 shell, exactly like
/// [`StageHooks`](crate::pipeline::StageHooks) and for the same reason: the
/// `sections` stage has to report twelve times inside ONE stage, which the
/// per-stage hook cannot express, and it must do so without holding an
/// `AppHandle`, an event channel, or any Tauri type. A test implements this
/// with an in-memory recorder.
///
/// `index`/`total` are the SECTION's position in the fan-out, not the stage's;
/// the L3 implementor pairs them with the stage index it already tracks.
pub trait SectionProgress: Send + Sync {
    /// A section's generation is about to start.
    fn section_started(&self, key: types::SectionKey, index: usize, total: usize);

    /// It finished. `produced` is false for a section that errored, was refused
    /// by the day's ceiling, or came back with nothing to render — the three
    /// cases a timeline must show as "no changes" rather than as done.
    fn section_finished(&self, key: types::SectionKey, index: usize, total: usize, produced: bool);

    /// One finished section's rendered TEXT, for the progressive assembly the
    /// output pane shows. Display-only: the run's completion signal is still
    /// its terminal `pipeline:stage` event, never this.
    fn section_text(&self, text: &str);
}

/// The mutable context one run threads through its stages.
///
/// Shared by BOTH staged depths. Quality fills [`Self::draft`] in one streamed
/// call; max fills [`Self::sections`] one call at a time and `assemble` renders
/// them into the same `draft` field — which is what keeps `validate`, `repair`,
/// the report and every downstream reader identical across depths.
pub struct QualityCtx<'a> {
    pub input: QualityInput<'a>,
    /// The resolved provider — routing is backend-owned, so a stage never
    /// chooses one.
    pub completer: &'a Completer,
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

    /// Which depth is running. Read by the stages that behave differently at
    /// max — the artifact detail the run persists, and the judge's own
    /// admission check — rather than by a second context type: `analyze_job`,
    /// `match_evidence`, `strategy`, `validate` and `repair` are the SAME
    /// stages at both depths, and a second context would mean a second copy of
    /// each.
    pub depth: GenerationDepth,
    /// The L3 observer a section-wise run reports to. `None` for quality depth
    /// and for every test that does not care.
    pub progress: Option<&'a dyn SectionProgress>,
    /// The finished sections, in render order — max depth only. `assemble`
    /// turns these into [`Self::draft`].
    pub sections: Vec<SectionResult>,

    pub analysis: JobAnalysis,
    pub evidence: EvidenceMap,
    pub strategy: ResumeStrategy,
    /// The résumé body. Written by `draft` (quality) or `assemble` (max),
    /// spliced by `repair`.
    pub draft: String,
    pub report: Option<ContentReport>,
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
        let cache_key =
            StageCacheKey::new(completer.provider_id().as_str(), completer.model(), &seed);
        Self {
            input,
            completer,
            cache,
            budget: Budget::RESUME_QUALITY,
            deadline,
            ledger,
            cache_key,
            depth: GenerationDepth::Quality,
            progress: None,
            sections: Vec::new(),
            analysis: JobAnalysis::default(),
            evidence: EvidenceMap::default(),
            strategy: ResumeStrategy::default(),
            draft: String::new(),
            report: None,
            letter_report: None,
        }
    }

    /// Switch this context to MAX depth: the max budget, and the L3 observer
    /// the section-wise generator reports to.
    ///
    /// A builder rather than a second constructor (or a second context type)
    /// because everything else about a run is identical at both depths —
    /// including the cache seed, which is deliberately depth-blind so the three
    /// shared stages hit the SAME cache entries a quality run wrote.
    pub fn for_max(mut self, progress: Option<&'a dyn SectionProgress>) -> Self {
        self.depth = GenerationDepth::Max;
        self.budget = Budget::RESUME_MAX;
        self.progress = progress;
        self
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
        .add(stages::Validate)
        .add(stages::Repair)
}

/// The stage names, in pipeline order — the vocabulary a `pipeline:stage`
/// event's `stage` field can carry at quality depth. Pinned by a test so the
/// renderer's timeline can key on them.
pub const QUALITY_STAGES: &[&str] = &[
    "analyze_job",
    "match_evidence",
    "strategy",
    "draft",
    "validate",
    "repair",
];

/// The MAX-depth stage list, in order.
///
/// The first three stages are the SAME `Stage` values quality depth runs, with
/// the same cache keys — a max run of a posting a quality run already analyzed
/// pays for neither the analysis nor the evidence again. What replaces `draft`
/// is the pair `sections` + `assemble`: one structured call per section, then a
/// pure renderer that produces the same plain-text body `draft` would have.
pub fn max_pipeline<'a>() -> Pipeline<QualityCtx<'a>> {
    Pipeline::new("resume_max")
        .add(stages::AnalyzeJob)
        .add(stages::MatchEvidence)
        .add(stages::Strategy)
        .add(stages::Sections)
        .add(assemble::Assemble)
        .add(stages::Validate)
        .add(stages::Repair)
        .add(stages::Judge)
}

/// The max-depth stage names, in pipeline order. Pinned by a test against
/// [`max_pipeline`] for the same reason [`QUALITY_STAGES`] is: the renderer's
/// timeline keys on them, and `stageToEvent` ignores a name it does not know —
/// so a rename here is silent on the other side.
pub const MAX_STAGES: &[&str] = &[
    "analyze_job",
    "match_evidence",
    "strategy",
    "sections",
    "assemble",
    "validate",
    "repair",
    "llm_judge",
];

// A max run must have a step for every section PLUS every framing stage, or it
// dies at `StoppedReason::MaxSteps` with the document half-written. Asserted at
// COMPILE time against the stage list itself rather than a transcribed number:
// adding a stage without raising the budget then fails `cargo build`. The `- 1`
// is `sections`, which is the per-section term rather than one of the framing
// stages around it.
const _: () = assert!(
    Budget::RESUME_MAX.max_steps >= Budget::RESUME_MAX.max_sections + MAX_STAGES.len() - 1,
    "RESUME_MAX.max_steps must fit one step per section PLUS every framing stage"
);

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

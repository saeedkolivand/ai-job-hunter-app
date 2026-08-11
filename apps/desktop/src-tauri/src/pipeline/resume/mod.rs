//! The staged résumé pipeline — quality depth.
//!
//! One run is a [`Pipeline`](crate::pipeline::Pipeline) of six stages over one
//! [`QualityCtx`]:
//!
//! | stage            | calls | what it produces                                  |
//! | ---------------- | ----- | ------------------------------------------------- |
//! | `analyze_job`    | 1     | [`JobAnalysis`] — what the posting asks for       |
//! | `match_evidence` | 1     | [`EvidenceMap`] — what the RÉSUMÉ can vouch for   |
//! | `strategy`       | 1     | [`ResumeStrategy`] — how to present it            |
//! | `draft`          | 1     | the résumé body, streamed for display             |
//! | `validate`       | 0     | the deterministic [`ContentReport`]               |
//! | `repair`         | ≤2×N  | section-scoped corrections, spliced and re-checked|
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

pub mod cache;
pub mod prompt_blocks;
pub mod prompts;
pub mod stages;
pub mod types;

#[cfg(test)]
mod test;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{json, Value};

use crate::pipeline::budget::{Budget, StoppedReason};
use crate::pipeline::cache::KvCache;
use crate::pipeline::{Completer, Pipeline};
use crate::validate::content::ContentReport;

use self::cache::StageCacheKey;
use self::types::{EvidenceMap, JobAnalysis, ResumeStrategy};

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
    /// The resolved provider — routing is backend-owned, so a stage never
    /// chooses one.
    pub completer: &'a Completer,
    /// `None` when the app has no `KvCache` managed (tests, an early failure at
    /// setup): every stage then simply runs.
    pub cache: Option<&'a KvCache>,
    pub budget: Budget,
    pub ledger: Arc<RunLedger>,
    /// The rolling cache identity — each stage extends it with the artifact it
    /// produced, so a later stage's key depends on everything upstream.
    pub cache_key: StageCacheKey,

    pub analysis: JobAnalysis,
    pub evidence: EvidenceMap,
    pub strategy: ResumeStrategy,
    /// The résumé body. Written by `draft`, spliced by `repair`.
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
            ledger,
            cache_key,
            analysis: JobAnalysis::default(),
            evidence: EvidenceMap::default(),
            strategy: ResumeStrategy::default(),
            draft: String::new(),
            report: None,
            letter_report: None,
        }
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

/// Whether a run that started at `started` has used up `deadline`.
pub fn deadline_passed(started: Instant, deadline: Duration) -> bool {
    started.elapsed() >= deadline
}

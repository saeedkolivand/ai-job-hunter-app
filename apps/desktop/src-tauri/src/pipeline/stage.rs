//! Generic [`Stage`]/[`Pipeline`] orchestration: an ordered sequence of
//! modular steps sharing a context, with optional per-stage lifecycle hooks
//! (cancellation checks, timing, timeout flagging) and structured tracing.

use async_trait::async_trait;

use crate::commands::ai_provider::call_trace::CallLog;
use crate::error::{AppError, AppResult};

/// One modular step of a workflow, operating on a shared mutable context `C`.
#[async_trait]
pub trait Stage<C>: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run(&self, ctx: &mut C) -> AppResult<()>;

    /// Whether entering this stage can cost a PROVIDER call.
    ///
    /// **Default `true`, because the safe direction to be wrong in is refusing
    /// to run.** A boundary deadline check exists to stop a run before it pays
    /// for the next call — not to throw away what it has already paid for.
    /// `validate` is a free stage (deterministic checks only): once the
    /// deadline has passed, its boundary check still lets it run rather than
    /// aborting, because `validate` is what turns the prior paid stages'
    /// output into a usable document — refusing to run it would discard that
    /// work instead of conserving it. Letting a free-stage boundary through is
    /// what conserves the work already done; aborting there would not.
    ///
    /// Surfaced to the hook through [`StageInfo::costs_a_call`]; the stop
    /// decision itself is
    /// `commands::resume_pipeline::hooks::apply_stop`. Cancellation is
    /// unaffected — a user who pressed Cancel wants the run to stop, free stage
    /// or not.
    fn costs_a_provider_call(&self) -> bool {
        true
    }
}

/// An ordered sequence of [`Stage`]s sharing a context. Runs each stage in order,
/// emitting a per-stage [`StageTrace`]; the first error aborts the pipeline.
pub struct Pipeline<C> {
    name: &'static str,
    stages: Vec<Box<dyn Stage<C>>>,
}

impl<C> Pipeline<C> {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            stages: Vec::new(),
        }
    }

    // Fluent builder verb (`Pipeline::new(..).add(stage).add(stage)`), not
    // arithmetic. Surfaced by clippy only once this became a library crate (a
    // bin crate has no public API for the lint to inspect). See `benches/`.
    #[allow(clippy::should_implement_trait)]
    pub fn add<S: Stage<C> + 'static>(mut self, stage: S) -> Self {
        self.stages.push(Box::new(stage));
        self
    }

    /// The stage names, in order.
    ///
    /// Exists for the `QUALITY_STAGES` pin: that constant is what the
    /// renderer's timeline keys on, and a pin that only compares a constant
    /// against a literal proves the literal, not the pipeline. With this,
    /// renaming or reordering a stage fails the pin.
    pub fn stage_names(&self) -> Vec<&'static str> {
        self.stages.iter().map(|stage| stage.name()).collect()
    }

    /// The stages that make no provider call, in order — the ones a boundary
    /// deadline check may let through.
    ///
    /// Read off the pipeline that actually runs, for the same reason
    /// [`stage_names`](Self::stage_names) is: a guard comparing one list of
    /// names against another list of names proves nothing about the stages.
    pub fn free_stage_names(&self) -> Vec<&'static str> {
        self.stages
            .iter()
            .filter(|stage| !stage.costs_a_provider_call())
            .map(|stage| stage.name())
            .collect()
    }

    pub async fn run(&self, ctx: &mut C) -> AppResult<()> {
        for stage in &self.stages {
            let trace = StageTrace::begin(self.name, stage.name());
            match stage.run(ctx).await {
                Ok(()) => trace.end(true),
                Err(e) => {
                    trace.end(false);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// [`run`](Self::run) with per-stage observation hooks.
    ///
    /// A deliberate second entry point rather than an `Option<&dyn StageHooks>`
    /// parameter on `run`: every existing caller stays on the untouched fast
    /// path with no per-stage branch, and a hook-less pipeline keeps its exact
    /// current cost.
    ///
    /// `before` runs BEFORE the stage and may abort the run by returning `Err` —
    /// that is where a hooked run checks cancellation, so a cancelled run stops
    /// at a stage boundary instead of paying for the next provider call. `after`
    /// runs for BOTH outcomes (it is the stage's `finish` event) before the
    /// error propagates, so a failed stage is never silently un-reported.
    pub async fn run_hooked(&self, ctx: &mut C, hooks: &dyn StageHooks) -> AppResult<()> {
        let total = self.stages.len();
        for (index, stage) in self.stages.iter().enumerate() {
            let info = StageInfo {
                pipeline: self.name,
                stage: stage.name(),
                index,
                total,
                costs_a_call: stage.costs_a_provider_call(),
            };
            hooks.before(&info).await?;
            let started = std::time::Instant::now();
            let trace = StageTrace::begin(self.name, stage.name());
            let result = match hooks.call_log() {
                Some(log) => log.scope(stage.run(ctx)).await,
                None => stage.run(ctx).await,
            };
            trace.end(result.is_ok());
            let outcome = StageOutcome {
                ok: result.is_ok(),
                ms: started.elapsed().as_millis() as u64,
                // A per-call HTTP deadline expiring INSIDE the stage body —
                // distinct from the stage-BOUNDARY run deadline `before()`
                // guards (that one never reaches `stage.run` at all). Computed
                // here, not by the hook, because only the raw `AppResult` — not
                // the `ok`/`ms` pair a hook receives — carries which `AppError`
                // variant fired.
                timed_out: matches!(&result, Err(AppError::Timeout(_))),
            };
            hooks.after(&info, outcome).await;
            result?;
        }
        Ok(())
    }
}

/// Which stage is about to run / just ran. Plain data — no Tauri types, no run
/// or job identity: this layer (L2) does not know either. The L3 implementor
/// that turns these into `pipeline:stage` events owns the `runId`/`jobId` it
/// already holds and pairs them with this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageInfo {
    /// The pipeline's name, as given to [`Pipeline::new`].
    pub pipeline: &'static str,
    /// This stage's [`Stage::name`].
    pub stage: &'static str,
    /// 0-based position in the pipeline.
    pub index: usize,
    /// How many stages the pipeline has in total.
    pub total: usize,
    /// [`Stage::costs_a_provider_call`] for this stage — what lets a hook's
    /// deadline check tell "do not pay for the next call" apart from "do not
    /// render what has already been paid for".
    pub costs_a_call: bool,
}

/// How a stage finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageOutcome {
    pub ok: bool,
    /// Wall-clock duration of the stage body.
    pub ms: u64,
    /// The stage's own `AppError` was [`crate::error::AppError::Timeout`] — a
    /// per-call HTTP deadline expiring, never a stage-boundary run-deadline
    /// stop (which never runs the stage at all) or any other failure kind.
    /// `false` whenever `ok` is `true`. What lets a [`StageHooks::after`]
    /// implementor (e.g. the résumé pipeline's `RunHooks`) record
    /// [`crate::pipeline::budget::StoppedReason::Timeout`] without being
    /// handed the error itself.
    pub timed_out: bool,
}

/// Per-stage lifecycle callbacks for [`Pipeline::run_hooked`].
///
/// Declared here (L2) but IMPLEMENTED only by the shell (L3), which is why the
/// signature carries no `AppHandle`, no event channel, and no Tauri type — a
/// hook implementor is free to emit an event, persist a run row, or do nothing,
/// and this layer stays testable with a plain in-memory recorder.
#[async_trait]
pub trait StageHooks: Send + Sync {
    /// Before the stage body. `Err` aborts the run WITHOUT running the stage —
    /// the cancellation check lives here.
    async fn before(&self, stage: &StageInfo) -> AppResult<()>;

    /// After the stage body, for success and failure alike. Returns nothing: an
    /// observer must not be able to turn a successful stage into a failed run.
    async fn after(&self, stage: &StageInfo, outcome: StageOutcome);

    /// The collector for the provider calls each stage body makes, scoped around
    /// the body by [`Pipeline::run_hooked`]. `None` (the default) collects
    /// nothing; the implementor drains it in [`after`](Self::after).
    fn call_log(&self) -> Option<CallLog> {
        None
    }
}

// ── Stage tracing ───────────────────────────────────────────────────────────────

/// Structured per-stage log over the shared [`crate::observability::Span`]:
/// `[pipeline:cover_letter] → stage=research` / `← stage=research duration=..ms ok=true`.
struct StageTrace {
    span: crate::observability::Span,
}

impl StageTrace {
    fn begin(pipeline: &'static str, stage: &'static str) -> Self {
        Self {
            span: crate::observability::Span::begin(
                format!("pipeline:{pipeline}"),
                format!("stage={stage}"),
            ),
        }
    }

    fn end(&self, ok: bool) {
        self.span.end(ok);
    }
}

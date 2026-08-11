//! The L3 half of a staged run: the ONE place a `pipeline:stage` event is
//! emitted, a `pipeline_run_events` row is written, or a run is stopped.
//!
//! `pipeline::Pipeline::run_hooked` calls `before`/`after` around every stage
//! and knows nothing about Tauri, the run store, or cancellation — which is
//! what lets the stages stay L2 and testable with a plain in-memory recorder.
//! Everything shell-shaped lives here.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, PIPELINE_STAGE};
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::{RunDeadline, RunLedger};
use crate::pipeline::runs::{PipelineRunStore, RunEventRow};
use crate::pipeline::{StageHooks, StageInfo, StageOutcome};

/// Payload of the `pipeline:stage` event — the hand-synced Rust counterpart of
/// `PipelineStageEvent` in `packages/shared/src/events/pipeline.ts`.
///
/// Content-free by construction (ADR-027): stage names, counts and durations
/// only. `sectionKey` is the one model-derived field, which is why the
/// contract's vocabulary for it is closed and why the emitter runs the
/// GENERATED `is_pipeline_section_key` guard before letting one on the wire.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStageEvent {
    pub run_id: String,
    pub job_id: String,
    pub stage: String,
    pub phase: &'static str,
    pub index: usize,
    pub total: usize,
    pub attempt: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub critical_count: Option<u32>,
}

/// The three legal `phase` values. Written as literals rather than read from
/// the generated `PIPELINE_STAGE_PHASES` slice because they are used as
/// `&'static str` in three fixed positions; the generated slice is what the
/// lock test compares them against.
const PHASE_START: &str = "start";
const PHASE_FINISH: &str = "finish";
const PHASE_ERROR: &str = "error";

/// The three literals above, for the lock test that compares them against the
/// GENERATED `PIPELINE_STAGE_PHASES`. Exposed rather than re-typed in the test,
/// which would only pin the test's own copy.
#[cfg(test)]
pub(crate) fn emitted_phases() -> [&'static str; 3] {
    [PHASE_START, PHASE_FINISH, PHASE_ERROR]
}

/// The run's STOP DECISION — the whole of it: which reason applies, recording
/// it, and the error that aborts the run.
///
/// Split out of [`StageHooks::before`] with no `AppHandle` for the same reason
/// `Completer::from_config` and `complete_json_with` are: this crate has no
/// Tauri test harness, and a decision only provable by reading the code is not
/// a guard. `before` is then the emit half plus one call to this.
///
/// **Cancellation outranks the deadline.** A user who pressed Cancel on a run
/// that also happens to be out of time asked for a cancel; answering "it timed
/// out" describes the same event less usefully, and the command maps the two to
/// different terminal job states.
pub(crate) fn apply_stop(
    ledger: &RunLedger,
    cancelled: bool,
    elapsed: Duration,
    deadline: Duration,
) -> AppResult<()> {
    if cancelled {
        ledger.stop(StoppedReason::Cancelled);
        return Err(AppError::Cancelled);
    }
    if elapsed >= deadline {
        ledger.stop(StoppedReason::RunTimeout);
        return Err(AppError::Message(format!(
            "This generation ran past its {}-minute limit and was stopped. \
             Try a lower reasoning effort, or a faster model.",
            deadline.as_secs() / 60
        )));
    }
    Ok(())
}

/// The run's TERMINAL STATE — status and stopped reason, derived together.
///
/// Together, because deriving them separately is what let a cancelled run
/// report `status=failed stoppedReason=done`. Two things go wrong on that path
/// and both are fixed here:
///
/// * **A cancelled stream does not reach the ledger.** `chat_stream` reports an
///   aborted stream as `AppError::Message("Job cancelled")`, not
///   [`AppError::Cancelled`] — and the draft's error propagates out of
///   `run_hooked` immediately, so no later `before()` runs [`apply_stop`] and
///   the ledger records nothing. The cancel TOKEN is the authority the ledger
///   missed, so it is consulted here (on the error path only: a run that
///   completed before the cancel landed completed).
/// * **A failed run must never report `done`.** `StoppedReason::Done` is "the
///   last stage completed"; writing it as a fallback for `stopped == None`
///   labelled every failure as a clean finish, and the renderer maps `done` to
///   a SUCCESS suffix. A failure with no recorded stop reason now carries
///   `None` — absent, which the wire contract already allows.
///
/// Returns the wire token rather than the enum so the caller writes one
/// `Option<String>` straight into the row.
pub(crate) fn terminal_state(
    ledger: &RunLedger,
    ok: bool,
    token_cancelled: bool,
    needs_review: bool,
) -> (&'static str, Option<String>) {
    if !ok && token_cancelled {
        // First-writer-wins, so a run that had already recorded WHY it was
        // stopping keeps its own reason.
        ledger.stop(StoppedReason::Cancelled);
    }
    let stopped = ledger.stopped();
    let status = match (ok, stopped == Some(StoppedReason::Cancelled)) {
        (_, true) => super::STATUS_CANCELLED,
        (false, _) => super::STATUS_FAILED,
        (true, _) if needs_review => super::STATUS_NEEDS_REVIEW,
        (true, _) => super::STATUS_COMPLETED,
    };
    let reason = stopped.or_else(|| ok.then_some(StoppedReason::Done));
    (status, reason.map(stopped_wire))
}

/// One [`StoppedReason`]'s wire token — the `snake_case` serde rename the
/// renderer's suffix map keys on.
fn stopped_wire(reason: StoppedReason) -> String {
    serde_json::to_value(reason)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        // Unreachable: every variant is a unit variant with a `snake_case`
        // rename. The fallback names the enum's own default rather than
        // inventing a token no consumer knows.
        .unwrap_or_else(|| "done".to_string())
}

/// Emits, persists, and stops. One instance per run.
pub struct RunHooks {
    app: AppHandle,
    run_id: String,
    job_id: String,
    cancel: CancellationToken,
    deadline: RunDeadline,
    ledger: Arc<RunLedger>,
    /// Monotonic `pipeline_run_events.seq`. Atomic because `StageHooks` takes
    /// `&self` — the trait is deliberately shaped so an observer cannot mutate
    /// the run.
    seq: AtomicU32,
}

impl RunHooks {
    /// `deadline` is the run's shared clock — the SAME value the `repair` stage
    /// holds through `QualityCtx`, so the boundary check here and the in-loop
    /// check there measure one run, not two `Instant::now()`s a few
    /// milliseconds apart.
    pub fn new(
        app: AppHandle,
        run_id: String,
        job_id: String,
        cancel: CancellationToken,
        deadline: RunDeadline,
        ledger: Arc<RunLedger>,
    ) -> Self {
        Self {
            app,
            run_id,
            job_id,
            cancel,
            deadline,
            ledger,
            seq: AtomicU32::new(0),
        }
    }

    /// Wall clock consumed so far — read by the command to report the run's
    /// duration without keeping a second `Instant`.
    pub fn elapsed_ms(&self) -> u64 {
        self.deadline.elapsed().as_millis() as u64
    }

    /// Emit ONE `pipeline:stage` event and append its durable twin.
    ///
    /// The two always happen together, and in this order: the event is what the
    /// UI is waiting on, and a store write that fails must not cost the user
    /// their progress bar. The row is best-effort for the same reason the run
    /// trail is a debugging aid — logged by the store itself on failure.
    fn report(
        &self,
        info: &StageInfo,
        phase: &'static str,
        ms: Option<u64>,
        artifact: Option<Value>,
    ) {
        let (issue_count, critical_count) = artifact
            .as_ref()
            .map(|value| {
                (
                    value
                        .get("issues")
                        .and_then(Value::as_u64)
                        .map(|n| n as u32),
                    value
                        .get("criticals")
                        .and_then(Value::as_u64)
                        .map(|n| n as u32),
                )
            })
            .unwrap_or((None, None));

        emit_event(
            &self.app,
            PIPELINE_STAGE,
            PipelineStageEvent {
                run_id: self.run_id.clone(),
                job_id: self.job_id.clone(),
                stage: info.stage.to_string(),
                phase,
                index: info.index,
                total: info.total,
                // Quality depth runs each stage once; the repair loop's own
                // rounds are reported inside the `repair` stage's artifact
                // rather than as repeated stage events. Phase 4's section-wise
                // generator is what makes per-attempt events real.
                attempt: 1,
                // No stage at this depth is section-scoped, so nothing here can
                // put a model-derived value on the wire. The field stays in the
                // payload because the contract is frozen and Phase 4 fills it —
                // and when it does, it must go through
                // `ipc_contracts::events::is_pipeline_section_key` first.
                section_key: None,
                ms,
                issue_count,
                critical_count,
            },
        );

        if let Some(store) = self.app.try_state::<PipelineRunStore>() {
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            let _ = store.append_event(&RunEventRow {
                run_id: self.run_id.clone(),
                seq,
                ts: crate::db::now_ms(),
                stage: info.stage.to_string(),
                phase: phase.to_string(),
                artifact_json: artifact
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "{}".to_string()),
            });
        }
    }
}

#[async_trait]
impl StageHooks for RunHooks {
    /// The run's two stop conditions, checked at a STAGE BOUNDARY — the one
    /// place stopping is free (nothing is in flight, and the next provider call
    /// has not been paid for). The decision itself is [`apply_stop`]; this is
    /// the emit half.
    async fn before(&self, stage: &StageInfo) -> AppResult<()> {
        apply_stop(
            &self.ledger,
            self.cancel.is_cancelled(),
            self.deadline.elapsed(),
            self.deadline.limit(),
        )?;
        self.report(stage, PHASE_START, None, None);
        Ok(())
    }

    async fn after(&self, stage: &StageInfo, outcome: StageOutcome) {
        let phase = if outcome.ok {
            PHASE_FINISH
        } else {
            PHASE_ERROR
        };
        self.report(
            stage,
            phase,
            Some(outcome.ms),
            self.ledger.artifact(stage.stage),
        );
    }
}

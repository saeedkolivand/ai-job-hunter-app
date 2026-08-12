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
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AiStreamChunk, AI_STREAM, PIPELINE_STAGE};
use crate::ipc_contracts::events::is_pipeline_section_key;
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::resume::{RunDeadline, RunLedger, SectionProgress};
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
/// different terminal job states. It also outranks `costs_a_call` below: a
/// cancel stops every stage, free or not.
///
/// **An expired deadline stops the next PAID stage, not the next stage.** The
/// boundary check exists because a stage boundary is where stopping is free —
/// nothing is in flight and the next provider call has not been paid for. That
/// reasoning says nothing about a stage that makes no call, and reading it as
/// "stop at the next boundary" cost a max run everything it had: `sections`
/// breaks out of its fan-out on the same clock, returns its finished sections
/// `Ok`, and the very next boundary (`assemble`, which is pure, followed by
/// `validate`, which is deterministic) aborted the run — empty draft, no
/// report, nothing persisted, `status=failed`, up to eleven paid answers
/// discarded. It also contradicted the reasoning that sized
/// `Budget::RESUME_MAX`'s deadline at the reachable 7 200 s rather than the
/// worst case: that a section-wise run KEEPS what it assembled when the clock
/// stops.
///
/// The reason is still recorded on the free path, so the run reports
/// `run_timeout` and [`terminal_state`] resolves it to `needsReview`/
/// `completed` exactly as the in-loop check already does — the run is stopped,
/// and it is also honest about what it produced. `repair` and the judge still
/// cost calls, so they are still refused.
/// Takes the whole [`StageInfo`], not a `costs_a_call` boolean, so the ONE link
/// in this chain that a test cannot reach is not a link at all: with a separate
/// argument, `before` could pass a literal `true` and every guard in the suite
/// stayed green — the stage's own answer never had to arrive. Handing over the
/// value the pipeline built removes the place where the mis-wiring would live.
pub(crate) fn apply_stop(
    ledger: &RunLedger,
    cancelled: bool,
    elapsed: Duration,
    deadline: Duration,
    stage: &StageInfo,
) -> AppResult<()> {
    if cancelled {
        ledger.stop(StoppedReason::Cancelled);
        return Err(AppError::Cancelled);
    }
    if elapsed >= deadline {
        ledger.stop(StoppedReason::RunTimeout);
        if stage.costs_a_call {
            // The same text `pipeline::resume::guard_deadline` reports from
            // inside a stage: which enforcement point saw the clock first is an
            // implementation detail, not something to tell the user two ways.
            return Err(crate::pipeline::resume::run_timeout_error(deadline));
        }
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
/// * **A timeout that still SAVED a document is not a failure.** The deadline
///   has two enforcement points and they used to disagree about the same event:
///   expiring INSIDE the repair loop ends the loop, keeps the accumulated
///   document and returns `Ok`, so the run lands `needsReview` + `run_timeout`;
///   expiring at the stage BOUNDARY one instant later ([`apply_stop`]) returns
///   `Err`, which came out `failed` — even though `persist_document` had already
///   written the same real, reviewable résumé. `persisted` is that document
///   (`quality_report.is_some()`, i.e. a non-empty draft, a report, and a
///   successful save), and it makes the two paths agree. Only `RunTimeout`
///   qualifies: a provider error or a JSON stage that never produced its
///   artifact leaves a document nothing downstream can honestly describe.
/// * **"The user acted" is what the TOKEN says, not what the ledger says.**
///   `RunLedger::stop` is first-writer-wins, so a run that had already recorded
///   `run_timeout`/`budgeted` keeps that reason when the cancel lands — and
///   reading the STATUS off the reason alone then reported `failed` for a run
///   the user cancelled, while the doc here argued the opposite. The reason is
///   still first-writer-wins (it answers "what stopped it", and the timeout DID
///   come first); the status is the token's, which is also what routes the job
///   event to `job_cancel` instead of `job_fail` in `execute`.
///
/// Returns the wire token rather than the enum so the caller writes one
/// `Option<String>` straight into the row.
pub(crate) fn terminal_state(
    ledger: &RunLedger,
    ok: bool,
    token_cancelled: bool,
    needs_review: bool,
    persisted: bool,
) -> (&'static str, Option<String>) {
    if !ok && token_cancelled {
        // First-writer-wins, so a run that had already recorded WHY it was
        // stopping keeps its own reason.
        ledger.stop(StoppedReason::Cancelled);
    }
    let stopped = ledger.stopped();
    // The run produced a usable document and stopped for the one reason that
    // does not invalidate it — the same outcome the in-loop deadline check
    // produces, reached from the boundary check.
    let timed_out_with_document = !ok && persisted && stopped == Some(StoppedReason::RunTimeout);
    let usable = ok || timed_out_with_document;
    // The token, not just the ledger's (first-writer-wins) reason — see the doc
    // above. Cancellation outranks every other outcome, exactly as `apply_stop`
    // already ranks it against the deadline.
    let cancelled = stopped == Some(StoppedReason::Cancelled) || (!ok && token_cancelled);
    let status = match (usable, cancelled) {
        (_, true) => super::STATUS_CANCELLED,
        (false, _) => super::STATUS_FAILED,
        (true, _) if needs_review => super::STATUS_NEEDS_REVIEW,
        (true, _) => super::STATUS_COMPLETED,
    };
    // Still the ledger's own reason (`run_timeout`) — the status changed, the
    // explanation did not. `Done` remains reserved for a run that actually
    // reached the end of its last stage.
    let reason = stopped.or_else(|| ok.then_some(StoppedReason::Done));
    (status, reason.map(stopped_wire))
}

/// The key a stage's FULL artifact rides under, inside the counts-only one.
///
/// Nested rather than replacing it, so the persisted row keeps the summary the
/// runs panel reads AND the payload a max-depth regenerate needs — and so a
/// reader that does not know about the detail sees exactly what it saw before.
pub(crate) const DETAIL_KEY: &str = "full";

/// Fold a stage's full artifact into its counts-only one, for the DB row.
///
/// The EVENT payload is built from the counts-only value upstream of this
/// (`issue_count`/`critical_count`), so the detail never reaches the wire — and
/// a stage with no detail (every stage at quality depth) produces a row
/// byte-identical to the one it produced before this existed.
///
/// **The detail-less case returns the ARTIFACT, not `None`.** Written the other
/// way round once (`let detail = detail?;`) it deleted the counts of every stage
/// that had no detail — which is all six at quality depth and five of eight at
/// max — so the persisted row became `"{}"` and `issueCount`/`criticalCount`
/// came out null on every `pipeline:stage` event the app has ever emitted at
/// these depths. Pinned by
/// `a_stage_without_a_detail_writes_exactly_the_artifact_it_always_did`.
pub(crate) fn with_detail(artifact: Option<Value>, detail: Option<Value>) -> Option<Value> {
    let Some(detail) = detail else {
        return artifact;
    };
    let mut artifact = artifact.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    match artifact.as_object_mut() {
        Some(object) => {
            object.insert(DETAIL_KEY.to_string(), detail);
            Some(artifact)
        }
        // A non-object artifact has nowhere to nest into. Keeping the counts is
        // the safer half: the detail is an optimization, the summary is the
        // trail. Unreachable today — every `record` call passes an object.
        None => Some(artifact),
    }
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

/// Which stage is currently running, as `before()` last saw it.
///
/// The section-wise generator reports up to twelve times from INSIDE one stage,
/// and `SectionProgress` (L2) deliberately knows nothing about stage indices —
/// so the position a per-section event carries is the one this observer already
/// tracks. `None` before the first stage.
#[derive(Debug, Clone, Copy, Default)]
struct CurrentStage {
    stage: Option<&'static str>,
    index: usize,
    total: usize,
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
    /// The stage a per-section event belongs to. Behind a lock for the same
    /// reason `seq` is atomic: both hook traits take `&self`.
    current: parking_lot::Mutex<CurrentStage>,
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
            current: parking_lot::Mutex::new(CurrentStage::default()),
        }
    }

    /// Wall clock consumed so far — read by the command to report the run's
    /// duration without keeping a second `Instant`.
    pub fn elapsed_ms(&self) -> u64 {
        self.deadline.elapsed().as_millis() as u64
    }

    /// Emit ONE per-SECTION `pipeline:stage` event and append its durable twin.
    ///
    /// The stage NAME stays the fan-out's own (`sections`) and the index/total
    /// stay the STAGE's position in the run: a timeline that re-scaled its
    /// progress bar to the section count mid-run would jump backwards. What
    /// makes these events per-section is `sectionKey`, which is also the ONE
    /// model-derived value on this wire — so it goes through the GENERATED
    /// [`is_pipeline_section_key`] grammar first, and an unrepresentable key is
    /// dropped from the payload rather than shipped.
    fn report_section(&self, key: SectionKey, phase: &'static str, artifact: Value) {
        let current = *self.current.lock();
        let Some(stage) = current.stage else {
            return; // no stage is running: nothing to attribute this to
        };
        let wire = key.to_wire();
        let section_key = is_pipeline_section_key(&wire).then_some(wire);
        debug_assert!(
            section_key.is_some(),
            "SectionKey::to_wire must round-trip through the generated grammar"
        );
        emit_event(
            &self.app,
            PIPELINE_STAGE,
            PipelineStageEvent {
                run_id: self.run_id.clone(),
                job_id: self.job_id.clone(),
                stage: stage.to_string(),
                phase,
                index: current.index,
                total: current.total,
                attempt: 1,
                section_key,
                ms: None,
                issue_count: None,
                critical_count: None,
            },
        );
        self.append(stage, phase, Some(artifact));
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
                // Every STAGE runs once, at both depths: the repair loop's
                // rounds are reported inside the `repair` artifact, and the
                // section fan-out's per-section events come through
                // [`Self::report_section`] — which hardcodes `1` as well. No
                // emitter threads a real attempt number today; the field is in
                // the payload because the contract is frozen.
                attempt: 1,
                // A per-STAGE event is never section-scoped — the fan-out's
                // per-section events are [`Self::report_section`]'s, and that is
                // where the model-derived key goes through the generated
                // `is_pipeline_section_key` grammar before reaching the wire.
                section_key: None,
                ms,
                issue_count,
                critical_count,
            },
        );

        self.append(info.stage, phase, artifact);
    }

    /// Append ONE durable `pipeline_run_events` row. Best-effort: the run trail
    /// is a debugging aid, and the store logs its own write failures.
    fn append(&self, stage: &str, phase: &'static str, artifact: Option<Value>) {
        let Some(store) = self.app.try_state::<PipelineRunStore>() else {
            return;
        };
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let _ = store.append_event(&RunEventRow {
            run_id: self.run_id.clone(),
            seq,
            ts: crate::db::now_ms(),
            stage: stage.to_string(),
            phase: phase.to_string(),
            artifact_json: artifact
                .map(|value| value.to_string())
                .unwrap_or_else(|| "{}".to_string()),
        });
    }
}

/// The section-wise half of the shell: per-section events, and the progressive
/// document stream.
///
/// Implemented HERE rather than anywhere in `pipeline::resume` for the same
/// reason [`StageHooks`] is: this is the only layer that may hold an
/// `AppHandle` or name an event channel.
impl SectionProgress for RunHooks {
    fn section_started(&self, key: SectionKey, index: usize, total: usize) {
        self.report_section(
            key,
            PHASE_START,
            json!({ "section": index, "sections": total }),
        );
    }

    fn section_finished(&self, key: SectionKey, index: usize, total: usize, produced: bool) {
        // `produced=false` is a FINISH, not an error: the section was attempted
        // and the document simply has nothing new for it. An `error` phase
        // would tell the timeline the run is breaking when it is degrading —
        // and the run's own failure is reported by the stage, not by a section.
        self.report_section(
            key,
            PHASE_FINISH,
            json!({ "section": index, "sections": total, "produced": produced }),
        );
    }

    fn section_text(&self, text: &str) {
        // The SAME `ai:stream` channel the quality draft streams on, under the
        // run's umbrella job id — so the output pane builds progressively with
        // no second event channel to subscribe to.
        //
        // `done: false`, always. The stream is DISPLAY-ONLY at both depths: the
        // run's completion signal is its terminal `pipeline:stage` event, and a
        // `done` here would resolve `awaitAiStream` while validate and repair
        // are still ahead — presenting an unchecked document as the final one.
        emit_event(
            &self.app,
            AI_STREAM,
            AiStreamChunk {
                job_id: self.job_id.clone(),
                delta: text.to_string(),
                done: false,
                error: None,
                thinking: None,
            },
        );
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
            stage,
        )?;
        // Recorded BEFORE the stage body runs, because the section-wise
        // generator reports from inside it.
        *self.current.lock() = CurrentStage {
            stage: Some(stage.stage),
            index: stage.index,
            total: stage.total,
        };
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
            with_detail(
                self.ledger.artifact(stage.stage),
                self.ledger.detail(stage.stage),
            ),
        );
    }
}

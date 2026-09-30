//! `Pipeline`/`Stage` orchestration: ordering, abort-on-first-error,
//! `run_hooked`'s before/after brackets, the free-stage flag, and
//! `StageOutcome::timed_out`.

use async_trait::async_trait;
use parking_lot::Mutex;

use crate::error::{AppError, AppResult};
use crate::pipeline::{Pipeline, Stage, StageHooks, StageInfo, StageOutcome};

// ── Pipeline ordering / abort ───────────────────────────────────────────────────

struct Ctx {
    log: Vec<&'static str>,
}

struct Step(&'static str);
#[async_trait]
impl Stage<Ctx> for Step {
    fn name(&self) -> &'static str {
        self.0
    }
    async fn run(&self, ctx: &mut Ctx) -> AppResult<()> {
        ctx.log.push(self.0);
        Ok(())
    }
}

struct Boom;
#[async_trait]
impl Stage<Ctx> for Boom {
    fn name(&self) -> &'static str {
        "boom"
    }
    async fn run(&self, ctx: &mut Ctx) -> AppResult<()> {
        ctx.log.push("boom");
        Err(AppError::Message("boom".to_string()))
    }
}

#[test]
fn pipeline_runs_stages_in_order() {
    tauri::async_runtime::block_on(async {
        let mut ctx = Ctx { log: Vec::new() };
        Pipeline::new("t")
            .add(Step("a"))
            .add(Step("b"))
            .add(Step("c"))
            .run(&mut ctx)
            .await
            .unwrap();
        assert_eq!(ctx.log, vec!["a", "b", "c"]);
    });
}

#[test]
fn pipeline_aborts_on_first_error() {
    tauri::async_runtime::block_on(async {
        let mut ctx = Ctx { log: Vec::new() };
        let res = Pipeline::new("t")
            .add(Step("a"))
            .add(Boom)
            .add(Step("c"))
            .run(&mut ctx)
            .await;
        assert!(res.is_err());
        // "c" must not run after the failing stage.
        assert_eq!(ctx.log, vec!["a", "boom"]);
    });
}

// ── Pipeline::run_hooked ─────────────────────────────────────────────────────────

/// An in-memory `StageHooks`: records every callback, and can abort at a chosen
/// stage index (standing in for the Phase-3 cancellation check in `before`).
#[derive(Default)]
struct Recorder {
    calls: Mutex<Vec<String>>,
    abort_at: Option<usize>,
}

impl Recorder {
    fn aborting_at(index: usize) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            abort_at: Some(index),
        }
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().clone()
    }
}

#[async_trait]
impl StageHooks for Recorder {
    async fn before(&self, stage: &StageInfo) -> AppResult<()> {
        self.calls.lock().push(format!(
            "before:{}:{}/{}",
            stage.stage, stage.index, stage.total
        ));
        if self.abort_at == Some(stage.index) {
            return Err(AppError::Message("cancelled".to_string()));
        }
        Ok(())
    }
    async fn after(&self, stage: &StageInfo, outcome: StageOutcome) {
        self.calls
            .lock()
            .push(format!("after:{}:ok={}", stage.stage, outcome.ok));
    }
}

#[test]
fn run_hooked_brackets_every_stage_with_position_info() {
    tauri::async_runtime::block_on(async {
        let mut ctx = Ctx { log: Vec::new() };
        let hooks = Recorder::default();
        Pipeline::new("t")
            .add(Step("a"))
            .add(Step("b"))
            .run_hooked(&mut ctx, &hooks)
            .await
            .unwrap();

        assert_eq!(ctx.log, vec!["a", "b"]);
        assert_eq!(
            hooks.calls(),
            vec![
                "before:a:0/2",
                "after:a:ok=true",
                "before:b:1/2",
                "after:b:ok=true",
            ]
        );
    });
}

/// `before` returning `Err` aborts WITHOUT running the stage — this is the
/// cancellation seam, so the run must not pay for the next provider call.
#[test]
fn a_before_hook_error_aborts_without_running_the_stage() {
    tauri::async_runtime::block_on(async {
        let mut ctx = Ctx { log: Vec::new() };
        let hooks = Recorder::aborting_at(1);
        let res = Pipeline::new("t")
            .add(Step("a"))
            .add(Step("b"))
            .add(Step("c"))
            .run_hooked(&mut ctx, &hooks)
            .await;

        assert!(res.is_err());
        assert_eq!(ctx.log, vec!["a"], "the aborted stage must not have run");
        assert_eq!(
            hooks.calls(),
            vec!["before:a:0/3", "after:a:ok=true", "before:b:1/3"],
            "no `after` is reported for a stage that never ran"
        );
    });
}

/// A failing stage still reports `after` (with `ok=false`) BEFORE the error
/// propagates — an observer must never miss the stage that broke the run.
#[test]
fn a_failing_stage_reports_after_then_propagates() {
    tauri::async_runtime::block_on(async {
        let mut ctx = Ctx { log: Vec::new() };
        let hooks = Recorder::default();
        let res = Pipeline::new("t")
            .add(Boom)
            .add(Step("c"))
            .run_hooked(&mut ctx, &hooks)
            .await;

        assert!(res.is_err());
        assert_eq!(ctx.log, vec!["boom"], "the pipeline stops at the failure");
        assert_eq!(
            hooks.calls(),
            vec!["before:boom:0/2", "after:boom:ok=false"]
        );
    });
}

/// Hooking must not change WHAT the pipeline does — only what it reports. Runs
/// the same stage list both ways and compares the resulting context, for the
/// success and the failure path alike.
#[test]
fn run_hooked_is_behaviorally_identical_to_run() {
    tauri::async_runtime::block_on(async {
        for expect_ok in [true, false] {
            let build = || {
                let p = Pipeline::new("t").add(Step("a")).add(Step("b"));
                if expect_ok {
                    p.add(Step("c"))
                } else {
                    p.add(Boom).add(Step("c"))
                }
            };

            let mut plain = Ctx { log: Vec::new() };
            let plain_result = build().run(&mut plain).await;

            let mut hooked = Ctx { log: Vec::new() };
            let hooked_result = build().run_hooked(&mut hooked, &Recorder::default()).await;

            assert_eq!(plain.log, hooked.log, "stage execution must be identical");
            assert_eq!(
                plain_result.is_ok(),
                hooked_result.is_ok(),
                "the outcome must be identical"
            );
            assert_eq!(plain_result.is_ok(), expect_ok);
        }
    });
}

// ── The free-stage flag ──────────────────────────────────────────────────────

/// A stage that makes no provider call.
struct Free(&'static str);
#[async_trait]
impl Stage<Ctx> for Free {
    fn name(&self) -> &'static str {
        self.0
    }
    async fn run(&self, ctx: &mut Ctx) -> AppResult<()> {
        ctx.log.push(self.0);
        Ok(())
    }
    fn costs_a_provider_call(&self) -> bool {
        false
    }
}

/// The deadline half of `commands::resume_pipeline::hooks::RunHooks`, reduced
/// to the one decision this test is about: refuse every stage that costs a
/// call, let the free ones through.
struct ExpiredClock;
#[async_trait]
impl StageHooks for ExpiredClock {
    async fn before(&self, stage: &StageInfo) -> AppResult<()> {
        if stage.costs_a_call {
            return Err(AppError::Message("out of time".to_string()));
        }
        Ok(())
    }
    async fn after(&self, _stage: &StageInfo, _outcome: StageOutcome) {}
}

/// **A run whose clock expires mid-fan-out still renders and checks what it
/// already paid for.**
///
/// `Stage::costs_a_provider_call` defaults to `true`, so a stage is refused
/// unless it says otherwise; `run_hooked` is what carries that answer to the
/// hook, and a boundary check that could not see it had to choose between
/// stopping every stage (which discarded up to eleven paid section answers at
/// max depth — empty draft, no report, `status=failed`) and stopping none.
///
/// Mutation check: drop `costs_a_call` from the `StageInfo` `run_hooked`
/// builds (hard-code `true`) and the two free stages stop running; hard-code
/// `false` and the paid stage after them runs too.
#[test]
fn run_hooked_tells_the_hook_which_stages_cost_a_provider_call() {
    tauri::async_runtime::block_on(async {
        let mut ctx = Ctx { log: Vec::new() };
        // The shape of a max run stopped by its own clock inside `sections`:
        // two free stages (render, check) and then one that would cost money.
        let result = Pipeline::new("t")
            .add(Free("assemble"))
            .add(Free("validate"))
            .add(Step("repair"))
            .run_hooked(&mut ctx, &ExpiredClock)
            .await;

        assert!(result.is_err(), "the run is still stopped by its deadline");
        assert_eq!(
            ctx.log,
            vec!["assemble", "validate"],
            "the free stages run and the paid one does not"
        );
    });

    // …and the flag is readable off the built pipeline, which is what the
    // résumé pipelines' own pins compare against.
    assert_eq!(
        Pipeline::new("t")
            .add(Free("assemble"))
            .add(Step("repair"))
            .free_stage_names(),
        vec!["assemble"]
    );
}

// ── StageOutcome::timed_out ──────────────────────────────────────────────────

struct TimeoutStep;
#[async_trait]
impl Stage<Ctx> for TimeoutStep {
    fn name(&self) -> &'static str {
        "timeout_step"
    }
    async fn run(&self, ctx: &mut Ctx) -> AppResult<()> {
        ctx.log.push("timeout_step");
        Err(AppError::Timeout("no response within 300s".to_string()))
    }
}

/// Captures `(stage name, StageOutcome::timed_out)` per `after` call — the one
/// field `Recorder` (above) doesn't expose, since it predates
/// `AppError::Timeout`.
#[derive(Default)]
struct TimeoutRecorder {
    seen: Mutex<Vec<(String, bool)>>,
}
#[async_trait]
impl StageHooks for TimeoutRecorder {
    async fn before(&self, _stage: &StageInfo) -> AppResult<()> {
        Ok(())
    }
    async fn after(&self, stage: &StageInfo, outcome: StageOutcome) {
        self.seen
            .lock()
            .push((stage.stage.to_string(), outcome.timed_out));
    }
}

/// `run_hooked` is the ONLY place `StageOutcome::timed_out` is computed — from
/// the stage's raw `AppResult`, which a `StageHooks::after` implementor never
/// sees directly. This is the seam `commands::resume_pipeline::hooks::apply_timeout`
/// (and, downstream, `StoppedReason::Timeout`) is built on.
///
/// Mutation check: hard-code `timed_out: false` in `run_hooked` and the first
/// assertion fails; compute it from `!outcome.ok` instead of matching
/// `AppError::Timeout` specifically and the second assertion (a non-timeout
/// failure) fails.
#[test]
fn run_hooked_flags_a_timeout_error_and_only_a_timeout_error() {
    tauri::async_runtime::block_on(async {
        let mut ctx = Ctx { log: Vec::new() };
        let hooks = TimeoutRecorder::default();
        let _ = Pipeline::new("t")
            .add(TimeoutStep)
            .run_hooked(&mut ctx, &hooks)
            .await;
        assert_eq!(
            hooks.seen.lock().clone(),
            vec![("timeout_step".to_string(), true)]
        );

        let mut ctx = Ctx { log: Vec::new() };
        let hooks = TimeoutRecorder::default();
        let _ = Pipeline::new("t")
            .add(Boom)
            .run_hooked(&mut ctx, &hooks)
            .await;
        assert_eq!(
            hooks.seen.lock().clone(),
            vec![("boom".to_string(), false)],
            "a non-timeout failure must not be flagged as one"
        );
    });
}

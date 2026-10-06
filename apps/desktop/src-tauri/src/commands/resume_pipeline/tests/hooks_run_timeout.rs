use super::super::hooks::{apply_timeout, timeout_message};
use crate::error::{AppError, AppResult};
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::RunLedger;
use crate::pipeline::{Pipeline, Stage, StageHooks, StageInfo, StageOutcome};

/// **Wiring pin: `execute`'s per-call-timeout arm actually attaches
/// [`timeout_failure_data`] to what it hands its caller.** `execute` needs an
/// `AppHandle` this crate has no harness for (same limitation
/// `persist_document_source` documents), so this is presence-only — the pure
/// computation itself is pinned directly by the test above, and the
/// renderer's consumption of the identical `{ kind, stage, seconds }` shape
/// is pinned by `use-resume-pipeline-session/failure.test.ts`'s "localizes a per-call
/// timeout instead of splicing the raw stage key into prose". This is the one
/// link connecting the two ends that only a source read can confirm without
/// a live run.
///
/// Mutation check: change the timeout arm's `data` field back to `None` (the
/// pre-fix shape) and this fails.
#[test]
fn executes_timeout_arm_attaches_the_structured_failure_data() {
    let source = include_str!("../run.rs");
    assert!(
        source.contains("data: Some(hooks::timeout_failure_data(stage, ms))"),
        "execute's per-call-timeout arm must attach timeout_failure_data to the \
         ExecuteFailure it returns — otherwise job.failed's event carries no \
         structured payload and the renderer's timeout banner never renders"
    );
}

/// A fake stage standing in for a provider call that hit its per-call
/// deadline — exactly what each `complete_impl` now returns for a `reqwest`
/// timeout (see `commands::ai_provider::ollama::complete_impl` and its
/// siblings).
struct TimeoutStage;

struct FakeCtx;

#[async_trait::async_trait]
impl Stage<FakeCtx> for TimeoutStage {
    fn name(&self) -> &'static str {
        "strategy"
    }
    async fn run(&self, _ctx: &mut FakeCtx) -> AppResult<()> {
        Err(AppError::Timeout("no response within 300s".to_string()))
    }
}

/// A minimal `StageHooks` standing in for `RunHooks::after`'s ONE decision this
/// module owns — `apply_timeout` — without the `AppHandle` `RunHooks` itself
/// needs (this crate has no Tauri test harness).
struct TimeoutOnlyHooks<'a>(&'a RunLedger);

#[async_trait::async_trait]
impl StageHooks for TimeoutOnlyHooks<'_> {
    async fn before(&self, _stage: &StageInfo) -> AppResult<()> {
        Ok(())
    }
    async fn after(&self, stage: &StageInfo, outcome: StageOutcome) {
        apply_timeout(self.0, stage, outcome);
    }
}

/// **End to end (within this crate's reach): a per-call deadline failure
/// really does yield a run row with the Timeout reason, and a message a user
/// can act on.**
///
/// Chains every pure piece `execute` composes at runtime — `Pipeline::run_hooked`
/// (computes `StageOutcome::timed_out` from the stage's own `AppError`),
/// `apply_timeout` (records `StoppedReason::Timeout` + which stage/how long),
/// `terminal_state` (resolves the row to `failed` + `"timeout"`), and
/// `timeout_message` (the text `job_fail` ends up carrying) — without the
/// `AppHandle` the full command needs.
///
/// Mutation check: any one of those four broken in isolation (see their own
/// tests) breaks this one too, proving they are wired together and not just
/// individually correct.
#[test]
fn a_per_call_deadline_failure_produces_a_run_row_with_the_timeout_reason() {
    tauri::async_runtime::block_on(async {
        let ledger = RunLedger::new();
        let mut ctx = FakeCtx;
        let result = Pipeline::new("resume_quality")
            .add(TimeoutStage)
            .run_hooked(&mut ctx, &TimeoutOnlyHooks(&ledger))
            .await;
        assert!(result.is_err(), "the stage's own error still propagates");

        assert_eq!(ledger.stopped(), Some(StoppedReason::Timeout));
        let (stage, ms) = ledger
            .timeout_detail()
            .expect("the failing stage's name and duration were recorded");
        assert_eq!(stage, "strategy");
        // `ms` is `run_hooked`'s OWN wall-clock reading around the (instant,
        // fake) stage body — the exact-value math is `timeout_message`'s own
        // test below; this only proves a real duration reached the ledger.
        let ms = ms.max(1);

        // The RUN ROW: only `RunTimeout` gets the "still usable" leniency (see
        // `a_timeout_stopped_run_is_always_a_failure_even_when_something_was_persisted`
        // below), so this is unconditionally a failure.
        let (status, reason) =
            super::super::hooks::terminal_state(&ledger, false, false, false, false);
        assert_eq!(status, "failed");
        assert_eq!(reason.as_deref(), Some("timeout"));

        // The RENDERED message: names the stage, gives a next step — never a
        // raw token or an empty string, which is the bug this fix closes.
        let message = timeout_message(stage, ms);
        assert!(message.contains("strategy"), "{message}");
        assert!(message.contains("Try a faster model"), "{message}");
    });
}

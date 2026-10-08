//! #1391/#1393: a cancel drops an opted-in stage's in-flight future, the
//! structured request carries its output cap, a failed call never records the
//! previous call's usage, and a failed stage logs its (redacted) error.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::commands::ai_provider::call_trace::{observe_usage, CallLog};
use crate::commands::ai_provider::ProviderId;
use crate::commands::ai_provider::Usage;
use crate::error::{AppError, AppResult};
use crate::pipeline::call_notes::{begin_call, note_failure};
use crate::pipeline::resume::stages::letter_ahead::Route;
use crate::pipeline::stage::error_field;
use crate::pipeline::structured::{structured_request, STRUCTURED_MAX_TOKENS};
use crate::pipeline::{Pipeline, Stage, StageHooks, StageInfo, StageOutcome};

/// Flips a flag when dropped: the stand-in for "the HTTP stream was closed".
struct DropFlag(Arc<AtomicBool>);
impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// A stage that never finishes on its own, like a model that will not stop.
struct Hangs {
    dropped: Arc<AtomicBool>,
    abandon: bool,
}
#[async_trait]
impl Stage<()> for Hangs {
    fn name(&self) -> &'static str {
        "strategy"
    }
    fn abandon_on_cancel(&self) -> bool {
        self.abandon
    }
    async fn run(&self, _: &mut ()) -> AppResult<()> {
        let _held = DropFlag(self.dropped.clone());
        std::future::pending::<()>().await;
        Ok(())
    }
}

/// Hooks whose run is already cancelled.
struct Cancelled;
#[async_trait]
impl StageHooks for Cancelled {
    async fn before(&self, _: &StageInfo) -> AppResult<()> {
        Ok(())
    }
    async fn after(&self, _: &StageInfo, _: StageOutcome) {}
    async fn cancelled(&self) {}
}

/// Mutation check: make `run_hooked` skip the `select!` and this times out.
#[tokio::test]
async fn a_cancel_drops_an_opted_in_stages_in_flight_future() {
    let dropped = Arc::new(AtomicBool::new(false));
    let pipeline = Pipeline::new("t").add(Hangs {
        dropped: dropped.clone(),
        abandon: true,
    });
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        pipeline.run_hooked(&mut (), &Cancelled),
    )
    .await
    .expect("a cancelled run must not wait out the stage");
    assert!(matches!(result, Err(AppError::Cancelled)));
    assert!(
        dropped.load(Ordering::SeqCst),
        "the in-flight future was dropped"
    );
}

/// A stage that did not opt in keeps its own cancel handling: not raced.
#[tokio::test]
async fn a_stage_that_did_not_opt_in_is_not_dropped_by_a_cancel() {
    let dropped = Arc::new(AtomicBool::new(false));
    let pipeline = Pipeline::new("t").add(Hangs {
        dropped: dropped.clone(),
        abandon: false,
    });
    let raced = tokio::time::timeout(
        Duration::from_millis(200),
        pipeline.run_hooked(&mut (), &Cancelled),
    )
    .await;
    assert!(raced.is_err(), "still running: the cancel did not touch it");
}

/// The cap is sent where the runaway happened (a one-request-at-a-time local
/// server) and only there: cloud reasoning models count thinking against it.
/// `structured_call` builds its request through `structured_request`.
#[test]
fn the_structured_request_is_capped_only_for_a_local_server() {
    let cap = |provider, base_url| {
        structured_request(
            Route { provider, base_url },
            "m",
            "s",
            "u",
            Some(4096),
            Some("low"),
        )
    };
    let local = cap(ProviderId::Ollama, None);
    assert_eq!(local.max_tokens, Some(STRUCTURED_MAX_TOKENS));
    assert_eq!(
        (local.context_window, local.effort.as_deref()),
        (Some(4096), Some("low"))
    );
    let lan = cap(
        ProviderId::OpenAiCompatible,
        Some("http://127.0.0.1:1234/v1"),
    );
    assert_eq!(lan.max_tokens, Some(STRUCTURED_MAX_TOKENS));
    assert_eq!(cap(ProviderId::OpenAi, None).max_tokens, None);
    let hosted = cap(
        ProviderId::OpenAiCompatible,
        Some("https://openrouter.ai/api/v1"),
    );
    assert_eq!(hosted.max_tokens, None);
}

/// A successful call followed by an erroring one: the error entry must carry no
/// usage. Mutation check: drop the `clear_observed_usage` in `begin_call` and
/// the token assertions fail (the previous call's 1304 leaks in).
#[tokio::test]
async fn an_erroring_call_does_not_inherit_the_previous_calls_usage() {
    let log = CallLog::default();
    log.scope(async {
        // Call 1 succeeds and reports usage (what `record_usage` does).
        let _ = begin_call();
        observe_usage(Usage {
            input_tokens: 1304,
            output_tokens: 99,
            ..Usage::default()
        });
        // Call 2 starts, then fails before reporting anything.
        let started = begin_call();
        note_failure(
            "ollama",
            "m",
            None,
            started,
            &AppError::Provider("x".into()),
        );
    })
    .await;
    let calls = log.take();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].error.as_deref(), Some("Provider"));
    assert_eq!((calls[0].input_tokens, calls[0].output_tokens), (0, 0));
}

#[test]
fn a_refusal_logs_a_fixed_message_not_the_models_words() {
    let field = error_field(&AppError::Refusal(
        "Ollama: the model refused — secret echo".into(),
    ));
    assert!(!field.contains("secret echo"), "{field}");
    assert!(field.contains("Refusal"), "{field}");
}

#[test]
fn a_failed_stage_logs_its_redacted_error_text() {
    let field = error_field(&AppError::Provider("stream broke at 10.0.0.5:11434".into()));
    assert!(field.contains("stream broke"), "{field}");
    assert!(
        !field.contains("10.0.0.5"),
        "host must be redacted: {field}"
    );
}

// ── One retry for a provider failure (#1391) ─────────────────────────────────────

/// Drive `complete_json_with` with canned answers; returns (result, charges, calls).
async fn drive(
    answers: Vec<AppResult<String>>,
    refuse_charge_at: Option<usize>,
) -> (AppResult<serde_json::Value>, usize, usize) {
    use std::sync::atomic::AtomicUsize;
    let (charges, calls) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let answers = parking_lot::Mutex::new(answers.into_iter());
    let out = crate::pipeline::complete_json_with(
        || {
            let n = charges.fetch_add(1, Ordering::SeqCst);
            if refuse_charge_at == Some(n) {
                return Err(AppError::RateLimited("run out of time".into()));
            }
            Ok(())
        },
        |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            let next = answers.lock().next().expect("scripted answer");
            async move { next.map(|text| (text, Usage::default())) }
        },
        |_| {},
    )
    .await;
    (
        out,
        charges.load(Ordering::SeqCst),
        calls.load(Ordering::SeqCst),
    )
}

fn broke() -> AppResult<String> {
    Err(AppError::Network(
        "the stream ended before the model finished".into(),
    ))
}

#[tokio::test]
async fn a_mid_stream_network_break_is_retried_once_and_charged_again() {
    let (out, charges, calls) = drive(vec![broke(), Ok(r#"{"a":1}"#.into())], None).await;
    assert!(out.is_ok());
    assert_eq!((charges, calls), (2, 2));
}

/// The retry goes back through `charge`, where the run-deadline guard lives.
#[tokio::test]
async fn the_provider_retry_respects_the_charge_guard() {
    let (out, _, calls) = drive(vec![broke()], Some(1)).await;
    assert!(matches!(out, Err(AppError::RateLimited(_))));
    assert_eq!(calls, 1, "the refused retry never reached the provider");
}

#[tokio::test]
async fn a_second_network_break_is_returned_not_retried_again() {
    let (out, _, calls) = drive(vec![broke(), broke()], None).await;
    assert!(matches!(out, Err(AppError::Network(_))));
    assert_eq!(calls, 2);
}

/// The output-limit cutoff is deterministic: retrying only doubles a runaway.
/// Mutation check: retry on every error and the call count becomes 2.
#[tokio::test]
async fn the_output_limit_cutoff_and_other_provider_errors_are_not_retried() {
    for err in [
        AppError::OutputLimit("cut off".into()),
        AppError::Provider("500".into()),
    ] {
        let (out, charges, calls) = drive(vec![Err(err)], None).await;
        assert!(out.is_err());
        assert_eq!((charges, calls), (1, 1));
    }
}

/// A 429 / busy `Network` error was already retried by the transport layer.
#[tokio::test]
async fn a_rate_limited_network_error_is_not_retried_again() {
    let busy = || {
        Err(AppError::Network(
            "openai: rate limit or quota reached.".into(),
        ))
    };
    let (out, charges, calls) = drive(vec![busy(), Ok(r#"{"a":1}"#.into())], None).await;
    assert!(matches!(out, Err(AppError::Network(_))));
    assert_eq!((charges, calls), (1, 1));
}

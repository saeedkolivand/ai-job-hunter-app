//! Per-stage call trace: collection through `run_hooked`, and the persisted
//! shape (content-free allow-list, SQL-queryable).

use std::collections::BTreeSet;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{
    attach_routing, clear_observed_usage, error_class, note, observe_usage, take_observed_usage,
    CallLog,
};
use crate::commands::ai_provider::{ProviderTimings, Usage};
use crate::error::{AppError, AppResult};
use crate::pipeline::runs::{PipelineRunStore, RunEventRow};
use crate::pipeline::{Pipeline, Stage, StageHooks, StageInfo, StageOutcome};

struct CallingStage;

#[async_trait]
impl Stage<()> for CallingStage {
    fn name(&self) -> &'static str {
        "analyze_job"
    }
    async fn run(&self, _: &mut ()) -> AppResult<()> {
        let usage = Usage {
            input_tokens: 120,
            output_tokens: 40,
            thinking_tokens: Some(7),
            timings: Some(ProviderTimings {
                load_ms: Some(2500),
                prompt_eval_ms: Some(150),
                eval_ms: Some(5700),
                eval_count: Some(40),
            }),
        };
        note(
            "ollama",
            "llama3.2:1b",
            Some("low"),
            Duration::from_millis(21_000),
            usage,
            None,
        );
        // A call that timed out: class only, never the message text.
        let err = AppError::Timeout("prompt text: my résumé at https://x.example".into());
        note(
            "ollama",
            "llama3.2:1b",
            Some("low"),
            Duration::from_millis(300_000),
            Usage::default(),
            Some(error_class(&err)),
        );
        Ok(())
    }
}

#[derive(Default)]
struct Hooks {
    log: CallLog,
    persisted: parking_lot::Mutex<Vec<Option<Value>>>,
}

#[async_trait]
impl StageHooks for Hooks {
    async fn before(&self, _: &StageInfo) -> AppResult<()> {
        Ok(())
    }
    async fn after(&self, _: &StageInfo, outcome: StageOutcome) {
        let artifact = attach_routing(
            Some(json!({ "cached": false })),
            &self.log.take(),
            outcome.ms,
        );
        self.persisted.lock().push(artifact);
    }
    fn call_log(&self) -> Option<CallLog> {
        Some(self.log.clone())
    }
}

/// Collected by `run_hooked`, persisted through the real store, and read back
/// with SQL-shaped access. Mutation check: add a content field (e.g. `prompt:
/// String`) to `CallRecord` and the allow-list assertion FAILS; stop scoping
/// the log in `run_hooked` and `calls` is empty.
#[tokio::test]
async fn a_stage_calls_are_collected_persisted_and_content_free() {
    let hooks = Hooks::default();
    Pipeline::new("t")
        .add(CallingStage)
        .run_hooked(&mut (), &hooks)
        .await
        .unwrap();
    let artifact = hooks.persisted.lock()[0].clone().expect("artifact");

    let dir = tempfile::tempdir().unwrap();
    let store = PipelineRunStore::open(dir.path()).unwrap();
    store
        .append_event(&RunEventRow {
            run_id: "r".into(),
            seq: 0,
            ts: 1,
            stage: "analyze_job".into(),
            phase: "finish".into(),
            artifact_json: artifact.to_string(),
        })
        .unwrap();
    let stored: Value = serde_json::from_str(&store.events_for_run("r")[0].artifact_json).unwrap();

    assert_eq!(
        stored["cached"],
        json!(false),
        "the stage's own keys survive"
    );
    let call = &stored["routing"]["calls"][0];
    assert_eq!(call["provider"], "ollama");
    assert_eq!(call["model"], "llama3.2:1b");
    assert_eq!(call["effort"], "low");
    assert_eq!(call["ms"], 21_000);
    assert_eq!(call["inputTokens"], 120);
    assert_eq!(call["outputTokens"], 40);
    assert_eq!(call["thinkingTokens"], 7);
    assert_eq!(call["loadMs"], 2500);
    assert_eq!(call["promptEvalMs"], 150);
    assert_eq!(call["evalMs"], 5700);
    assert_eq!(call["evalCount"], 40);

    // The privacy contract: ONLY these keys, absolute, hand-written.
    let allowed: BTreeSet<&str> = [
        "provider",
        "model",
        "effort",
        "ms",
        "inputTokens",
        "outputTokens",
        "thinkingTokens",
        "loadMs",
        "promptEvalMs",
        "evalMs",
        "evalCount",
    ]
    .into_iter()
    .collect();
    let keys: BTreeSet<&str> = call
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, allowed, "a call record may carry no content field");

    // The failed call: base keys + `error`, the class only.
    let failed = &stored["routing"]["calls"][1];
    assert_eq!(failed["error"], "Timeout");
    let failed_keys: BTreeSet<&str> = failed
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut expected: BTreeSet<&str> = BTreeSet::from([
        "provider",
        "model",
        "effort",
        "ms",
        "inputTokens",
        "outputTokens",
        "thinkingTokens",
    ]);
    expected.insert("error");
    // `thinkingTokens: None` serializes as null, so it is present.
    assert_eq!(failed_keys, expected);
    assert!(
        !stored.to_string().contains("résumé"),
        "no message text persists"
    );
    let routing_keys: BTreeSet<&str> = stored["routing"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(routing_keys, BTreeSet::from(["ms", "calls"]));
}

/// Outside a scope `note` is a no-op, and a stage with no calls leaves its
/// artifact byte-identical (cached / free stages are unchanged).
#[test]
fn no_scope_and_no_calls_change_nothing() {
    note("openai", "m", None, Duration::ZERO, Usage::default(), None);
    let artifact = Some(json!({ "cached": true }));
    assert_eq!(attach_routing(artifact.clone(), &[], 5), artifact);
}

/// Observe-then-take consumes the slot, and a cleared slot is not inherited by
/// the next call (a provider that returns Ok without `record_usage`).
/// Mutation check: make `clear_observed_usage` a no-op and the last assert fails.
#[tokio::test]
async fn observed_usage_is_consumed_and_never_inherited() {
    let log = CallLog::default();
    log.scope(async {
        observe_usage(Usage {
            input_tokens: 9,
            ..Usage::default()
        });
        assert_eq!(take_observed_usage().map(|u| u.input_tokens), Some(9));
        assert!(take_observed_usage().is_none(), "second take is empty");
        observe_usage(Usage {
            input_tokens: 9,
            ..Usage::default()
        });
        clear_observed_usage();
        assert!(take_observed_usage().is_none(), "stale usage must not leak");
    })
    .await;
}

/// The error class is the variant name, with none of the message.
#[test]
fn error_class_is_the_variant_only() {
    assert_eq!(error_class(&AppError::Timeout("secret".into())), "Timeout");
    assert_eq!(error_class(&AppError::Network("secret".into())), "Network");
}

/// More calls than the cap keep the first N and count the rest.
#[test]
fn calls_past_the_cap_are_counted_not_stored() {
    let call = |ms| super::CallRecord {
        provider: "p".into(),
        model: "m".into(),
        effort: None,
        ms,
        input_tokens: 0,
        output_tokens: 0,
        thinking_tokens: None,
        timings: None,
        error: None,
    };
    let calls: Vec<_> = (0..15).map(call).collect();
    let routing = attach_routing(None, &calls, 1).unwrap()["routing"].clone();
    assert_eq!(routing["calls"].as_array().unwrap().len(), 12);
    assert_eq!(routing["dropped"], 3);
}

//! Per-stage call trace: collection through `run_hooked`, and the persisted
//! shape (content-free allow-list, SQL-queryable).

use std::collections::BTreeSet;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{attach_routing, note, CallLog};
use crate::commands::ai_provider::{ProviderTimings, Usage};
use crate::error::AppResult;
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
    note("openai", "m", None, Duration::ZERO, Usage::default());
    let artifact = Some(json!({ "cached": true }));
    assert_eq!(attach_routing(artifact.clone(), &[], 5), artifact);
}

//! Content-free routing + timing for each provider call a pipeline stage makes.
//!
//! A [`CallLog`] is scoped around ONE stage body (`Pipeline::run_hooked`); the
//! completer seam notes a [`CallRecord`] per completed round-trip and the run
//! hooks persist the drained list inside the stage's `artifact_json`. Outside a
//! scope every function is a no-op, so no other caller changes.
//!
//! Scope: only [`Completer`](crate::pipeline::Completer) round-trips made INSIDE
//! a stage body's scope are traced. The #1371 early company research is polled
//! outside any stage scope, and calls from `tokio::spawn`ed tasks do not inherit
//! the task-local, so neither appears here.
//!
//! Privacy: a [`CallRecord`] holds only the provider id, model name, effort
//! token and numbers. It never holds prompt/answer text, job or résumé content,
//! a URL or a key — and the fields are a closed struct, so a content field
//! cannot be added without the allow-list test failing.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};

use super::{ProviderTimings, Usage};

/// One completed provider round-trip, as persisted.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRecord {
    pub provider: String,
    pub model: String,
    /// The effort token passed to the call (after stage/cheapest resolution;
    /// an adapter may still gate it for a model without that lever).
    pub effort: Option<String>,
    /// Wall duration of the round-trip.
    pub ms: u64,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub thinking_tokens: Option<u32>,
    #[serde(flatten)]
    pub timings: Option<ProviderTimings>,
    /// The error CLASS (the `AppError` variant name) of a failed call — never
    /// the message text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Most calls kept per stage; the rest only bump `dropped`. Keeps the routing
/// block far under the artifact clamp so a stage's own keys are never cut.
const MAX_CALLS: usize = 12;

/// The variant name of `err` (Debug text up to the payload) — class only.
pub(crate) fn error_class(err: &crate::error::AppError) -> String {
    format!("{err:?}")
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect()
}

#[derive(Default)]
struct Inner {
    calls: Vec<CallRecord>,
    last_usage: Option<Usage>,
}

/// The per-run collector; cheap to clone (shared).
#[derive(Clone, Default)]
pub struct CallLog(Arc<Mutex<Inner>>);

tokio::task_local! {
    static LOG: CallLog;
}

impl CallLog {
    /// Run `fut` with this log as the ambient collector.
    pub async fn scope<F: std::future::Future>(&self, fut: F) -> F::Output {
        LOG.scope(self.clone(), fut).await
    }

    /// Drain what has been noted since the last drain.
    pub fn take(&self) -> Vec<CallRecord> {
        std::mem::take(&mut self.0.lock().calls)
    }
}

/// Remember the latest usage a provider reported (called by `record_usage`).
pub(crate) fn observe_usage(usage: Usage) {
    let _ = LOG.try_with(|log| log.0.lock().last_usage = Some(usage));
}

/// Forget any observed usage — call before a call whose usage is read back.
pub(crate) fn clear_observed_usage() {
    let _ = LOG.try_with(|log| log.0.lock().last_usage = None);
}

/// The usage `record_usage` last observed in this scope, consumed — for the
/// streaming path, whose usage is recorded deep inside the stream loop.
pub(crate) fn take_observed_usage() -> Option<Usage> {
    LOG.try_with(|log| log.0.lock().last_usage.take())
        .ok()
        .flatten()
}

/// Note one completed round-trip. No-op outside a [`CallLog::scope`].
pub(crate) fn note(
    provider: &str,
    model: &str,
    effort: Option<&str>,
    elapsed: Duration,
    usage: Usage,
    error: Option<String>,
) {
    let _ = LOG.try_with(|log| {
        log.0.lock().calls.push(CallRecord {
            provider: provider.to_string(),
            model: model.to_string(),
            effort: effort.map(str::to_string),
            ms: elapsed.as_millis() as u64,
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            thinking_tokens: usage.thinking_tokens,
            timings: usage.timings,
            error,
        });
    });
}

/// Merge `calls` into a stage artifact as `routing`; `None` calls leaves the
/// artifact untouched (cached and free stages stay exactly as they were).
pub fn attach_routing(
    artifact: Option<Value>,
    calls: &[CallRecord],
    stage_ms: u64,
) -> Option<Value> {
    if calls.is_empty() {
        return artifact;
    }
    let mut routing = json!({ "ms": stage_ms, "calls": &calls[..calls.len().min(MAX_CALLS)] });
    if calls.len() > MAX_CALLS {
        routing["dropped"] = json!(calls.len() - MAX_CALLS);
    }
    let mut base = match artifact {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    base.insert("routing".to_string(), routing);
    Some(Value::Object(base))
}

#[cfg(test)]
mod tests;

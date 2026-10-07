//! Content-free routing + timing for each provider call a pipeline stage makes.
//!
//! A [`CallLog`] is scoped around ONE stage body (`Pipeline::run_hooked`); the
//! completer seam notes a [`CallRecord`] per completed round-trip and the run
//! hooks persist the drained list inside the stage's `artifact_json`. Outside a
//! scope every function is a no-op, so no other caller changes.
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
    /// The effort token actually sent (after stage/cheapest resolution).
    pub effort: Option<String>,
    /// Wall duration of the round-trip.
    pub ms: u64,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub thinking_tokens: Option<u32>,
    #[serde(flatten)]
    pub timings: Option<ProviderTimings>,
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
    let routing = json!({ "ms": stage_ms, "calls": calls });
    let mut base = match artifact {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    base.insert("routing".to_string(), routing);
    Some(Value::Object(base))
}

#[cfg(test)]
mod tests;

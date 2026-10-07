//! [`Completer`]'s per-call accounting: spend recording plus the content-free
//! routing/timing note (`commands::ai_provider::call_trace`). Split out of
//! `completion.rs` (R8 line budget).

use std::time::{Duration, Instant};

use crate::commands::ai_provider::{call_trace, record_usage, Usage};
use crate::error::AppResult;

use super::completer::Completer;

impl Completer {
    /// Record ONE completed round-trip's REAL reported usage against today's
    /// spend. Post-call by necessity: the token counts come from the response.
    ///
    /// Also notes the call's routing + timing for the stage's persisted trail
    /// (`effort` is what was actually sent; `started` is when the request left).
    pub(super) fn record_spend(&self, usage: Usage, effort: Option<&str>, started: Instant) {
        self.note_call(effort, started.elapsed(), usage);
        record_usage(
            &self.app,
            self.provider.id().as_str(),
            &self.model,
            usage,
            self.base_url.as_deref(),
        );
    }

    /// Note one completed round-trip in the ambient per-stage call log.
    pub(super) fn note_call(&self, effort: Option<&str>, elapsed: Duration, usage: Usage) {
        self.note_with(effort, elapsed, usage, None);
    }

    fn note_with(
        &self,
        effort: Option<&str>,
        elapsed: Duration,
        usage: Usage,
        error: Option<String>,
    ) {
        call_trace::note(
            self.provider.id().as_str(),
            &self.model,
            effort,
            elapsed,
            usage,
            error,
        );
    }

    /// Pass `result` through, noting a FAILED call (error class only) first —
    /// a timed-out call is exactly the one the trail must show.
    pub(super) fn or_note<T>(
        &self,
        effort: Option<&str>,
        started: Instant,
        result: AppResult<T>,
    ) -> AppResult<T> {
        if let Err(e) = &result {
            let usage = call_trace::take_observed_usage().unwrap_or_default();
            let class = call_trace::error_class(e);
            self.note_with(effort, started.elapsed(), usage, Some(class));
        }
        result
    }
}

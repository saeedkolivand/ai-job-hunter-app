//! Centralized observability — timed, structured operation spans.
//!
//! `Span` is the single owner of the begin/elapsed/end log mechanics shared by
//! every subsystem. It emits a `→` line at start and a `←` line with duration
//! and outcome at end, in one consistent format:
//!
//! ```text
//! [<target>] → <fields>
//! [<target>] ← <fields> [<extra>] duration=<n>ms ok=<bool>
//! ```
//!
//! `target` is the log prefix (`ai`, `scrape`, `apply`, `autopilot`,
//! `pipeline:<name>`); `fields` are pre-rendered `key=value` pairs. Domain
//! wrappers (`RequestTrace`, `StageTrace`) compose this instead of reimplementing
//! the timing logic.

use std::time::Instant;

pub struct Span {
    target: String,
    fields: String,
    start: Instant,
}

impl Span {
    /// Begin a span: logs `[target] → fields` and starts the timer.
    pub fn begin(target: impl Into<String>, fields: impl Into<String>) -> Self {
        let target = target.into();
        let fields = fields.into();
        log::info!("[{target}] → {fields}");
        Self {
            target,
            fields,
            start: Instant::now(),
        }
    }

    /// End the span: logs `[target] ← fields duration=<n>ms ok=<ok>`.
    pub fn end(&self, ok: bool) {
        log::info!(
            "[{}] ← {} duration={}ms ok={}",
            self.target,
            self.fields,
            self.start.elapsed().as_millis(),
            ok
        );
    }

    /// End with trailing fields rendered before `duration` (e.g. `status=200`,
    /// `count=12`). Empty `extra` is equivalent to [`Span::end`].
    pub fn end_with(&self, extra: &str, ok: bool) {
        if extra.is_empty() {
            return self.end(ok);
        }
        log::info!(
            "[{}] ← {} {} duration={}ms ok={}",
            self.target,
            self.fields,
            extra,
            self.start.elapsed().as_millis(),
            ok
        );
    }
}

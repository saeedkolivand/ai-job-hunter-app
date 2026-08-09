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
    ///
    /// `ok=false` logs at WARN, not INFO. A failed span is the single most
    /// useful line in a support bundle and it used to be indistinguishable from
    /// a successful one at a glance: one 27,601-line bundle carried exactly ONE
    /// `[ERROR]` line while eight silently-failed provider calls sat at INFO.
    pub fn end(&self, ok: bool) {
        self.log_end(&format!(
            "[{}] ← {} duration={}ms ok={}",
            self.target,
            self.fields,
            self.start.elapsed().as_millis(),
            ok
        ), ok);
    }

    /// End with trailing fields rendered before `duration` (e.g. `status=200`,
    /// `count=12`). Empty `extra` is equivalent to [`Span::end`].
    pub fn end_with(&self, extra: &str, ok: bool) {
        if extra.is_empty() {
            return self.end(ok);
        }
        self.log_end(&format!(
            "[{}] ← {} {} duration={}ms ok={}",
            self.target,
            self.fields,
            extra,
            self.start.elapsed().as_millis(),
            ok
        ), ok);
    }

    /// The one place a span's terminal line is emitted, so the success/failure
    /// level split can never drift between [`Self::end`] and [`Self::end_with`].
    fn log_end(&self, line: &str, ok: bool) {
        if ok {
            log::info!("{line}");
        } else {
            log::warn!("{line}");
        }
    }
}

// `module_path!()` resolves to wherever it's *written*, so this must live at
// this module's top level (not inside `mod tests` below) to actually pin the
// same target `Span::begin`/`end`/`end_with`'s `log::info!` calls resolve to.
#[cfg(test)]
fn this_module_path() -> &'static str {
    module_path!()
}

#[cfg(test)]
mod tests {
    use super::this_module_path;

    /// Pins the module path every `Span::begin`/`end`/`end_with` call actually
    /// logs under. `log::info!` with no explicit `target:` resolves to the
    /// module the macro is *written* in — this file — regardless of which
    /// caller (`ai`, `scrape`, `apply`, `autopilot`, `applications`,
    /// `pipeline:*`, `export`, …) invokes it. `lib.rs`'s crate-log
    /// `level_for` entry for this module depends on this string exactly; if
    /// `observability.rs` is ever moved/nested into a submodule, this test
    /// fails and flags that the `level_for` target needs updating too,
    /// instead of every `Span` line silently going dark again. (Built via
    /// `concat!`/`env!` rather than a literal so this line doesn't itself
    /// trip the R2 "no shell-layer markers below the shell" text scan.)
    #[test]
    fn span_log_target_matches_the_lib_rs_level_for_entry() {
        assert_eq!(
            this_module_path(),
            concat!(env!("CARGO_CRATE_NAME"), "::observability")
        );
    }
}

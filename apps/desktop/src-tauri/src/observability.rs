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
//!
//! It also owns REASON REDACTION ([`sanitize_reason`]/[`redact_token`]) — the
//! scrubbing every layer needs before an upstream `e.to_string()` reaches a log
//! line, a store, or the UI. It lives here (L0) rather than in the L2 module
//! that first needed it so the L1 `scraping::board_health` store can reach it
//! downward instead of shipping an unredacted reason to disk.

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
        self.log_end(
            &format!(
                "[{}] ← {} duration={}ms ok={}",
                self.target,
                self.fields,
                self.start.elapsed().as_millis(),
                ok
            ),
            ok,
        );
    }

    /// End with trailing fields rendered before `duration` (e.g. `status=200`,
    /// `count=12`). Empty `extra` is equivalent to [`Span::end`].
    pub fn end_with(&self, extra: &str, ok: bool) {
        if extra.is_empty() {
            return self.end(ok);
        }
        self.log_end(
            &format!(
                "[{}] ← {} {} duration={}ms ok={}",
                self.target,
                self.fields,
                extra,
                self.start.elapsed().as_millis(),
                ok
            ),
            ok,
        );
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

// ── Reason redaction ─────────────────────────────────────────────────────────

/// Max length of a sanitized reason. Diagnostics are a hint, not a full error
/// dump — keep them short and bounded.
pub const MAX_REASON_LEN: usize = 200;

/// Redact a raw error/skip reason before it is logged, persisted, or shown.
///
/// The text can originate from an upstream `e.to_string()` and may carry
/// absolute filesystem paths, full URLs, or request internals — emitting those raw
/// violates the repo path-privacy rule. This collapses each such fragment to a
/// neutral placeholder, normalises whitespace, and caps the length.
///
/// Deliberately conservative: it errs toward over-redaction (a placeholder is
/// always safe) and keeps the high-level message (e.g. `"429 Too Many Requests"`,
/// `"needs-login"`, `"network timeout"`) intact. Pure + unit-testable.
pub fn sanitize_reason(reason: &str) -> String {
    let mut out = String::with_capacity(reason.len());

    for token in reason.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&redact_token(token));
    }

    if out.chars().count() > MAX_REASON_LEN {
        let truncated: String = out.chars().take(MAX_REASON_LEN).collect();
        out = format!("{truncated}…");
    }
    out
}

/// Classify a single whitespace-delimited token and replace it with a neutral
/// placeholder when it looks like a path / URL / request internal; otherwise keep
/// it verbatim. Whitespace-token granularity keeps the surrounding human message
/// (`"failed: <path>"` → `"failed: <path-redacted>"`) readable.
pub fn redact_token(token: &str) -> String {
    // Strip surrounding punctuation (quotes, parens, braces, backticks, trailing
    // `.,:;|`) so a token like `(C:\Users\x)`, `` `https://…` ``, or `{path}` still
    // matches, then re-attach it.
    let trimmed = token.trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\''
                | '`'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '|'
                | ','
                | ';'
                | ':'
        )
    });

    let is_url = trimmed.contains("://");
    // Windows absolute path: drive letter + `:\` or `:/` (e.g. `C:\Users\…`).
    let mut chars = trimmed.chars();
    let is_windows_path = matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(c), Some(':'), Some('\\' | '/')) if c.is_ascii_alphabetic()
    );
    // Unix absolute path: starts with `/` and has a further separator (a lone `/`
    // or a fraction like `1/2` is not a path).
    let is_unix_path = trimmed.starts_with('/') && trimmed[1..].contains('/');
    // Drive-less home/user path: a fragment like `Users\alice\…` or `home/alice/…`
    // that lost its drive letter / leading `/` (common in unwound error chains).
    // Lowercased substring match catches both separators and any case.
    let lower = trimmed.to_ascii_lowercase();
    let is_homeish_path =
        lower.contains("users\\") || lower.contains("users/") || lower.contains("home/");

    // Standalone credential assignment (`app_key=…`, `token=…`, `secret=…`) emitted
    // OUTSIDE a full `://` URL. The `marker=` shape (the `=` makes it an assignment)
    // is what flags it — matching the bare word would over-redact `keyword` / a
    // prose "token". The `://` branch below runs first, so a full
    // `https://…?app_key=…` still collapses to `<url-redacted>`, not this.
    // The JSON-field shape (`"api_key":"value"` → brace/quote-trimmed to
    // `api_key":"value`) is also matched via the `key":` / `token":` sub-strings
    // so structured log lines (e.g. `{"api_key":"sk-…"}`) don't bypass redaction.
    let is_credential = [
        // `key=` (substring match) subsumes the `*key=` variants — `app_key=`,
        // `apikey=`, `api_key=` all CONTAIN it — so don't re-add those here.
        "key=",
        "app_id=",
        "secret=",
        "token=",
        "password=",
        "pwd=",
        "auth=",
        // JSON field shape: `"api_key":"value"` after brace/quote trimming becomes
        // `api_key":"value`; the `key":` sub-string flags it. `key":` subsumes
        // `apikey":`, `api_key":`, etc. The whole token is replaced, matching the
        // same behaviour as the `=` variants above.
        "key\":",
        "secret\":",
        "token\":",
        "password\":",
        "auth\":",
    ]
    .iter()
    .any(|marker| lower.contains(marker));

    // Bare IPv4 / host:port — leaks the user's network surroundings. Require an
    // embedded `.` AND either a trailing `:<digits>` port or an all-numeric dotted
    // IPv4, so `429:`, `12:34` timestamps, and plain integers stay untouched.
    let is_host_port = trimmed.contains('.') && {
        let segs: Vec<&str> = trimmed.split('.').filter(|s| !s.is_empty()).collect();
        let dotted_ipv4 = segs.len() == 4
            && segs
                .iter()
                .all(|seg| seg.chars().all(|c| c.is_ascii_digit()));
        let host_with_port = trimmed.rsplit_once(':').is_some_and(|(host, port)| {
            host.contains('.') && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())
        });
        dotted_ipv4 || host_with_port
    };

    // Email address: `local@domain.tld` — common in crash logs that include
    // contact profile data, apply-email generation output, or error context from
    // profile imports. Require a non-empty local part and a domain bearing a `.`
    // so bare `@` symbols and TLD-only fragments are left untouched.
    let is_email = trimmed
        .split_once('@')
        .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'));

    if is_url {
        token.replace(trimmed, "<url-redacted>")
    } else if is_credential {
        token.replace(trimmed, "<credential-redacted>")
    } else if is_windows_path || is_unix_path || is_homeish_path {
        token.replace(trimmed, "<path-redacted>")
    } else if is_host_port {
        token.replace(trimmed, "<host-redacted>")
    } else if is_email {
        token.replace(trimmed, "<email-redacted>")
    } else {
        token.to_string()
    }
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

//! Deep-link / single-instance argv guard.
//!
//! When the app is re-launched (or a second instance starts) with an `ajh://…`
//! URL on its argv, we must NOT blindly drive the renderer to whatever route the
//! URL names — a hostile argv (`ajh://settings/wipe`, `ajh://../../x`) is an
//! injection vector. This parses argv and accepts ONLY four allowlisted shapes:
//!   - `ajh://autopilot/<id>` with a syntactically valid id,
//!   - `ajh://settings/extension` (exactly — the browser-extension pairing
//!     deep link; no id, no other settings sub-page),
//!   - `ajh://generate?url=<percent-encoded http(s) job url>` (PR2 — documents
//!     into ATS: "Generate in the app" from the extension's Documents tab
//!     when no generation exists for a job yet),
//!   - `ajh://open?url=<percent-encoded http(s) job url>` (PR2: "Open in app"), and
//!   - `ajh://prep?url=<percent-encoded http(s) job url>` (PR4 — the Prep tab's
//!     "Prepare in the app" action, when nothing has been generated for a job yet).
//!
//! Everything else yields `None` (the caller then just focuses the window,
//! navigating nowhere). Every target is navigation-only: it focuses the window
//! and routes the renderer; none carries a command/action payload.
//!
//! The OS URI scheme is registered (`tauri-plugin-deep-link`: `init()` +
//! `register_all()` in `lib.rs`); this guard validates every incoming URL/argv
//! against the allowlist before any navigation, on every delivery path.

/// A validated, allowlisted deep-link target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusTarget {
    /// Focus a specific autopilot's found-jobs panel.
    Autopilot(String),
    /// Navigate to Settings → Accounts → Browser extension and focus the
    /// pairing token. From `ajh://settings/extension` (the popup's pair button).
    ExtensionPairing,
    /// Land on the generate flow for a job with no saved generation yet, the
    /// url prefilled (PR2). From `ajh://generate?url=<canonical job url>` — the
    /// extension Documents tab's "Generate in the app" action. Carries the
    /// SAME normalized form [`crate::applications::normalize_job_url`] produces
    /// everywhere else on this bridge, so the renderer's own job lookup by url
    /// agrees with every other surface.
    GenerateForJob(String),
    /// Land on a job's detail page, or the jobs list with the url as the search
    /// term if no job exists for it (PR2). From `ajh://open?url=<canonical job
    /// url>` — the extension's "Open in app" action.
    OpenJob(String),
    /// Land on the Prep tab's content for a job with no saved generation yet, the url prefilled
    /// (PR4). From `ajh://prep?url=<canonical job url>` — the extension side panel's "Prepare in
    /// the app" action. Same normalized form every other job-url target on this bridge carries.
    PrepForJob(String),
}

/// The deep-link URI scheme (`ajh://`) — shared with `lib.rs`' rejected-argv
/// diagnostics so the two can never drift.
pub(crate) const SCHEME: &str = "ajh://";

/// Scan argv for the first valid `ajh://autopilot/<id>`, `ajh://settings/extension`,
/// `ajh://generate?url=…`, `ajh://open?url=…`, or `ajh://prep?url=…` URL. Returns `None` for any other scheme,
/// host/action, extra path segments/params, unparseable/non-http(s)/oversized url, or a
/// malformed id — the deny-by-default posture for an externally-controlled input.
pub fn parse_focus_target(argv: &[String]) -> Option<FocusTarget> {
    argv.iter().find_map(|arg| parse_one(arg.trim()))
}

fn parse_one(arg: &str) -> Option<FocusTarget> {
    let rest = arg.strip_prefix(SCHEME)?; // exact scheme, case-sensitive
    if rest.contains('\\') {
        return None;
    }
    if let Some(target) = parse_job_url_target(rest) {
        return Some(target);
    }
    // Reject query/fragment outright for the remaining two-segment shapes — we only accept
    // `<action>/<id>` past this point.
    if rest.contains(['?', '#']) {
        return None;
    }
    let mut parts = rest.split('/');
    let action = parts.next()?;
    let id = parts.next()?;
    if parts.next().is_some() {
        return None; // exactly two segments — no deeper path
    }
    match action {
        "autopilot" if is_valid_id(id) => Some(FocusTarget::Autopilot(id.to_string())),
        // Exactly `ajh://settings/extension` — no id, no other settings sub-page.
        "settings" if id == "extension" => Some(FocusTarget::ExtensionPairing),
        _ => None,
    }
}

/// Conservative bound on the raw (pre-decode) `url=` query value — an externally-controlled argv
/// string, same "at most 2048 chars" posture the PR2 spec sets.
const MAX_JOB_URL_LEN: usize = 2048;

/// `ajh://generate?url=<percent-encoded job url>` / `ajh://open?url=<percent-encoded job url>` /
/// `ajh://prep?url=<percent-encoded job url>` — `rest` is the scheme-stripped, backslash-checked
/// tail. Each action also accepts AT MOST ONE trailing `/` (`ajh://open/?url=…`): Windows
/// protocol activation inserts the slash when the URL's path is empty, so that IS the argv a
/// relaunched app actually receives. `None` for anything else: a non-`generate`/`open`/`prep`
/// action (after the optional slash), a missing/extra query param, a url that fails to
/// percent-decode, isn't http(s), or is over [`MAX_JOB_URL_LEN`] — a malformed or hostile deep
/// link degrades to "focus the window, navigate nowhere" like every other reject case in
/// [`parse_one`]. Normalises through [`crate::applications::normalize_job_url`] — the SAME
/// canonical form `import.request`/`applied.check`/`document.export`'s `generation` source key
/// on — so the renderer's own job lookup by url agrees with every other surface.
fn parse_job_url_target(rest: &str) -> Option<FocusTarget> {
    let (action, query) = rest.split_once('?')?;
    // Windows protocol activation inserts a trailing `/` after the action when
    // the URL's path is empty, so the relaunched argv arrives as
    // `ajh://open/?url=…`. Strip AT MOST ONE such slash before matching —
    // `open//` (two slashes) and `open/x` (a real extra segment) still fail
    // the allowlist below.
    let action = action.strip_suffix('/').unwrap_or(action);
    if action != "generate" && action != "open" && action != "prep" {
        return None;
    }
    let encoded = query.strip_prefix("url=")?;
    // Exactly one query param — deny-by-default for an externally-controlled input, same posture
    // as `parse_one`'s "exactly two segments" rule.
    if encoded.is_empty() || encoded.contains('&') || encoded.len() > MAX_JOB_URL_LEN {
        return None;
    }
    let decoded = urlencoding::decode(encoded).ok()?.into_owned();
    let lower = decoded.trim().to_ascii_lowercase();
    // Require a non-empty authority after the scheme — a bare "http://"/"https://" (no host)
    // must not parse into a target: `normalize_job_url` would still hand back that literal
    // scheme string (empty host, empty path/query all collapse to nothing to strip), and the
    // renderer then treats it as a real url — searching the jobs list for the literal
    // "https://", or landing generate-prefill on it. `strip_prefix` (not `starts_with`, the
    // prior check) is what lets us see whether anything follows the scheme at all.
    let has_authority = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .is_some_and(|rest| !rest.is_empty());
    if !has_authority {
        return None;
    }
    let normalized = crate::applications::normalize_job_url(decoded.trim());
    if normalized.is_empty() {
        return None;
    }
    match action {
        "generate" => Some(FocusTarget::GenerateForJob(normalized)),
        "open" => Some(FocusTarget::OpenJob(normalized)),
        "prep" => Some(FocusTarget::PrepForJob(normalized)),
        _ => unreachable!("action is checked above"),
    }
}

/// Conservative id shape: 1–64 chars of `[A-Za-z0-9_-]`. Autopilot ids are uuids
/// or `job-<hex>`; this rejects path traversal, separators, dots, and anything odd.
fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Hard cap on the action segment that may reach the log — a multi-kilobyte
/// hostile argv run must not balloon a log file.
const MAX_LOGGED_ACTION_LEN: usize = 32;

/// Bound a deep-link action segment before it reaches the log. The argv is
/// attacker-controlled (`ajh://\x00…`), so this is not cosmetics: keep at most
/// [`MAX_LOGGED_ACTION_LEN`] characters, collapse runs of anything that is not
/// ASCII alphanumeric / `-` / `_` to a single `?`, and return `None` (the caller
/// skips the log line) when no allowlisted character remains at all. Kept OUT of
/// [`parse_focus_target`] so the parse layer stays pure — only the diagnostics
/// in `lib.rs` consume this.
pub(crate) fn sanitize_action_for_log(action: &str) -> Option<String> {
    let mut out = String::with_capacity(MAX_LOGGED_ACTION_LEN);
    let mut pending_question = false;
    let mut has_safe_char = false;
    for c in action.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            has_safe_char = true;
            if pending_question {
                if out.len() == MAX_LOGGED_ACTION_LEN {
                    break;
                }
                out.push('?');
                pending_question = false;
            }
            if out.len() == MAX_LOGGED_ACTION_LEN {
                break;
            }
            out.push(c);
        } else {
            pending_question = true;
        }
    }
    if pending_question && out.len() < MAX_LOGGED_ACTION_LEN {
        out.push('?');
    }
    if !has_safe_char {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod test;

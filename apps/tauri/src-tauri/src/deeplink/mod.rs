//! Deep-link / single-instance argv guard.
//!
//! When the app is re-launched (or a second instance starts) with an `ajh://…`
//! URL on its argv, we must NOT blindly drive the renderer to whatever route the
//! URL names — a hostile argv (`ajh://settings/wipe`, `ajh://../../x`) is an
//! injection vector. This parses argv and accepts ONLY the allowlisted
//! `ajh://autopilot/<id>` shape with a syntactically valid id; everything else
//! yields `None` (the caller then just focuses the window, navigating nowhere).
//!
//! The OS URI scheme itself is not registered yet (a follow-up will add
//! `tauri-plugin-deep-link`); the guard lives here first so it is in place and
//! unit-tested before any externally-controlled URL can reach navigation.

/// A validated, allowlisted deep-link target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusTarget {
    /// Focus a specific autopilot's found-jobs panel.
    Autopilot(String),
}

const SCHEME: &str = "ajh://";

/// Scan argv for the first valid `ajh://autopilot/<id>` URL. Returns `None` for
/// any other scheme, host/action, extra path segments, query/fragment, or a
/// malformed id — the deny-by-default posture for an externally-controlled input.
pub fn parse_focus_target(argv: &[String]) -> Option<FocusTarget> {
    argv.iter().find_map(|arg| parse_one(arg.trim()))
}

fn parse_one(arg: &str) -> Option<FocusTarget> {
    let rest = arg.strip_prefix(SCHEME)?; // exact scheme, case-sensitive
                                          // Reject query/fragment/backslash outright — we only accept `<action>/<id>`.
    if rest.contains(['?', '#', '\\']) {
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
        _ => None,
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

#[cfg(test)]
mod test;

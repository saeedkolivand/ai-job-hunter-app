//! Deep-link / single-instance argv guard.
//!
//! When the app is re-launched (or a second instance starts) with an `ajh://…`
//! URL on its argv, we must NOT blindly drive the renderer to whatever route the
//! URL names — a hostile argv (`ajh://settings/wipe`, `ajh://../../x`) is an
//! injection vector. This parses argv and accepts ONLY two allowlisted shapes:
//!   - `ajh://autopilot/<id>` with a syntactically valid id, and
//!   - `ajh://settings/extension` (exactly — the browser-extension pairing
//!     deep link; no id, no other settings sub-page).
//! Everything else yields `None` (the caller then just focuses the window,
//! navigating nowhere). Both targets are navigation-only: they focus the window
//! and route the renderer; they carry no command/action payload.
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
}

const SCHEME: &str = "ajh://";

/// Scan argv for the first valid `ajh://autopilot/<id>` or
/// `ajh://settings/extension` URL. Returns `None` for any other scheme,
/// host/action, extra path segments, query/fragment, or a malformed id — the
/// deny-by-default posture for an externally-controlled input.
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
        // Exactly `ajh://settings/extension` — no id, no other settings sub-page.
        "settings" if id == "extension" => Some(FocusTarget::ExtensionPairing),
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

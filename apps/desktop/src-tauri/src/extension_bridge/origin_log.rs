//! Rejected-origin + handshake-failure log noise suppression — split out of
//! `auth.rs` (R8 relief): the origin-allowlist DECISION stays there, this file
//! owns only how a rejection/failure is logged (once per origin, shape-sanitized)
//! so an unpairable extension's ~10s reconnect loop can't flood the log.

/// Origins already reported this session, so an extension that can never pair
/// logs once instead of on every reconnect.
///
/// A browser does not expose a failed WebSocket handshake's HTTP status to
/// script, so the extension genuinely cannot tell "desktop refused my origin"
/// from "desktop isn't running" — its ~10s reconnect loop is correct behaviour,
/// not a bug to fix client-side. What was wrong is that each attempt wrote TWO
/// warnings: one unpairable build produced 1,932 rejections + 1,932 paired
/// failures in a single reported session.
pub(super) fn warn_rejected_origin_once(origin: &str) {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};

    /// Ceiling on remembered origins. The `Origin` header is attacker-supplied
    /// (any loopback client can send one), so an unbounded set is a memory sink
    /// a hostile local process could grow at will. Past the cap nothing new is
    /// remembered and further origins log at debug — the suppression is what
    /// matters, and a real deployment has ONE bad origin, not 64.
    const MAX_REMEMBERED: usize = 64;

    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let mut seen = match SEEN.get_or_init(Default::default).lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    // Shape-constrain once, up front — every branch below logs this same
    // value. `sanitize_reason` (credential-token redaction) is the wrong tool
    // here: the origin is the ONE piece of diagnostic information a developer
    // needs, verbatim, to add to `extensionDevOrigins`, so the value itself
    // must survive; only its *shape* (length, control chars) is constrained.
    let logged = sanitize_log_origin(origin);
    if seen.len() >= MAX_REMEMBERED && !seen.contains(origin) {
        log::debug!("[extension_bridge] rejected handshake from origin: {logged:?} (cap reached)");
        return;
    }
    if seen.insert(origin.to_string()) {
        // First time only, and deliberately actionable: an unpaired extension is
        // otherwise indistinguishable from a desktop that simply isn't running.
        log::warn!(
            "[extension_bridge] rejected handshake from origin: {logged:?} — not an allowed \
             extension origin. A development build needs its origin in the \
             `extensionDevOrigins` config; a store build should already match. \
             Further rejections from this origin are logged at debug."
        );
    } else {
        log::debug!("[extension_bridge] rejected handshake from origin: {logged:?}");
    }
}

/// Ceiling on a logged origin's length, after control-character stripping. The
/// `Origin` header is attacker-supplied and unbounded (HTTP puts no length
/// cap on a header value beyond the server's own buffer size); every
/// legitimate origin this bridge ever compares against
/// (`chrome-extension://<32-char id>`, `moz-extension://<36-char uuid>`,
/// `null`, the native-host sentinel) is well under this, so nothing genuine is
/// ever truncated.
const MAX_LOGGED_ORIGIN_LEN: usize = 128;

/// Shape-constrain an attacker-supplied `Origin` before it enters a log line
/// support bundles ship verbatim: strip control characters (newline, CR, tab,
/// ESC/ANSI, NUL, …) so the origin cannot forge a second log line or an
/// escape-sequence trick, then cap the length so it cannot flood the log.
///
/// `sanitize_reason` (this module's sibling in [`crate::observability`]) is
/// deliberately NOT reused here: it redacts credential-*shaped* tokens, which
/// would risk mangling a legitimate-looking origin the developer needs to
/// read back exactly. Only the log-injection SHAPE is the concern for an
/// origin, not its content, so a narrower, dedicated filter is the correct
/// tool — see the review note this fixes.
///
/// Note: the current call site's `origin` already passed through
/// `http::HeaderValue::to_str()` (visible-ASCII only; the underlying HTTP
/// header parser also can never carry a raw CR/LF inside a header value —
/// that byte pair is what terminates the header line), so a raw newline
/// cannot reach here via that path today. This filter is kept anyway as a
/// caller-independent safety net — cheap, and correct even if a future
/// call site feeds it an origin from a different transport — while the
/// length cap is the concern that IS live today: nothing bounds how long an
/// `Origin` header itself can be.
fn sanitize_log_origin(origin: &str) -> String {
    let stripped: String = origin.chars().filter(|c| !c.is_control()).collect();
    if stripped.chars().count() > MAX_LOGGED_ORIGIN_LEN {
        let truncated: String = stripped.chars().take(MAX_LOGGED_ORIGIN_LEN).collect();
        format!("{truncated}…")
    } else {
        stripped
    }
}

/// Log a failed WebSocket handshake at the right level.
///
/// The 403 this module's own origin check produced is already reported in full
/// (with the origin) by [`warn_rejected_origin_once`], so re-logging it here
/// would double every attempt — which is precisely what turned one unpairable
/// extension into ~3,900 log lines in a single session. Any OTHER handshake
/// failure (a malformed request line, a torn-down connection) is genuinely new
/// information and stays at warn.
pub(super) fn log_handshake_failure(e: &tokio_tungstenite::tungstenite::Error) {
    use tokio_tungstenite::tungstenite::http::StatusCode;
    use tokio_tungstenite::tungstenite::Error;

    let forbidden = matches!(e, Error::Http(r) if r.status() == StatusCode::FORBIDDEN);
    let reason = crate::observability::sanitize_reason(&e.to_string());
    if forbidden {
        log::debug!("[extension_bridge] handshake refused (forbidden origin): {reason}");
    } else {
        log::warn!("[extension_bridge] handshake rejected/failed: {reason}");
    }
}

#[cfg(test)]
mod tests;

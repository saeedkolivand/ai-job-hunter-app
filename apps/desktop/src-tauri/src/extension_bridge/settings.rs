//! `settings.get` → `settings.result` / `settings.set` → `settings.result` (R7,
//! ADR-0009 amendment) — the extension's own opt-in switches (autofill,
//! aiAssist, autotrack), toggleable from the paired extension AND the app.
//! Extension caller ONLY: `settings.get` answers regardless of the
//! Assisted-autofill gate (it is how the user turns it on), and both verbs
//! are refused for the CLI (it already has the `Effect::Reversible` rows for
//! these same opt-ins via `agent.call`) and any other caller — see
//! `super::CallerClass`, gated at `super::advance_authenticated`.
//!
//! `settings.set` applies through the SAME `BridgeState` setters the Tauri
//! Settings commands use (`set_autofill_enabled`/`set_ai_assist`/
//! `set_autotrack_enabled`) — never a second write path — so the desktop
//! stays the single source of truth and still enforces every gate at use
//! time (a switch flipped from the extension is just as real, and just as
//! re-checkable, as one flipped in the app). Every `settings.set` that
//! actually changes a switch also raises a Notification Center entry
//! (`push_and_notify`) so a flip made from the extension is never silent
//! (R7's guard rail #3) — a request that merely re-states the switch's
//! current value is a no-op and neither re-applies nor notifies, see
//! `resolve_settings_set`'s doc. Throttled per pairing (#4) via
//! [`SettingsSetThrottle`].
//!
//! [`SettingsKey`] is deliberately the ONE place that knows the wire
//! name/getter/setter/label for each switch, so adding the fourth
//! (`saveAnswersOnSubmit`, PR4) is one variant plus one arm per method here
//! — never a second hand-typed mapping.
//!
//! [`resolve_settings_set`] takes no `AppHandle` (mirrors
//! `status_update::resolve_status_update`'s split) so it stays directly
//! unit-testable — this crate has no `tauri::test` mock-app harness; only
//! [`handle_settings_set`]'s thin notify tail needs a live `AppHandle`.

use serde_json::{json, Value};
use tauri::AppHandle;

use super::{agent_call, msg, BridgeState};

/// One of the extension's own opt-in switches, wire camelCase. Adding a
/// fourth key (`saveAnswersOnSubmit`, PR4) is one variant here plus one arm
/// in each of [`Self::from_wire`]/[`Self::wire`]/[`Self::get`]/[`Self::set`]/
/// [`Self::label`] — never a second hand-typed key list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsKey {
    Autofill,
    AiAssist,
    Autotrack,
}

impl SettingsKey {
    const ALL: [SettingsKey; 3] = [Self::Autofill, Self::AiAssist, Self::Autotrack];

    fn from_wire(s: &str) -> Option<Self> {
        match s {
            "autofill" => Some(Self::Autofill),
            "aiAssist" => Some(Self::AiAssist),
            "autotrack" => Some(Self::Autotrack),
            _ => None,
        }
    }

    fn wire(self) -> &'static str {
        match self {
            Self::Autofill => "autofill",
            Self::AiAssist => "aiAssist",
            Self::Autotrack => "autotrack",
        }
    }

    fn get(self, state: &BridgeState) -> bool {
        match self {
            Self::Autofill => state.autofill_enabled(),
            Self::AiAssist => state.ai_assist_enabled(),
            Self::Autotrack => state.autotrack_enabled(),
        }
    }

    /// Apply through the SAME setter the Tauri Settings command uses.
    fn set(self, state: &BridgeState, enabled: bool) {
        match self {
            Self::Autofill => state.set_autofill_enabled(enabled),
            Self::AiAssist => state.set_ai_assist(enabled),
            Self::Autotrack => state.set_autotrack_enabled(enabled),
        }
    }

    /// The Notification Center body's naming of which switch changed.
    fn label(self) -> &'static str {
        match self {
            Self::Autofill => "Assisted autofill",
            Self::AiAssist => "AI answer assist",
            Self::Autotrack => "Auto-track",
        }
    }
}

/// `{ autofill, aiAssist, autotrack }` off the live `BridgeState` — the same
/// shape both `settings.get` and a successful `settings.set` reply with.
fn settings_value(state: &BridgeState) -> Value {
    let mut map = serde_json::Map::new();
    for key in SettingsKey::ALL {
        map.insert(key.wire().to_string(), json!(key.get(state)));
    }
    Value::Object(map)
}

fn settings_ok_reply(req_id: &str, state: &BridgeState) -> String {
    json!({
        "type": msg::SETTINGS_RESULT,
        "reqId": req_id,
        "payload": { "ok": true, "settings": settings_value(state) },
    })
    .to_string()
}

fn settings_error_reply(req_id: &str, error: &str) -> String {
    json!({
        "type": msg::SETTINGS_RESULT,
        "reqId": req_id,
        "payload": { "ok": false, "error": error },
    })
    .to_string()
}

/// Refused for any caller that isn't the extension (the CLI, or any other
/// origin) — same fixed-sentinel discipline as `agent_read::CLI_ONLY_MESSAGE`
/// mirrored the other direction.
const ERR_EXTENSION_ONLY: &str = "extension_only";

/// `settings.get`/`settings.set` from a non-extension caller.
pub(super) fn origin_refused_reply(req_id: &str) -> String {
    settings_error_reply(req_id, ERR_EXTENSION_ONLY)
}

/// A missing/unrecognized `key`, or a non-boolean `enabled`.
const ERR_INVALID_SETTINGS_REQUEST: &str = "invalid_settings_request";

/// Answer a `settings.get`: the live switches, always `{ ok: true, settings }`
/// — no consent gate on reading the user's own device-local settings (same
/// discipline as `autotrack.check`/`autofill.check`).
pub(super) fn handle_settings_get(req_id: &str, state: &BridgeState) -> String {
    settings_ok_reply(req_id, state)
}

/// What a validated `settings.set` changed — [`handle_settings_set`]'s only
/// use for this is deciding whether to notify + building the notification
/// body; nothing here needs an `AppHandle`.
#[derive(Debug)]
struct SettingsSetOk {
    key: SettingsKey,
    enabled: bool,
    /// `false` when `enabled` already matched the switch's current value —
    /// see [`resolve_settings_set`]'s doc for why that skips the write.
    changed: bool,
}

/// The pure half of `settings.set`: validate `{ key, enabled }` and, on
/// success, APPLY it to `state` through the SAME setter the Tauri Settings
/// command uses — but only when `enabled` actually differs from the switch's
/// current value. A request that merely re-states the current value is a
/// no-op: skipping the write (and, via [`SettingsSetOk::changed`],
/// [`handle_settings_set`]'s notification) keeps "a Notification Center entry
/// per actual change" (R7's guard rail #3) true in the literal sense, not
/// once per redundant request. Takes no `AppHandle` — directly unit-testable
/// (mirrors `status_update::resolve_status_update`'s pure/impure split). The
/// write happens here (not a separate step) so a caller can never observe a
/// validated-but-not-yet-applied gap.
fn resolve_settings_set(
    state: &BridgeState,
    payload: &Value,
) -> Result<SettingsSetOk, &'static str> {
    let key = payload
        .get("key")
        .and_then(Value::as_str)
        .and_then(SettingsKey::from_wire)
        .ok_or(ERR_INVALID_SETTINGS_REQUEST)?;
    let enabled = payload
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or(ERR_INVALID_SETTINGS_REQUEST)?;
    let changed = key.get(state) != enabled;
    if changed {
        key.set(state, enabled);
    }
    Ok(SettingsSetOk {
        key,
        enabled,
        changed,
    })
}

/// Answer a `settings.set`: [`resolve_settings_set`], then — on success —
/// push a Notification Center entry naming what changed, UNLESS the request
/// was a no-op ([`SettingsSetOk::changed`] is `false`), and reply with the
/// full (possibly unchanged) `settings` object either way. A malformed
/// request is refused before anything is written or notified.
pub(super) fn handle_settings_set(
    app: &AppHandle,
    req_id: &str,
    state: &BridgeState,
    payload: &Value,
) -> String {
    match resolve_settings_set(state, payload) {
        Ok(ok) => {
            if ok.changed {
                crate::commands::notifications::push_and_notify(
                    app,
                    crate::notifications::NewNotification {
                        kind: "extension.settings".to_string(),
                        title: "Browser extension changed a setting".to_string(),
                        body: format!(
                            "{} turned {} from the browser extension",
                            ok.key.label(),
                            if ok.enabled { "on" } else { "off" }
                        ),
                        route: Some(crate::notifications::NotificationRoute {
                            to: "/settings".to_string(),
                            search: None,
                        }),
                    },
                    crate::commands::notifications::OsBanner::WhenUnfocused,
                );
            }
            settings_ok_reply(req_id, state)
        }
        Err(sentinel) => settings_error_reply(req_id, sentinel),
    }
}

// ── Throttle (on BridgeState, per pairing — same reconnect-proof reasoning
// as `match_live::MatchLiveThrottle`, deliberately its OWN instance rather
// than a shared one — see that struct's doc for why a future verb never
// reuses another verb's bucket). ────────────────────────────────────────────

/// Requests allowed in quick succession before the bucket empties. A user
/// flipping several switches in the Settings page in one sitting must not be
/// throttled; an automated flood of `settings.set` frames must be bounded.
const SETTINGS_SET_BURST: f64 = 5.0;
/// Seconds to refill one token.
const SETTINGS_SET_REFILL_SECS: f64 = 2.0;

pub(super) struct SettingsSetThrottle {
    tokens: f64,
    last: std::time::Instant,
}

impl SettingsSetThrottle {
    pub(super) fn new() -> Self {
        Self {
            tokens: SETTINGS_SET_BURST,
            last: std::time::Instant::now(),
        }
    }

    fn try_acquire_at(&mut self, now: std::time::Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed / SETTINGS_SET_REFILL_SECS).min(SETTINGS_SET_BURST);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub(super) fn try_acquire(&mut self) -> bool {
        self.try_acquire_at(std::time::Instant::now())
    }
}

pub(super) fn throttled_reply(req_id: &str) -> String {
    settings_error_reply(req_id, agent_call::ERR_RATE_LIMITED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn state() -> (tempfile::TempDir, BridgeState) {
        let dir = tempfile::tempdir().unwrap();
        let state = BridgeState::load(dir.path());
        (dir, state)
    }

    #[test]
    fn settings_get_reports_all_three_defaults_off() {
        let (_dir, state) = state();
        let reply = handle_settings_get("req-1", &state);
        let v: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(v["type"], msg::SETTINGS_RESULT);
        assert_eq!(v["payload"]["ok"], true);
        assert_eq!(v["payload"]["settings"]["autofill"], false);
        assert_eq!(v["payload"]["settings"]["aiAssist"], false);
        assert_eq!(v["payload"]["settings"]["autotrack"], false);
    }

    #[test]
    fn resolve_settings_set_applies_through_the_same_setter() {
        let (_dir, state) = state();
        let ok = resolve_settings_set(&state, &json!({ "key": "autofill", "enabled": true }))
            .expect("valid request");
        assert!(ok.enabled);
        assert!(ok.changed, "the switch actually flipped");
        assert!(state.autofill_enabled(), "the same BridgeState setter ran");
        assert!(!state.ai_assist_enabled(), "only the named key changed");
        assert!(!state.autotrack_enabled());
    }

    /// Setting a key to its own current value is a no-op: no re-apply
    /// ([`SettingsSetOk::changed`] is `false`) and — pinned the same
    /// source-text way [`handle_settings_set_notifies_on_a_successful_change`]
    /// pins the opposite branch, since this crate has no `tauri::test` mock-app
    /// harness to actually invoke [`handle_settings_set`] and observe whether
    /// `push_and_notify` ran — [`handle_settings_set`]'s notify call is gated
    /// on `ok.changed`, not unconditional.
    #[test]
    fn resolve_settings_set_no_op_does_not_reapply_or_notify() {
        let (_dir, state) = state();
        // Every switch defaults to off — requesting `false` again is a no-op.
        let ok = resolve_settings_set(&state, &json!({ "key": "autofill", "enabled": false }))
            .expect("valid request");
        assert!(
            !ok.changed,
            "requesting the current value must not report a change"
        );
        assert!(!state.autofill_enabled());

        let src = include_str!("settings.rs");
        let start = src
            .find("pub(super) fn handle_settings_set")
            .expect("handle_settings_set must still exist under this exact signature");
        let body = &src[start..];
        let end = body
            .find("\n}\n")
            .expect("handle_settings_set's closing brace")
            + "\n}\n".len();
        let body = &body[..end];
        let notify_idx = body
            .find("push_and_notify(")
            .expect("handle_settings_set must still call push_and_notify");
        let guard_idx = body
            .find("if ok.changed")
            .expect("the notify call must be gated on SettingsSetOk::changed");
        assert!(
            guard_idx < notify_idx,
            "push_and_notify must sit inside the `if ok.changed` guard, not unconditional"
        );
    }

    #[test]
    fn resolve_settings_set_covers_every_key() {
        let (_dir, state) = state();
        for (wire, get, enabled) in [
            (
                "autofill",
                (|s: &BridgeState| s.autofill_enabled()) as fn(&BridgeState) -> bool,
                true,
            ),
            ("aiAssist", (|s: &BridgeState| s.ai_assist_enabled()), true),
            ("autotrack", (|s: &BridgeState| s.autotrack_enabled()), true),
        ] {
            resolve_settings_set(&state, &json!({ "key": wire, "enabled": enabled })).unwrap();
            assert!(get(&state), "key {wire} did not apply");
        }
    }

    #[test]
    fn resolve_settings_set_rejects_an_unknown_key_without_writing_anything() {
        let (_dir, state) = state();
        let err = resolve_settings_set(&state, &json!({ "key": "notARealKey", "enabled": true }))
            .unwrap_err();
        assert_eq!(err, ERR_INVALID_SETTINGS_REQUEST);
        assert!(!state.autofill_enabled());
        assert!(!state.ai_assist_enabled());
        assert!(!state.autotrack_enabled());
    }

    #[test]
    fn resolve_settings_set_rejects_a_non_boolean_enabled() {
        let (_dir, state) = state();
        let err = resolve_settings_set(&state, &json!({ "key": "autofill", "enabled": "yes" }))
            .unwrap_err();
        assert_eq!(err, ERR_INVALID_SETTINGS_REQUEST);
        assert!(!state.autofill_enabled());
    }

    #[test]
    fn resolve_settings_set_rejects_a_missing_key() {
        let (_dir, state) = state();
        let err = resolve_settings_set(&state, &json!({ "enabled": true })).unwrap_err();
        assert_eq!(err, ERR_INVALID_SETTINGS_REQUEST);
    }

    #[test]
    fn origin_refused_reply_carries_the_extension_only_sentinel() {
        let v: Value = serde_json::from_str(&origin_refused_reply("req-5")).unwrap();
        assert_eq!(v["type"], msg::SETTINGS_RESULT);
        assert_eq!(v["payload"]["ok"], false);
        assert_eq!(v["payload"]["error"], ERR_EXTENSION_ONLY);
    }

    #[test]
    fn throttled_reply_carries_the_shared_rate_limited_sentinel() {
        let v: Value = serde_json::from_str(&throttled_reply("req-6")).unwrap();
        assert_eq!(v["payload"]["ok"], false);
        assert_eq!(v["payload"]["error"], agent_call::ERR_RATE_LIMITED);
    }

    /// [`handle_settings_set`] needs a live `AppHandle` to call `push_and_notify` — this crate has
    /// no `tauri::test` mock-app harness (see `resolve_settings_set`'s tests above for the
    /// pure half this crate CAN drive directly). Mirrors `agent_read::tests`'
    /// `handle_agent_query_routes_its_reply_through_the_frame_capped_builder`: a source-text scan
    /// pins that the success branch still calls `push_and_notify` exactly once, so R7's "never
    /// silent" guard rail can't silently regress to a write with no notification.
    #[test]
    fn handle_settings_set_notifies_on_a_successful_change() {
        let src = include_str!("settings.rs");
        let start = src
            .find("pub(super) fn handle_settings_set")
            .expect("handle_settings_set must still exist under this exact signature");
        let body = &src[start..];
        let end = body
            .find("\n}\n")
            .expect("handle_settings_set's closing brace")
            + "\n}\n".len();
        let body = &body[..end];
        assert_eq!(
            body.matches("push_and_notify(").count(),
            1,
            "a successful settings.set must push exactly one Notification Center entry"
        );
    }

    #[test]
    fn settings_set_throttle_empties_then_refills() {
        let mut throttle = SettingsSetThrottle::new();
        let t0 = Instant::now();
        for _ in 0..SETTINGS_SET_BURST as u32 {
            assert!(throttle.try_acquire_at(t0));
        }
        assert!(
            !throttle.try_acquire_at(t0),
            "burst exhausted — the next request must be refused"
        );
        let refilled = t0 + Duration::from_secs_f64(SETTINGS_SET_REFILL_SECS);
        assert!(
            throttle.try_acquire_at(refilled),
            "one refill period later, one token is available again"
        );
    }
}

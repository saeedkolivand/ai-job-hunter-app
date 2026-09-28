//! `settings.get` → `settings.result` (R7, ADR-0009 amendment) — the extension's own opt-in
//! switches (autofill, aiAssist, autotrack, saveAnswersOnSubmit), toggleable from the paired
//! extension AND the app. Extension caller ONLY: `settings.get` answers regardless of the
//! Assisted-autofill gate (it is how the user turns it on), and is refused for the CLI (it
//! already has the `Effect::Reversible` rows for these same opt-ins via `agent.call`) and any
//! other caller — see `super::CallerClass`, gated at `super::advance_authenticated`.
//!
//! [`SettingsKey`] is deliberately the ONE place that knows the wire name/getter/setter/label for
//! each switch — `saveAnswersOnSubmit` (PR4) is one variant plus one arm per method here, never a
//! second hand-typed mapping. This module also owns the shared reply builders
//! ([`settings_ok_reply`]/[`settings_error_reply`]) and the per-pairing throttle
//! ([`SettingsSetThrottle`]) both verbs use. The write half — `settings.set`'s
//! validate-then-apply-then-notify path — lives in the sibling `settings_set` module (R8 relief);
//! see its own module doc.

use serde_json::{json, Value};

use super::{agent_call, msg, BridgeState};

/// One of the extension's own opt-in switches, wire camelCase. Adding a
/// fourth key (`saveAnswersOnSubmit`, PR4) is one variant here plus one arm
/// in each of [`Self::from_wire`]/[`Self::wire`]/[`Self::get`]/[`Self::set`]/
/// [`Self::label`] — never a second hand-typed key list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsKey {
    Autofill,
    AiAssist,
    Autotrack,
    SaveAnswersOnSubmit,
}

impl SettingsKey {
    pub(super) const ALL: [SettingsKey; 4] = [
        Self::Autofill,
        Self::AiAssist,
        Self::Autotrack,
        Self::SaveAnswersOnSubmit,
    ];

    pub(super) fn from_wire(s: &str) -> Option<Self> {
        match s {
            "autofill" => Some(Self::Autofill),
            "aiAssist" => Some(Self::AiAssist),
            "autotrack" => Some(Self::Autotrack),
            "saveAnswersOnSubmit" => Some(Self::SaveAnswersOnSubmit),
            _ => None,
        }
    }

    fn wire(self) -> &'static str {
        match self {
            Self::Autofill => "autofill",
            Self::AiAssist => "aiAssist",
            Self::Autotrack => "autotrack",
            Self::SaveAnswersOnSubmit => "saveAnswersOnSubmit",
        }
    }

    fn get(self, state: &BridgeState) -> bool {
        match self {
            Self::Autofill => state.autofill_enabled(),
            Self::AiAssist => state.ai_assist_enabled(),
            Self::Autotrack => state.autotrack_enabled(),
            Self::SaveAnswersOnSubmit => state.save_answers_on_submit_enabled(),
        }
    }

    /// Apply through the SAME setter the Tauri Settings command uses, and
    /// return whether it actually changed the value. The setter itself is
    /// now the one critical section that compares-against-current-value AND
    /// writes (`BridgeState::optin_write_lock`), so this is a pure
    /// passthrough of that outcome — not a second, separately-racy compare.
    pub(super) fn set(self, state: &BridgeState, enabled: bool) -> bool {
        match self {
            Self::Autofill => state.set_autofill_enabled(enabled),
            Self::AiAssist => state.set_ai_assist(enabled),
            Self::Autotrack => state.set_autotrack_enabled(enabled),
            Self::SaveAnswersOnSubmit => state.set_save_answers_on_submit_enabled(enabled),
        }
    }

    /// The Notification Center body's naming of which switch changed.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Autofill => "Assisted autofill",
            Self::AiAssist => "AI answer assist",
            Self::Autotrack => "Auto-track",
            Self::SaveAnswersOnSubmit => "Save answers on submit",
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

pub(super) fn settings_ok_reply(req_id: &str, state: &BridgeState) -> String {
    json!({
        "type": msg::SETTINGS_RESULT,
        "reqId": req_id,
        "payload": { "ok": true, "settings": settings_value(state) },
    })
    .to_string()
}

pub(super) fn settings_error_reply(req_id: &str, error: &str) -> String {
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

/// Answer a `settings.get`: the live switches, always `{ ok: true, settings }`
/// — no consent gate on reading the user's own device-local settings (same
/// discipline as `autotrack.check`/`autofill.check`).
pub(super) fn handle_settings_get(req_id: &str, state: &BridgeState) -> String {
    settings_ok_reply(req_id, state)
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
mod tests;

//! `settings.set` → `settings.result` — the write half of R7's settings surface. Split from
//! `settings.rs` (R8 relief): that file keeps `SettingsKey` (the shared wire vocabulary),
//! `settings.get`, the shared reply builders, and the per-pairing throttle; this file owns only
//! the validate-then-apply-then-notify path for `settings.set` — see `settings`'s own module doc
//! for the full R7/ADR-0009 design (single write path, no second compare, guard rail #3).

use serde_json::Value;
use tauri::AppHandle;

use super::settings::{settings_error_reply, settings_ok_reply, SettingsKey};
use super::BridgeState;

/// A missing/unrecognized `key`, or a non-boolean `enabled`.
const ERR_INVALID_SETTINGS_REQUEST: &str = "invalid_settings_request";

/// What a validated `settings.set` changed — [`notification_for`]'s only
/// use for this is deciding whether to notify + building the notification
/// body; nothing here needs an `AppHandle`.
#[derive(Debug)]
struct SettingsSetOk {
    key: SettingsKey,
    enabled: bool,
    /// `false` when `enabled` already matched the switch's current value —
    /// see [`resolve_settings_set`]'s doc for why the SETTER still persists
    /// regardless (only the notification below is conditioned on this).
    changed: bool,
}

/// The pure half of `settings.set`: validate `{ key, enabled }` and, on
/// success, APPLY it to `state` through the SAME setter the Tauri Settings
/// command uses. The compare-against-current-value + write is now ONE
/// critical section INSIDE that setter (`BridgeState::optin_write_lock`) —
/// this function no longer reads-then-decides itself, so a concurrent writer
/// (a second paired browser's `settings.set`, or the desktop Settings command
/// on another thread) can never land between this function's own read and
/// write. [`SettingsSetOk::changed`] is exactly the setter's return value.
/// The setter itself still persists unconditionally (even on a no-op value —
/// see `BridgeState::set_autofill_enabled`'s doc for why); it is only
/// [`handle_settings_set`]'s NOTIFICATION that `changed` gates, which is what
/// keeps "a Notification Center entry per actual change" (R7's guard rail #3)
/// true in the literal sense, not once per redundant request. Takes no
/// `AppHandle` — directly unit-testable (mirrors
/// `status_update::resolve_status_update`'s pure/impure split).
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
    let changed = key.set(state, enabled);
    Ok(SettingsSetOk {
        key,
        enabled,
        changed,
    })
}

/// Notification Center title for a switch flipped from the extension side —
/// pulled out as a constant so [`notification_for`]'s tests assert against
/// it instead of duplicating the literal.
const SETTINGS_CHANGED_TITLE: &str = "Browser extension changed a setting";

/// The Notification Center entry a validated `settings.set` should raise, or
/// `None` when the request was a no-op ([`SettingsSetOk::changed`] is
/// `false`) — keeps "a Notification Center entry per actual change" (R7's
/// guard rail #3) true in the literal sense, not once per redundant request.
/// Pure — no `AppHandle` — so it is directly unit-testable; only turning the
/// result into a live push (`push_and_notify`) needs one.
fn notification_for(ok: &SettingsSetOk) -> Option<crate::notifications::NewNotification> {
    if !ok.changed {
        return None;
    }
    Some(crate::notifications::NewNotification {
        kind: "extension.settings".to_string(),
        title: SETTINGS_CHANGED_TITLE.to_string(),
        body: format!(
            "{} turned {} from the browser extension",
            ok.key.label(),
            if ok.enabled { "on" } else { "off" }
        ),
        route: Some(crate::notifications::NotificationRoute {
            to: "/settings".to_string(),
            search: None,
        }),
    })
}

/// Answer a `settings.set`: [`resolve_settings_set`], then — on success —
/// push whatever [`notification_for`] returns (nothing for a no-op), and
/// reply with the full (possibly unchanged) `settings` object either way. A
/// malformed request is refused before anything is written or notified.
pub(super) fn handle_settings_set(
    app: &AppHandle,
    req_id: &str,
    state: &BridgeState,
    payload: &Value,
) -> String {
    match resolve_settings_set(state, payload) {
        Ok(ok) => {
            if let Some(notification) = notification_for(&ok) {
                crate::commands::notifications::push_and_notify(
                    app,
                    notification,
                    crate::commands::notifications::OsBanner::WhenUnfocused,
                );
            }
            settings_ok_reply(req_id, state)
        }
        Err(sentinel) => settings_error_reply(req_id, sentinel),
    }
}

#[cfg(test)]
mod tests;

//! Extension-bridge control commands — the renderer's IPC surface over the local
//! WebSocket bridge ([`crate::extension_bridge`]).
//!
//! `status` (bound port + whether an extension is paired + the current token)
//! and `regenerate_token` (rotate the pairing secret; existing sockets must
//! re-pair), plus two independent opt-in pairs — assisted autofill
//! (`autofill_enabled`/`set_autofill_enabled`) and AI answer-assist
//! (`ai_assist_enabled`/`set_ai_assist_enabled`, a SEPARATE gate: billable
//! provider spend is a different consent class from the local/free autofill
//! verbs). All resolve the managed [`BridgeState`] from app state and return a
//! `serde_json::Value`, matching the neighbouring command style (e.g.
//! `commands::applications`).

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::extension_bridge::BridgeState;

/// `{ port, connected, token }`. `port` is `null` when the bridge failed to bind
/// (disabled). When `BridgeState` isn't managed at all (start-up failure before
/// `manage`), report a disabled bridge with an empty token rather than erroring.
#[tauri::command]
pub async fn extension_bridge_status(app: AppHandle) -> Value {
    match app.try_state::<BridgeState>() {
        Some(state) => json!({
            "port": state.port(),
            "connected": state.is_connected(),
            "token": state.token(),
        }),
        None => json!({ "port": Value::Null, "connected": false, "token": "" }),
    }
}

/// Rotate the pairing token and return the new value: `{ token }`. If the bridge
/// state is unavailable, return an empty token (nothing to rotate).
#[tauri::command]
pub async fn extension_bridge_regenerate_token(app: AppHandle) -> Value {
    match app.try_state::<BridgeState>() {
        Some(state) => json!({ "token": state.regenerate_token() }),
        None => json!({ "token": "" }),
    }
}

/// Current assisted-autofill opt-in: `{ enabled }`. Default OFF when the bridge
/// state isn't managed (start-up failure) — the safe state.
#[tauri::command]
pub async fn extension_bridge_autofill_enabled(app: AppHandle) -> Value {
    let enabled = app
        .try_state::<BridgeState>()
        .map(|s| s.autofill_enabled())
        .unwrap_or(false);
    json!({ "enabled": enabled })
}

/// Set (and persist) the assisted-autofill opt-in; echoes the stored value:
/// `{ enabled }`. A no-op returning `false` when the bridge state is unavailable.
#[tauri::command]
pub async fn extension_bridge_set_autofill_enabled(app: AppHandle, enabled: bool) -> Value {
    match app.try_state::<BridgeState>() {
        Some(state) => {
            state.set_autofill_enabled(enabled);
            json!({ "enabled": state.autofill_enabled() })
        }
        None => json!({ "enabled": false }),
    }
}

/// Current AI-answer-assist opt-in: `{ enabled }` — a SEPARATE opt-in from
/// `extension_bridge_autofill_enabled` (billable provider spend is a
/// materially different consent class). No provider/model snapshot is
/// surfaced: a draft resolves the active provider from the backend
/// `AiConfigStore` at answer-time (task #16), so Settings reads that store
/// (`ai_active_config`) for its "Using: X · Y" label. Default OFF when the
/// bridge state isn't managed (start-up failure) — the safe state.
#[tauri::command]
pub async fn extension_bridge_ai_assist_enabled(app: AppHandle) -> Value {
    let enabled = app
        .try_state::<BridgeState>()
        .map(|s| s.ai_assist_enabled())
        .unwrap_or(false);
    json!({ "enabled": enabled })
}

/// Set (and persist) the AI-answer-assist opt-in; echoes the stored value:
/// `{ enabled }`. A bare boolean — the billable-AI consent gate (ADR-0011).
/// It no longer snapshots the renderer's active provider: a draft resolves
/// the active provider from the backend `AiConfigStore` via
/// `Completer::from_active` at answer-time (task #16). A no-op returning
/// `false` when the bridge state is unavailable.
#[tauri::command]
pub async fn extension_bridge_set_ai_assist_enabled(app: AppHandle, enabled: bool) -> Value {
    match app.try_state::<BridgeState>() {
        Some(state) => {
            state.set_ai_assist(enabled);
            json!({ "enabled": state.ai_assist_enabled() })
        }
        None => json!({ "enabled": false }),
    }
}

/// Current auto-track opt-in: `{ enabled }` (Task #22) — default OFF. Gates the
/// extension's gesture submit-watcher (the extension reads it via
/// `autotrack.check`) AND the desktop honoring an AUTO `status.update`. Default
/// OFF when the bridge state isn't managed (start-up failure) — the safe state.
#[tauri::command]
pub async fn extension_bridge_auto_track_enabled(app: AppHandle) -> Value {
    let enabled = app
        .try_state::<BridgeState>()
        .map(|s| s.autotrack_enabled())
        .unwrap_or(false);
    json!({ "enabled": enabled })
}

/// Set (and persist) the auto-track opt-in; echoes the stored value:
/// `{ enabled }`. A bare boolean — the auto-mark-applied consent gate. A no-op
/// returning `false` when the bridge state is unavailable.
#[tauri::command]
pub async fn extension_bridge_set_auto_track_enabled(app: AppHandle, enabled: bool) -> Value {
    match app.try_state::<BridgeState>() {
        Some(state) => {
            state.set_autotrack_enabled(enabled);
            json!({ "enabled": state.autotrack_enabled() })
        }
        None => json!({ "enabled": false }),
    }
}

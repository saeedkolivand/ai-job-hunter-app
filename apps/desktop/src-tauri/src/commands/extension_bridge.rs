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

/// Current AI-answer-assist opt-in: `{ enabled, provider?, model? }` — a
/// SEPARATE opt-in from `extension_bridge_autofill_enabled` (billable
/// provider spend is a materially different consent class). `provider`/
/// `model` are the pinned snapshot (present only while `enabled`) so the
/// Settings row can show which provider/model answer drafts will actually
/// use. Default OFF/no snapshot when the bridge state isn't managed
/// (start-up failure) — the safe state.
#[tauri::command]
pub async fn extension_bridge_ai_assist_enabled(app: AppHandle) -> Value {
    match app.try_state::<BridgeState>() {
        Some(state) => {
            let cfg = state.ai_assist_snapshot();
            json!({ "enabled": cfg.enabled, "provider": cfg.provider, "model": cfg.model })
        }
        None => json!({ "enabled": false }),
    }
}

/// Set (and persist) the AI-answer-assist opt-in; echoes the stored value:
/// `{ enabled, provider?, model? }`. `provider`/`model`/`baseUrl` snapshot the
/// renderer's CURRENT active AI provider (see `useGenerateConfig`) — the
/// bridge is a headless context with no renderer to read it from at
/// answer-time, so the toggle must capture it up front (mirrors
/// `Autopilot::assistant_provider`'s pattern; see
/// `extension_bridge::answer_assist`'s module doc). Turning the opt-in OFF
/// clears the snapshot. A no-op returning `false` when the bridge state is
/// unavailable.
#[tauri::command]
pub async fn extension_bridge_set_ai_assist_enabled(
    app: AppHandle,
    enabled: bool,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
) -> Value {
    match app.try_state::<BridgeState>() {
        Some(state) => {
            state.set_ai_assist(enabled, provider, model, base_url);
            let cfg = state.ai_assist_snapshot();
            json!({ "enabled": cfg.enabled, "provider": cfg.provider, "model": cfg.model })
        }
        None => json!({ "enabled": false }),
    }
}

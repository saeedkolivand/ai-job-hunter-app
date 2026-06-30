//! Extension-bridge control commands — the renderer's IPC surface over the local
//! WebSocket bridge ([`crate::extension_bridge`]).
//!
//! Two read/rotate capabilities (no creation triggers): `status` (bound port +
//! whether an extension is paired + the current token) and `regenerate_token`
//! (rotate the pairing secret; existing sockets must re-pair). Both resolve the
//! managed [`BridgeState`] from app state and return a `serde_json::Value`,
//! matching the neighbouring command style (e.g. `commands::applications`).

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

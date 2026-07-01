//! Centralized Tauri-event emit layer (L3 shell). One helper + the generated
//! channel constants (source of truth: packages/shared/src/events/).
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub use crate::ipc_contracts::event_payloads::*;
pub use crate::ipc_contracts::events::*;

/// Emit an app event to all windows. The one place `app.emit` for app events
/// lives, generalizing the old emit_stream_error/emit_changed/dispatch helpers.
pub fn emit_event(app: &AppHandle, channel: &str, payload: impl Serialize + Clone) {
    let _ = app.emit(channel, payload);
}

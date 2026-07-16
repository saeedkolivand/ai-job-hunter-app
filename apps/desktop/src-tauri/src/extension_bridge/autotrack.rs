//! Auto-track opt-in (Task #22) — persistence, the `BridgeState` accessors,
//! and the `autotrack.check` reply. Split out of `mod.rs` to keep that module
//! under the R8 hard LOC cap (`tests/architecture.rs`); mirrors
//! `status_update.rs`'s split — the `BridgeState` field itself and the
//! `FrameDecision::AutotrackCheck` dispatch stay in the parent module (they're
//! part of the connection state machine), while this module holds the
//! opt-in's persistence + the accessors' bodies (a second `impl BridgeState`
//! block — legal since a private field stays visible to the defining module's
//! descendants) and the reply builder.

use std::path::Path;
use std::sync::atomic::Ordering;

use serde_json::json;

use super::{msg, BridgeState};

/// File under the app data dir holding the auto-track opt-in flag (`"1"` = on,
/// anything else / absent = off), persisted beside `AUTOFILL_OPTIN_FILE`.
/// Default OFF: the desktop arms nothing on its own — the extension only reads
/// this (via `autotrack.check`) to decide whether to arm its gesture
/// submit-watcher, and the desktop re-checks it before honoring an AUTO
/// `status.update` (Task #22). This is the consent gate for auto-marking a job
/// `applied` from a detected form submit.
const AUTOTRACK_OPTIN_FILE: &str = "extension_autotrack_optin";

impl BridgeState {
    /// Whether auto-track is opted in (Task #22). Read by the `autotrack.check`
    /// verb (so the extension can gate ARMING its submit-watcher) AND
    /// re-checked before honoring an AUTO `status.update` — see
    /// `crate::extension_bridge::status_update::auto_write_refused`.
    pub fn autotrack_enabled(&self) -> bool {
        self.autotrack_enabled.load(Ordering::Relaxed)
    }

    /// Set (and persist) the auto-track opt-in. A persist failure is non-fatal
    /// but leaves the in-memory value authoritative for this run — same
    /// discipline as `set_autofill_enabled`.
    pub fn set_autotrack_enabled(&self, enabled: bool) {
        self.autotrack_enabled.store(enabled, Ordering::Relaxed);
        if let Err(e) = persist_autotrack_optin(&self.data_dir, enabled) {
            log::warn!("[extension_bridge] failed to persist auto-track opt-in (non-fatal): {e}");
        }
    }
}

/// Read the persisted auto-track opt-in (`"1"` ⇒ on). Absent / any other value
/// ⇒ OFF, so a first run and a corrupt flag both default to the safe (off)
/// state — mirrors `load_autofill_optin`'s degrade-to-off discipline.
pub(super) fn load_autotrack_optin(data_dir: &Path) -> bool {
    std::fs::read_to_string(data_dir.join(AUTOTRACK_OPTIN_FILE))
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

pub(super) fn persist_autotrack_optin(data_dir: &Path, enabled: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(
        data_dir.join(AUTOTRACK_OPTIN_FILE),
        if enabled { "1" } else { "0" },
    )
}

/// Build the `autotrack.result` reply (Task #22): the current auto-track opt-in
/// as `{ enabled }`. A pure read — no consent gate on READING the user's own
/// device-local setting (the enforced boundary is the AUTO `status.update` write).
pub(super) fn autotrack_result_reply(req_id: &str, enabled: bool) -> String {
    json!({
        "type": msg::AUTOTRACK_RESULT,
        "reqId": req_id,
        "payload": { "enabled": enabled },
    })
    .to_string()
}

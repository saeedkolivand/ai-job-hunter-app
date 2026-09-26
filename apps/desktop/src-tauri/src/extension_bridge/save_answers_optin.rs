//! `saveAnswersOnSubmit` opt-in — persistence + the `BridgeState` accessors. The `BridgeState`
//! field itself and its initialization stay in `mod.rs`; this module holds the opt-in's
//! persistence and the accessors' bodies (another `impl BridgeState` block).
//!
//! This is its OWN consent class, distinct from auto-marking `applied`: saving the answers a user
//! typed at submit time is triggered by a detected event rather than a click, and writes
//! page-derived answer TEXT rather than flipping one status value. Server-side enforcement lives
//! in `answers_save::auto_save_refused` — the extension's own client-side check is defense-in-depth
//! only.

use std::path::Path;
use std::sync::atomic::Ordering;

use super::BridgeState;
use crate::platform::fs::write_atomic;

/// File under the app data dir holding the `saveAnswersOnSubmit` opt-in flag (`"1"` = on,
/// anything else / absent = off), persisted beside `AUTOTRACK_OPTIN_FILE`. Default OFF: an
/// AUTO-flagged `answers.save` is refused (see `answers_save::auto_save_refused`) until the user
/// turns this on from Settings or the extension's options page.
const SAVE_ANSWERS_ON_SUBMIT_OPTIN_FILE: &str = "extension_save_answers_on_submit_optin";

impl BridgeState {
    /// Whether saving the answers typed at submit time is opted in. Read by
    /// `settings.get`/`settings.set` and re-checked before honoring an AUTO `answers.save` — see
    /// `answers_save::auto_save_refused`.
    pub fn save_answers_on_submit_enabled(&self) -> bool {
        self.save_answers_on_submit_enabled.load(Ordering::Relaxed)
    }

    /// Set (and persist) the `saveAnswersOnSubmit` opt-in; returns `true` iff this call actually
    /// changed the value.
    ///
    /// **Persist-before-publish** (unlike the sibling opt-in setters, which swap-then-persist):
    /// the disk write happens FIRST, via [`persist_save_answers_on_submit_optin`]'s
    /// write-then-rename, and the in-memory atomic is only updated once that succeeds. A failed
    /// write leaves memory matching whatever is STILL on disk, never ahead of it. The previous
    /// swap-first order let a disable that failed to persist flip memory to `false` while the file
    /// still read `"1"`; restart then re-enabled automatic saving with no further action from the
    /// user.
    pub fn set_save_answers_on_submit_enabled(&self, enabled: bool) -> bool {
        let _guard = self.optin_write_lock.lock();
        let prev = self.save_answers_on_submit_enabled.load(Ordering::Relaxed);
        if let Err(e) = persist_save_answers_on_submit_optin(&self.data_dir, enabled) {
            log::warn!(
                "[extension_bridge] failed to persist save-answers-on-submit opt-in — consent \
                 left unchanged in memory too, matching what is still on disk: {}",
                crate::observability::sanitize_reason(&e.to_string())
            );
            return false;
        }
        self.save_answers_on_submit_enabled
            .store(enabled, Ordering::Relaxed);
        prev != enabled
    }
}

/// Read the persisted opt-in (`"1"` ⇒ on). Absent / any other value ⇒ OFF — mirrors
/// `load_autotrack_optin`'s degrade-to-off discipline.
pub(super) fn load_save_answers_on_submit_optin(data_dir: &Path) -> bool {
    std::fs::read_to_string(data_dir.join(SAVE_ANSWERS_ON_SUBMIT_OPTIN_FILE))
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

/// Atomic write using the shared helper: writes to a sibling `.tmp` file,
/// syncs it, then renames — so a crash or kill at any point leaves either the
/// old file or the new one, never a truncated or zero-filled one.
pub(super) fn persist_save_answers_on_submit_optin(
    data_dir: &Path,
    enabled: bool,
) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join(SAVE_ANSWERS_ON_SUBMIT_OPTIN_FILE);
    write_atomic(&path, if enabled { b"1" } else { b"0" })
}

#[cfg(test)]
mod tests;

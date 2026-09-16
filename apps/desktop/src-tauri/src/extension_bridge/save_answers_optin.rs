//! `saveAnswersOnSubmit` opt-in (PR4) — persistence + the `BridgeState` accessors. Split into its
//! own file per the established one-file-per-flag pattern (mirrors `autotrack.rs`'s split): the
//! `BridgeState` field itself and its initialization stay in `mod.rs` (part of the struct/state
//! machine), while this module holds the opt-in's persistence and the accessors' bodies (another
//! `impl BridgeState` block — legal since a private field stays visible to the defining module's
//! descendants).
//!
//! This is its OWN consent class — a further amendment to 0009's Task #22 auto-track amendment:
//! saving the answers a user typed at submit time is a distinct decision from auto-marking
//! `applied`, because the trigger is a detected event rather than a click and it writes
//! page-derived answer TEXT rather than flipping one status value. Server-side enforcement lives
//! in `answers_save::auto_save_refused` (mirrors `status_update::auto_write_refused` exactly) —
//! the extension's own client-side check is defense-in-depth only.

use std::path::Path;
use std::sync::atomic::Ordering;

use super::BridgeState;

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
    /// changed the value. Same critical section + always-persist discipline as
    /// `set_autotrack_enabled` (see that method's doc for why the write itself is unconditional).
    pub fn set_save_answers_on_submit_enabled(&self, enabled: bool) -> bool {
        let _guard = self.optin_write_lock.lock();
        let prev = self
            .save_answers_on_submit_enabled
            .swap(enabled, Ordering::Relaxed);
        if let Err(e) = persist_save_answers_on_submit_optin(&self.data_dir, enabled) {
            log::warn!(
                "[extension_bridge] failed to persist save-answers-on-submit opt-in (non-fatal): {}",
                crate::observability::sanitize_reason(&e.to_string())
            );
        }
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

pub(super) fn persist_save_answers_on_submit_optin(
    data_dir: &Path,
    enabled: bool,
) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(
        data_dir.join(SAVE_ANSWERS_ON_SUBMIT_OPTIN_FILE),
        if enabled { "1" } else { "0" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_off_and_round_trips_through_the_setter() {
        let dir = tempfile::tempdir().unwrap();
        let state = BridgeState::load(dir.path());
        assert!(!state.save_answers_on_submit_enabled());

        assert!(state.set_save_answers_on_submit_enabled(true));
        assert!(state.save_answers_on_submit_enabled());
        assert!(load_save_answers_on_submit_optin(dir.path()));

        // Re-setting the same value reports no change, mirrors every sibling flag's setter.
        assert!(!state.set_save_answers_on_submit_enabled(true));
    }
}

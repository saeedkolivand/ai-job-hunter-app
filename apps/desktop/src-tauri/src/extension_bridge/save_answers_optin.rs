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
    /// changed the value.
    ///
    /// **Persist-before-publish** (PR #1209 review — unlike the three sibling opt-in setters,
    /// which still swap-then-persist; see this method's own doc note below for why THIS one
    /// diverges): the disk write happens FIRST, via [`persist_save_answers_on_submit_optin`]'s
    /// write-then-rename, and the in-memory atomic is only updated once that succeeds. A failed
    /// write leaves memory matching whatever is STILL on disk, never ahead of it — so a write
    /// failure while disabling can never leave a MORE permissive value on disk than what this
    /// session's memory (and the caller's reply) reports. The previous swap-first order let a
    /// disable that failed to persist flip memory to `false` while the file still read `"1"`;
    /// restart then re-enabled automatic saving with no further action from the user.
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

/// Write-then-rename (mirrors `postings::PostingStore::save`): the target file is only ever
/// REPLACED, never truncated in place, so a write that fails partway (disk full, a crash) leaves
/// the previous persisted value intact rather than a corrupt or empty one — required for
/// [`BridgeState::set_save_answers_on_submit_enabled`]'s persist-before-publish order to actually
/// mean something (an `Ok` here must be trustworthy before memory ever moves).
pub(super) fn persist_save_answers_on_submit_optin(
    data_dir: &Path,
    enabled: bool,
) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join(SAVE_ANSWERS_ON_SUBMIT_OPTIN_FILE);
    let tmp = path.with_extension("tmp");
    if let Err(e) = std::fs::write(&tmp, if enabled { "1" } else { "0" }) {
        std::fs::remove_file(&tmp).ok();
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, &path) {
        std::fs::remove_file(&tmp).ok();
        return Err(e);
    }
    Ok(())
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

    /// The regression the review flagged (PR #1209): a persist failure must never leave memory
    /// MORE permissive than disk. Sabotage the next write (a directory sitting at the `.tmp`
    /// rename source makes `std::fs::write` fail there, never touching the real, already-`"1"`
    /// file) and disable — the flip must be refused, with memory left exactly where disk still
    /// is, not flipped to the requested (unpersisted) value.
    #[test]
    fn a_persist_failure_leaves_memory_matching_what_is_still_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let state = BridgeState::load(dir.path());
        assert!(state.set_save_answers_on_submit_enabled(true));
        assert!(state.save_answers_on_submit_enabled());

        let tmp_path = dir
            .path()
            .join(SAVE_ANSWERS_ON_SUBMIT_OPTIN_FILE)
            .with_extension("tmp");
        std::fs::create_dir(&tmp_path).unwrap();

        assert!(
            !state.set_save_answers_on_submit_enabled(false),
            "a persist failure must report no change"
        );
        assert!(
            state.save_answers_on_submit_enabled(),
            "memory must stay in sync with the still-persisted (\"1\") value on a failed write"
        );
        assert!(
            load_save_answers_on_submit_optin(dir.path()),
            "the on-disk file must be untouched by the failed write"
        );
    }
}

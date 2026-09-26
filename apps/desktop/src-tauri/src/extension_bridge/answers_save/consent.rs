//! The `answers.save` consent-lock helper on [`BridgeState`]. See
//! [`BridgeState::with_answers_save_consent_locked`]'s own doc for the TOCTOU
//! it closes.

use super::BridgeState;

impl BridgeState {
    /// Read both `answers.save` consent gates — the assisted-autofill opt-in
    /// (`autofill_enabled`) and the AUTO-only `saveAnswersOnSubmit` opt-in
    /// (`save_answers_on_submit_enabled`) — and run `f` with them, all under ONE hold of
    /// `optin_write_lock`: the SAME lock every consent setter already shares.
    ///
    /// Closes a TOCTOU the plain "read `save_on_submit_enabled`, then merge" order left open: a
    /// `settings.set` disabling the switch could land between the check and
    /// [`crate::applications::ApplicationStore::merge_answers`], so an AUTO-flagged capture could
    /// still be persisted after the user turned the switch off. Holding the setters' own lock
    /// across `f` forces that `settings.set` to BLOCK until this whole check-and-merge finishes.
    ///
    /// Safe against deadlock with the setters: they only ever touch `optin_write_lock` — never
    /// [`crate::applications::ApplicationStore`]'s own `conn` mutex, which `f`'s merge acquires
    /// internally — so `optin_write_lock → conn` is the only nesting order either side ever takes.
    pub(in crate::extension_bridge) fn with_answers_save_consent_locked<T>(
        &self,
        f: impl FnOnce(bool, bool) -> T,
    ) -> T {
        let _guard = self.optin_write_lock.lock();
        f(
            self.autofill_enabled(),
            self.save_answers_on_submit_enabled(),
        )
    }
}

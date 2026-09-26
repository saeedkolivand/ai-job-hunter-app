//! The `auto` flag on an `answers.save` payload — validation + the
//! `saveAnswersOnSubmit` consent check it gates.

use serde_json::Value;

/// Refusal text for an AUTO-flagged `answers.save` (the submit-watch injected
/// entry's synchronous capture) while the dedicated `saveAnswersOnSubmit`
/// opt-in is off.
pub(super) const SAVE_ANSWERS_ON_SUBMIT_OFF_MESSAGE: &str =
    "Save answers on submit is off. Turn it on in AI Job Hunter → Settings → Browser extension.";

/// Refusal text when `auto` is PRESENT but not a JSON boolean — see
/// [`auto_flag_is_malformed`]'s doc for why this must be a hard refusal, never a silent downgrade.
pub(super) const MALFORMED_AUTO_FLAG_MESSAGE: &str =
    "malformed answers.save request: auto must be a boolean";

/// Whether the `auto` field is PRESENT on the payload but not a JSON boolean. A malformed `auto`
/// must be a HARD refusal, never a silent downgrade to "manual" via
/// [`is_auto_answers_save`]'s `unwrap_or(false)`: a manual save is gated only on the weaker
/// assisted-autofill opt-in, not the dedicated `saveAnswersOnSubmit` opt-in this verb's AUTO path
/// exists to require — so a malformed `auto` reading as `false` would let an automated capture
/// through on the wrong, weaker consent class. `auto` absent is unaffected (still defaults to
/// manual).
pub(in crate::extension_bridge) fn auto_flag_is_malformed(payload: &Value) -> bool {
    matches!(payload.get("auto"), Some(v) if !v.is_boolean())
}

/// The `auto` flag on an `answers.save` payload (default false when absent) — `true` marks the
/// AUTOMATED submit-time capture from the `saveAnswersOnSubmit` opt-in, as opposed to the
/// deliberate popup "Save my answers" click. Callers MUST check [`auto_flag_is_malformed`] first —
/// this function's own `unwrap_or(false)` treats a malformed value like an absent one, which is
/// correct ONLY once the malformed case has already been refused upstream.
pub(in crate::extension_bridge) fn is_auto_answers_save(payload: &Value) -> bool {
    payload
        .get("auto")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Whether an AUTO `answers.save` must be REFUSED: an auto-flagged save is honored only while the
/// dedicated `saveAnswersOnSubmit` opt-in is on — the decisive server-side boundary. A non-auto
/// (deliberate popup click) save is never refused here — the extension's own client-side check
/// (whether it even arms the submit-watch capture) is defense-in-depth only; this is the real gate.
pub(in crate::extension_bridge) fn auto_save_refused(
    payload: &Value,
    save_on_submit_enabled: bool,
) -> bool {
    is_auto_answers_save(payload) && !save_on_submit_enabled
}

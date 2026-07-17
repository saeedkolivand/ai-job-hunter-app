//! `autofill.check` (Task #30) — split out to keep `mod.rs` under the R8 hard
//! LOC cap (mirrors `autotrack.rs`'s split). UNLIKE `autotrack.rs`, the
//! `BridgeState::autofill_enabled` field + accessors already live in
//! `mod.rs` (shared with `profile.get`/`answers.save`/`answers.suggest`'s
//! consent gate) — this module holds ONLY the `autofill.check` reply
//! builder; the `FrameDecision::AutofillCheck` dispatch stays in `mod.rs`
//! (it's part of the connection state machine, same discipline as
//! `FrameDecision::AutotrackCheck`).

use serde_json::json;

use super::msg;

/// Build the `autofill.result` reply: the current assisted-autofill opt-in
/// as `{ enabled }`. A pure read — no consent gate on reading the user's own
/// device-local setting (the enforced boundary is what the opt-in gates:
/// `profile.get`/`answers.save`/`answers.suggest`, never this read).
pub(super) fn autofill_check_result_reply(req_id: &str, enabled: bool) -> String {
    json!({
        "type": msg::AUTOFILL_RESULT,
        "reqId": req_id,
        "payload": { "enabled": enabled },
    })
    .to_string()
}

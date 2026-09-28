//! `autofill.check`/`autofill.result` — redistributed from the crate-level `test.rs` (R8 relief).

use super::*;

#[test]
fn autofill_check_result_reply_carries_the_flag() {
    let on: serde_json::Value =
        serde_json::from_str(&autofill_check_result_reply("req-1", true)).unwrap();
    assert_eq!(on["type"], msg::AUTOFILL_RESULT);
    assert_eq!(on["reqId"], "req-1");
    assert_eq!(on["payload"]["enabled"], true);

    let off: serde_json::Value =
        serde_json::from_str(&autofill_check_result_reply("req-2", false)).unwrap();
    assert_eq!(off["payload"]["enabled"], false);
}

#[test]
fn advance_authenticated_routes_autofill_check() {
    use crate::extension_bridge::caller_gate::advance_authenticated;
    use crate::extension_bridge::test_support::bridge_state;
    use crate::extension_bridge::{CallerClass, FrameDecision};
    use serde_json::Value;

    let envelope = serde_json::json!({
        "type": msg::AUTOFILL_CHECK,
        "reqId": "req-10",
        "payload": Value::Null,
    });
    let (_dir, state) = bridge_state();
    let decision = advance_authenticated(
        &state,
        msg::AUTOFILL_CHECK,
        "req-10".to_string(),
        &envelope,
        CallerClass::Other,
    );
    match decision {
        FrameDecision::AutofillCheck { req_id } => assert_eq!(req_id, "req-10"),
        other => panic!("expected FrameDecision::AutofillCheck, got {other:?}"),
    }
}

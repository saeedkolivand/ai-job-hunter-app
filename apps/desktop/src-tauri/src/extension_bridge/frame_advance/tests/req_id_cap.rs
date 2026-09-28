//! `reqId` bound (`advance_frame_from`, `MAX_REQ_ID_BYTES`) -- redistributed from the
//! crate-level `test.rs` (R8 relief).
//!
//! No existing test exercised `advance_frame_from` itself before this pair -- every other test
//! in this module goes through `advance_frame` directly. Both go through the outer function so
//! the cap is proven to run BEFORE the type dispatch, not just inside one handler.

use super::super::*;

use super::super::super::test_support::bridge_state;

#[test]
fn advance_frame_from_passes_through_a_req_id_at_exactly_the_cap() {
    let (_dir, state) = bridge_state();
    let req_id = "r".repeat(MAX_REQ_ID_BYTES);
    let text = serde_json::json!({
        "type": msg::SETTINGS_GET,
        "reqId": req_id,
        "payload": Value::Null,
    })
    .to_string();
    let decision = advance_frame_from(
        &state,
        &ConnState::Authenticated,
        &text,
        CallerClass::Extension,
    );
    match decision {
        FrameDecision::SettingsGet { req_id: got } => assert_eq!(got, req_id),
        other => panic!("expected FrameDecision::SettingsGet, got {other:?}"),
    }
}

#[test]
fn advance_frame_from_refuses_an_oversized_req_id_without_echoing_it() {
    let (_dir, state) = bridge_state();
    let req_id = "r".repeat(MAX_REQ_ID_BYTES + 1);
    let text = serde_json::json!({
        "type": msg::SETTINGS_GET,
        "reqId": req_id,
        "payload": Value::Null,
    })
    .to_string();
    let decision = advance_frame_from(
        &state,
        &ConnState::Authenticated,
        &text,
        CallerClass::Extension,
    );
    let FrameDecision::Reply(reply) = decision else {
        panic!("expected FrameDecision::Reply (a bounded refusal), got {decision:?}");
    };
    assert!(
        reply.len() < 512,
        "the refusal itself must stay small regardless of the oversized input: got {} bytes",
        reply.len()
    );
    assert!(
        !reply.contains(&req_id),
        "the oversized reqId must never be echoed back on the wire"
    );
}

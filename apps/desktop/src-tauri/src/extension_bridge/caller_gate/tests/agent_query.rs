//! `agent.query` is gated on `CallerClass` (finding #5, security review; extended by PR1's
//! extension read tier) — redistributed from the crate-level `test.rs` (R8 relief).

use super::super::*;
use super::state;

#[test]
fn advance_authenticated_routes_agent_query_for_the_cli_regardless_of_autofill() {
    let envelope = serde_json::json!({
        "type": msg::AGENT_QUERY,
        "reqId": "req-11",
        "payload": { "resource": "schema" },
    });
    let (_dir, state) = state();
    // Autofill stays OFF (default) — the CLI's own gate is unaffected by that opt-in.
    let decision = advance_authenticated(
        &state,
        msg::AGENT_QUERY,
        "req-11".to_string(),
        &envelope,
        CallerClass::Cli,
    );
    match decision {
        FrameDecision::AgentQuery { req_id, caller, .. } => {
            assert_eq!(req_id, "req-11");
            assert_eq!(caller, CallerClass::Cli);
        }
        other => panic!("expected FrameDecision::AgentQuery, got {other:?}"),
    }
}

#[test]
fn advance_authenticated_refuses_agent_query_from_a_non_cli_non_extension_origin() {
    // The exact case finding #5 closed: an authenticated connection whose
    // handshake Origin was neither the CLI's nor the extension's must never
    // reach `FrameDecision::AgentQuery`, even though it is fully authenticated.
    let envelope = serde_json::json!({
        "type": msg::AGENT_QUERY,
        "reqId": "req-12",
        "payload": { "resource": "schema" },
    });
    let (_dir, state) = state();
    let decision = advance_authenticated(
        &state,
        msg::AGENT_QUERY,
        "req-12".to_string(),
        &envelope,
        CallerClass::Other,
    );
    let FrameDecision::Reply(text) = decision else {
        panic!("expected FrameDecision::Reply (a refusal), got {decision:?}");
    };
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["type"], msg::AGENT_RESULT);
    assert_eq!(v["reqId"], "req-12");
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(v["payload"]["resource"], "schema");
}

#[test]
fn advance_authenticated_refuses_agent_query_from_the_extension_while_autofill_is_off() {
    let envelope = serde_json::json!({
        "type": msg::AGENT_QUERY,
        "reqId": "req-ext-1",
        "payload": { "resource": "schema" },
    });
    let (_dir, state) = state();
    assert!(!state.autofill_enabled());
    let decision = advance_authenticated(
        &state,
        msg::AGENT_QUERY,
        "req-ext-1".to_string(),
        &envelope,
        CallerClass::Extension,
    );
    let FrameDecision::Reply(text) = decision else {
        panic!("expected FrameDecision::Reply (a refusal), got {decision:?}");
    };
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        v["payload"]["error"],
        crate::extension_bridge::agent_call::ERR_EXTENSION_READ_GATE
    );
}

#[test]
fn advance_authenticated_routes_agent_query_for_the_extension_once_autofill_is_on() {
    let envelope = serde_json::json!({
        "type": msg::AGENT_QUERY,
        "reqId": "req-ext-2",
        "payload": { "resource": "schema" },
    });
    let (_dir, state) = state();
    state.set_autofill_enabled(true);
    let decision = advance_authenticated(
        &state,
        msg::AGENT_QUERY,
        "req-ext-2".to_string(),
        &envelope,
        CallerClass::Extension,
    );
    match decision {
        FrameDecision::AgentQuery { req_id, caller, .. } => {
            assert_eq!(req_id, "req-ext-2");
            assert_eq!(caller, CallerClass::Extension);
        }
        other => panic!("expected FrameDecision::AgentQuery, got {other:?}"),
    }
}

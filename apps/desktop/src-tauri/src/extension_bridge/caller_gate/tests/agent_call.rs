//! `agent.call`: same `CallerClass` gate, plus the extension's own Read-only effect gate
//! (ADR-038 §2, Phase 2; PR1 decision 1) — redistributed from the crate-level `test.rs` (R8
//! relief).

use super::super::*;
use super::state;

#[test]
fn advance_authenticated_routes_agent_call_for_the_cli_regardless_of_effect_or_autofill() {
    let envelope = serde_json::json!({
        "type": msg::AGENT_CALL,
        "reqId": "req-13",
        "payload": { "namespace": "applications", "command": "applications_delete", "input": {} },
    });
    let (_dir, state) = state();
    let decision = advance_authenticated(
        &state,
        msg::AGENT_CALL,
        "req-13".to_string(),
        &envelope,
        CallerClass::Cli,
    );
    match decision {
        FrameDecision::AgentCall { req_id, caller, .. } => {
            assert_eq!(req_id, "req-13");
            assert_eq!(caller, CallerClass::Cli);
        }
        other => panic!("expected FrameDecision::AgentCall, got {other:?}"),
    }
}

#[test]
fn advance_authenticated_refuses_agent_call_from_a_non_cli_non_extension_origin() {
    let envelope = serde_json::json!({
        "type": msg::AGENT_CALL,
        "reqId": "req-14",
        "payload": { "namespace": "jobs", "command": "jobs_list", "input": {} },
    });
    let (_dir, state) = state();
    let decision = advance_authenticated(
        &state,
        msg::AGENT_CALL,
        "req-14".to_string(),
        &envelope,
        CallerClass::Other,
    );
    let FrameDecision::Reply(text) = decision else {
        panic!("expected FrameDecision::Reply (a refusal), got {decision:?}");
    };
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["type"], msg::AGENT_CALL_RESULT);
    assert_eq!(v["reqId"], "req-14");
    assert_eq!(v["payload"]["dispatched"], false);
    assert_eq!(v["payload"]["error"], "cli_only");
}

#[test]
fn advance_authenticated_refuses_agent_call_from_the_extension_while_autofill_is_off() {
    let envelope = serde_json::json!({
        "type": msg::AGENT_CALL,
        "reqId": "req-ext-3",
        "payload": { "namespace": "jobs", "command": "jobs_list", "input": {} },
    });
    let (_dir, state) = state();
    let decision = advance_authenticated(
        &state,
        msg::AGENT_CALL,
        "req-ext-3".to_string(),
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
fn advance_authenticated_dispatches_agent_call_for_the_extension_on_a_read_row() {
    let envelope = serde_json::json!({
        "type": msg::AGENT_CALL,
        "reqId": "req-ext-4",
        // Namespace is the WIRE shape `split_path` derives from the policy path's middle segment
        // (`"commands::jobs::jobs_list"` → `("jobs", "jobs_list")`), never the full `path` string.
        "payload": { "namespace": "jobs", "command": "jobs_list", "input": {} },
    });
    let (_dir, state) = state();
    state.set_autofill_enabled(true);
    let decision = advance_authenticated(
        &state,
        msg::AGENT_CALL,
        "req-ext-4".to_string(),
        &envelope,
        CallerClass::Extension,
    );
    match decision {
        FrameDecision::AgentCall { req_id, caller, .. } => {
            assert_eq!(req_id, "req-ext-4");
            assert_eq!(caller, CallerClass::Extension);
        }
        other => panic!("expected FrameDecision::AgentCall, got {other:?}"),
    }
}

#[test]
fn advance_authenticated_refuses_agent_call_for_the_extension_on_a_non_read_row_without_a_confirm_ceremony(
) {
    let envelope = serde_json::json!({
        "type": msg::AGENT_CALL,
        "reqId": "req-ext-5",
        "payload": {
            "namespace": "applications",
            "command": "applications_delete",
            "input": {},
        },
    });
    let (_dir, state) = state();
    state.set_autofill_enabled(true);
    let decision = advance_authenticated(
        &state,
        msg::AGENT_CALL,
        "req-ext-5".to_string(),
        &envelope,
        CallerClass::Extension,
    );
    let FrameDecision::Reply(text) = decision else {
        panic!("expected FrameDecision::Reply (a refusal), got {decision:?}");
    };
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["payload"]["dispatched"], false);
    assert_eq!(
        v["payload"]["error"], "effect_not_allowed_for_extension",
        "an Irreversible row must refuse in-band, never enter the confirm ceremony"
    );
    assert!(
        v["payload"].get("confirm").is_none(),
        "the refusal must carry no confirm ceremony hint at all"
    );
}

//! `document.export` (PR2 — documents into ATS) — extension caller only, gated on the SAME
//! Assisted-autofill opt-in as `agent.query`/`agent.call`'s own extension arm — redistributed
//! from the crate-level `test.rs` (R8 relief).

use super::super::*;
use super::state;

#[test]
fn advance_authenticated_refuses_document_export_for_the_cli_and_other() {
    for caller in [CallerClass::Cli, CallerClass::Other] {
        let envelope = serde_json::json!({
            "type": msg::DOCUMENT_EXPORT,
            "reqId": "req-doc-1",
            "payload": {
                "source": { "kind": "generation", "url": "https://example.com/job/1" },
                "kind": "resume",
                "format": "pdf",
                "templateId": "classic",
            },
        });
        let (_dir, state) = state();
        let decision = advance_authenticated(
            &state,
            msg::DOCUMENT_EXPORT,
            "req-doc-1".to_string(),
            &envelope,
            caller,
        );
        let FrameDecision::Reply(text) = decision else {
            panic!("expected FrameDecision::Reply (a refusal) for {caller:?}, got {decision:?}");
        };
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["type"], msg::DOCUMENT_RESULT);
        assert_eq!(v["payload"]["ok"], false);
        assert_eq!(v["payload"]["error"], "origin_refused");
    }
}

#[test]
fn advance_authenticated_refuses_document_export_from_the_extension_while_autofill_is_off() {
    let envelope = serde_json::json!({
        "type": msg::DOCUMENT_EXPORT,
        "reqId": "req-doc-2",
        "payload": {
            "source": { "kind": "generation", "url": "https://example.com/job/1" },
            "kind": "resume",
            "format": "pdf",
            "templateId": "classic",
        },
    });
    let (_dir, state) = state();
    assert!(!state.autofill_enabled());
    let decision = advance_authenticated(
        &state,
        msg::DOCUMENT_EXPORT,
        "req-doc-2".to_string(),
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
fn advance_authenticated_routes_document_export_for_the_extension_once_autofill_is_on() {
    let envelope = serde_json::json!({
        "type": msg::DOCUMENT_EXPORT,
        "reqId": "req-doc-3",
        "payload": {
            "source": { "kind": "document", "id": "doc-1" },
            "kind": "resume",
            "format": "docx",
            "templateId": "classic",
        },
    });
    let (_dir, state) = state();
    state.set_autofill_enabled(true);
    let decision = advance_authenticated(
        &state,
        msg::DOCUMENT_EXPORT,
        "req-doc-3".to_string(),
        &envelope,
        CallerClass::Extension,
    );
    match decision {
        FrameDecision::DocumentExport { req_id, payload } => {
            assert_eq!(req_id, "req-doc-3");
            assert_eq!(payload["source"]["kind"], "document");
        }
        other => panic!("expected FrameDecision::DocumentExport, got {other:?}"),
    }
}

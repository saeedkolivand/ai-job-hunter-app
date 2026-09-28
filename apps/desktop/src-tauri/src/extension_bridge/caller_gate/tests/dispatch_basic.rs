//! `CallerClass::resolve` (PR1 -- extension read tier) and the plain pass-through dispatch
//! arms that need no extension-specific gating (`assist.cancel`) -- redistributed from the
//! crate-level `test.rs` (R8 relief).

use super::super::*;
use super::state;

#[test]
fn caller_class_resolve_matches_the_agent_cli_sentinel() {
    assert_eq!(
        CallerClass::resolve(auth::AGENT_CLI_ORIGIN, &[]),
        CallerClass::Cli
    );
}

#[test]
fn caller_class_resolve_matches_a_known_extension_origin() {
    assert_eq!(
        CallerClass::resolve("chrome-extension://oaoekkgkhmgdfnpmfkpphgiikliaicll", &[]),
        CallerClass::Extension
    );
    // The REAL Firefox background-script origin.
    assert_eq!(CallerClass::resolve("null", &[]), CallerClass::Extension);
    // The native-messaging relay forwards the paired extension's frames 1:1, so it
    // resolves to the extension too -- only the CLI sentinel is carved out.
    assert_eq!(
        CallerClass::resolve(auth::NATIVE_HOST_ORIGIN, &[]),
        CallerClass::Extension
    );
}

#[test]
fn caller_class_resolve_falls_back_to_other_for_everything_else() {
    assert_eq!(
        CallerClass::resolve("https://evil.example.com", &[]),
        CallerClass::Other
    );
    assert_eq!(CallerClass::resolve("", &[]), CallerClass::Other);
}

#[test]
fn advance_authenticated_routes_assist_cancel_by_req_id() {
    let envelope = serde_json::json!({
        "type": msg::ASSIST_CANCEL,
        "reqId": "req-7",
        "payload": Value::Null,
    });
    let (_dir, bridge_state) = state();
    let decision = advance_authenticated(
        &bridge_state,
        msg::ASSIST_CANCEL,
        "req-7".to_string(),
        &envelope,
        CallerClass::Other,
    );
    match decision {
        FrameDecision::AssistCancel { req_id } => assert_eq!(req_id, "req-7"),
        other => panic!("expected FrameDecision::AssistCancel, got {other:?}"),
    }
}

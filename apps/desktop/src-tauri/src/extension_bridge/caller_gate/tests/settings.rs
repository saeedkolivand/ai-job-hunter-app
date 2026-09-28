//! `settings.get`/`settings.set` (R7) — extension caller only — redistributed from the
//! crate-level `test.rs` (R8 relief).

use super::super::*;
use super::state;

#[test]
fn advance_authenticated_routes_settings_get_for_the_extension_regardless_of_autofill() {
    let envelope = serde_json::json!({
        "type": msg::SETTINGS_GET,
        "reqId": "req-set-1",
        "payload": Value::Null,
    });
    let (_dir, state) = state();
    assert!(!state.autofill_enabled());
    let decision = advance_authenticated(
        &state,
        msg::SETTINGS_GET,
        "req-set-1".to_string(),
        &envelope,
        CallerClass::Extension,
    );
    match decision {
        FrameDecision::SettingsGet { req_id } => assert_eq!(req_id, "req-set-1"),
        other => panic!("expected FrameDecision::SettingsGet, got {other:?}"),
    }
}

#[test]
fn advance_authenticated_refuses_settings_get_for_the_cli_and_other() {
    for caller in [CallerClass::Cli, CallerClass::Other] {
        let envelope = serde_json::json!({
            "type": msg::SETTINGS_GET,
            "reqId": "req-set-2",
            "payload": Value::Null,
        });
        let (_dir, state) = state();
        let decision = advance_authenticated(
            &state,
            msg::SETTINGS_GET,
            "req-set-2".to_string(),
            &envelope,
            caller,
        );
        let FrameDecision::Reply(text) = decision else {
            panic!("expected FrameDecision::Reply (a refusal) for {caller:?}, got {decision:?}");
        };
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["payload"]["ok"], false);
        assert_eq!(v["payload"]["error"], "extension_only");
    }
}

#[test]
fn advance_authenticated_routes_settings_set_for_the_extension() {
    let envelope = serde_json::json!({
        "type": msg::SETTINGS_SET,
        "reqId": "req-set-3",
        "payload": { "key": "autofill", "enabled": true },
    });
    let (_dir, state) = state();
    let decision = advance_authenticated(
        &state,
        msg::SETTINGS_SET,
        "req-set-3".to_string(),
        &envelope,
        CallerClass::Extension,
    );
    match decision {
        FrameDecision::SettingsSet { req_id, payload } => {
            assert_eq!(req_id, "req-set-3");
            assert_eq!(payload["key"], "autofill");
        }
        other => panic!("expected FrameDecision::SettingsSet, got {other:?}"),
    }
}

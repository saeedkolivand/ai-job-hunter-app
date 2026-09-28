use super::*;
use serde_json::json;

fn state() -> (tempfile::TempDir, BridgeState) {
    let dir = tempfile::tempdir().unwrap();
    let state = BridgeState::load(dir.path());
    (dir, state)
}

#[test]
fn resolve_settings_set_applies_through_the_same_setter() {
    let (_dir, state) = state();
    let ok = resolve_settings_set(&state, &json!({ "key": "autofill", "enabled": true }))
        .expect("valid request");
    assert!(ok.enabled);
    assert!(ok.changed, "the switch actually flipped");
    assert!(state.autofill_enabled(), "the same BridgeState setter ran");
    assert!(!state.ai_assist_enabled(), "only the named key changed");
    assert!(!state.autotrack_enabled());
    assert!(!state.save_answers_on_submit_enabled());
}

/// Setting a key to its own current value is a no-op: no re-apply
/// ([`SettingsSetOk::changed`] is `false`), and [`notification_for`]
/// agrees — see its own tests below for the changed-branch behaviour.
#[test]
fn resolve_settings_set_no_op_does_not_reapply_or_notify() {
    let (_dir, state) = state();
    // Every switch defaults to off — requesting `false` again is a no-op.
    let ok = resolve_settings_set(&state, &json!({ "key": "autofill", "enabled": false }))
        .expect("valid request");
    assert!(
        !ok.changed,
        "requesting the current value must not report a change"
    );
    assert!(!state.autofill_enabled());
    assert!(
        notification_for(&ok).is_none(),
        "a no-op request must not produce a notification"
    );
}

#[test]
fn resolve_settings_set_covers_every_key() {
    let (_dir, state) = state();
    for (wire, get, enabled) in [
        (
            "autofill",
            (|s: &BridgeState| s.autofill_enabled()) as fn(&BridgeState) -> bool,
            true,
        ),
        ("aiAssist", (|s: &BridgeState| s.ai_assist_enabled()), true),
        ("autotrack", (|s: &BridgeState| s.autotrack_enabled()), true),
        (
            "saveAnswersOnSubmit",
            (|s: &BridgeState| s.save_answers_on_submit_enabled()),
            true,
        ),
    ] {
        resolve_settings_set(&state, &json!({ "key": wire, "enabled": enabled })).unwrap();
        assert!(get(&state), "key {wire} did not apply");
    }
}

#[test]
fn resolve_settings_set_rejects_an_unknown_key_without_writing_anything() {
    let (_dir, state) = state();
    let err = resolve_settings_set(&state, &json!({ "key": "notARealKey", "enabled": true }))
        .unwrap_err();
    assert_eq!(err, ERR_INVALID_SETTINGS_REQUEST);
    assert!(!state.autofill_enabled());
    assert!(!state.ai_assist_enabled());
    assert!(!state.autotrack_enabled());
    assert!(!state.save_answers_on_submit_enabled());
}

#[test]
fn resolve_settings_set_rejects_a_non_boolean_enabled() {
    let (_dir, state) = state();
    let err =
        resolve_settings_set(&state, &json!({ "key": "autofill", "enabled": "yes" })).unwrap_err();
    assert_eq!(err, ERR_INVALID_SETTINGS_REQUEST);
    assert!(!state.autofill_enabled());
}

#[test]
fn resolve_settings_set_rejects_a_missing_key() {
    let (_dir, state) = state();
    let err = resolve_settings_set(&state, &json!({ "enabled": true })).unwrap_err();
    assert_eq!(err, ERR_INVALID_SETTINGS_REQUEST);
}

/// A no-op ([`SettingsSetOk::changed`] is `false`) must never notify —
/// the behavioural half of what used to be a source-text scan on
/// [`handle_settings_set`]; [`notification_for`] is pure and directly
/// testable, so there is no need to grep the function body anymore.
#[test]
fn notification_for_is_none_when_nothing_changed() {
    let ok = SettingsSetOk {
        key: SettingsKey::Autofill,
        enabled: false,
        changed: false,
    };
    assert!(notification_for(&ok).is_none());
}

/// A real change, for every key and either direction, names the switch
/// and its new state in the body and routes to the Settings page — R7's
/// "never silent" guard rail, pinned on the pure function instead of the
/// live `push_and_notify` call this crate has no mock `AppHandle` for.
#[test]
fn notification_for_names_the_key_and_state_for_every_settings_key() {
    for key in SettingsKey::ALL {
        for enabled in [true, false] {
            let ok = SettingsSetOk {
                key,
                enabled,
                changed: true,
            };
            let notification = notification_for(&ok).expect("a changed request must notify");
            assert_eq!(notification.title, SETTINGS_CHANGED_TITLE);
            let expected_state = if enabled { "on" } else { "off" };
            assert!(
                notification.body.contains(key.label()),
                "body must name the switch that changed: {}",
                notification.body
            );
            assert!(
                notification.body.contains(expected_state),
                "body must say whether it turned on or off: {}",
                notification.body
            );
            assert_eq!(
                notification.route.as_ref().map(|r| r.to.as_str()),
                Some("/settings")
            );
        }
    }
}

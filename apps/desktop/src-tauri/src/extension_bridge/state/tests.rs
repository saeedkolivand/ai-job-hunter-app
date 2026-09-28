//! Core `BridgeState` unit tests (load/token/port/`is_connected`/`Resettable`) — redistributed
//! from the crate-level `test.rs` (R8 relief). Live-connection-count tests live in
//! `throttle::tests`; pairing-revocation tests live in `rotation::tests`.

use super::*;

use super::super::test_support::bridge_state;

#[test]
fn token_is_persisted_and_reloaded() {
    let (dir, s1) = bridge_state();
    let t1 = s1.token();
    assert_eq!(t1.len(), 64, "token is 32 bytes hex = 64 chars");
    assert!(t1.chars().all(|c| c.is_ascii_hexdigit()));

    // A second load from the same dir reuses the persisted token.
    let s2 = BridgeState::load(dir.path());
    assert_eq!(s2.token(), t1, "token persists across loads");
}

#[test]
fn fresh_state_has_no_port_and_is_disconnected() {
    let (_dir, s) = bridge_state();
    assert_eq!(s.port(), None);
    assert!(!s.is_connected());
}

#[test]
fn reset_rotates_token() {
    use crate::data_store::Resettable;
    let (_dir, s) = bridge_state();
    let before = s.token();
    s.reset();
    assert_ne!(s.token(), before, "factory reset rotates the pairing token");
}

// ── The three independent opt-ins (autofill / ai-assist / auto-track): each is default OFF,
// persisted, and independent of the others — table-driven over the 3 flags so a 4th opt-in
// only adds a row, not a copy-pasted test (R8 shrink pass). ──────────────────────────────

/// One opt-in flag's accessor pair, for the table-driven tests below.
struct OptinFlag {
    get: fn(&BridgeState) -> bool,
    set: fn(&BridgeState, bool) -> bool,
    off_message: &'static str,
    reset_message: &'static str,
}

const OPTIN_FLAGS: [OptinFlag; 3] = [
    OptinFlag {
        get: BridgeState::autofill_enabled,
        set: BridgeState::set_autofill_enabled,
        off_message: "autofill opt-in defaults OFF",
        reset_message: "factory reset returns the autofill opt-in to its default OFF",
    },
    OptinFlag {
        get: BridgeState::ai_assist_enabled,
        set: BridgeState::set_ai_assist,
        off_message: "ai-assist opt-in defaults OFF",
        reset_message: "factory reset returns the ai-assist opt-in to its default OFF",
    },
    OptinFlag {
        get: BridgeState::autotrack_enabled,
        set: BridgeState::set_autotrack_enabled,
        off_message: "auto-track opt-in defaults OFF",
        reset_message: "factory reset returns the auto-track opt-in to its default OFF",
    },
];

#[test]
fn optin_defaults_off_and_persists() {
    for flag in OPTIN_FLAGS {
        let (dir, s) = bridge_state();
        assert!(!(flag.get)(&s), "{}", flag.off_message);

        (flag.set)(&s, true);
        assert!((flag.get)(&s));

        // A fresh load from the same dir reads back the persisted opt-in.
        let reloaded = BridgeState::load(dir.path());
        assert!((flag.get)(&reloaded), "opt-in persists across loads");

        // Turning it back off persists too.
        (flag.set)(&reloaded, false);
        assert!(!(flag.get)(&BridgeState::load(dir.path())));
    }
}

#[test]
fn reset_disables_every_optin() {
    use crate::data_store::Resettable;
    for flag in OPTIN_FLAGS {
        let (_dir, s) = bridge_state();
        (flag.set)(&s, true);
        s.reset();
        assert!(!(flag.get)(&s), "{}", flag.reset_message);
    }
}

/// Issue #1203-r1-2 (settings-switch race): each of the three consent
/// setters now returns whether it actually changed the value, and
/// `resolve_settings_set` (`settings.rs`) relies on this instead of its own
/// separate compare — a stale `true` here would silently break "a
/// Notification Center entry per actual change" (R7 guard rail #3).
#[test]
fn optin_setters_report_false_on_a_redundant_same_value_call() {
    let (_dir, state) = bridge_state();

    assert!(
        state.set_autofill_enabled(true),
        "off → on is a real change"
    );
    assert!(
        !state.set_autofill_enabled(true),
        "requesting autofill's already-current value must report no change"
    );

    assert!(state.set_ai_assist(true), "off → on is a real change");
    assert!(
        !state.set_ai_assist(true),
        "requesting ai-assist's already-current value must report no change"
    );

    assert!(
        state.set_autotrack_enabled(true),
        "off → on is a real change"
    );
    assert!(
        !state.set_autotrack_enabled(true),
        "requesting autotrack's already-current value must report no change"
    );
}

/// Back-compat: an OLD opt-in file (pre-task-#16) also carried a
/// `provider`/`model`/`base_url` snapshot alongside `enabled`. Loading it must
/// still honor the persisted `enabled` flag and simply ignore the extra fields
/// — a user who opted in before the store landed stays opted in (the active
/// provider now resolves from the backend `AiConfigStore`, never that stale
/// snapshot), so no silent forced re-consent on upgrade.
#[test]
fn ai_assist_optin_reads_an_old_snapshot_file_and_ignores_the_extra_fields() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(crate::extension_bridge::AI_ASSIST_OPTIN_FILE),
        r#"{"enabled":true,"provider":"openai","model":"gpt-4o","base_url":"https://attacker.example/v1"}"#,
    )
    .unwrap();

    let s = BridgeState::load(dir.path());
    assert!(
        s.ai_assist_enabled(),
        "an old snapshot file's `enabled` flag is still honored on load"
    );

    // Rewriting drops the stale snapshot: the persisted file is now the bare flag.
    s.set_ai_assist(true);
    let persisted = std::fs::read_to_string(
        dir.path()
            .join(crate::extension_bridge::AI_ASSIST_OPTIN_FILE),
    )
    .unwrap();
    assert!(
        !persisted.contains("attacker"),
        "the stale attacker base_url snapshot is dropped on the next write"
    );
}

/// Each opt-in is a SEPARATE gate — turning on the "other" ones must never turn this one on
/// too. Table-driven: autofill has no such test (nothing precedes it), ai-assist checks
/// independence from autofill, auto-track checks independence from both.
#[test]
fn optin_is_independent_of_the_others() {
    struct Case {
        turn_on: &'static [fn(&BridgeState, bool) -> bool],
        get: fn(&BridgeState) -> bool,
        message: &'static str,
    }
    let cases: [Case; 2] = [
        Case {
            turn_on: &[BridgeState::set_autofill_enabled],
            get: BridgeState::ai_assist_enabled,
            message: "turning autofill on must never turn ai-assist on too — separate gates",
        },
        Case {
            turn_on: &[
                BridgeState::set_autofill_enabled,
                BridgeState::set_ai_assist,
            ],
            get: BridgeState::autotrack_enabled,
            message:
                "turning autofill/ai-assist on must never turn auto-track on too — separate gates",
        },
    ];
    for case in cases {
        let (_dir, s) = bridge_state();
        for on in case.turn_on {
            on(&s, true);
        }
        assert!(!(case.get)(&s), "{}", case.message);
    }
}

//! Rust ↔ TS protocol parity + bridge-state unit tests.
//!
//! The parity test mirrors the Feature-1 stage-registry approach: it reads the
//! shared TS protocol source
//! (`packages/shared/src/ipc/extension-protocol-constants.ts`) as text and
//! asserts every Rust message-type constant in [`super::msg`] appears as the
//! exact string literal on the TS side. If either side renames a wire `type`
//! without the other, this fails — the two can't drift.

use super::*;

/// Path from this crate's manifest dir to the shared TS protocol constants
/// source — the zod-free module that holds the `EXTENSION_MESSAGE_TYPES`
/// literal strings (`extension-protocol.ts` only re-exports them).
const TS_PROTOCOL: &str = "../../../packages/shared/src/ipc/extension-protocol-constants.ts";

fn ts_protocol_source() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(TS_PROTOCOL);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()))
}

#[test]
fn message_type_constants_match_ts() {
    let ts = ts_protocol_source();
    // Each Rust constant must appear as a single-quoted string literal in the TS
    // `EXTENSION_MESSAGE_TYPES` map (e.g. `import.request: 'import.request'`).
    for literal in [
        msg::AUTH,
        msg::IMPORT_REQUEST,
        msg::IMPORT_RESULT,
        msg::PROFILE_GET,
        msg::PROFILE_RESULT,
        msg::MATCH_LIVE,
        msg::APPLIED_CHECK,
    ] {
        let needle = format!("'{literal}'");
        assert!(
            ts.contains(&needle),
            "wire type {literal:?} (Rust) not found as {needle} in extension-protocol-constants.ts — \
             the Rust msg:: constants drifted from the shared TS EXTENSION_MESSAGE_TYPES"
        );
    }
}

#[test]
fn reserved_types_are_distinct() {
    // Every wire type must be a distinct string.
    let all = [
        msg::AUTH,
        msg::IMPORT_REQUEST,
        msg::IMPORT_RESULT,
        msg::PROFILE_GET,
        msg::PROFILE_RESULT,
        msg::MATCH_LIVE,
        msg::APPLIED_CHECK,
    ];
    let set: std::collections::HashSet<_> = all.iter().collect();
    assert_eq!(set.len(), all.len(), "wire type constants must be unique");
}

#[test]
fn applications_changed_event_name_is_stable() {
    // Pinned so the frontend slice's subscription string can rely on it.
    assert_eq!(crate::events::APPLICATIONS_CHANGED, "applications:changed");
}

// ── Token lifecycle ──────────────────────────────────────────────────────────

#[test]
fn token_is_persisted_and_reloaded() {
    let dir = tempfile::tempdir().unwrap();
    let s1 = BridgeState::load(dir.path());
    let t1 = s1.token();
    assert_eq!(t1.len(), 64, "token is 32 bytes hex = 64 chars");
    assert!(t1.chars().all(|c| c.is_ascii_hexdigit()));

    // A second load from the same dir reuses the persisted token.
    let s2 = BridgeState::load(dir.path());
    assert_eq!(s2.token(), t1, "token persists across loads");
}

#[test]
fn regenerate_rotates_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());
    let before = s.token();
    let after = s.regenerate_token();
    assert_ne!(before, after, "regenerate produces a new token");
    assert_eq!(s.token(), after, "state holds the rotated token");

    // The rotated token is the one a fresh load reads back.
    let reloaded = BridgeState::load(dir.path());
    assert_eq!(reloaded.token(), after);
}

#[test]
fn fresh_state_has_no_port_and_is_disconnected() {
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());
    assert_eq!(s.port(), None);
    assert!(!s.is_connected());
}

#[test]
fn reset_rotates_token() {
    use crate::data_store::Resettable;
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());
    let before = s.token();
    s.reset();
    assert_ne!(s.token(), before, "factory reset rotates the pairing token");
}

// ── Assisted-autofill opt-in (default OFF, persisted) ─────────────────────────

#[test]
fn autofill_optin_defaults_off_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());
    assert!(!s.autofill_enabled(), "autofill opt-in defaults OFF");

    s.set_autofill_enabled(true);
    assert!(s.autofill_enabled());

    // A fresh load from the same dir reads back the persisted opt-in.
    let reloaded = BridgeState::load(dir.path());
    assert!(reloaded.autofill_enabled(), "opt-in persists across loads");

    // Turning it off persists too.
    reloaded.set_autofill_enabled(false);
    assert!(!BridgeState::load(dir.path()).autofill_enabled());
}

#[test]
fn reset_disables_autofill_optin() {
    use crate::data_store::Resettable;
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());
    s.set_autofill_enabled(true);
    s.reset();
    assert!(
        !s.autofill_enabled(),
        "factory reset returns the autofill opt-in to its default OFF"
    );
}

// ── profile.get consent gate (resolve_profile) ────────────────────────────────

#[test]
fn resolve_profile_refuses_when_opt_in_off() {
    use crate::contact_profile::ContactProfile;
    let profile = ContactProfile {
        email: Some("a@b.com".to_string()),
        ..Default::default()
    };
    // Even with a profile present, opt-in OFF returns a clear refusal, never data.
    let err = resolve_profile(false, Some(&profile)).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Autofill is off"), "refusal message: {msg}");
}

#[test]
fn resolve_profile_projects_when_opt_in_on() {
    use crate::contact_profile::{ContactProfile, LocalizedText};
    let profile = ContactProfile {
        full_name: Some("Saeed Kolivand".to_string()),
        email: Some("saeed@example.com".to_string()),
        phone: Some("  +31 6 12  ".to_string()), // trimmed on projection
        location: Some(LocalizedText {
            default: "Amsterdam, Netherlands".to_string(),
            ..Default::default()
        }),
        linkedin: Some("https://linkedin.com/in/saeed".to_string()),
        website: Some("   ".to_string()), // whitespace-only → dropped
        ..Default::default()
    };
    let out = resolve_profile(true, Some(&profile)).expect("opt-in on returns the profile");
    assert_eq!(out.full_name.as_deref(), Some("Saeed Kolivand"));
    assert_eq!(out.email.as_deref(), Some("saeed@example.com"));
    assert_eq!(out.phone.as_deref(), Some("+31 6 12"));
    assert_eq!(out.location.as_deref(), Some("Amsterdam, Netherlands"));
    assert_eq!(
        out.linkedin.as_deref(),
        Some("https://linkedin.com/in/saeed")
    );
    assert_eq!(out.website, None, "whitespace-only fields are dropped");
    assert_eq!(out.github, None);
}

#[test]
fn resolve_profile_errors_when_store_missing() {
    // opt-in on but no profile available (store not managed) → a Config error, not a panic.
    assert!(resolve_profile(true, None).is_err());
}

#[test]
fn profile_result_reply_carries_type_and_req_id() {
    use crate::contact_profile::ContactProfile;
    let out = resolve_profile(
        true,
        Some(&ContactProfile {
            email: Some("x@y.z".to_string()),
            ..Default::default()
        }),
    );
    let reply = profile_result_reply("req-42", out);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::PROFILE_RESULT);
    assert_eq!(v["reqId"], "req-42");
    assert_eq!(v["payload"]["email"], "x@y.z");
    assert!(v["payload"].get("error").is_none());
}

#[test]
fn profile_result_reply_carries_refusal_error() {
    let reply = profile_result_reply("req-7", resolve_profile(false, None));
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::PROFILE_RESULT);
    assert!(v["payload"]["error"]
        .as_str()
        .unwrap()
        .contains("Autofill is off"));
}

// ── Spawn-from-no-runtime regression (boot panic) ────────────────────────────

/// Regression guard for the boot panic: `start()` is called from the Tauri
/// `setup` hook, which runs on the main thread with **no** ambient Tokio
/// reactor. A bare `tokio::spawn` there panics with "there is no reactor
/// running, must be called from the context of a Tokio 1.x runtime", taking the
/// whole app down at boot. `start()` now routes through [`super::spawn_detached`]
/// ([`tauri::async_runtime::spawn`]), which does not need an ambient reactor.
///
/// This is a plain `#[test]` (NOT `#[tokio::test]`) **on purpose**: there is no
/// ambient runtime in scope, exactly like the real `setup` call site. Driving
/// the spawn entry-point from here means a regression to bare `tokio::spawn`
/// inside `spawn_detached` would panic this test. Deterministic: the spawned
/// future is trivial — no sleeps, no socket binds, no app state.
///
/// (A full mock-`AppHandle` test of `start()` itself is deferred: it would
/// require enabling Tauri's `test` feature — a build-config change with its own
/// review/risk surface and zero current usage in this crate — so we guard the
/// no-runtime spawn mechanism directly instead.)
#[test]
fn spawn_detached_runs_without_an_ambient_tokio_runtime() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // No `#[tokio::test]`, no `Runtime::block_on` — there is intentionally NO
    // reactor in this thread's scope. A bare `tokio::spawn` would panic right
    // here; `spawn_detached` (Tauri async runtime) must not.
    let ran = Arc::new(AtomicBool::new(false));
    let ran_in_task = Arc::clone(&ran);
    spawn_detached(async move {
        ran_in_task.store(true, Ordering::SeqCst);
    });

    // The point of the test is that the line above did not panic. We don't join
    // the detached task (that would reintroduce timing/flakiness); we only assert
    // the closure type-checks against the same `Future<Output = ()> + Send` bound
    // `start()` relies on, by handing it a real future. Reaching this line proves
    // the no-runtime spawn path is intact.
    let _ = ran;
}

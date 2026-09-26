//! Tests for `dispatch_direct`'s contact-profile photo-restore wiring and its state-read failure paths (`dispatch.rs`).

use super::super::dispatch::stored_profile_value;
use super::super::reshape::*;
use super::super::*;

/// P-r2-R2-F1 (HIGH), reopened round 3 (P-r3-AC-R3-F1): a source-text guard
/// on the call site passed under a mutation that made `stored_profile_value`
/// itself return an empty profile (`.map(|_store| ContactProfile::default())`
/// at the read, not the call) — the exact round-1 CRITICAL, with the whole
/// suite green. This composes `stored_profile_value` with
/// `restore_local_only_contact_fields` exactly as `dispatch_direct` does,
/// against a REAL `ContactProfileStore` over a `TempDir` (the same
/// `_inner`/`Option<&Store>` split `commands/contact_profile.rs` already
/// uses for the same "no `tauri::test` mock app" gap), so a stored photo
/// must survive a projected `contact_profile_get` → `contact_profile_set`
/// round trip.
#[test]
fn dispatch_direct_wires_the_real_stored_profile_into_restore_local_only_contact_fields() {
    use tempfile::TempDir;

    use crate::contact_profile::{ContactProfile, ContactProfileStore};

    let dir = TempDir::new().expect("tempdir");
    let store = ContactProfileStore::open(&dir.path().to_path_buf()).expect("open store");
    store
        .set(&ContactProfile {
            full_name: Some("Jane Doe".to_string()),
            photo: Some("data:image/png;base64,AAAA".to_string()),
            ..Default::default()
        })
        .unwrap();

    // What a `contact_profile_get` caller can ever produce, since
    // `project_contact_profile_get` already stripped `photo` from the read.
    let mut input = json!({ "profile": { "fullName": "Jane Doe" } });
    let stored_profile = stored_profile_value(Some(&store))
        .unwrap_or_else(|_| panic!("real store read must succeed"));
    restore_local_only_contact_fields("contact_profile_set", &mut input, stored_profile.as_ref());

    assert_eq!(
        input["profile"]["photo"], "data:image/png;base64,AAAA",
        "a projected agent read-modify-write must not delete the stored photo"
    );
}

/// The composition test above proves the wiring reads REAL state once it is
/// wired up; it never touches `dispatch_direct` itself, so a mutation that
/// simply dropped the wiring (or the whole `if` block) would still pass it
/// (P-r1-AC-R4-F1 / P-r1-SEC-1180-01, round 4 — the wiring guard round 2
/// added was deleted in round 3 as if the composition test superseded it,
/// but the two catch disjoint mutation classes: this one catches "is the
/// call even made", the composition test above catches "does the call read
/// real state"). Kept alongside it, not instead of it.
///
/// P-r2-AC-R5-F3 (MEDIUM, round-2 review): the call-line assertion alone
/// pins the TEXT of the call, never the CONDITION under which it runs —
/// disabling the `if` (e.g. `if command == CONTACT_PROFILE_SET_COMMAND &&
/// false {`) leaves the call-site text intact and the whole suite green
/// while the photo-deleting read-modify-write is fully restored. Pin the
/// block opener and the read call alongside the call-site text so a gate
/// that never runs fails HERE too.
#[test]
fn dispatch_direct_calls_the_local_only_contact_field_restore_with_the_real_stored_profile() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/extension_bridge/agent_call/dispatch.rs"
    ));
    assert!(
        SOURCE.contains("if command == CONTACT_PROFILE_SET_COMMAND {"),
        "dispatch_direct must gate the restore on the real command check, not a disabled one"
    );
    assert!(
        SOURCE.contains("stored_profile_value("),
        "dispatch_direct must read the CURRENT stored profile before restoring"
    );
    assert!(
        SOURCE.contains(
            "restore_local_only_contact_fields(command, &mut input, stored_profile.as_ref());"
        ),
        "dispatch_direct must pass the REAL stored profile, not a hardcoded None"
    );
}

/// The other half of [`stored_profile_value`]'s branch: an unmanaged store
/// degrades to `None`, the same "no state to read" shape
/// `restore_local_only_contact_fields_is_a_no_op_when_nothing_is_stored`
/// already covers on the pure side.
#[test]
fn stored_profile_value_is_none_when_the_store_is_unmanaged() {
    assert!(matches!(stored_profile_value(None), Ok(None)));
}

/// P-r1-AC-R4-F3 (MEDIUM): `stored_profile_value` must read through
/// [`crate::contact_profile::ContactProfileStore::try_get`], never `get`
/// — `get` degrades a locked/busy read or a corrupt stored row to
/// `ContactProfile::default()`, indistinguishable from "nothing stored" and
/// a re-run of the round-1 CRITICAL. A real `ContactProfileStore` has no
/// public way to land a corrupt row (`set`/`import` only ever write valid
/// JSON) and the private `conn` field a raw-SQL test would need is only
/// visible inside `contact_profile`'s own module tree, not here — so this
/// source-guards the call, the same shape already used for `dispatch_direct`
/// above for the identical "no mock, no reachable seam" gap.
/// [`crate::contact_profile::test`] separately proves `try_get`'s error
/// behaviour for real, against a row it CAN reach and corrupt.
#[test]
fn stored_profile_value_reads_through_try_get_and_refuses_on_its_error() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/extension_bridge/agent_call/dispatch.rs"
    ));
    assert!(
        SOURCE.contains("store\n        .try_get()\n        .map_err(|e| Refusal::StateUnreadable(e.to_string()))?;"),
        "stored_profile_value must read via try_get() and refuse (not swallow) its error"
    );
}

/// P-r2-AC-R5-F4 (MEDIUM, round-2 review, issue #1180): an app-state read
/// failure (e.g. [`stored_profile_value`]'s `try_get` error) must sentinel
/// as its own `state_unreadable`, distinct from [`Refusal::DispatchFailed`]'s
/// `dispatch_failed` — collapsing the two hid a real app-state failure
/// behind a sentinel whose own doc guarantees a fixed, framework-only
/// message, and would send a debugger to the webview dispatch path instead
/// of the app-state read that actually failed.
#[test]
fn state_unreadable_has_its_own_sentinel_distinct_from_dispatch_failed() {
    let refusal = Refusal::StateUnreadable("boom".to_string());
    assert_eq!(refusal.sentinel(), "state_unreadable");
    assert_ne!(
        refusal.sentinel(),
        Refusal::DispatchFailed(String::new()).sentinel()
    );
    assert!(refusal.detail().contains("boom"));
}

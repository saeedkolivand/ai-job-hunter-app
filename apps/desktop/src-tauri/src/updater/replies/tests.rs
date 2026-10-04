use super::*;

// ── status_reply: `updater_status`'s read-only reply (round 5,
// `B1-r1-ACLI-R5-1`) — no network, no `UpdaterState` write, no event ────────

#[test]
fn test_status_reply_unknown_when_nothing_pending_and_never_checked() {
    assert_eq!(
        status_reply(&UpdaterState::default(), None),
        json!({ "available": false, "checked": false })
    );
}

/// `B1-r2-ACLI-R6-2` — the sole regression this whole field exists to fix:
/// a caller must be able to tell "checked, genuinely current" apart from
/// "no check has ever run" / "the last one failed". Both used to be the
/// exact same `{"available": false}`.
#[test]
fn test_status_reply_checked_and_current_differs_from_never_checked() {
    let checked = UpdaterState {
        checked: true,
        ..UpdaterState::default()
    };
    let never_checked = UpdaterState::default();
    assert_eq!(
        status_reply(&checked, None),
        json!({ "available": false, "checked": true })
    );
    assert_ne!(
        status_reply(&checked, None),
        status_reply(&never_checked, None)
    );
}

/// A packaged build never runs a network check at all — `checked` stays
/// `false` forever on that flavour, so without consulting `flavour` first
/// this reply would be indistinguishable from "never checked" on a build
/// that will NEVER check, rather than its own `managedBy` marker. Exercised
/// for every flavour, since each is a different wire value.
#[test]
fn test_status_reply_store_managed_wins_over_checked_state() {
    let state = UpdaterState {
        checked: true,
        ..UpdaterState::default()
    };
    for (flavour, wire) in [
        (PackageFlavour::MsStore, "msstore"),
        (PackageFlavour::Snap, "snap"),
    ] {
        assert_eq!(
            status_reply(&state, Some(flavour)),
            json!({ "available": false, "managedBy": wire })
        );
    }
}

#[test]
fn test_status_reply_available_with_the_pending_version_once_checked() {
    let state = UpdaterState {
        pending_version: Some("2.5.0".to_string()),
        ..UpdaterState::default()
    };
    assert_eq!(
        status_reply(&state, None),
        json!({ "available": true, "version": "2.5.0" })
    );
}

#[test]
fn test_status_reply_reads_pending_version_not_downloaded_bytes() {
    // A finished download still reports the PENDING version — `updater_install`'s proof source
    // reads it here, not off `downloaded_bytes`, which carries no version string of its own.
    let state = UpdaterState {
        pending_version: Some("3.0.0".to_string()),
        downloaded_bytes: Some(vec![1, 2, 3]),
        ..UpdaterState::default()
    };
    assert_eq!(
        status_reply(&state, None),
        json!({ "available": true, "version": "3.0.0" })
    );
}

// ── Packaged-build flavours (MSIX / Snap) ──────────────────────────────────────
//
// An NSIS/MSI/AppImage/.deb install (no flavour) must keep checking GitHub —
// `test_status_reply_unknown_when_nothing_pending_and_never_checked` above
// already covers `status_reply`'s `None` case. `store_managed` itself no
// longer takes an `Option` (round-2 review: nothing kept it in sync with
// `updater_check`'s hand-rolled reply — `updater_check` now calls it
// directly), so there is nothing left to assert of it for the unpackaged
// case.

/// Anchored on the FIELDS — the thing `UpdateCheckResult` in
/// `packages/shared/src/ipc/contracts/updater.ts` actually declares — so a
/// renamed or dropped field fails while a serializer that reorders keys does
/// not. (Comparing serialized strings would invent a key-order invariant the
/// IPC contract does not have.) Every flavour gets its own wire value — the
/// whole point of the finding this replaced (`managedBy: "store"` telling a
/// Snap user they installed from the Microsoft Store).
#[test]
fn test_store_managed_has_the_contract_shape() {
    assert_eq!(
        store_managed(PackageFlavour::MsStore),
        json!({ "available": false, "managedBy": "msstore" })
    );
    assert_eq!(
        store_managed(PackageFlavour::Snap),
        json!({ "available": false, "managedBy": "snap" })
    );
}

/// The pushed shape the renderer's `managed` status variant matches on, one
/// flavour at a time.
#[test]
fn test_managed_status_has_the_contract_shape() {
    assert_eq!(
        managed_status(PackageFlavour::MsStore),
        json!({ "state": "managed", "by": "msstore" })
    );
    assert_eq!(
        managed_status(PackageFlavour::Snap),
        json!({ "state": "managed", "by": "snap" })
    );
}

/// A packaged build's download/install refusal is an `error` reply — the
/// shape the renderer already renders — not a silent no-op that would look
/// like success. Each flavour names ITSELF, not always "the Microsoft
/// Store" — a Snap user must not be told they installed from the Store.
#[test]
fn test_store_managed_refusal_is_an_error_reply() {
    let msstore = store_managed_refusal(PackageFlavour::MsStore);
    assert!(msstore
        .get("error")
        .and_then(|e| e.as_str())
        .is_some_and(|m| m.contains("Microsoft Store")));

    let snap = store_managed_refusal(PackageFlavour::Snap);
    assert!(snap
        .get("error")
        .and_then(|e| e.as_str())
        .is_some_and(|m| m.contains("Snap Store") && !m.contains("Microsoft Store")));
}

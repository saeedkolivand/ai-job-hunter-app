//! Pairing-revocation-on-rotation tests — redistributed from the crate-level `test.rs` (R8
//! relief).
//!
//! A rotation used to leave live authenticated sockets running and the `connected` count up: the
//! desktop reported "connected" for a pairing whose secret no longer existed, and the extension —
//! whose reconnect gets the deliberately silent failed-handshake close (indistinguishable from a
//! crashed app) — retried the dead token forever instead of showing its pairing view. Rotation now
//! revokes first: signal every live socket, zero the count, THEN swap the secret.

use super::super::*;

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
fn rotation_revokes_live_sockets_then_swaps_the_token() {
    use tokio::sync::broadcast::error::TryRecvError;

    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());
    // A connection task that exists BEFORE the rotation (subscribed at accept
    // time, exactly as `handle_connection` does).
    let mut live = s.subscribe_revoke();
    assert!(
        matches!(live.try_recv(), Err(TryRecvError::Empty)),
        "no revoke signal before a rotation"
    );
    s.inc_connected();
    assert!(s.is_connected());
    let before = s.token();

    let after = s.regenerate_token();

    assert!(
        live.try_recv().is_ok(),
        "every socket live at rotation time is signalled to revoke its pairing"
    );
    assert_ne!(before, after, "and the token itself is rotated");
    assert_eq!(s.token(), after);
    assert!(
        !s.is_connected(),
        "no pairing survives a rotation — the live-connection count is zeroed \
         immediately, not once each socket happens to finish tearing down"
    );

    // The revoked socket's own teardown still runs `dec_connected`; it must
    // saturate at zero rather than wrap `AtomicUsize` (which `is_connected`
    // would misread as connected again).
    assert!(
        !s.dec_connected(),
        "a revoked socket's teardown reports no 1→0 transition (already zero)"
    );
    assert!(!s.is_connected());

    // A connection accepted AFTER the rotation is handshaking against the NEW
    // token — it must never receive the previous rotation's signal (a broadcast
    // is an edge, not replayed state), or it would revoke itself on connect.
    let mut fresh = s.subscribe_revoke();
    assert!(
        matches!(fresh.try_recv(), Err(TryRecvError::Empty)),
        "a socket accepted after the rotation is not told its pairing was revoked"
    );
}

#[test]
fn factory_reset_revokes_live_sockets_too() {
    use crate::data_store::Resettable;

    // The reset hook and Settings → "Regenerate" must not diverge: both go
    // through `regenerate_token`, so both revoke.
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());
    let mut live = s.subscribe_revoke();
    s.inc_connected();

    s.reset();

    assert!(
        live.try_recv().is_ok(),
        "a factory reset revokes live pairings, not just Settings → Regenerate"
    );
    assert!(!s.is_connected());
}

#[test]
fn a_revoked_sockets_late_teardown_cannot_steal_a_newer_pairings_count() {
    // The count-theft race: socket A is parked in a long dispatch await (an
    // `import.request` fetch, a `match.live` scrape), so it has NOT polled its
    // revoke receiver yet when the token rotates. Meanwhile browser C re-pairs
    // on the NEW token. When A finally drains the buffered revoke and tears
    // down, a blind `dec_connected` would take C's live pairing 1→0 — leaving
    // `is_connected()` reporting "no extension" while C is genuinely paired,
    // with nothing to correct it until C's own socket closes.
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());

    // Socket A authenticates and stamps the epoch it counted itself under.
    s.inc_connected();
    let a_epoch = s.rotation_epoch();
    assert!(s.is_connected());

    // Rotation: A's pairing is revoked, the count is zeroed, the epoch moves.
    s.regenerate_token();
    assert!(!s.is_connected());

    // Browser C re-pairs on the new token, under the NEW epoch.
    s.inc_connected();
    let c_epoch = s.rotation_epoch();
    assert_ne!(a_epoch, c_epoch, "the rotation must move the epoch");
    assert!(s.is_connected());

    // A finally tears down — long after the rotation. It must NOT give back a
    // count that now belongs to C.
    assert!(
        !s.dec_connected_for_epoch(a_epoch),
        "a revoked socket's late teardown reports no transition"
    );
    assert!(
        s.is_connected(),
        "C's live pairing must survive A's late teardown"
    );

    // C's own teardown still works normally — the guard only rejects stale epochs.
    assert!(
        s.dec_connected_for_epoch(c_epoch),
        "the current epoch's socket still reports the real 1→0 transition"
    );
    assert!(!s.is_connected());
}

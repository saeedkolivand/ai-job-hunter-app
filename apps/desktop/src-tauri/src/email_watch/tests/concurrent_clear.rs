use super::*;

// ── Concurrent-clear guard on the trailing writes ─────────────────────────────
//
// `set_enabled`/`record_check` can be called by a command mid-`spawn_blocking`
// IMAP validation (`connect`/`check_now`); a `disconnect` (`clear()`) racing
// in first must make the trailing write a no-op instead of resurrecting a
// field on the wiped row (worst case: `enabled=1` with `address=NULL`, which
// a LATER `connect` would silently inherit since `connect` never touches
// `enabled`). These tests interleave `clear()` between a "read" (the account
// was connected) and each trailing write to pin the guard.

#[test]
fn set_enabled_after_a_concurrent_clear_stays_disabled_and_address_stays_null() {
    let (_dir, store) = connected_store();

    // Simulate a `disconnect` landing between the caller's read and this
    // write (e.g. while a sibling command was awaiting `spawn_blocking`).
    store.clear().unwrap();

    assert!(
        !store.set_enabled(true).unwrap(),
        "set_enabled must report a no-op after a concurrent clear"
    );
    let status = store.status();
    assert!(
        !status.enabled,
        "enabled must NOT be resurrected on the wiped row"
    );
    assert!(status.address.is_none(), "address must stay cleared");
}

#[test]
fn record_check_after_a_concurrent_clear_leaves_last_check_ms_null() {
    let (_dir, store) = connected_store();

    store.clear().unwrap();

    assert!(
        !store.record_check(5_000).unwrap(),
        "record_check must report a no-op after a concurrent clear"
    );
    assert!(
        store.status().last_check_at.is_none(),
        "last_check_ms must NOT be resurrected on the wiped row"
    );
}

// The poller's tick (`email_watch_scheduler::run_check_inner`) awaits a
// multi-second `spawn_blocking` IMAP round trip BEFORE calling any of these
// three — a `disconnect`/factory reset landing during that window must make
// each a no-op, exactly like `set_enabled`/`record_check` above (/review
// second HIGH — these three were the ones missing the guard).

#[test]
fn advance_last_uid_after_a_concurrent_clear_leaves_last_uid_null() {
    let (_dir, store) = connected_store();

    store.clear().unwrap();

    assert!(
        !store.advance_last_uid(100).unwrap(),
        "advance_last_uid must report a no-op after a concurrent clear"
    );
    assert_eq!(
        store.account().last_uid,
        None,
        "last_uid must NOT be resurrected on the wiped row"
    );
}

#[test]
fn mark_seen_after_a_concurrent_clear_does_not_insert_a_row() {
    let (_dir, store) = connected_store();

    store.clear().unwrap();

    assert!(
        !store.mark_seen("uid-1", Some("app-1"), 5_000).unwrap(),
        "mark_seen must report a no-op after a concurrent clear"
    );
    assert!(
        !store.has_seen("uid-1"),
        "no seen row may be inserted against a just-wiped account"
    );
}

#[test]
fn reset_on_uidvalidity_change_after_a_concurrent_clear_stays_a_no_op() {
    let (_dir, store) = connected_store();
    store.reset_on_uidvalidity_change(42).unwrap();

    store.clear().unwrap();

    assert!(
        !store.reset_on_uidvalidity_change(43).unwrap(),
        "reset_on_uidvalidity_change must report a no-op after a concurrent clear"
    );
    let account = store.account();
    assert_eq!(
        account.uidvalidity, None,
        "uidvalidity must NOT be resurrected on the wiped row"
    );
    assert_eq!(account.last_uid, None);
}

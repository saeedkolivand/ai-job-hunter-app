use super::*;

// ── Seen dedupe ───────────────────────────────────────────────────────────────

#[test]
fn seen_dedupe_insert_and_check() {
    let (_dir, store) = connected_store();
    assert!(!store.has_seen("uid-1"));
    store.mark_seen("uid-1", None, 1_000).unwrap();
    assert!(store.has_seen("uid-1"));
    // Re-marking the same uid must not error (INSERT OR IGNORE) and must not
    // clobber the dedupe row's presence.
    store.mark_seen("uid-1", Some("app-1"), 2_000).unwrap();
    assert!(store.has_seen("uid-1"));
    assert!(
        !store.has_seen("uid-2"),
        "an unmarked uid must read as unseen"
    );
}

#[test]
fn last_match_at_reflects_only_matched_seen_rows() {
    let (_dir, store) = connected_store();
    store.mark_seen("uid-1", None, 1_000).unwrap();
    assert!(
        store.status().last_match_at.is_none(),
        "an unmatched seen row must not count as a match"
    );
    store.mark_seen("uid-2", Some("app-1"), 2_000).unwrap();
    assert_eq!(store.status().last_match_at, Some(2_000));
}

// ── UIDVALIDITY reset semantics ───────────────────────────────────────────────

#[test]
fn uidvalidity_change_resets_last_uid_only_when_it_actually_changes() {
    let (_dir, store) = connected_store();
    // First observation: nothing stored yet → always reported as "changed".
    assert!(store.reset_on_uidvalidity_change(42).unwrap());
    store.advance_last_uid(100).unwrap();
    assert_eq!(store.account().last_uid, Some(100));

    // Same uidvalidity again → no-op; the watermark must survive untouched.
    assert!(!store.reset_on_uidvalidity_change(42).unwrap());
    assert_eq!(store.account().last_uid, Some(100));

    // A genuinely new uidvalidity → reset; the stale watermark is dropped.
    assert!(store.reset_on_uidvalidity_change(43).unwrap());
    assert_eq!(store.account().last_uid, None);
    assert_eq!(store.account().uidvalidity, Some(43));
}

#[test]
fn uidvalidity_change_wipes_stale_seen_rows_but_a_same_value_flip_does_not() {
    // /review HIGH: uids are unique only per (mailbox, uidvalidity)
    // generation — a `seen` row surviving a renumber would make a REUSED low
    // uid in the re-scan window read as already-considered, silently
    // swallowing a real confirmation forever. Mirrors `connect`'s own
    // address-changed branch, which already wipes `seen` for the identical
    // per-generation hazard.
    let (_dir, store) = connected_store();
    store.reset_on_uidvalidity_change(42).unwrap(); // first observation
    store.advance_last_uid(10).unwrap();
    store.mark_seen("10", None, 1_000).unwrap();
    assert!(store.has_seen("10"));

    // Same uidvalidity again → no-op; watermark AND seen both survive untouched.
    assert!(!store.reset_on_uidvalidity_change(42).unwrap());
    assert!(
        store.has_seen("10"),
        "a same-value flip must not touch seen"
    );
    assert_eq!(store.account().last_uid, Some(10));

    // A genuinely new uidvalidity → the OLD generation's seen row must be
    // gone (uid "10" could be reused under the new numbering).
    assert!(store.reset_on_uidvalidity_change(43).unwrap());
    assert!(
        !store.has_seen("10"),
        "a stale seen row from the OLD uidvalidity generation must not survive a reset"
    );
    assert_eq!(store.account().last_uid, None);
}

#[test]
fn advance_last_uid_never_rewinds_the_watermark() {
    // The `MAX` in the UPDATE enforces this at the database, not just by
    // caller convention (rust-backend-architect advisory #2) — a lower uid
    // than what's already stored (a stale caller, or a reordered concurrent
    // write) must be a no-op, never a rewind.
    let (_dir, store) = connected_store();
    store.advance_last_uid(100).unwrap();
    assert_eq!(store.account().last_uid, Some(100));

    store.advance_last_uid(50).unwrap();
    assert_eq!(
        store.account().last_uid,
        Some(100),
        "a lower uid must not rewind it"
    );

    store.advance_last_uid(150).unwrap();
    assert_eq!(
        store.account().last_uid,
        Some(150),
        "a higher uid still advances it"
    );
}

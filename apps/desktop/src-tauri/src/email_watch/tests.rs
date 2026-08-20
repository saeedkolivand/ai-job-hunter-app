//! Store tests for [`EmailWatchStore`] — the invariant lock for PR A's
//! foundation (no poller/parser/matcher exists yet; these tests lock the
//! schema/API contract those will build on in PR B).

use tempfile::TempDir;

use super::{EmailWatchAccount, EmailWatchStore, CREDENTIAL_SLOT};
use crate::credentials::{install_mock_keyring, CredentialStore};

fn new_store() -> (TempDir, EmailWatchStore) {
    let dir = TempDir::new().unwrap();
    let store = EmailWatchStore::open(&dir.path().to_path_buf()).expect("open store");
    (dir, store)
}

// ── Defaults ────────────────────────────────────────────────────────────────

#[test]
fn unconfigured_store_has_no_account() {
    let (_dir, store) = new_store();
    let status = store.status();
    assert!(!status.connected);
    assert!(status.address.is_none());
    assert!(!status.enabled);
    assert!(status.last_check_at.is_none());
    assert!(status.last_match_at.is_none());

    assert_eq!(store.account(), EmailWatchAccount::default());
}

/// Pins `auto_write_enabled`'s default explicitly. Originally shipped
/// default ON; the owner's decision after a residual the parser cannot
/// close by content inspection alone was found (a GENUINE
/// `Authentication-Results` stamp from a known-stamping host that simply
/// carries no `dmarc=` clause for the attacker's chosen `From:` domain —
/// indistinguishable from real grammar, because it IS real grammar) is that
/// auto-write ships OFF, opt-in only, adjudication remaining the backstop.
/// A future change to the migration's `DEFAULT` must be a deliberate,
/// reviewed decision — this test fails loudly if it silently reverts.
/// True even before any account is ever connected (the account row always
/// exists post-migration — see `create_email_watch`'s own `INSERT OR
/// IGNORE`).
#[test]
fn auto_write_enabled_defaults_off() {
    let (_dir, store) = new_store();
    assert!(!store.auto_write_enabled());
    assert!(!store.status().auto_write_enabled);
}

/// Forces `auto_write_enabled_conn`'s error branch to actually run — the
/// test above only ever exercises the happy-path row read (a real `0`),
/// leaving the fallback itself uncovered. Drops the exact column the read
/// selects so the query genuinely errors (not a stand-in for "no row" or
/// "wrong type" — a real `rusqlite::Error` from the same store this whole
/// module otherwise treats as healthy), then asserts the read still comes
/// back `false`. This is the safe direction now that the shipped default
/// is OFF; it was NOT the safe direction when this fallback was written
/// (see `auto_write_enabled_conn`'s own doc) — a future default flip must
/// re-derive this fallback's direction too, not just the migration.
#[test]
fn auto_write_enabled_fails_closed_on_a_genuine_read_error() {
    let (_dir, store) = new_store();
    store
        .conn
        .lock()
        .execute_batch("ALTER TABLE account DROP COLUMN auto_write_enabled;")
        .expect("drop column to force a genuine read error");

    assert!(
        !store.auto_write_enabled(),
        "a read error must fail CLOSED (false), not toward the old ON default"
    );
    assert!(!store.status().auto_write_enabled);
}

// ── Connect / disconnect roundtrip (mock keyring for the credential half) ────

#[test]
fn connect_persists_account_and_credential_then_disconnect_clears_both() {
    install_mock_keyring();
    let (dir, store) = new_store();
    let credentials = CredentialStore::new(&dir.path().to_path_buf());

    store
        .connect("jane@gmail.com", "imap.gmail.com", 993)
        .expect("connect");
    credentials
        .set(CREDENTIAL_SLOT, "jane@gmail.com", "app-password-1234")
        .expect("set credential");

    let status = store.status();
    assert!(status.connected);
    assert_eq!(status.address.as_deref(), Some("jane@gmail.com"));
    assert!(!status.enabled, "connect must not auto-enable the poller");
    assert_eq!(
        credentials.get_decrypted(CREDENTIAL_SLOT),
        Some((
            "jane@gmail.com".to_string(),
            "app-password-1234".to_string()
        )),
    );

    // Disconnect: the command layer clears the store AND removes the
    // credential separately — exercise both halves here.
    store.clear().expect("clear");
    credentials
        .remove(CREDENTIAL_SLOT)
        .expect("remove credential");

    let status = store.status();
    assert!(!status.connected);
    assert!(status.address.is_none());
    assert_eq!(credentials.get_decrypted(CREDENTIAL_SLOT), None);
}

#[test]
fn reconnect_preserves_enabled_and_watermark() {
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    store.set_enabled(true).unwrap();
    store.record_check(1_000).unwrap();

    // Reconnecting (e.g. re-entering the app password) must not reset the
    // opt-in or the last-check watermark.
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    let status = store.status();
    assert!(status.enabled, "reconnect must preserve the enabled flag");
    assert_eq!(status.last_check_at, Some(1_000));
}

#[test]
fn connect_to_a_different_address_clears_uid_watermark_and_seen_but_not_enabled() {
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    store.set_enabled(true).unwrap();
    // reset_on_uidvalidity_change(7) itself nulls last_uid as a side effect of
    // the "changed" branch (nothing was stored yet) — advance it afterward so
    // there is a real non-null watermark + seen row to prove gets cleared.
    store.reset_on_uidvalidity_change(7).unwrap();
    store.advance_last_uid(100).unwrap();
    store.mark_seen("uid-100", Some("app-1"), 1_000).unwrap();
    assert_eq!(store.account().last_uid, Some(100));
    assert_eq!(store.account().uidvalidity, Some(7));
    assert!(store.has_seen("uid-100"));

    // Reconnecting to the SAME address must preserve the watermark + seen row
    // (mirrors `reconnect_preserves_enabled_and_watermark` for these fields).
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    assert_eq!(
        store.account().last_uid,
        Some(100),
        "same-address reconnect must preserve last_uid"
    );
    assert_eq!(
        store.account().uidvalidity,
        Some(7),
        "same-address reconnect must preserve uidvalidity"
    );
    assert!(
        store.has_seen("uid-100"),
        "same-address reconnect must preserve seen rows"
    );

    // Connecting to a DIFFERENT address must clear the UID watermark and the
    // seen table (numeric UIDs are per-mailbox — carrying one over could
    // collide with the new mailbox's own numbering and silently suppress a
    // real future match), but must NOT touch the enabled opt-in.
    store.connect("b@gmail.com", "imap.gmail.com", 993).unwrap();
    let account = store.account();
    assert_eq!(
        account.last_uid, None,
        "a different address must clear last_uid"
    );
    assert_eq!(
        account.uidvalidity, None,
        "a different address must clear uidvalidity"
    );
    assert!(
        !store.has_seen("uid-100"),
        "a different address must clear seen rows"
    );
    assert!(
        store.status().enabled,
        "enabled is independent of the address and must survive"
    );
}

// ── Enabled toggle ────────────────────────────────────────────────────────────

#[test]
fn set_enabled_toggles_once_an_account_is_connected() {
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    assert!(!store.status().enabled, "default is OFF");
    assert!(
        store.set_enabled(true).unwrap(),
        "must report the row updated"
    );
    assert!(store.status().enabled);
    assert!(store.set_enabled(false).unwrap());
    assert!(!store.status().enabled);
}

#[test]
fn set_enabled_is_a_no_op_without_a_connected_account() {
    // No `connect()` — address is NULL, same shape as a just-cleared account.
    let (_dir, store) = new_store();
    assert!(
        !store.set_enabled(true).unwrap(),
        "set_enabled must report a no-op with no account configured"
    );
    assert!(!store.status().enabled, "enabled must stay OFF");
}

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
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();

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
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();

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
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();

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
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();

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
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
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

// ── Seen dedupe ───────────────────────────────────────────────────────────────

#[test]
fn seen_dedupe_insert_and_check() {
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
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
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
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
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
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
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
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
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
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

// ── Factory reset (Resettable calls `clear()`; see commands/privacy.rs) ──────

#[test]
fn clear_wipes_account_and_seen_rows() {
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    store.set_enabled(true).unwrap();
    store.record_check(5_000).unwrap();
    store.mark_seen("uid-1", Some("app-1"), 5_000).unwrap();
    assert!(store.status().connected, "precondition: account configured");

    store.clear().expect("clear");

    let status = store.status();
    assert!(!status.connected);
    assert!(status.address.is_none());
    assert!(!status.enabled);
    assert!(status.last_check_at.is_none());
    assert!(status.last_match_at.is_none());
    assert!(!store.has_seen("uid-1"), "seen rows must be wiped too");
}

/// MEDIUM fix: `clear()` (disconnect/reconnect of the SAME mailbox address)
/// deliberately does NOT reset `auto_write_enabled` — pins the behavior
/// (see `clear()`'s own doc for why). This is now DELIBERATELY narrower
/// than an earlier version of this test claimed: `clear()` is disconnect
/// ONLY — a privacy factory reset goes through [`super::EmailWatchStore::
/// factory_reset`] instead (see `factory_reset_does_reset_the_auto_write_opt_in`
/// right below), which DOES reset this column, because the next-connected
/// mailbox after a factory reset may be a different account than whichever
/// one made this opt-in choice.
///
/// **Tests the OPT-IN direction, not opt-out** — `auto_write_enabled` now
/// defaults OFF (flipped from its original default-ON; see the
/// `add_auto_write_enabled` migration's own doc). Opting OUT (`false`) is
/// now the SAME value as the default, so a test that set `false` and
/// asserted it stayed `false` after `clear()` would pass even if a future
/// regression made `clear()` reset the column to its default — a test that
/// passes for the wrong reason, catching nothing. Setting the NON-default
/// value (`true`, an explicit opt-IN) is what makes this test actually
/// exercise "`clear()` does not touch this column at all", regardless of
/// which direction is the default.
#[test]
fn clear_does_not_reset_the_auto_write_opt_in() {
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    assert!(
        store.set_auto_write_enabled(true).unwrap(),
        "precondition: the toggle write must succeed while connected"
    );
    assert!(store.status().auto_write_enabled, "precondition: opted in");

    store.clear().expect("clear");

    // `auto_write_enabled` has no `address IS NOT NULL` guard on its OWN
    // read path, so it is readable even post-clear — reconnecting the SAME
    // account afterward must not find it silently reset to the (now OFF)
    // default.
    assert!(
        store.status().auto_write_enabled,
        "a user's DELIBERATE auto-write opt-in must survive a disconnect (clear())"
    );
}

/// MEDIUM fix, the other half: unlike `clear()`, a privacy factory reset
/// MUST zero `auto_write_enabled` — the account connected next may not be
/// the one that made this choice, and ADR-0013 lists "opt-in default,
/// nobody exposed without asking" as mitigation #1 for the
/// `dmarc_pass_aligned` residual. `Resettable::reset` in `commands/privacy.rs`
/// calls `factory_reset`, not `clear` — this pins that `factory_reset`
/// itself has the right behavior independent of that wiring (which is its
/// own, separate concern).
#[test]
fn factory_reset_does_reset_the_auto_write_opt_in() {
    let (_dir, store) = new_store();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    assert!(
        store.set_auto_write_enabled(true).unwrap(),
        "precondition: the toggle write must succeed while connected"
    );
    assert!(store.status().auto_write_enabled, "precondition: opted in");

    store.factory_reset().expect("factory_reset");

    assert!(
        !store.status().auto_write_enabled,
        "a factory reset must clear the opt-in — the next connected mailbox \
         may be a different account than whichever one chose it"
    );
}

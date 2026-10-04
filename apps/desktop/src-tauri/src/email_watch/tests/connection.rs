use super::*;
use crate::credentials::{install_mock_keyring, CredentialStore};

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
    let (_dir, store) = connected_store();
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
    let (_dir, store) = connected_store();
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

/// MEDIUM fix: `connect()` to a DIFFERENT address used to carry the
/// PREVIOUS mailbox's `auto_write_enabled` opt-in onto the new one —
/// consent is per-account, and nothing about a different address implies
/// that account made the same choice. Currently unreachable through
/// `EmailWatchSection` (the connect form only renders when disconnected),
/// but the invariant belongs to the store's own state transition, not to
/// which form a renderer happens to show — see `connect()`'s own doc.
/// Scoped to `auto_write_enabled` alone: `enabled` must still survive
/// (already pinned by
/// `connect_to_a_different_address_clears_uid_watermark_and_seen_but_not_enabled`
/// right above).
#[test]
fn connect_to_a_different_address_resets_the_auto_write_opt_in() {
    let (_dir, store) = connected_store();
    assert!(store.set_auto_write_enabled(true).unwrap());
    assert!(store.status().auto_write_enabled, "precondition: opted in");

    // Reconnecting to the SAME address must preserve the opt-in — this is
    // the ordinary "re-enter a rotated app password" path, not a new
    // account, and mirrors the same-address preservation already pinned
    // for the UID watermark/seen rows above.
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    assert!(
        store.status().auto_write_enabled,
        "same-address reconnect must preserve the auto-write opt-in"
    );

    // A DIFFERENT address must reset it — mailbox B never made this choice.
    store.connect("b@gmail.com", "imap.gmail.com", 993).unwrap();
    assert!(
        !store.status().auto_write_enabled,
        "a different address must reset the auto-write opt-in — it is \
         per-account consent, not per-store"
    );
}

// ── Enabled toggle ────────────────────────────────────────────────────────────

#[test]
fn set_enabled_toggles_once_an_account_is_connected() {
    let (_dir, store) = connected_store();
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

// ── Factory reset (Resettable calls `clear()`; see commands/privacy.rs) ──────

#[test]
fn clear_wipes_account_and_seen_rows() {
    let (_dir, store) = connected_store();
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

/// HIGH fix: `clear()` used to preserve `auto_write_enabled` across a
/// disconnect, reasoning that a disconnect/reconnect is "the user
/// re-authenticating the SAME mailbox address they already made this
/// choice about" — the premise was unverifiable BY CONSTRUCTION: the SAME
/// `UPDATE` that would preserve the flag also sets `address = NULL`, so
/// nothing survives to tell a later `connect()` whether the reconnecting
/// mailbox is the same one or a stranger. `EmailWatchSection` only ever
/// offers Disconnect (no in-place re-auth), so this was reachable for
/// real, not hypothetical: connect A, opt in, disconnect, connect B — B's
/// switch would render ON. `clear()` now resets `auto_write_enabled`
/// unconditionally (see `clear()`'s own doc for the fuller account,
/// including why a prior attempt to split this into `clear`/
/// `factory_reset` — preserve on disconnect, reset on privacy reset — was
/// itself wrong: it just hid the unverifiable premise behind whichever
/// call site happened to run, rather than removing it). This test is
/// INVERTED from an earlier version that asserted the opposite (opt-in
/// survives `clear()`) — see git history for that version, which pinned
/// exactly the behavior this fix removes.
///
/// **Tests the OPT-IN direction, not opt-out** — `auto_write_enabled`
/// defaults OFF, so opting OUT (`false`) is the SAME value the column
/// would read as even if `clear()` did nothing to it at all; a test that
/// set `false` and asserted `false` after `clear()` would pass whether or
/// not the reset actually ran, catching nothing. Setting the NON-default
/// value (`true`) first is what makes "still `false` after `clear()`"
/// mean the reset genuinely happened.
#[test]
fn clear_resets_the_auto_write_opt_in() {
    let (_dir, store) = connected_store();
    assert!(
        store.set_auto_write_enabled(true).unwrap(),
        "precondition: the toggle write must succeed while connected"
    );
    assert!(store.status().auto_write_enabled, "precondition: opted in");

    store.clear().expect("clear");

    assert!(
        !store.status().auto_write_enabled,
        "clear() must reset the auto-write opt-in unconditionally — a \
         disconnect/reconnect cannot verify it is the same mailbox that \
         made this choice, since clear() itself destroys the only thing \
         (the address) that could have proven it"
    );
}

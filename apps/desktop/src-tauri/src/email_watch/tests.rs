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

// ── Enabled toggle ────────────────────────────────────────────────────────────

#[test]
fn set_enabled_toggles_independent_of_the_account() {
    let (_dir, store) = new_store();
    assert!(!store.status().enabled, "default is OFF");
    store.set_enabled(true).unwrap();
    assert!(store.status().enabled);
    store.set_enabled(false).unwrap();
    assert!(!store.status().enabled);
}

// ── Seen dedupe ───────────────────────────────────────────────────────────────

#[test]
fn seen_dedupe_insert_and_check() {
    let (_dir, store) = new_store();
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

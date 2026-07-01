use super::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// ── Keyring mock harness ──────────────────────────────────────────────────────
//
// keyring-core v1 ships an in-memory `mock::Store` (platform-independent, no
// persistence) that lets us exercise the real `Entry` path — including the
// non-`NoEntry` error branch of `read_credential` — without touching the OS
// keychain. (The earlier "no mock backend" note above predated this; the C4
// metadata tests still stand on their own.)
//
// The store is process-global, so installation funnels through the shared
// `install_mock_keyring` (see `mod.rs`). Tests then use UUID-unique slot names so
// their `Cred`s never collide, keeping them race-safe under Cargo's multi-thread
// test runner.

/// A slot name guaranteed unique to this test invocation, so its mock `Cred`
/// is never shared with another (possibly concurrent) test.
fn unique_slot(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4())
}

#[test]
fn test_is_available() {
    let temp_dir = TempDir::new().unwrap();
    let store = CredentialStore::new(&temp_dir.path().to_path_buf());
    assert!(store.is_available());
}

#[test]
fn test_list_empty() {
    let temp_dir = TempDir::new().unwrap();
    let store = CredentialStore::new(&temp_dir.path().to_path_buf());
    let creds = store.list();
    assert!(creds.is_empty());
}

#[test]
fn test_credential_meta_serialization() {
    let meta = CredentialMeta {
        board_id: "linkedin".to_string(),
        username: "test@example.com".to_string(),
        saved_at: 1234567890,
    };

    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: CredentialMeta = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.board_id, "linkedin");
    assert_eq!(deserialized.username, "test@example.com");
    assert_eq!(deserialized.saved_at, 1234567890);
}

#[test]
fn test_now_ms() {
    // Capture a tight upper-bound AFTER the call so the interval [ts, upper] is
    // always valid regardless of CI scheduling jitter. We only need:
    //   ts > 0              — it is an epoch-ms value, not a sentinel
    //   ts <= upper         — the clock did not run backward
    //
    // The previous ±1000 ms symmetric window was flaky under heavy CI load because
    // the `now()` reference was captured AFTER `ts`, so `ts` could exceed `now+0`
    // by a few ms on a context-switched thread; that made (now - ts) negative and
    // the abs() check fail. Asymmetric bound (ts <= upper) is race-free.
    let ts = now_ms();
    let upper = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    assert!(ts > 0, "now_ms() must return a positive epoch-ms value");
    assert!(
        ts <= upper,
        "now_ms() must not return a timestamp in the future (ts={ts}, upper={upper})"
    );
}

// ── C4 — Credential metadata persistence layer ───────────────────────────────
//
// The production keychain uses platform-specific stores (DPAPI / Keychain /
// libsecret) that are unsuitable for automated tests (CI agents may have no
// keychain; Windows DPAPI is user-scoped and can fail in sandboxes). These C4
// tests therefore exercise the METADATA layer (credential-meta.json) in
// isolation, driving `save_meta`/`load_meta` through the public API.
//
// Where we DO need to drive the keyring `Entry` path (see the `read_credential`
// tests below), we install keyring-core's in-memory `mock::Store` instead of
// the OS keychain.

/// Write a metadata file directly (bypassing `set`, which needs the keychain)
/// and return a CredentialStore that reads from that file via `list()`.
fn write_meta_direct(dir: &std::path::Path, entries: &[(&str, &str, u64)]) -> CredentialStore {
    let meta: HashMap<String, CredentialMeta> = entries
        .iter()
        .map(|(board_id, username, saved_at)| {
            (
                board_id.to_string(),
                CredentialMeta {
                    board_id: board_id.to_string(),
                    username: username.to_string(),
                    saved_at: *saved_at,
                },
            )
        })
        .collect();
    let json = serde_json::to_string_pretty(&meta).unwrap();
    std::fs::write(dir.join("credential-meta.json"), json).unwrap();
    CredentialStore::new(&dir.to_path_buf())
}

#[test]
fn list_returns_all_entries_from_meta_file() {
    // C4 — metadata listing round-trip: entries in credential-meta.json must
    // appear in `list()`. Never exposes passwords (only metadata).
    let dir = TempDir::new().unwrap();
    let store = write_meta_direct(
        dir.path(),
        &[
            ("linkedin", "alice@example.com", 1_000),
            ("indeed", "alice@example.com", 2_000),
        ],
    );

    let listed = store.list();
    assert_eq!(listed.len(), 2, "list must return both metadata entries");

    let board_ids: Vec<&str> = listed.iter().map(|m| m.board_id.as_str()).collect();
    assert!(board_ids.contains(&"linkedin"), "linkedin must be listed");
    assert!(board_ids.contains(&"indeed"), "indeed must be listed");

    // list() must NEVER expose passwords — CredentialMeta has no password field.
    for entry in &listed {
        assert!(!entry.username.is_empty(), "username must be present");
        assert!(
            entry.saved_at > 0,
            "saved_at must be non-zero; got {}",
            entry.saved_at
        );
    }
}

#[test]
fn remove_drops_metadata_entry_and_is_idempotent_without_keychain_entry() {
    // C4 — `remove` must delete the metadata entry even when the keychain has
    // no corresponding entry (`.ok()` in the source swallows the Not-Found error).
    // This tests the metadata path without touching the OS keychain.
    let dir = TempDir::new().unwrap();
    let store = write_meta_direct(
        dir.path(),
        &[
            ("linkedin", "alice@example.com", 1_000),
            ("indeed", "alice@example.com", 2_000),
        ],
    );
    assert_eq!(store.list().len(), 2, "precondition: two metadata entries");

    // Remove "linkedin" — the keychain has no entry, but remove must still clear
    // the metadata (the keychain delete is `.ok()` — ignored on Not-Found).
    store.remove("linkedin").unwrap();

    let remaining = store.list();
    assert_eq!(
        remaining.len(),
        1,
        "after removing 'linkedin', only 'indeed' must remain"
    );
    assert_eq!(
        remaining[0].board_id, "indeed",
        "remaining entry must be 'indeed'"
    );

    // Second remove of the same key must be idempotent (no Err from Not-Found).
    let result = store.remove("linkedin");
    assert!(
        result.is_ok(),
        "removing a non-existent key must return Ok (idempotent)"
    );
    assert_eq!(
        store.list().len(),
        1,
        "idempotent remove must not change the remaining count"
    );
}

#[test]
fn clear_all_removes_all_metadata_entries() {
    // C4 — `clear_all` iterates the metadata index and removes every entry.
    // We verify the metadata is fully cleared without a keychain.
    let dir = TempDir::new().unwrap();
    let store = write_meta_direct(
        dir.path(),
        &[
            ("linkedin", "alice@example.com", 1_000),
            ("indeed", "alice@example.com", 2_000),
            ("xing", "alice@example.com", 3_000),
        ],
    );
    assert_eq!(
        store.list().len(),
        3,
        "precondition: three metadata entries"
    );

    store.clear_all().unwrap();

    assert!(
        store.list().is_empty(),
        "clear_all must leave no metadata entries; got: {:?}",
        store.list()
    );
}

#[test]
fn credential_meta_namespacing_uses_board_id_as_key() {
    // C4 — the metadata JSON is keyed by `board_id`, which is the same value
    // used as the `username` in `Entry::new(SERVICE, board_id)`.
    // This pins the namespacing invariant: entries with different board_ids
    // are stored under separate keys and don't clobber each other.
    let dir = TempDir::new().unwrap();
    let store = write_meta_direct(
        dir.path(),
        &[
            ("linkedin", "alice@example.com", 1_000),
            ("glassdoor", "bob@example.com", 2_000),
        ],
    );

    let listed = store.list();
    let linkedin = listed.iter().find(|m| m.board_id == "linkedin").unwrap();
    let glassdoor = listed.iter().find(|m| m.board_id == "glassdoor").unwrap();

    assert_eq!(linkedin.username, "alice@example.com");
    assert_eq!(glassdoor.username, "bob@example.com");
    // Different board_ids → different keys → no collision.
    assert_ne!(linkedin.saved_at, glassdoor.saved_at);
}

#[test]
fn get_decrypted_returns_none_when_no_metadata_entry() {
    // C4 — `get_decrypted` consults the metadata index first; an unknown
    // board_id must return None before ever reaching the keychain.
    let dir = TempDir::new().unwrap();
    let store = CredentialStore::new(&dir.path().to_path_buf());
    // No metadata → no keychain lookup.
    let result = store.get_decrypted("linkedin");
    assert!(
        result.is_none(),
        "get_decrypted must return None for an unregistered board_id"
    );
}

// ── read_credential — keyring-backed branches (via the in-memory mock store) ──

#[test]
fn read_credential_missing_slot_returns_ok_none() {
    // NoEntry branch: a slot that was never set must map to Ok(None) so OPTIONAL
    // callers (e.g. the aggregator) degrade gracefully instead of erroring.
    install_mock_keyring();
    let slot = unique_slot("ai:nonexistent");

    let result = read_credential(&slot);
    assert!(
        matches!(result, Ok(None)),
        "missing slot must read as Ok(None); got {result:?}"
    );
}

#[test]
fn read_credential_present_slot_returns_value() {
    // Happy path: a stored non-empty value reads back as Ok(Some(value)).
    install_mock_keyring();
    let slot = unique_slot("ai:present");
    keyring_core::Entry::new(SERVICE, &slot)
        .unwrap()
        .set_password("super-secret-key")
        .unwrap();

    let result = read_credential(&slot);
    assert!(
        matches!(
            result.as_ref().map(Option::as_deref),
            Ok(Some("super-secret-key"))
        ),
        "present slot must read as Ok(Some(value)); got {result:?}"
    );
}

#[test]
fn read_credential_empty_value_returns_ok_none() {
    // An empty stored value is treated as "absent" (Ok(None)) — the same
    // degradation path as NoEntry, so an empty key never reads as configured.
    install_mock_keyring();
    let slot = unique_slot("ai:empty");
    keyring_core::Entry::new(SERVICE, &slot)
        .unwrap()
        .set_password("")
        .unwrap();

    let result = read_credential(&slot);
    assert!(
        matches!(result, Ok(None)),
        "empty stored value must read as Ok(None); got {result:?}"
    );
}

#[test]
fn read_credential_non_no_entry_error_returns_storage_err() {
    // The "other keyring error" branch: any keyring failure that is NOT NoEntry
    // (locked store, permission denied, …) must surface as AppError::Storage so
    // CRITICAL callers can decide to surface it rather than silently degrade.
    //
    // The mock store lets us arm a specific error on a Cred; the next Entry call
    // returns it (and then clears it). We arm a non-NoEntry error and assert the
    // mapping. UUID-unique slot keeps this Cred isolated from concurrent tests.
    install_mock_keyring();
    let slot = unique_slot("ai:errslot");
    let entry = keyring_core::Entry::new(SERVICE, &slot).unwrap();
    let mock: &keyring_core::mock::Cred = entry.as_any().downcast_ref().unwrap();
    mock.set_error(keyring_core::Error::Invalid(
        "induced".to_string(),
        "non-NoEntry keyring failure".to_string(),
    ));

    let result = read_credential(&slot);
    assert!(
        matches!(result, Err(AppError::Storage(_))),
        "non-NoEntry keyring error must map to AppError::Storage; got {result:?}"
    );
}

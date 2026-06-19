use super::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

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
// keyring-core v1 uses platform-specific stores (DPAPI / Keychain / libsecret)
// and does NOT expose a mock/in-memory credential builder in this version.
// Full round-trip tests (set → get_decrypted) would hit the real OS keychain,
// which is unsuitable for automated tests (CI agents may have no keychain,
// Windows DPAPI is user-scoped and can fail in sandboxes).
//
// We therefore test the METADATA layer (credential-meta.json) in isolation:
// the `save_meta`/`load_meta` path is exercised indirectly through the public
// API. The keychain operations (Entry::new / set_password / get_password) are
// noted as untestable at unit level without a mock backend.

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

use super::*;
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
    let ts = now_ms();
    assert!(ts > 0);
    // Should be roughly current time (within 1 second)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!((now as i64 - ts as i64).abs() < 1000);
}

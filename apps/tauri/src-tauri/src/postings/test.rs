use tempfile::TempDir;

use super::*;

#[test]
fn test_postings_cache_default() {
    let cache = PostingsCache::default();
    assert!(cache.get_all().is_empty());
}

#[test]
fn test_postings_cache_add() {
    let mut cache = PostingsCache::default();
    let item = serde_json::json!({"id": "1", "title": "Test"});
    cache.add(item);
    assert_eq!(cache.get_all().len(), 1);
}

#[test]
fn test_postings_cache_clear() {
    let mut cache = PostingsCache::default();
    let item = serde_json::json!({"id": "1", "title": "Test"});
    cache.add(item);
    cache.clear_all();
    assert!(cache.get_all().is_empty());
}

#[test]
fn test_interaction_record_serialization() {
    let record = InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1234567890,
        title: "Software Engineer".to_string(),
        company: "Test Corp".to_string(),
        url: "https://example.com".to_string(),
        source: "linkedin".to_string(),
        location: "Remote".to_string(),
    };
    let json = serde_json::to_string(&record).unwrap();
    let deserialized: InteractionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.job_id, record.job_id);
    assert_eq!(deserialized.interaction_type, record.interaction_type);
}

#[test]
fn test_interaction_store_new() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let store = InteractionStore::new(&data_dir);
    assert!(store.data_file.ends_with("interactions.json"));
}

#[test]
fn test_interaction_store_list_no_filter() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let mut store = InteractionStore::new(&data_dir);
    let records = store.list(None);
    assert!(records.is_empty());
}

#[test]
fn test_interaction_store_list_with_filter() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let mut store = InteractionStore::new(&data_dir);
    let record = InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1234567890,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    };
    store.upsert(record.clone());
    let viewed = store.list(Some("viewed"));
    assert_eq!(viewed.len(), 1);
    let applied = store.list(Some("applied"));
    assert!(applied.is_empty());
}

#[test]
fn test_interaction_store_upsert_new() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let mut store = InteractionStore::new(&data_dir);
    let record = InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1234567890,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    };
    store.upsert(record.clone());
    let records = store.list(None);
    assert_eq!(records.len(), 1);
}

#[test]
fn test_interaction_store_upsert_update() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let mut store = InteractionStore::new(&data_dir);
    let mut record = InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1234567890,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    };
    store.upsert(record.clone());
    record.timestamp = 1234567891;
    store.upsert(record.clone());
    let records = store.list(None);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].timestamp, 1234567891);
}

#[test]
fn test_interaction_store_clear_all() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let mut store = InteractionStore::new(&data_dir);
    let record = InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1234567890,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    };
    store.upsert(record);
    store.clear_all();
    let records = store.list(None);
    assert!(records.is_empty());
}

#[test]
fn test_interaction_store_export_all() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let mut store = InteractionStore::new(&data_dir);
    let record = InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1234567890,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    };
    store.upsert(record.clone());
    let exported = store.export_all();
    assert_eq!(exported.len(), 1);
}

#[test]
fn corrupt_interactions_file_is_preserved_not_overwritten() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let data_file = data_dir.join("interactions.json");

    // Simulate a file that exists on disk but is malformed (truncated write,
    // disk corruption, manual edit). The old loader swallowed the parse error
    // and started from an empty map, so the next save wiped every interaction.
    std::fs::write(&data_file, b"{ this is not valid json ]").unwrap();

    let mut store = InteractionStore::new(&data_dir);
    // First access hydrates the cache; the corrupt file is moved aside.
    let records = store.list(None);
    assert!(records.is_empty(), "corrupt file loads as empty in-memory");

    // The original bytes are preserved in the backup, NOT silently discarded.
    let backup = data_dir.join("interactions.json.corrupt");
    assert!(backup.exists(), "corrupt file is backed up");
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        "{ this is not valid json ]",
        "backup keeps the original corrupt bytes"
    );

    // A subsequent mutation rewrites the primary file (now valid), but the
    // backup still holds the recoverable original.
    store.upsert(InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    });
    assert!(backup.exists(), "backup survives the next save");
}

#[test]
fn corrupt_file_with_failed_backup_blocks_save_keeping_original() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let data_file = data_dir.join("interactions.json");
    let backup = data_dir.join("interactions.json.corrupt");

    // Corrupt primary file on disk.
    let original = b"{ this is not valid json ]";
    std::fs::write(&data_file, original).unwrap();

    // Force the backup rename to FAIL cross-platform: the backup target already
    // exists as a NON-EMPTY directory, so renaming a file onto it errors on every
    // OS. This simulates the rename failing (file locked / cross-device / perms).
    std::fs::create_dir(&backup).unwrap();
    std::fs::write(backup.join("sentinel"), b"x").unwrap();

    let mut store = InteractionStore::new(&data_dir);
    // Hydrating sees the corrupt file, attempts the backup, and the rename fails.
    let records = store.list(None);
    assert!(records.is_empty(), "corrupt file loads as empty in-memory");
    assert!(store.block_save, "failed backup arms the save guard");

    // A mutation would normally rewrite the primary file. With the guard armed,
    // save MUST skip the write so the un-backed-up corrupt original is preserved.
    store.upsert(InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    });

    assert_eq!(
        std::fs::read(&data_file).unwrap(),
        original,
        "save did not overwrite the un-backed-up corrupt original"
    );
}

#[test]
fn successful_backup_leaves_save_unblocked() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let data_file = data_dir.join("interactions.json");

    // Corrupt primary file, no obstruction at the backup path → rename succeeds.
    std::fs::write(&data_file, b"{ not json ]").unwrap();

    let mut store = InteractionStore::new(&data_dir);
    store.list(None);
    assert!(
        !store.block_save,
        "a successful backup must NOT arm the save guard"
    );

    // save proceeds: the now-free primary path is rewritten with valid JSON.
    store.upsert(InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    });
    let written = std::fs::read_to_string(&data_file).unwrap();
    let parsed: Vec<InteractionRecord> = serde_json::from_str(&written).unwrap();
    assert_eq!(parsed.len(), 1, "save wrote fresh data to the freed path");
}

#[test]
fn missing_interactions_file_loads_empty_without_backup() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    // No interactions.json written — first run.
    let mut store = InteractionStore::new(&data_dir);
    assert!(store.list(None).is_empty());
    // A missing file is normal, not corruption: no .corrupt backup is created.
    assert!(
        !data_dir.join("interactions.json.corrupt").exists(),
        "missing file must not be treated as corrupt"
    );
}

#[test]
fn test_interaction_store_import_bundle() {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let mut store = InteractionStore::new(&data_dir);
    let records = vec![InteractionRecord {
        job_id: "job-1".to_string(),
        interaction_type: "viewed".to_string(),
        timestamp: 1234567890,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    }];
    let imported = store.import_bundle(records);
    assert_eq!(imported, 1);
}

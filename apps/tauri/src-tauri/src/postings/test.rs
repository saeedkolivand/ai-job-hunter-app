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

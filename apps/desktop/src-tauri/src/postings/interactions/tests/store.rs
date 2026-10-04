use super::{support::*, *};

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
    let (_dir, _, mut store) = open_store();
    let records = store.list(None);
    assert!(records.is_empty());
}

#[test]
fn test_interaction_store_list_with_filter() {
    let (_dir, _, mut store) = open_store();
    let record = InteractionRecord {
        timestamp: 1234567890,
        ..interaction("job-1", "viewed")
    };
    store.upsert(record.clone());
    let viewed = store.list(Some("viewed"));
    assert_eq!(viewed.len(), 1);
    let applied = store.list(Some("applied"));
    assert!(applied.is_empty());
}

#[test]
fn test_interaction_store_upsert_new() {
    let (_dir, _, mut store) = open_store();
    let record = InteractionRecord {
        timestamp: 1234567890,
        ..interaction("job-1", "viewed")
    };
    store.upsert(record.clone());
    let records = store.list(None);
    assert_eq!(records.len(), 1);
}

#[test]
fn test_interaction_store_upsert_update() {
    let (_dir, _, mut store) = open_store();
    let mut record = InteractionRecord {
        timestamp: 1234567890,
        ..interaction("job-1", "viewed")
    };
    store.upsert(record.clone());
    record.timestamp = 1234567891;
    store.upsert(record.clone());
    let records = store.list(None);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].timestamp, 1234567891);
}

#[test]
fn remove_deletes_only_the_matching_job_id_and_type_pair_leaving_siblings_intact() {
    let (_dir, data_dir, mut store) = open_store();

    // A same-job sibling under a DIFFERENT type, and a same-type sibling under
    // a DIFFERENT job — both must survive the removal untouched.
    store.upsert(interaction("job-1", "dismissed"));
    store.upsert(interaction("job-1", "viewed"));
    store.upsert(interaction("job-2", "dismissed"));

    let removed = store.remove("job-1", "dismissed");

    assert!(
        removed,
        "an existing (job_id, interaction_type) pair must report removed"
    );
    let remaining = store.list(None);
    assert_eq!(remaining.len(), 2, "only the exact pair should be removed");
    assert!(
        !remaining
            .iter()
            .any(|r| r.job_id == "job-1" && r.interaction_type == "dismissed"),
        "the dismissed job-1 record must be gone"
    );
    assert!(
        remaining
            .iter()
            .any(|r| r.job_id == "job-1" && r.interaction_type == "viewed"),
        "job-1's viewed record must survive (different type)"
    );
    assert!(
        remaining
            .iter()
            .any(|r| r.job_id == "job-2" && r.interaction_type == "dismissed"),
        "job-2's dismissed record must survive (different job_id)"
    );

    // A fresh store hydrating from disk must not see the removed record — the
    // write actually persisted, not just the in-memory cache.
    let mut reopened = InteractionStore::new(&data_dir);
    assert_eq!(reopened.list(None).len(), 2);
}

#[test]
fn remove_on_a_nonexistent_pair_returns_false_and_does_not_corrupt_the_file() {
    let (_dir, data_dir, mut store) = open_store();
    store.upsert(interaction("job-1", "viewed"));

    let removed = store.remove("job-1", "dismissed");

    assert!(
        !removed,
        "a pair that was never written must report not-removed"
    );
    let remaining = store.list(None);
    assert_eq!(
        remaining.len(),
        1,
        "the untouched record must still be there"
    );

    // The on-disk file must still parse cleanly (no truncation/corruption from
    // a no-op remove).
    let data_file = data_dir.join("interactions.json");
    let on_disk: Vec<InteractionRecord> =
        serde_json::from_str(&std::fs::read_to_string(&data_file).unwrap()).unwrap();
    assert_eq!(on_disk.len(), 1);
}

#[test]
fn test_interaction_store_clear_all() {
    let (_dir, _, mut store) = open_store();
    let record = InteractionRecord {
        timestamp: 1234567890,
        ..interaction("job-1", "viewed")
    };
    store.upsert(record);
    store.clear_all();
    let records = store.list(None);
    assert!(records.is_empty());
}

#[test]
fn test_interaction_store_export_all() {
    let (_dir, _, mut store) = open_store();
    let record = InteractionRecord {
        timestamp: 1234567890,
        ..interaction("job-1", "viewed")
    };
    store.upsert(record.clone());
    let exported = store.export_all();
    assert_eq!(exported.len(), 1);
}

#[test]
fn test_interaction_store_import_bundle() {
    let (_dir, _, mut store) = open_store();
    let records = vec![InteractionRecord {
        timestamp: 1234567890,
        ..interaction("job-1", "viewed")
    }];
    let imported = store.import_bundle(records);
    assert_eq!(imported, 1);
}

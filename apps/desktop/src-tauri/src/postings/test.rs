use tempfile::TempDir;

use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector};

use super::*;

// ── helpers ──────────────────────────────────────────────────────────────────

fn fake_embedding() -> EmbeddingVector {
    EmbeddingVector {
        values: vec![0.1, 0.2, 0.3],
        space: EmbeddingSpace {
            provider: "test".to_string(),
            model: "test-model".to_string(),
            dim: 3,
        },
    }
}

fn interaction(job_id: &str, interaction_type: &str) -> InteractionRecord {
    InteractionRecord {
        job_id: job_id.to_string(),
        interaction_type: interaction_type.to_string(),
        timestamp: 0,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    }
}

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
fn add_upserts_by_id_keeping_latest_fields() {
    // "Show more" re-streams the same posting (same id). The second add must
    // replace the first in place — not append a duplicate — and the newer fields
    // must win.
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "1", "title": "Old", "description": "v0"}));
    cache.add(serde_json::json!({"id": "1", "title": "New", "description": "v1"}));

    assert_eq!(
        cache.get_all().len(),
        1,
        "re-adding the same id must not duplicate the entry"
    );
    let item = &cache.get_all()[0];
    assert_eq!(
        item.get("title").and_then(serde_json::Value::as_str),
        Some("New"),
        "the latest copy of a re-added id must win"
    );
    assert_eq!(
        item.get("description").and_then(serde_json::Value::as_str),
        Some("v1"),
        "all fields from the latest copy must win"
    );
}

#[test]
fn add_upsert_invalidates_cached_embedding() {
    // A re-streamed posting (same id) may carry changed text, so the cached
    // embedding for that id must be dropped on replace — otherwise the next score
    // would reuse a vector built from the stale content. Mirrors the invalidation
    // `update_description` performs on a text change.
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "1", "title": "Old"}));
    cache.set_embedding("1".to_string(), fake_embedding());
    assert!(
        cache.get_embedding("1").is_some(),
        "embedding must be present before the re-add"
    );

    // Re-add the same id with newer fields — the upsert replaces in place.
    cache.add(serde_json::json!({"id": "1", "title": "New"}));

    assert!(
        cache.get_embedding("1").is_none(),
        "the cached embedding must be invalidated when the id is replaced"
    );
    assert_eq!(
        cache.get_all().len(),
        1,
        "re-adding the same id must not duplicate the entry"
    );
    assert_eq!(
        cache.get_all()[0]
            .get("title")
            .and_then(serde_json::Value::as_str),
        Some("New"),
        "the latest copy of a re-added id must win"
    );
}

#[test]
fn add_upsert_preserves_insertion_order() {
    // Re-adding an existing id must replace it in place, NOT move it to the end —
    // the streamed order the user sees in the list must stay stable.
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "1", "title": "A"}));
    cache.add(serde_json::json!({"id": "2", "title": "B"}));
    cache.add(serde_json::json!({"id": "3", "title": "C"}));

    // Re-add the middle entry with updated fields.
    cache.add(serde_json::json!({"id": "2", "title": "B-updated"}));

    let ids: Vec<&str> = cache
        .get_all()
        .iter()
        .filter_map(|p| p.get("id").and_then(serde_json::Value::as_str))
        .collect();
    assert_eq!(
        ids,
        ["1", "2", "3"],
        "re-adding id 2 must keep it in its original position, not move it to the end"
    );

    // Entry 2 was updated in place.
    let second = &cache.get_all()[1];
    assert_eq!(
        second.get("title").and_then(serde_json::Value::as_str),
        Some("B-updated"),
        "the in-place entry must carry the updated fields"
    );
}

#[test]
fn add_keeps_distinct_ids() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "1"}));
    cache.add(serde_json::json!({"id": "2"}));
    cache.add(serde_json::json!({"id": "3"}));

    assert_eq!(
        cache.get_all().len(),
        3,
        "distinct ids must each get their own row"
    );
}

#[test]
fn add_never_collapses_id_less_items() {
    // Items with no `"id"` (or a null id) must always push — two distinct id-less
    // rows must not be collapsed onto each other.
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({}));
    cache.add(serde_json::json!({}));

    assert_eq!(
        cache.get_all().len(),
        2,
        "id-less items must always push, never collapse"
    );

    // A null id behaves like a missing id (serde `as_str` on null is None).
    cache.add(serde_json::json!({"id": serde_json::Value::Null}));
    cache.add(serde_json::json!({"id": serde_json::Value::Null}));
    assert_eq!(
        cache.get_all().len(),
        4,
        "null-id items must also always push, never collapse"
    );
}

#[test]
fn attach_interactions_joins_records_by_job_id() {
    // `scrape_list_postings` joins InteractionStore records onto each posting so
    // the jobs list can render viewed/applied/saved badges. The join keys on the
    // posting's string `"id"` == record `job_id`, and serializes the records in
    // the renderer's camelCase `JobInteraction` shape.
    let items = vec![
        serde_json::json!({"id": "1", "title": "Has two"}),
        serde_json::json!({"id": "2", "title": "Has one"}),
        serde_json::json!({"id": "3", "title": "Has none"}),
        serde_json::json!({"title": "No id"}),
    ];
    let interactions = vec![
        interaction("1", "viewed"),
        interaction("1", "applied"),
        interaction("2", "bookmarked"),
    ];

    let joined = attach_interactions(&items, &interactions);
    assert_eq!(joined.len(), 4, "every input item is returned, in order");

    // Item "1" collects both of its interactions, exposed under camelCase keys.
    let first = joined[0]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("item 1 must carry an interactions array");
    assert_eq!(first.len(), 2, "item 1 has two interactions");
    let types: Vec<&str> = first
        .iter()
        .filter_map(|i| i.get("interactionType").and_then(serde_json::Value::as_str))
        .collect();
    assert!(
        types.contains(&"viewed") && types.contains(&"applied"),
        "item 1 must carry viewed + applied under the camelCase interactionType key, got {types:?}"
    );

    // The projected object must carry EXACTLY the `JobInteraction` contract keys —
    // no extra storage-only fields can leak if `InteractionRecord` grows later.
    let obj = first[0]
        .as_object()
        .expect("each interaction must be a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "company",
            "interactionType",
            "jobId",
            "location",
            "source",
            "timestamp",
            "title",
            "url",
        ],
        "interaction object must contain exactly the JobInteraction contract keys"
    );

    // Item "2" gets exactly its one interaction.
    let second = joined[1]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("item 2 must carry an interactions array");
    assert_eq!(second.len(), 1, "item 2 has one interaction");

    // Item "3" has an id but no recorded interactions → empty array (stable shape).
    let third = joined[2]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("item 3 must carry an interactions array even with no matches");
    assert!(
        third.is_empty(),
        "an id with no interactions gets an empty array"
    );

    // The id-less item must not panic and gets an empty array too.
    let fourth = joined[3]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("an id-less item must still get an interactions array");
    assert!(fourth.is_empty(), "an id-less item gets an empty array");
}

#[test]
fn attach_interactions_clamps_unknown_interaction_type_to_viewed() {
    // The persisted `interaction_type` is a free String on disk, but the shared
    // `JobInteraction` contract is a strict union. A corrupt/out-of-union value
    // must be coerced to "viewed" so it never breaks the cross-layer contract;
    // valid types pass through unchanged.
    let items = vec![serde_json::json!({"id": "1"})];
    let interactions = vec![
        interaction("1", "garbage"),
        interaction("1", "applied"),
        interaction("1", "opened"),
    ];

    let joined = attach_interactions(&items, &interactions);
    let arr = joined[0]
        .get("interactions")
        .and_then(serde_json::Value::as_array)
        .expect("item 1 must carry an interactions array");

    let types: Vec<&str> = arr
        .iter()
        .filter_map(|i| i.get("interactionType").and_then(serde_json::Value::as_str))
        .collect();
    assert_eq!(
        types,
        ["viewed", "applied", "opened"],
        "an unknown type is coerced to \"viewed\"; valid types pass through unchanged"
    );
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
fn update_description_patches_existing_item_in_place() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "job-1", "title": "Engineer", "description": "short"}));
    cache.add(serde_json::json!({"id": "job-2", "title": "Designer", "description": "other"}));

    let updated = cache.update_description("job-1", "the full, much longer description text");
    assert!(updated, "updating an existing id must return true");

    // No duplicate row was created — still exactly two items.
    assert_eq!(cache.get_all().len(), 2, "update must not push a new entry");

    // The new text is readable back via get_all on the SAME entry.
    let item = cache
        .get_all()
        .iter()
        .find(|p| p.get("id").and_then(serde_json::Value::as_str) == Some("job-1"))
        .expect("job-1 must still be present");
    assert_eq!(
        item.get("description").and_then(serde_json::Value::as_str),
        Some("the full, much longer description text"),
        "description must be replaced with the full text"
    );
    // Sibling untouched.
    let other = cache
        .get_all()
        .iter()
        .find(|p| p.get("id").and_then(serde_json::Value::as_str) == Some("job-2"))
        .expect("job-2 must be untouched");
    assert_eq!(
        other.get("description").and_then(serde_json::Value::as_str),
        Some("other"),
        "unrelated postings must not be mutated"
    );
}

#[test]
fn update_description_unknown_id_returns_false_and_adds_no_row() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "job-1", "description": "short"}));

    let updated = cache.update_description("does-not-exist", "ignored");
    assert!(!updated, "unknown id must return false");
    assert_eq!(
        cache.get_all().len(),
        1,
        "a missing id must NOT create a new row"
    );
    // The existing entry is unchanged.
    assert_eq!(
        cache.get_all()[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("short"),
        "existing entry must be untouched on a miss"
    );
}

/// When `update_description` writes new text, the previously cached embedding for
/// that id must be invalidated so the next score re-embeds the full description
/// instead of reusing the stale snippet vector.
#[test]
fn update_description_invalidates_cached_embedding_on_change() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "job-1", "description": "short snippet"}));

    // Prime the embedding cache with a synthetic vector for this posting.
    cache.set_embedding("job-1".to_string(), fake_embedding());
    assert!(
        cache.get_embedding("job-1").is_some(),
        "embedding must be present before update"
    );

    // Update the description with new (longer) text — different from the current.
    let updated = cache.update_description("job-1", "the full, much longer description text");
    assert!(updated, "update must succeed on a known id");

    // Stale embedding must be gone.
    assert!(
        cache.get_embedding("job-1").is_none(),
        "cached embedding must be invalidated after description change"
    );
}

/// When `update_description` is called with the SAME text that is already stored,
/// the cached embedding must NOT be invalidated (it is still valid).
#[test]
fn update_description_keeps_embedding_when_text_unchanged() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "job-1", "description": "full description"}));
    cache.set_embedding("job-1".to_string(), fake_embedding());

    // Call update_description with the identical text.
    let updated = cache.update_description("job-1", "full description");
    assert!(updated, "update must return true even for a no-op change");

    // Embedding must still be present (nothing changed).
    assert!(
        cache.get_embedding("job-1").is_some(),
        "embedding must be preserved when description text is unchanged"
    );
}

#[test]
fn update_description_does_not_create_duplicates_on_repeat() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "job-1", "description": "v0"}));

    assert!(cache.update_description("job-1", "v1"));
    assert!(cache.update_description("job-1", "v2"));

    assert_eq!(
        cache.get_all().len(),
        1,
        "repeated updates of the same id must never duplicate the entry"
    );
    assert_eq!(
        cache.get_all()[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("v2"),
        "the latest update wins"
    );
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

// ── PostingsCache::apply_cluster_annotations (ADR-029) ────────────────────────

#[test]
fn apply_cluster_annotations_patches_matching_items_by_id_and_skips_others() {
    let mut cache = PostingsCache::default();
    cache.add(json!({ "id": "j1", "title": "Rust Developer" }));
    cache.add(json!({ "id": "j2", "title": "Sales Manager" }));

    let mut by_id = std::collections::HashMap::new();
    by_id.insert(
        "j1".to_string(),
        json!({
            "clusterId": "j1",
            "clusterCanonical": true,
            "clusterMembers": [{ "key": "j1", "url": "https://x/1" }],
            "isAgency": false,
        }),
    );
    // An annotation for an id NOT in the cache must be a no-op (no row created).
    by_id.insert("gone".to_string(), json!({ "clusterId": "gone" }));

    cache.apply_cluster_annotations(&by_id);

    let items = cache.get_all();
    assert_eq!(items.len(), 2, "no row is created for a missing id");
    let j1 = items.iter().find(|i| i.get("id") == Some(&json!("j1"))).unwrap();
    assert_eq!(j1.get("clusterId"), Some(&json!("j1")), "j1 gets its annotation");
    assert_eq!(j1.get("clusterCanonical"), Some(&json!(true)));
    // The untouched item carries no cluster fields.
    let j2 = items.iter().find(|i| i.get("id") == Some(&json!("j2"))).unwrap();
    assert!(j2.get("clusterId").is_none(), "an un-annotated item is left alone");
}

#[test]
fn apply_cluster_annotations_empty_map_is_a_noop() {
    let mut cache = PostingsCache::default();
    cache.add(json!({ "id": "j1", "title": "Rust Developer" }));
    cache.apply_cluster_annotations(&std::collections::HashMap::new());
    let j1 = &cache.get_all()[0];
    assert!(j1.get("clusterId").is_none());
}

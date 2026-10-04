use serde_json::json;

use super::{support::*, *};

#[test]
fn test_postings_cache_clear() {
    let mut cache = PostingsCache::default();
    let item = serde_json::json!({"id": "1", "title": "Test"});
    cache.add(item);
    cache.clear_all();
    assert!(cache.get_all().is_empty());
}

/// A hybrid search snapshots `generation()` at start and must be able to
/// detect a `clear_all()` that lands mid-search — see the field doc. `add`
/// and `update_description` must NOT bump it: a result already found is
/// still a valid, still-present posting after either. Nor must
/// `clear_embeddings()`: it leaves `items` untouched, so an already-computed
/// ranking is still correct over what's still there — bumping on it would
/// falsely report a changed corpus for a run whose postings never moved.
#[test]
fn generation_bumps_only_on_clear_all() {
    let mut cache = PostingsCache::default();
    let g0 = cache.generation();
    cache.add(serde_json::json!({
        "id": "job-1", "url": "https://example.com/jobs/1", "title": "Engineer", "description": "short",
    }));
    assert_eq!(
        cache.generation(),
        g0,
        "add() must not invalidate an in-flight search"
    );
    let updated = cache.update_description("https://example.com/jobs/1", "a longer description");
    assert!(updated, "the url must match the item just added");
    assert_eq!(
        cache.generation(),
        g0,
        "update_description() must not invalidate an in-flight search"
    );
    cache.set_embedding("job-1".to_string(), fake_embedding());
    cache.clear_embeddings();
    assert_eq!(
        cache.generation(),
        g0,
        "clear_embeddings() must not invalidate an in-flight search — items are untouched"
    );
    cache.clear_all();
    assert_ne!(
        cache.generation(),
        g0,
        "clear_all() must invalidate an in-flight search"
    );
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
    let j1 = items
        .iter()
        .find(|i| i.get("id") == Some(&json!("j1")))
        .unwrap();
    assert_eq!(
        j1.get("clusterId"),
        Some(&json!("j1")),
        "j1 gets its annotation"
    );
    assert_eq!(j1.get("clusterCanonical"), Some(&json!(true)));
    // The untouched item carries no cluster fields.
    let j2 = items
        .iter()
        .find(|i| i.get("id") == Some(&json!("j2")))
        .unwrap();
    assert!(
        j2.get("clusterId").is_none(),
        "an un-annotated item is left alone"
    );
}

#[test]
fn apply_cluster_annotations_empty_map_is_a_noop() {
    let mut cache = PostingsCache::default();
    cache.add(json!({ "id": "j1", "title": "Rust Developer" }));
    cache.apply_cluster_annotations(&std::collections::HashMap::new());
    let j1 = &cache.get_all()[0];
    assert!(j1.get("clusterId").is_none());
}

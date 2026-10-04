use super::{support::*, *};

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

/// "Show more" re-streams the SAME search signature, so a re-fetched posting
/// upsert is usually byte-identical apart from bookkeeping fields — that must
/// NOT wipe an already-computed embedding (the defect this PR fixes: `add`
/// used to invalidate unconditionally on every upsert).
#[test]
fn add_keeps_embedding_when_title_and_description_are_unchanged() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({
        "id": "job-1", "title": "Engineer", "description": "full text", "capturedAt": 1,
    }));
    cache.set_embedding("job-1".to_string(), fake_embedding());

    // Re-stream the identical posting, but with a fresh `capturedAt` — the
    // shape a re-scrape actually produces.
    cache.add(serde_json::json!({
        "id": "job-1", "title": "Engineer", "description": "full text", "capturedAt": 2,
    }));

    assert!(
        cache.get_embedding("job-1").is_some(),
        "an upsert that doesn't change title/description must keep the cached embedding"
    );
}

/// The mirror case: a re-streamed posting whose title or description DID
/// change must still invalidate — this is not a blanket "never invalidate".
#[test]
fn add_invalidates_embedding_when_description_changes() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "job-1", "title": "Engineer", "description": "v1"}));
    cache.set_embedding("job-1".to_string(), fake_embedding());

    cache.add(serde_json::json!({"id": "job-1", "title": "Engineer", "description": "v2"}));

    assert!(
        cache.get_embedding("job-1").is_none(),
        "an upsert that changes the embedded text must invalidate the cached embedding"
    );
}

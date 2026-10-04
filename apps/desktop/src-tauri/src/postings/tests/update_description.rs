use super::{support::*, *};

#[test]
fn update_description_patches_existing_item_in_place() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({
        "id": "job-1", "url": "https://example.com/jobs/1", "title": "Engineer", "description": "short",
    }));
    cache.add(serde_json::json!({
        "id": "job-2", "url": "https://example.com/jobs/2", "title": "Designer", "description": "other",
    }));

    let updated = cache.update_description(
        "https://example.com/jobs/1",
        "the full, much longer description text",
    );
    assert!(updated, "updating an existing url must return true");

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
fn update_description_unknown_url_returns_false_and_adds_no_row() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "job-1", "url": "https://example.com/jobs/1", "description": "short"}));

    let updated = cache.update_description("https://nowhere.example.com/x", "ignored");
    assert!(!updated, "unknown url must return false");
    assert_eq!(
        cache.get_all().len(),
        1,
        "a missing url must NOT create a new row"
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
    cache.add(serde_json::json!({"id": "job-1", "url": "https://example.com/jobs/1", "description": "short snippet"}));

    // Prime the embedding cache with a synthetic vector for this posting.
    cache.set_embedding("job-1".to_string(), fake_embedding());
    assert!(
        cache.get_embedding("job-1").is_some(),
        "embedding must be present before update"
    );

    // Update the description with new (longer) text — different from the current.
    let updated = cache.update_description(
        "https://example.com/jobs/1",
        "the full, much longer description text",
    );
    assert!(updated, "update must succeed on a known url");

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
    cache.add(serde_json::json!({"id": "job-1", "url": "https://example.com/jobs/1", "description": "full description"}));
    cache.set_embedding("job-1".to_string(), fake_embedding());

    // Call update_description with the identical text.
    let updated = cache.update_description("https://example.com/jobs/1", "full description");
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
    cache.add(serde_json::json!({"id": "job-1", "url": "https://example.com/jobs/1", "description": "v0"}));

    assert!(cache.update_description("https://example.com/jobs/1", "v1"));
    assert!(cache.update_description("https://example.com/jobs/1", "v2"));

    assert_eq!(
        cache.get_all().len(),
        1,
        "repeated updates of the same url must never duplicate the entry"
    );
    assert_eq!(
        cache.get_all()[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("v2"),
        "the latest update wins"
    );
}

/// Two board-synthetic ids can legitimately share one url within a session
/// (issue #1106) — every matching item must be patched, not just the first.
#[test]
fn update_description_patches_every_item_sharing_the_url() {
    let mut cache = PostingsCache::default();
    cache.add(serde_json::json!({"id": "job-a", "url": "https://example.com/jobs/1", "description": "old-a"}));
    cache.add(serde_json::json!({"id": "job-b", "url": "https://example.com/jobs/1", "description": "old-b"}));
    cache.add(serde_json::json!({"id": "job-c", "url": "https://example.com/jobs/2", "description": "unrelated"}));

    let updated = cache.update_description("https://example.com/jobs/1", "shared correction");
    assert!(updated);

    for id in ["job-a", "job-b"] {
        let item = cache
            .get_all()
            .iter()
            .find(|p| p.get("id").and_then(serde_json::Value::as_str) == Some(id))
            .unwrap_or_else(|| panic!("{id} must still be present"));
        assert_eq!(
            item.get("description").and_then(serde_json::Value::as_str),
            Some("shared correction"),
            "{id} shares the url and must be patched too"
        );
    }
    let other = cache
        .get_all()
        .iter()
        .find(|p| p.get("id").and_then(serde_json::Value::as_str) == Some("job-c"))
        .expect("job-c must still be present");
    assert_eq!(
        other.get("description").and_then(serde_json::Value::as_str),
        Some("unrelated"),
        "a different url must not be touched"
    );
}

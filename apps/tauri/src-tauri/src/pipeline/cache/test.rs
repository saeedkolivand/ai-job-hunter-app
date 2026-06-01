use super::*;
use tempfile::TempDir;

fn cache() -> (TempDir, KvCache) {
    let dir = TempDir::new().expect("tempdir");
    let cache = KvCache::open(dir.path()).expect("open cache");
    (dir, cache)
}

#[test]
fn set_then_get_round_trips_within_ttl() {
    let (_dir, cache) = cache();
    cache.set("company_brief", "acme", "{\"size\":\"large\"}");
    assert_eq!(
        cache.get("company_brief", "acme", 3600),
        Some("{\"size\":\"large\"}".to_string())
    );
}

#[test]
fn get_returns_none_for_missing_key() {
    let (_dir, cache) = cache();
    assert_eq!(cache.get("ns", "nope", 3600), None);
}

#[test]
fn get_treats_entries_older_than_ttl_as_expired() {
    let (_dir, cache) = cache();
    cache.set("ns", "k", "v");
    // A negative TTL pushes the cutoff into the future, so the fresh row is
    // considered stale.
    assert_eq!(cache.get("ns", "k", -10), None);
}

#[test]
fn set_overwrites_an_existing_value() {
    let (_dir, cache) = cache();
    cache.set("ns", "k", "first");
    cache.set("ns", "k", "second");
    assert_eq!(cache.get("ns", "k", 3600), Some("second".to_string()));
}

#[test]
fn keys_are_case_insensitive_per_schema() {
    let (_dir, cache) = cache();
    cache.set("ns", "Acme", "v");
    assert_eq!(cache.get("ns", "acme", 3600), Some("v".to_string()));
}

#[test]
fn clear_drops_every_entry() {
    let (_dir, cache) = cache();
    cache.set("company_brief", "acme", "v1");
    cache.set("ocr", "doc1", "v2");
    cache.clear();
    assert_eq!(cache.get("company_brief", "acme", 3600), None);
    assert_eq!(cache.get("ocr", "doc1", 3600), None);
}

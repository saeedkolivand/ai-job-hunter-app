//! `pipeline::cache::KvCache`: round-trip, TTL expiry, and namespace isolation.

use tempfile::TempDir;

use crate::pipeline::cache::KvCache;

#[test]
fn kv_cache_roundtrip_ttl_and_namespace_isolation() {
    let dir = TempDir::new().unwrap();
    let cache = KvCache::open(dir.path()).unwrap();

    cache.set("ns1", "acme", "brief-v1");
    assert_eq!(cache.get("ns1", "acme", 3600), Some("brief-v1".to_string()));

    // Different namespace, same key → miss.
    assert_eq!(cache.get("ns2", "acme", 3600), None);

    // ttl = 0 → the entry is considered expired immediately.
    assert_eq!(cache.get("ns1", "acme", 0), None);

    // Overwrite.
    cache.set("ns1", "acme", "brief-v2");
    assert_eq!(cache.get("ns1", "acme", 3600), Some("brief-v2".to_string()));

    // Key match is case-insensitive (COLLATE NOCASE).
    assert_eq!(cache.get("ns1", "ACME", 3600), Some("brief-v2".to_string()));
}

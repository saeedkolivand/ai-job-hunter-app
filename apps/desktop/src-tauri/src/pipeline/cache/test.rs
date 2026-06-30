use super::*;
use tempfile::TempDir;

fn cache() -> (TempDir, KvCache) {
    let dir = TempDir::new().expect("tempdir");
    let cache = KvCache::open(dir.path()).expect("open cache");
    (dir, cache)
}

/// Insert a row with an explicit `created_at` so row-ordering is deterministic
/// (wall-clock `now_secs()` can collide within a single test).
fn insert_at(cache: &KvCache, key: &str, created_at: i64) {
    let conn = cache.conn.lock();
    conn.execute(
        "INSERT OR REPLACE INTO kv_cache (namespace, key, value, created_at) VALUES (?1, ?2, ?3, ?4)",
        params!["ns", key, "v", created_at],
    )
    .expect("insert row");
}

fn row_count(cache: &KvCache) -> i64 {
    let conn = cache.conn.lock();
    conn.query_row("SELECT COUNT(*) FROM kv_cache", [], |row| row.get(0))
        .expect("count rows")
}

/// Whether a key still exists, ignoring TTL (the row-cap tests use synthetic
/// low `created_at` values that the TTL-aware `get` would treat as expired).
fn row_exists(cache: &KvCache, key: &str) -> bool {
    let conn = cache.conn.lock();
    conn.query_row(
        "SELECT 1 FROM kv_cache WHERE namespace = ?1 AND key = ?2",
        params!["ns", key],
        |_| Ok(()),
    )
    .is_ok()
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

#[test]
fn prune_max_rows_keeps_exactly_the_n_newest() {
    let (_dir, cache) = cache();
    // Distinct, strictly-increasing created_at so ordering is deterministic.
    insert_at(&cache, "oldest", 100);
    insert_at(&cache, "middle", 200);
    insert_at(&cache, "newer", 300);
    insert_at(&cache, "newest", 400);
    assert_eq!(row_count(&cache), 4);

    cache.prune(None, Some(2));

    assert_eq!(row_count(&cache), 2, "exactly the 2 newest rows remain");
    assert!(row_exists(&cache, "newest"));
    assert!(row_exists(&cache, "newer"));
    assert!(!row_exists(&cache, "middle"));
    assert!(!row_exists(&cache, "oldest"));
}

#[test]
fn prune_max_rows_zero_clears_the_table() {
    let (_dir, cache) = cache();
    insert_at(&cache, "a", 100);
    insert_at(&cache, "b", 200);
    assert_eq!(row_count(&cache), 2);

    cache.prune(None, Some(0));

    assert_eq!(row_count(&cache), 0, "max_rows = Some(0) empties the table");
}

use super::*;
use tempfile::TempDir;

fn open() -> (TempDir, DiscoveredCompanyStore) {
    let dir = TempDir::new().unwrap();
    let store = DiscoveredCompanyStore::open(dir.path()).unwrap();
    (dir, store)
}

fn r(
    ats: &str,
    slug: &str,
    name: Option<&str>,
    source: &str,
) -> (String, String, Option<String>, String) {
    (
        ats.to_string(),
        slug.to_string(),
        name.map(str::to_string),
        source.to_string(),
    )
}

/// Record one anonymous `scrape` sighting of `(ats, slug)`.
fn sight(store: &DiscoveredCompanyStore, ats: &str, slug: &str) {
    store.upsert_batch(&[r(ats, slug, None, "scrape")]).unwrap();
}

#[test]
fn upsert_insert_then_resighting_bumps_seen_count() {
    let (_dir, store) = open();
    sight(&store, "greenhouse", "stripe");
    // A fresh insert starts at seen_count 1 (not bumped).
    let first = store.search("stripe", 10);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].seen_count, 1, "a first sighting is seen_count 1");

    // Re-sighting the same (ats, slug) bumps to 2.
    sight(&store, "greenhouse", "stripe");
    let again = store.search("stripe", 10);
    assert_eq!(again.len(), 1, "unique(ats,slug) keeps it one row");
    assert_eq!(again[0].seen_count, 2, "a re-sighting bumps seen_count");
}

#[test]
fn display_name_backfills_but_never_overwrites() {
    let (_dir, store) = open();
    // First sighting has no display name.
    sight(&store, "lever", "spotify");
    assert_eq!(store.search("spotify", 10)[0].display_name, None);

    // A later sighting WITH a name backfills the empty one.
    store
        .upsert_batch(&[r("lever", "spotify", Some("Spotify"), "scrape")])
        .unwrap();
    assert_eq!(
        store.search("spotify", 10)[0].display_name.as_deref(),
        Some("Spotify"),
        "an empty display_name must be backfilled"
    );

    // A still-later sighting with a DIFFERENT name must NOT overwrite it.
    store
        .upsert_batch(&[r("lever", "spotify", Some("Spotify AB"), "scrape")])
        .unwrap();
    assert_eq!(
        store.search("spotify", 10)[0].display_name.as_deref(),
        Some("Spotify"),
        "a non-empty display_name is never overwritten"
    );
}

#[test]
fn set_starred_materializes_a_seed_row_when_missing() {
    let (_dir, store) = open();
    // Starring a company that was never harvested materializes a seed row.
    store.set_starred("ashby", "Linear", true).unwrap();
    let watched = store.watched();
    assert_eq!(watched, vec![("ashby".to_string(), "Linear".to_string())]);
    // The materialized row is source='seed'.
    assert_eq!(store.search("Linear", 10)[0].source, "seed");

    // Unstarring removes it from the watched set.
    store.set_starred("ashby", "Linear", false).unwrap();
    assert!(store.watched().is_empty(), "unstarred → not watched");
}

#[test]
fn unstarring_a_missing_company_is_a_noop() {
    let (_dir, store) = open();
    store.set_starred("greenhouse", "ghost", false).unwrap();
    // No junk row was materialized.
    assert!(store.search("ghost", 10).is_empty());
    assert!(store.watched().is_empty());
}

#[test]
fn search_ranks_starred_first_then_by_seen_count() {
    let (_dir, store) = open();
    // seen twice, unstarred
    sight(&store, "greenhouse", "acme");
    sight(&store, "greenhouse", "acme");
    // seen once, unstarred
    sight(&store, "greenhouse", "acorn");
    // seen once, but STARRED → must rank first despite the lower count
    sight(&store, "greenhouse", "aced");
    store.set_starred("greenhouse", "aced", true).unwrap();

    let results = store.search("ac", 10);
    let slugs: Vec<&str> = results.iter().map(|c| c.slug.as_str()).collect();
    assert_eq!(
        slugs,
        vec!["aced", "acme", "acorn"],
        "starred first, then seen_count desc"
    );
}

#[test]
fn search_escapes_like_wildcards() {
    let (_dir, store) = open();
    sight(&store, "greenhouse", "acme");
    // A `%` query must not match everything — it's escaped to a literal.
    assert!(
        store.search("%", 10).is_empty(),
        "a literal % must not act as a wildcard"
    );
    // Empty query returns everything (top-N).
    assert_eq!(store.search("", 10).len(), 1);
}

#[test]
fn export_import_round_trips() {
    let (_dir, store) = open();
    store
        .upsert_batch(&[
            r("greenhouse", "stripe", Some("Stripe"), "scrape"),
            r("lever", "spotify", None, "extension"),
        ])
        .unwrap();
    store.set_starred("greenhouse", "stripe", true).unwrap();
    let bundle = store.export();

    let (_dir2, store2) = open();
    let restored = store2.import(&bundle).unwrap();
    assert_eq!(restored, 2);
    // Watched state survives the round-trip.
    assert_eq!(
        store2.watched(),
        vec![("greenhouse".to_string(), "stripe".to_string())]
    );
    // Display name + source survive.
    let stripe = &store2.search("stripe", 10)[0];
    assert_eq!(stripe.display_name.as_deref(), Some("Stripe"));
    assert_eq!(stripe.source, "scrape");
}

#[test]
fn import_with_a_malformed_row_errors_and_preserves_existing_rows() {
    let (_dir, store) = open();
    sight(&store, "greenhouse", "keep");

    // One malformed row (missing required `slug`) must fail the whole import
    // BEFORE any DELETE runs — deserialize-all-before-mutate.
    let bundle = serde_json::json!([
        { "atsKind": "lever", "slug": "ok", "firstSeenAt": 1, "lastSeenAt": 1, "seenCount": 1, "source": "scrape", "starred": false },
        { "atsKind": "lever", "firstSeenAt": 2, "lastSeenAt": 2, "seenCount": 1, "source": "scrape", "starred": false }
    ]);
    assert!(
        store.import(&bundle).is_err(),
        "malformed row must fail import"
    );
    // The pre-existing row survives untouched.
    assert_eq!(store.search("keep", 10).len(), 1);
    assert!(store.search("ok", 10).is_empty(), "no partial insert");
}

#[test]
fn watched_companies_returns_full_starred_rows_only() {
    let (_dir, store) = open();
    store
        .upsert_batch(&[
            r("greenhouse", "stripe", Some("Stripe"), "scrape"),
            r("greenhouse", "stripe", None, "scrape"), // bump to seen_count 2
            r("ashby", "Linear", None, "scrape"),
        ])
        .unwrap();
    store.set_starred("greenhouse", "stripe", true).unwrap();

    let watched = store.watched_companies();
    assert_eq!(watched.len(), 1, "only the starred row is returned");
    assert_eq!(watched[0].slug, "stripe");
    assert_eq!(watched[0].display_name.as_deref(), Some("Stripe"));
    assert_eq!(watched[0].seen_count, 2, "full row carries the seen_count");
    assert!(watched[0].starred);
    // The unstarred ashby row is excluded.
    assert!(watched.iter().all(|c| c.slug != "Linear"));
}

#[test]
fn watched_queries_are_bounded_to_the_limit() {
    let (_dir, store) = open();
    // Star WATCHED_LIMIT + 1 companies (a pathological/hostile set) — both
    // watched queries must cap at WATCHED_LIMIT (CWE-770), never read them all.
    let over = (WATCHED_LIMIT + 1) as usize;
    for i in 0..over {
        store
            .set_starred("greenhouse", &format!("co-{i}"), true)
            .unwrap();
    }
    assert_eq!(
        store.watched().len() as i64,
        WATCHED_LIMIT,
        "watched() (pairs) must be bounded to WATCHED_LIMIT"
    );
    assert_eq!(
        store.watched_companies().len() as i64,
        WATCHED_LIMIT,
        "watched_companies() (full rows) must be bounded to WATCHED_LIMIT"
    );
}

#[test]
fn clear_all_empties_the_store() {
    let (_dir, store) = open();
    sight(&store, "greenhouse", "stripe");
    assert!(!store.search("stripe", 10).is_empty());
    store.clear_all();
    assert!(store.search("stripe", 10).is_empty());
}

#[test]
fn reopening_the_same_db_is_migration_idempotent() {
    let dir = TempDir::new().unwrap();
    {
        let store = DiscoveredCompanyStore::open(dir.path()).unwrap();
        sight(&store, "greenhouse", "stripe");
    }
    // Second open re-runs run_migrations (no-op) and keeps the data.
    let store = DiscoveredCompanyStore::open(dir.path()).unwrap();
    assert_eq!(store.search("stripe", 10).len(), 1);
}

#[test]
fn upsert_skips_empty_and_byte_clamps() {
    let (_dir, store) = open();
    let big = "z".repeat(500);
    store
        .upsert_batch(&[
            r("", "slug", None, "scrape"),         // empty ats → skipped
            r("greenhouse", "  ", None, "scrape"), // whitespace slug → skipped
            r("greenhouse", &big, None, "scrape"), // over-cap slug → clamped
        ])
        .unwrap();
    let all = store.search("", 100);
    assert_eq!(all.len(), 1, "only the clamped valid row survives");
    assert!(all[0].slug.len() <= MAX_FIELD_BYTES, "slug byte-clamped");
}

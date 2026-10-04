//! The `help_vectors` cache (the help corpus' dense arm): keyed by TEXT hash, scoped to one
//! embedding space and one vector-format version, and wiped by `clear_all`.

use super::{support::*, *};

// ── help_vectors (the help-corpus vector cache) ──────────────────────────────

/// The `create_help_vectors` migration runs through a real `open()` — not a
/// hand-issued `CREATE TABLE` in the test — so an APPEND that landed in the
/// wrong position (the migration list is position-indexed) fails here.
#[test]
#[serial]
fn a_help_vector_round_trips_through_a_freshly_migrated_store() {
    let (_dir, store) = open_store();
    let active = cfg_ollama();

    store
        .upsert_help_vector("hash-a", &ev(vec![0.5, 0.25]))
        .unwrap();

    let got = store
        .get_help_vector("hash-a", &active)
        .expect("the row must be readable in the space it was written in");
    assert_eq!(got.values, vec![0.5, 0.25]);
    assert_eq!(got.space.provider, "ollama");
    assert_eq!(got.space.dim, 2);
    assert_eq!(got.space.version, EMBEDDING_VECTOR_VERSION);
    // A hash nobody wrote is a plain miss, not an error.
    assert!(store
        .get_help_vector("hash-nobody-wrote", &active)
        .is_none());

    // The same migration must also create the `created_at` index the row-cap
    // sweep is written for: `prune_caches` prunes `help_vectors` through
    // `sql::prune_table_locked`, whose cap delete is an `ORDER BY created_at
    // DESC LIMIT 1 OFFSET ?` subquery. Asserted against `sqlite_master` (not
    // an EXPLAIN plan) so it fails on the index being ABSENT — the thing the
    // migration owns — rather than on a planner decision.
    let index_exists: i64 = store
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'index' AND name = 'idx_help_vectors_created_at' AND tbl_name = 'help_vectors'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        index_exists, 1,
        "create_help_vectors must create idx_help_vectors_created_at in the SAME migration as \
         the table — prune_table_locked's row-cap subquery degrades to a full-table sort without it"
    );
}

/// The cache is keyed by the TEXT hash, so an edited answer is a natural miss
/// with no invalidation step anywhere — the property the whole
/// `sha256_hex(body)` key exists for.
#[test]
#[serial]
fn a_different_text_hash_is_a_help_vector_miss() {
    let (_dir, store) = open_store();
    let active = cfg_ollama();

    store
        .upsert_help_vector(&sha256_hex("the original answer"), &ev(vec![1.0, 0.0]))
        .unwrap();

    assert!(store
        .get_help_vector(&sha256_hex("the original answer"), &active)
        .is_some());
    assert!(
        store
            .get_help_vector(&sha256_hex("the edited answer"), &active)
            .is_none(),
        "an edited answer must not read back the vector of the old one"
    );
}

/// A row written in ANOTHER embedding space must read as a miss even though
/// its `text_hash` matches exactly — a hit here would score a
/// `text-embedding-3-small` vector against a `nomic-embed-text` query.
#[test]
#[serial]
fn a_help_vector_from_another_embedding_space_is_a_miss() {
    let (_dir, store) = open_store();
    let active = cfg_ollama();

    store
        .upsert_help_vector(
            "hash-a",
            &ev_in("openai", "text-embedding-3-small", vec![0.5, 0.25]),
        )
        .unwrap();

    assert!(
        store.get_help_vector("hash-a", &active).is_none(),
        "a vector from another embedding space must never be handed back"
    );
    // Same row, read under the config that DID write it: a hit. This is what
    // makes the miss above about the SPACE rather than about the row being
    // unreadable for some other reason.
    let other = EmbeddingConfig {
        provider: "openai".to_string(),
        model: "text-embedding-3-small".to_string(),
        base_url: None,
    };
    assert!(store.get_help_vector("hash-a", &other).is_some());
}

/// A stale-FORMAT row (an older `EMBEDDING_VECTOR_VERSION`) is a miss too —
/// `EmbeddingConfig::matches` compares the version it persisted, so a format
/// bump re-embeds instead of silently mixing formats.
#[test]
#[serial]
fn a_help_vector_in_an_older_format_version_is_a_miss() {
    let (_dir, store) = open_store();
    let active = cfg_ollama();

    let mut stale = ev(vec![0.5, 0.25]);
    stale.space.version = EMBEDDING_VECTOR_VERSION - 1;
    store.upsert_help_vector("hash-a", &stale).unwrap();

    assert!(store.get_help_vector("hash-a", &active).is_none());
}

/// Re-writing the same hash replaces the row rather than erroring on the
/// primary key — the path a space change takes after `clear_help_vectors`
/// missed a row, and the one `run_dense_arm` takes on every miss.
#[test]
#[serial]
fn upserting_the_same_help_hash_replaces_the_row() {
    let (_dir, store) = open_store();
    let active = cfg_ollama();

    store.upsert_help_vector("hash-a", &ev(vec![1.0])).unwrap();
    store
        .upsert_help_vector("hash-a", &ev(vec![0.0, 1.0]))
        .unwrap();

    let got = store.get_help_vector("hash-a", &active).unwrap();
    assert_eq!(got.values, vec![0.0, 1.0], "the newer write must win");
    assert_eq!(got.space.dim, 2);
}

/// `ai_set_embedding_config`'s space-change branch calls this; every row in
/// the cache is unreachable after a flip, so it must actually be emptied
/// rather than left as dead weight.
#[test]
#[serial]
fn clear_help_vectors_empties_the_cache() {
    let (_dir, store) = open_store();
    let active = cfg_ollama();

    store.upsert_help_vector("hash-a", &ev(vec![1.0])).unwrap();
    store.upsert_help_vector("hash-b", &ev(vec![0.0])).unwrap();

    store.clear_help_vectors().unwrap();

    assert!(store.get_help_vector("hash-a", &active).is_none());
    assert!(store.get_help_vector("hash-b", &active).is_none());
    // Idempotent on an already-empty cache.
    store.clear_help_vectors().unwrap();
}

/// Factory reset ("erase my data") must take the help cache with it. It holds
/// no user content, but it is derived from the app's own corpus and leaving it
/// behind would make a reset visibly incomplete.
#[test]
#[serial]
fn clear_all_empties_the_help_vector_cache_too() {
    let (_dir, store) = open_store();
    let active = cfg_ollama();
    store.upsert_help_vector("hash-a", &ev(vec![1.0])).unwrap();
    // A sibling row, so a `clear_all` that silently stopped early would show up.
    store
        .upsert_posting_vector("job-1", "hash-j", &ev(vec![1.0]))
        .unwrap();

    store.clear_all();

    assert!(
        store.get_help_vector("hash-a", &active).is_none(),
        "clear_all must delete from help_vectors"
    );
    assert!(store.get_posting_vector("job-1").is_none());
}

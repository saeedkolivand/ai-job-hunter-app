//! Document embedding vectors, the active-space bookkeeping around them, and text hashing.

use super::{support::*, *};

/// A restored-backup vector is an OLD-FORMAT value (pre-chunk-pool, truncated
/// prefix), so `import()` tags it `version: 0` to force a re-embed. The write
/// path used to bind `EMBEDDING_VECTOR_VERSION` literally, silently advancing it
/// to the current version — the row then read as fresh and was never re-embedded,
/// which is exactly the cross-format mixing the version field exists to prevent.
#[test]
fn upsert_vector_persists_an_older_space_version_instead_of_force_advancing_it() {
    let (_dir, store) = open_store();

    let mut imported = ev(vec![0.1, 0.2, 0.3]);
    imported.space.version = 0; // what `import()` tags a restored backup with
    store.upsert_vector("doc-imported", &imported).unwrap();

    let stored = store
        .get_vector("doc-imported")
        .expect("vector round-trips");
    assert_eq!(
        stored.space.version, 0,
        "import's stale tag must survive the write, or the vector is never re-embedded"
    );
    assert!(
        !EmbeddingConfig {
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            base_url: None,
        }
        .matches(&stored.space),
        "a version-0 vector must read as stale for the active space"
    );

    // A freshly-produced vector still lands at the current version.
    store
        .upsert_vector("doc-fresh", &ev(vec![0.4, 0.5, 0.6]))
        .unwrap();
    assert_eq!(
        store.get_vector("doc-fresh").unwrap().space.version,
        EMBEDDING_VECTOR_VERSION
    );
}

/// `vectors` is the DOCUMENT index: the Embeddings panel counts every row in it
/// (`count_vectors_in_space`, no join to `documents`) and derives `stale` as
/// `total_docs - indexed`, and NOTHING deletes a row whose document does not
/// exist (delete/re-embed iterate real documents; `prune_caches` only touches
/// `posting_vectors`/`match_scores`). So one synthetic scoring id written here
/// — an Autopilot run's résumé snapshot, say — would permanently inflate
/// "indexed", clamp `stale` to 0 through the `saturating_sub`, and report
/// "N/N indexed" over a genuinely stale index. The write refuses it.
#[test]
fn the_document_vector_index_refuses_a_synthetic_scoring_id() {
    let (_dir, store) = open_store();

    // One real, UNINDEXED document — the "genuinely stale index" baseline.
    store
        .insert(&DocumentRecord {
            title: "CV".into(),
            name: "CV".into(),
            created_at: 0,
            ..record("doc-real", "rust engineer")
        })
        .unwrap();
    let indexed_before = store.count_vectors_in_space("ollama", "nomic-embed-text");
    assert_eq!(indexed_before, 0);

    // What an Autopilot semantic run would have written under its
    // content-addressed résumé id.
    let synthetic = crate::commands::match_resume::autopilot_resume_id("an autopilot résumé");
    assert!(store
        .upsert_vector(&synthetic, &ev(vec![0.1, 0.2]))
        .is_err());
    assert!(store.get_vector(&synthetic).is_none());
    // The extension bridge's ad-hoc namespace is refused on the same rule.
    assert!(store
        .upsert_vector("adhoc:abc123", &ev(vec![0.1, 0.2]))
        .is_err());

    let indexed_after = store.count_vectors_in_space("ollama", "nomic-embed-text");
    assert_eq!(
        indexed_after, indexed_before,
        "an autopilot semantic run must leave the document index untouched: count before == after"
    );
    // The Embeddings panel's arithmetic (`total.saturating_sub(indexed)`) is
    // therefore still honest about the one unindexed document.
    assert_eq!(
        store.list().len().saturating_sub(indexed_after),
        1,
        "stale must still be 1 — an orphan row is exactly what would clamp it to 0"
    );

    // The guard is narrow: a real document id still indexes normally.
    store
        .upsert_vector("doc-real", &ev(vec![0.1, 0.2]))
        .unwrap();
    assert_eq!(
        store.count_vectors_in_space("ollama", "nomic-embed-text"),
        1
    );
    assert_eq!(store.list().len().saturating_sub(1), 0);
}

#[test]
fn test_upsert_vector() {
    let (_dir, store) = open_store();

    let doc_id = "doc-123";
    let vector = vec![0.1, 0.2, 0.3, 0.4];

    store.upsert_vector(doc_id, &ev(vector.clone())).unwrap();
    assert_eq!(store.get_vector(doc_id).map(|e| e.values), Some(vector));

    // Update the vector
    let new_vector = vec![0.5, 0.6, 0.7, 0.8];
    store
        .upsert_vector(doc_id, &ev(new_vector.clone()))
        .unwrap();
    assert_eq!(store.get_vector(doc_id).map(|e| e.values), Some(new_vector));
}

#[test]
fn test_get_vector() {
    let (_dir, store) = open_store();

    let doc_id = "doc-123";
    let vector = vec![0.1, 0.2, 0.3];

    store.upsert_vector(doc_id, &ev(vector.clone())).unwrap();
    assert_eq!(store.get_vector(doc_id).map(|e| e.values), Some(vector));
    assert!(store.get_vector("nonexistent").is_none());
}

#[test]
fn count_vectors_in_space_excludes_old_format_rows_sharing_the_same_provider_and_model() {
    // Same {provider, model} as `ev(..)`, but tagged with the OLD (pre-bump)
    // vector format — `EmbeddingConfig::matches` rejects these, so the
    // status strip's `indexedInActiveSpace` figure (and therefore its
    // derived `stale` count) must too, or a stale index would report "N/N
    // indexed" with `stale: 0` and the settings warning would never fire.
    let (_dir, store) = open_store();

    store
        .upsert_vector("doc-current", &ev(vec![0.1, 0.2]))
        .unwrap();
    assert_eq!(
        store.count_vectors_in_space("ollama", "nomic-embed-text"),
        1
    );

    let mut stale = ev(vec![0.3, 0.4]);
    stale.space.version = 0; // pre-chunk-pool format, same provider/model
    store.upsert_vector("doc-stale", &stale).unwrap();

    // The raw row count is 2, but the CURRENT-format count must still be 1 —
    // the stale row is invisible to the space-count that drives "indexed".
    assert_eq!(
        store.count_vectors_in_space("ollama", "nomic-embed-text"),
        1,
        "a version-0 row must not count as indexed in the current space"
    );
}

// ── embedding_space_changed (ai_set_embedding_config eviction gate) ───────────
//
// Pins the decision `ai_set_embedding_config` uses to decide whether to evict
// the posting_vectors / match_scores caches. False → no eviction; true → evict.

#[test]
fn embedding_space_changed_tracks_every_config_field() {
    for (old, new, changed, why) in [
        // Identical config → not a change → caches are NOT evicted.
        (cfg_ollama(), cfg_ollama(), false, "identical config"),
        // A different provider is a real space change → evict.
        (
            cfg_ollama(),
            EmbeddingConfig {
                provider: "openai".to_string(),
                ..cfg_ollama()
            },
            true,
            "provider change",
        ),
        // A different model (same provider) is a real space change → evict.
        (
            cfg_ollama(),
            EmbeddingConfig {
                model: "mxbai-embed-large".to_string(),
                ..cfg_ollama()
            },
            true,
            "model change",
        ),
        // A different base_url (same provider+model) still counts as a change → evict.
        (
            cfg_ollama(),
            EmbeddingConfig {
                base_url: Some("http://localhost:11434".to_string()),
                ..cfg_ollama()
            },
            true,
            "base_url change",
        ),
    ] {
        assert_eq!(embedding_space_changed(&old, &new), changed, "{why}");
    }
}

// ── Hash determinism ──────────────────────────────────────────────────────────

#[test]
fn test_sha256_hex_is_deterministic_and_distinct() {
    // Same input → same hash across calls (not RandomState/per-process salt).
    assert_eq!(sha256_hex("hello world"), sha256_hex("hello world"));
    // Different input → different hash.
    assert_ne!(sha256_hex("hello world"), sha256_hex("hello worlds"));
    // Lowercase hex, 64 chars (SHA-256 = 32 bytes).
    let h = sha256_hex("x");
    assert_eq!(h.len(), 64);
    assert!(h
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    // Known vector: sha256("") = e3b0c442...
    assert_eq!(
        sha256_hex(""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

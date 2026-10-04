//! The `posting_vectors` cache: round trip, the space / hash / format-version freshness guards
//! and single-row deletion.

use super::{support::*, *};

// ── Posting-vector cache ──────────────────────────────────────────────────────

#[test]
#[serial]
fn test_posting_vector_round_trip() {
    let (_dir, store) = open_store();

    let v = ev(vec![0.1, 0.2, 0.3]);
    let hash = sha256_hex("the exact job text that was embedded");
    store.upsert_posting_vector("job-1", &hash, &v).unwrap();

    let (got, got_hash) = store
        .get_posting_vector("job-1")
        .expect("posting vector must exist after upsert");
    assert_eq!(got.values, v.values);
    assert_eq!(got.space, v.space);
    assert_eq!(got_hash, hash);

    assert!(store.get_posting_vector("nonexistent").is_none());
}

// ── posting_vector_is_fresh (resolver cache-precedence predicate) ──────────────
//
// These exercise the SAME helper `posting_vector_or_embed` calls, so a reverted
// or loosened cache check (e.g. dropping the space or hash guard) fails here.

#[test]
fn posting_vector_is_fresh_needs_a_row_in_the_active_space_with_the_requested_hash() {
    let hash = sha256_hex("job text");
    let other_space = EmbeddingConfig {
        provider: "openai".to_string(),
        model: "text-embedding-3-small".to_string(),
        base_url: None,
    };
    // (active config, requested hash, cached row, expected, why)
    let cases = [
        // HIT: cached row's space matches the active config AND the requested hash
        // equals the stored hash.
        (
            cfg_ollama(),
            hash.clone(),
            Some((ev(vec![0.1, 0.2]), hash.clone())),
            true,
            "space and hash match",
        ),
        // MISS on space mismatch: a different provider/model means the stored vector is
        // in an incompatible space, even with a matching hash and a present row.
        (
            other_space,
            hash.clone(),
            Some((ev(vec![0.1, 0.2]), hash.clone())), // stored in ollama/nomic
            false,
            "space mismatch",
        ),
        // MISS on hash mismatch: same space, but the requested text differs (e.g. a
        // different translation of the posting) → different hash → stale row.
        (
            cfg_ollama(),
            sha256_hex("german job text"),
            Some((ev(vec![0.1, 0.2]), sha256_hex("english job text"))),
            false,
            "hash mismatch",
        ),
        // MISS when there is no cached row at all (`None`).
        (cfg_ollama(), hash.clone(), None, false, "absent"),
    ];
    for (active, requested, cached, fresh, why) in cases {
        assert_eq!(
            posting_vector_is_fresh(&active, &requested, cached.as_ref()),
            fresh,
            "{why}"
        );
    }
}

// The cache guard is space + hash, end-to-end through the store: a stored vector
// under provider/model A must not be trusted when the active config is
// provider/model B (space miss), even though the row is present and hash matches.
#[test]
#[serial]
fn test_posting_vector_space_miss() {
    let (_dir, store) = open_store();

    let text = "job text";
    let hash = sha256_hex(text);
    // Store under ollama/nomic-embed-text (what `ev` builds).
    store
        .upsert_posting_vector("job-1", &hash, &ev(vec![0.1, 0.2]))
        .unwrap();

    let cached = store.get_posting_vector("job-1");
    // Active config in a different space → resolver miss (via the real helper).
    let active_other = EmbeddingConfig {
        provider: "openai".to_string(),
        model: "text-embedding-3-small".to_string(),
        base_url: None,
    };
    assert!(!posting_vector_is_fresh(
        &active_other,
        &hash,
        cached.as_ref()
    ));
    // Same-space config with the same hash → hit.
    assert!(posting_vector_is_fresh(
        &cfg_ollama(),
        &hash,
        cached.as_ref()
    ));
}

// A matching space but a different text_hash (e.g. a different translation of
// the same posting) must miss — exercised through the store + real helper.
#[test]
#[serial]
fn test_posting_vector_text_hash_miss() {
    let (_dir, store) = open_store();

    let stored_hash = sha256_hex("english job text");
    store
        .upsert_posting_vector("job-1", &stored_hash, &ev(vec![0.1, 0.2]))
        .unwrap();

    let cached = store.get_posting_vector("job-1");
    let computed = sha256_hex("german job text"); // different text → different hash
                                                  // Space matches, but the hash guard fails → overall miss.
    assert!(!posting_vector_is_fresh(
        &cfg_ollama(),
        &computed,
        cached.as_ref()
    ));
}

#[test]
#[serial]
fn test_posting_vector_upsert_replaces() {
    let (_dir, store) = open_store();

    let h1 = sha256_hex("v1 text");
    store
        .upsert_posting_vector("job-1", &h1, &ev(vec![0.1]))
        .unwrap();
    let h2 = sha256_hex("v2 text");
    store
        .upsert_posting_vector("job-1", &h2, &ev(vec![0.9, 0.8]))
        .unwrap();

    let (v, hash) = store.get_posting_vector("job-1").unwrap();
    assert_eq!(v.values, vec![0.9, 0.8]);
    assert_eq!(hash, h2);

    store.clear_posting_vectors().unwrap();
    assert!(store.get_posting_vector("job-1").is_none());
}

// ── posting_vectors.version (self-detecting staleness, not hand-maintained) ────
//
// `get_posting_vector` used to ALWAYS synthesize the current
// `EMBEDDING_VECTOR_VERSION` because the table had no persisted `version`
// column — `EmbeddingConfig::matches` was structurally incapable of ever
// rejecting a row here on format version (see the `add_version_to_posting_vectors`
// migration doc comment). `upsert_posting_vector` now persists `v.space.version`
// and `get_posting_vector` reads it back, so a stale-format row is a REAL cache
// miss instead of a hand-maintained invariant.

#[test]
#[serial]
fn posting_vector_stored_at_an_older_version_is_a_cache_miss() {
    let (_dir, store) = open_store();

    let hash = sha256_hex("job text");
    let mut stale = ev(vec![0.1, 0.2]);
    stale.space.version = 0; // pre-migration / pre-bump format
    store.upsert_posting_vector("job-1", &hash, &stale).unwrap();

    let (got, got_hash) = store
        .get_posting_vector("job-1")
        .expect("row still round-trips — staleness is a MATCHES miss, not a read failure");
    assert_eq!(
        got.space.version, 0,
        "the persisted version must survive the write, or the row silently reads as fresh"
    );
    assert_eq!(got_hash, hash);

    // Same provider/model/hash, but the active config must reject the space on
    // version alone.
    assert!(
        !cfg_ollama().matches(&got.space),
        "a version-0 row must not match the active (current-version) space"
    );
    assert!(
        !posting_vector_is_fresh(&cfg_ollama(), &hash, Some(&(got, got_hash))),
        "a version-0 row must be an overall cache MISS even with a matching provider/model/hash"
    );
}

#[test]
#[serial]
fn posting_vector_at_the_current_version_with_matching_hash_is_a_cache_hit() {
    let (_dir, store) = open_store();

    let hash = sha256_hex("job text");
    store
        .upsert_posting_vector("job-1", &hash, &ev(vec![0.1, 0.2]))
        .unwrap();

    let cached = store.get_posting_vector("job-1");
    assert_eq!(
        cached.as_ref().map(|(v, _)| v.space.version),
        Some(EMBEDDING_VECTOR_VERSION),
        "a freshly-written row must persist the CURRENT version, not a placeholder"
    );
    assert!(
        posting_vector_is_fresh(&cfg_ollama(), &hash, cached.as_ref()),
        "a current-version row with a matching space + hash must be a HIT"
    );
}

// ── one posting-vector row can be dropped with its producer ──────────────────

/// The autopilot re-rank's résumé snapshot lives in this cache, so deleting the
/// autopilot needs a single-row delete (the cache is otherwise bounded only by
/// its TTL and row cap — see `commands::autopilot::drop_orphaned_resume_cache`).
#[test]
#[serial]
fn delete_posting_vector_removes_only_that_row() {
    let (_dir, store) = open_store();
    store
        .upsert_posting_vector("autopilot-resume:aaa", "hash-a", &ev(vec![0.1, 0.2]))
        .unwrap();
    store
        .upsert_posting_vector("autopilot:bbb", "hash-b", &ev(vec![0.3, 0.4]))
        .unwrap();

    store.delete_posting_vector("autopilot-resume:aaa").unwrap();

    assert!(store.get_posting_vector("autopilot-resume:aaa").is_none());
    assert!(
        store.get_posting_vector("autopilot:bbb").is_some(),
        "the neighbouring posting row is untouched"
    );
    // Idempotent: deleting a missing row is not an error.
    store.delete_posting_vector("autopilot-resume:aaa").unwrap();
}

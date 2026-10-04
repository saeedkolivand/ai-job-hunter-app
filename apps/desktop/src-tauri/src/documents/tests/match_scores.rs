//! The `match_scores` result cache: every axis of the primary key is a miss when it changes, and
//! nothing but an explicit upsert can populate it.

use super::{support::*, *};

#[test]
#[serial]
fn test_match_score_round_trip_and_key_sensitivity() {
    let (_dir, store) = open_store();

    let hash = sha256_hex("job text");
    let key = match_key("resume-1", "job-1", 1, 1, &hash);
    let payload = serde_json::json!({ "combined": 87.0, "ats": 80.0 });
    let s = serde_json::to_string(&payload).unwrap();
    store.upsert_match_score(&key, &s).unwrap();

    // Identical key → hit (same JSON back).
    let got = store.get_match_score(&key).expect("identical key must hit");
    assert_eq!(got, payload);

    // Changing formula_version → miss.
    let key_v2 = match_key("resume-1", "job-1", 1, 2, &hash);
    assert!(store.get_match_score(&key_v2).is_none());

    // Changing job_text_hash → miss.
    let other_hash = sha256_hex("different job text");
    let key_h2 = match_key("resume-1", "job-1", 1, 1, &other_hash);
    assert!(store.get_match_score(&key_h2).is_none());

    // Changing semantic_enabled → miss.
    let key_s0 = match_key("resume-1", "job-1", 0, 1, &hash);
    assert!(store.get_match_score(&key_s0).is_none());

    // Changing vector_version (with everything else, including
    // formula_version, held identical) → miss. A semantic score is derived
    // from embedding vectors, so a vector-format bump must invalidate on its
    // own, not just piggyback on a coincidental formula_version bump.
    let mut key_vv2 = match_key("resume-1", "job-1", 1, 1, &hash);
    key_vv2.vector_version = 2;
    assert!(store.get_match_score(&key_vv2).is_none());
}

// Invalidation matrix — the embedding-space axis of the PK. A score cached in the
// ollama/nomic space must MISS when looked up under a different provider OR a
// different model. Guards against dropping the provider/model columns from the
// match_scores primary key.
#[test]
#[serial]
fn test_match_score_invalidates_on_provider_or_model_change() {
    let (_dir, store) = open_store();

    let hash = sha256_hex("job text");
    // Baseline: cache a score in the ollama/nomic space.
    let base = match_key_in_space("r", "j", "ollama", "nomic-embed-text", 1, 1, &hash);
    store
        .upsert_match_score(&base, "{\"combined\":50}")
        .unwrap();
    assert!(
        store.get_match_score(&base).is_some(),
        "baseline key must hit"
    );

    // Different provider (same model name) → miss.
    let other_provider = match_key_in_space("r", "j", "openai", "nomic-embed-text", 1, 1, &hash);
    assert!(
        store.get_match_score(&other_provider).is_none(),
        "changing provider must be a cache miss"
    );

    // Different model (same provider) → miss.
    let other_model = match_key_in_space("r", "j", "ollama", "text-embedding-ada-002", 1, 1, &hash);
    assert!(
        store.get_match_score(&other_model).is_none(),
        "changing model must be a cache miss"
    );

    // Both changed → miss.
    let both = match_key_in_space("r", "j", "openai", "text-embedding-ada-002", 1, 1, &hash);
    assert!(
        store.get_match_score(&both).is_none(),
        "changing provider+model must be a cache miss"
    );
}

// HIGH 3 — errors-never-cached (store half of the invariant). `match_resume`
// returns "resume/job not found" BEFORE any cache code runs (see the INVARIANT
// comment at its guard site), so an error path can never pre-populate
// match_scores. This pins that at the store level: a key never written must read
// back `None` — i.e. a `get_match_score` cannot conjure a row, so the only way a
// row exists is a prior `upsert_match_score` (which the error paths never reach).
#[test]
#[serial]
fn errors_never_populate_match_scores_cache() {
    let (_dir, store) = open_store();

    // A key for a (resume, job) pair that an error path would have rejected.
    let hash = sha256_hex("job text");
    let key = match_key("missing-resume", "missing-job", 1, 1, &hash);

    // No upsert_match_score has run → the cache must be empty for this key.
    assert!(
        store.get_match_score(&key).is_none(),
        "a get without a prior upsert must miss — errors cannot pre-populate the cache"
    );
}

#[test]
#[serial]
fn test_match_score_upsert_replaces_and_clear() {
    let (_dir, store) = open_store();

    let hash = sha256_hex("job text");
    let key = match_key("resume-1", "job-1", 1, 1, &hash);
    store.upsert_match_score(&key, "{\"combined\":10}").unwrap();
    store.upsert_match_score(&key, "{\"combined\":99}").unwrap();
    let got = store.get_match_score(&key).unwrap();
    assert_eq!(got["combined"], serde_json::json!(99));

    store.clear_match_scores().unwrap();
    assert!(store.get_match_score(&key).is_none());
}

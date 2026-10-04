use super::*;

// Pins the production `semantic_enabled_bit` helper (used by both the cache
// key and the skip-branch): only `Some(true)` → 1 (enabled); `Some(false)`
// AND `None` → 0 (keyword-only) so an omitted flag defaults OFF, matching the
// app-wide default. Tests the real fn, not an inline re-implementation.
#[test]
fn semantic_enabled_bit_maps_flag_to_key_column() {
    assert_eq!(semantic_enabled_bit(Some(false)), 0, "explicit disable → 0");
    assert_eq!(semantic_enabled_bit(Some(true)), 1, "explicit enable → 1");
    assert_eq!(
        semantic_enabled_bit(None),
        0,
        "default (unset) → keyword-only (semantic OFF)"
    );
}

// A bump to MATCH_FORMULA_VERSION must change the cache key, so a score
// cached under the current version is a miss under the next one. Exercises
// self-invalidation end-to-end against a real store.
//
// The same holds for the vector-FORMAT axis, and that is the defect the second
// half pins: a semantic score is derived from embedding vectors,
// so a vector-FORMAT bump (`EMBEDDING_VECTOR_VERSION`) changes what a cached
// score means even when `formula_version` and the job text are unchanged.
// Before `vector_version` joined the key, this bump only self-invalidated
// by accident (a coincidental MATCH_FORMULA_VERSION bump, e.g. #933) — a
// future vector-format bump with no coincidental formula bump would have
// silently served a stale semantic score forever. Two otherwise-identical
// keys differing ONLY in `vector_version` must not collide.
#[test]
fn a_bump_of_either_cache_key_version_invalidates_cached_score() {
    use crate::documents::{sha256_hex, DocumentStore, MatchScoreKey};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let hash = sha256_hex("job text");
    let key = |fv: i64, vv: i64| MatchScoreKey {
        resume_id: "r",
        job_id: "j",
        provider: "ollama",
        model: "nomic-embed-text",
        semantic_enabled: 1,
        formula_version: fv,
        vector_version: vv,
        job_text_hash: &hash,
    };
    let current = key(MATCH_FORMULA_VERSION, EMBEDDING_VECTOR_VERSION);

    // Cache a score under the current formula and vector versions → hit.
    store
        .upsert_match_score(&current, "{\"combined\":50}")
        .unwrap();
    assert!(store.get_match_score(&current).is_some());

    // The next formula version is a different key → miss (stale on bump).
    assert!(store
        .get_match_score(&key(MATCH_FORMULA_VERSION + 1, EMBEDDING_VECTOR_VERSION))
        .is_none());

    // The next vector version is a different key → miss (stale on bump),
    // with formula_version and every other field held identical — proves
    // vector_version alone, not some other field, drives the invalidation.
    assert!(store
        .get_match_score(&key(MATCH_FORMULA_VERSION, EMBEDDING_VECTOR_VERSION + 1))
        .is_none());
}

// MATCH_FORMULA_VERSION guard: if a maintainer bumps the constant they MUST
// also bump the expected value here and invalidate any affected caches.
// Failing here is intentional — it's the reminder that a bump is breaking.
#[test]
fn formula_version_constant_is_pinned() {
    assert_eq!(
        MATCH_FORMULA_VERSION, 3,
        "MATCH_FORMULA_VERSION changed — update this assert AND invalidate \
         cached match scores (clear match_scores table or bump the stored version)"
    );
}

// Round-trip parity: a 7-field MatchScore JSON blob survives
// upsert_match_score → get_match_score with every field name and type intact.
// Guards against a future rename/drop of any result-cache field.
#[test]
fn match_score_round_trip_preserves_all_seven_fields() {
    use crate::documents::{sha256_hex, DocumentStore, MatchScoreKey};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let hash = sha256_hex("round trip job text");
    let key = MatchScoreKey {
        resume_id: "resume-rt",
        job_id: "job-rt",
        provider: "ollama",
        model: "nomic-embed-text",
        semantic_enabled: 1,
        formula_version: MATCH_FORMULA_VERSION,
        vector_version: EMBEDDING_VECTOR_VERSION,
        job_text_hash: &hash,
    };

    // Build a known 7-field score JSON that mirrors the shape score_one produces.
    let score_json = serde_json::json!({
        "resumeId":       "resume-rt",
        "jobId":          "job-rt",
        "ats":            60.0_f64,
        "semantic":       75.0_f64,
        "combined":       70.0_f64,
        "gaps":           ["kubernetes", "terraform"],
        "recommendations": ["Consider adding evidence of: kubernetes, terraform."]
    });
    store
        .upsert_match_score(&key, &serde_json::to_string(&score_json).unwrap())
        .unwrap();

    let got = store
        .get_match_score(&key)
        .expect("score must be present after upsert");

    assert_eq!(
        got["resumeId"], "resume-rt",
        "resumeId field must survive round-trip"
    );
    assert_eq!(
        got["jobId"], "job-rt",
        "jobId field must survive round-trip"
    );
    assert_eq!(
        got["ats"], 60.0_f64,
        "ats field must survive round-trip as a number"
    );
    assert_eq!(
        got["semantic"], 75.0_f64,
        "semantic field must survive round-trip as a number"
    );
    assert_eq!(
        got["combined"], 70.0_f64,
        "combined field must survive round-trip as a number"
    );
    assert!(
        got["gaps"].is_array(),
        "gaps must survive round-trip as an array"
    );
    assert_eq!(
        got["gaps"].as_array().unwrap().len(),
        2,
        "gaps array length must be preserved"
    );
    assert!(
        got["recommendations"].is_array(),
        "recommendations must survive round-trip as an array"
    );
    // Distinct values: ats != semantic != combined — guards against field swap.
    assert_ne!(
        got["ats"], got["combined"],
        "ats and combined must be distinct"
    );
    assert_ne!(
        got["semantic"], got["combined"],
        "semantic and combined must be distinct"
    );
}

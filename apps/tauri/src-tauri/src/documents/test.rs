use super::*;
use tempfile::TempDir;

/// Build a space-tagged vector for the default (Ollama/nomic) space in tests.
fn ev(values: Vec<f64>) -> EmbeddingVector {
    let dim = values.len();
    EmbeddingVector {
        values,
        space: EmbeddingSpace {
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            dim,
        },
    }
}

#[test]
fn test_open_store() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let docs = store.list();
    assert!(docs.is_empty());
}

#[test]
fn test_insert_document() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let doc = DocumentRecord {
        id: make_doc_id(),
        title: "Resume".to_string(),
        name: "resume.pdf".to_string(),
        locale: Some("en".to_string()),
        text: "Software Engineer with 5 years experience".to_string(),
        pages: Some(2),
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    };

    store.insert(&doc).unwrap();
    let docs = store.list();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].title, "Resume");
    // First document should be auto-set as default
    assert!(docs[0].is_default);
}

#[test]
fn test_list_documents() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let doc1 = DocumentRecord {
        id: make_doc_id(),
        title: "Resume".to_string(),
        name: "resume.pdf".to_string(),
        locale: None,
        text: "Text 1".to_string(),
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    };

    let doc2 = DocumentRecord {
        id: make_doc_id(),
        title: "CV".to_string(),
        name: "cv.pdf".to_string(),
        locale: None,
        text: "Text 2".to_string(),
        pages: None,
        created_at: now_ms() + 1000,
        indexed: false,
        is_default: false,
        keywords_json: None,
    };

    store.insert(&doc1).unwrap();
    store.insert(&doc2).unwrap();

    let docs = store.list();
    assert_eq!(docs.len(), 2);
    // Should be sorted by created_at desc
    assert_eq!(docs[0].title, "CV");
    assert_eq!(docs[1].title, "Resume");
}

#[test]
fn test_set_indexed() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let doc = DocumentRecord {
        id: make_doc_id(),
        title: "Resume".to_string(),
        name: "resume.pdf".to_string(),
        locale: None,
        text: "Text".to_string(),
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    };

    store.insert(&doc).unwrap();
    store.set_indexed(&doc.id).unwrap();

    let docs = store.list();
    assert!(docs[0].indexed);
}

#[test]
fn test_remove_document() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let doc = DocumentRecord {
        id: make_doc_id(),
        title: "Resume".to_string(),
        name: "resume.pdf".to_string(),
        locale: None,
        text: "Text".to_string(),
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    };

    store.insert(&doc).unwrap();
    store.remove(&doc.id).unwrap();

    let docs = store.list();
    assert!(docs.is_empty());
}

#[test]
fn test_set_default() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let doc1 = DocumentRecord {
        id: make_doc_id(),
        title: "Resume".to_string(),
        name: "resume.pdf".to_string(),
        locale: None,
        text: "Text 1".to_string(),
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    };

    let doc2 = DocumentRecord {
        id: make_doc_id(),
        title: "CV".to_string(),
        name: "cv.pdf".to_string(),
        locale: None,
        text: "Text 2".to_string(),
        pages: None,
        created_at: now_ms() + 1000,
        indexed: false,
        is_default: false,
        keywords_json: None,
    };

    store.insert(&doc1).unwrap();
    store.insert(&doc2).unwrap();

    // Set doc2 as default
    store.set_default(&doc2.id).unwrap();

    let docs = store.list();
    assert!(!docs.iter().find(|d| d.id == doc1.id).unwrap().is_default);
    assert!(docs.iter().find(|d| d.id == doc2.id).unwrap().is_default);
}

#[test]
fn test_upsert_vector() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let doc_id = "doc-123";
    let vector = vec![0.1, 0.2, 0.3];

    store.upsert_vector(doc_id, &ev(vector.clone())).unwrap();
    assert_eq!(store.get_vector(doc_id).map(|e| e.values), Some(vector));
    assert!(store.get_vector("nonexistent").is_none());
}

#[test]
fn test_all_vectors() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    store.upsert_vector("doc-1", &ev(vec![0.1, 0.2])).unwrap();
    store.upsert_vector("doc-2", &ev(vec![0.3, 0.4])).unwrap();

    let vectors = store.all_vectors();
    assert_eq!(vectors.len(), 2);
}

#[test]
fn test_extract_text_plain() {
    let result = crate::extraction::route("test.txt", b"Hello, World!").unwrap();
    assert_eq!(result.text, "Hello, World!");
}

#[test]
fn test_extract_text_markdown() {
    let result = crate::extraction::route("test.md", b"# Heading\nContent").unwrap();
    assert_eq!(result.text, "# Heading\nContent");
}

#[test]
fn test_extract_text_unsupported() {
    let result = crate::extraction::route("test.xyz", b"content");
    assert!(result.is_err());
}

#[test]
fn test_cosine_similarity() {
    let a = vec![1.0, 2.0, 3.0];
    let b = vec![1.0, 2.0, 3.0];
    let sim = cosine_similarity(&a, &b);
    assert!((sim - 1.0).abs() < 0.001);
}

#[test]
fn test_cosine_similarity_orthogonal() {
    let a = vec![1.0, 0.0];
    let b = vec![0.0, 1.0];
    let sim = cosine_similarity(&a, &b);
    assert!((sim - 0.0).abs() < 0.001);
}

#[test]
fn test_cosine_similarity_edge_cases() {
    // Empty vectors
    assert_eq!(cosine_similarity(&[], &[]), 0.0);

    // Mismatched lengths
    assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);

    // Zero vectors
    assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
}

// Verify that keywords_json survives an insert → list → get round-trip without
// any column-position corruption from future migrations.
#[test]
fn test_keywords_json_round_trip() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let keywords_payload = Some("[\"rust\",\"typescript\"]".to_string());
    let doc = DocumentRecord {
        id: make_doc_id(),
        title: "Resume".to_string(),
        name: "resume.pdf".to_string(),
        locale: None,
        text: "Rust and TypeScript developer".to_string(),
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: keywords_payload.clone(),
    };

    store.insert(&doc).unwrap();

    // list() path
    let docs = store.list();
    assert_eq!(docs.len(), 1);
    assert_eq!(
        docs[0].keywords_json, keywords_payload,
        "keywords_json must survive list() unchanged"
    );

    // get() path
    let fetched = store
        .get(&doc.id)
        .expect("document must exist after insert");
    assert_eq!(
        fetched.keywords_json, keywords_payload,
        "keywords_json must survive get() unchanged"
    );
}

#[test]
fn test_data_store_export_import_round_trip() {
    use crate::data_store::DataStore;

    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let a = DocumentRecord {
        id: "doc-a".to_string(),
        title: "A".to_string(),
        name: "a.pdf".to_string(),
        locale: None,
        text: "first".to_string(),
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    let b = DocumentRecord {
        id: "doc-b".to_string(),
        title: "B".to_string(),
        name: "b.pdf".to_string(),
        locale: None,
        text: "second".to_string(),
        pages: None,
        created_at: now_ms() + 1,
        indexed: false,
        is_default: true,
        keywords_json: None,
    };
    store.insert(&a).unwrap();
    store.insert(&b).unwrap();
    store.set_default("doc-b").unwrap();
    store
        .upsert_vector("doc-b", &ev(vec![0.1, 0.2, 0.3]))
        .unwrap();

    let bundle = store.export();

    // Restore into a fresh store.
    let temp2 = TempDir::new().unwrap();
    let restored = DocumentStore::open(&temp2.path().to_path_buf()).unwrap();
    let count = restored.import(&bundle).unwrap();

    assert_eq!(count, 2);
    let docs = restored.list();
    assert_eq!(docs.len(), 2);
    // The originally-default doc stays default after restore.
    assert_eq!(
        docs.iter().find(|d| d.is_default).map(|d| d.id.as_str()),
        Some("doc-b")
    );
    // Vectors survive the round trip.
    assert_eq!(
        restored.get_vector("doc-b").map(|e| e.values),
        Some(vec![0.1, 0.2, 0.3])
    );
}

// ── Posting-vector cache ──────────────────────────────────────────────────────

#[test]
fn test_posting_vector_round_trip() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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

/// The default (Ollama/nomic) embedding config — the space `ev` builds vectors in.
fn cfg_ollama() -> EmbeddingConfig {
    EmbeddingConfig {
        provider: "ollama".to_string(),
        model: "nomic-embed-text".to_string(),
        base_url: None,
    }
}

// ── posting_vector_is_fresh (resolver cache-precedence predicate) ──────────────
//
// These exercise the SAME helper `posting_vector_or_embed` calls, so a reverted
// or loosened cache check (e.g. dropping the space or hash guard) fails here.

// HIT: cached row's space matches the active config AND the requested hash
// equals the stored hash.
#[test]
fn posting_vector_is_fresh_hit_when_space_and_hash_match() {
    let hash = sha256_hex("job text");
    let cached = (ev(vec![0.1, 0.2]), hash.clone());
    assert!(posting_vector_is_fresh(&cfg_ollama(), &hash, Some(&cached)));
}

// MISS on space mismatch: a different provider/model means the stored vector is
// in an incompatible space, even with a matching hash and a present row.
#[test]
fn posting_vector_is_fresh_miss_on_space_mismatch() {
    let hash = sha256_hex("job text");
    let cached = (ev(vec![0.1, 0.2]), hash.clone()); // stored in ollama/nomic
    let active_other = EmbeddingConfig {
        provider: "openai".to_string(),
        model: "text-embedding-3-small".to_string(),
        base_url: None,
    };
    assert!(!posting_vector_is_fresh(
        &active_other,
        &hash,
        Some(&cached)
    ));
}

// MISS on hash mismatch: same space, but the requested text differs (e.g. a
// different translation of the posting) → different hash → stale row.
#[test]
fn posting_vector_is_fresh_miss_on_hash_mismatch() {
    let stored_hash = sha256_hex("english job text");
    let cached = (ev(vec![0.1, 0.2]), stored_hash);
    let requested = sha256_hex("german job text");
    assert!(!posting_vector_is_fresh(
        &cfg_ollama(),
        &requested,
        Some(&cached)
    ));
}

// MISS when there is no cached row at all (`None`).
#[test]
fn posting_vector_is_fresh_miss_when_absent() {
    let hash = sha256_hex("job text");
    assert!(!posting_vector_is_fresh(&cfg_ollama(), &hash, None));
}

// The cache guard is space + hash, end-to-end through the store: a stored vector
// under provider/model A must not be trusted when the active config is
// provider/model B (space miss), even though the row is present and hash matches.
#[test]
fn test_posting_vector_space_miss() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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
fn test_posting_vector_text_hash_miss() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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
fn test_posting_vector_upsert_replaces() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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

// ── embedding_space_changed (ai_set_embedding_config eviction gate) ───────────
//
// Pins the decision `ai_set_embedding_config` uses to decide whether to evict
// the posting_vectors / match_scores caches. False → no eviction; true → evict.

// Identical config → not a change → caches are NOT evicted.
#[test]
fn embedding_space_changed_false_for_identical_config() {
    let cfg = cfg_ollama();
    assert!(!embedding_space_changed(&cfg, &cfg.clone()));
}

// A different provider is a real space change → evict.
#[test]
fn embedding_space_changed_true_on_provider_change() {
    let old = cfg_ollama();
    let new = EmbeddingConfig {
        provider: "openai".to_string(),
        ..cfg_ollama()
    };
    assert!(embedding_space_changed(&old, &new));
}

// A different model (same provider) is a real space change → evict.
#[test]
fn embedding_space_changed_true_on_model_change() {
    let old = cfg_ollama();
    let new = EmbeddingConfig {
        model: "mxbai-embed-large".to_string(),
        ..cfg_ollama()
    };
    assert!(embedding_space_changed(&old, &new));
}

// A different base_url (same provider+model) still counts as a change → evict.
#[test]
fn embedding_space_changed_true_on_base_url_change() {
    let old = cfg_ollama();
    let new = EmbeddingConfig {
        base_url: Some("http://localhost:11434".to_string()),
        ..cfg_ollama()
    };
    assert!(embedding_space_changed(&old, &new));
}

// ── Match-result cache ────────────────────────────────────────────────────────

fn match_key<'a>(
    resume_id: &'a str,
    job_id: &'a str,
    semantic_enabled: i64,
    formula_version: i64,
    job_text_hash: &'a str,
) -> MatchScoreKey<'a> {
    MatchScoreKey {
        resume_id,
        job_id,
        provider: "ollama",
        model: "nomic-embed-text",
        semantic_enabled,
        formula_version,
        job_text_hash,
    }
}

/// Like [`match_key`] but with the embedding space (provider/model) parameterized,
/// so tests can vary the space axis of the cache PK.
fn match_key_in_space<'a>(
    resume_id: &'a str,
    job_id: &'a str,
    provider: &'a str,
    model: &'a str,
    semantic_enabled: i64,
    formula_version: i64,
    job_text_hash: &'a str,
) -> MatchScoreKey<'a> {
    MatchScoreKey {
        resume_id,
        job_id,
        provider,
        model,
        semantic_enabled,
        formula_version,
        job_text_hash,
    }
}

#[test]
fn test_match_score_round_trip_and_key_sensitivity() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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
}

// Invalidation matrix — the embedding-space axis of the PK. A score cached in the
// ollama/nomic space must MISS when looked up under a different provider OR a
// different model. Guards against dropping the provider/model columns from the
// match_scores primary key.
#[test]
fn test_match_score_invalidates_on_provider_or_model_change() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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
fn errors_never_populate_match_scores_cache() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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
fn test_match_score_upsert_replaces_and_clear() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let hash = sha256_hex("job text");
    let key = match_key("resume-1", "job-1", 1, 1, &hash);
    store.upsert_match_score(&key, "{\"combined\":10}").unwrap();
    store.upsert_match_score(&key, "{\"combined\":99}").unwrap();
    let got = store.get_match_score(&key).unwrap();
    assert_eq!(got["combined"], serde_json::json!(99));

    store.clear_match_scores().unwrap();
    assert!(store.get_match_score(&key).is_none());
}

// `clear_all()` (the factory-reset path: `Resettable::reset()` → `clear_all()`)
// must wipe ALL FOUR tables — documents, vectors, posting_vectors, match_scores —
// otherwise a user's "delete all data" leaves résumés, embeddings, and match
// scores at rest. Guards the data-retention contract for the full table set.
#[test]
fn test_clear_all_wipes_posting_vectors_and_match_scores() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Populate all four tables: a document, its résumé vector, a posting vector,
    // and a match score.
    let doc = DocumentRecord {
        id: "doc-1".to_string(),
        title: "Resume".to_string(),
        name: "resume.pdf".to_string(),
        locale: None,
        text: "Rust developer".to_string(),
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    store.insert(&doc).unwrap();
    store
        .upsert_vector("doc-1", &ev(vec![0.4, 0.5, 0.6]))
        .unwrap();

    let hash = sha256_hex("job text");
    store
        .upsert_posting_vector("job-1", &hash, &ev(vec![0.1, 0.2, 0.3]))
        .unwrap();
    let key = match_key("resume-1", "job-1", 1, 1, &hash);
    store.upsert_match_score(&key, "{\"combined\":87}").unwrap();

    // Sanity: all four present before reset.
    assert!(!store.list().is_empty(), "document present before reset");
    assert!(
        store.get_vector("doc-1").is_some(),
        "vector present before reset"
    );
    assert!(store.get_posting_vector("job-1").is_some());
    assert!(store.get_match_score(&key).is_some());

    store.clear_all();

    // All four tables must be empty after a full reset.
    assert!(store.list().is_empty(), "clear_all() must wipe documents");
    assert!(
        store.get_vector("doc-1").is_none(),
        "clear_all() must wipe vectors"
    );
    assert!(
        store.get_posting_vector("job-1").is_none(),
        "clear_all() must wipe posting_vectors"
    );
    assert!(
        store.get_match_score(&key).is_none(),
        "clear_all() must wipe match_scores"
    );
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

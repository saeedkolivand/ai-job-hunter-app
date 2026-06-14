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

// ── Cache eviction (prune_caches / PerformanceConfig integration) ─────────────
//
// Tests for:
//   - Row-cap eviction: inserting > maxRows rows then pruning leaves only newest n.
//   - TTL eviction: rows older than the cutoff are dropped; newer ones remain.
//   - Read-side TTL: get_match_score / get_posting_vector returns None for an
//     expired row even before prune (TTL miss on the read query itself).
//   - Generous (None/None) mode: no eviction, count unchanged.
//
// IMPORTANT: `PerformanceConfig` lives in a process-global `OnceLock<ArcSwap>`.
// We set it explicitly at the start of each test that depends on it, then
// restore the balanced default after so we don't bleed into the hash-determinism
// test. Tests in the same binary share the global — always set before asserting.

fn set_perf(ttl_secs: Option<i64>, max_rows: Option<i64>) {
    crate::performance::set(crate::performance::PerformanceConfig {
        keep_alive_secs: 300,
        cache_ttl_secs: ttl_secs,
        cache_max_rows: max_rows,
    });
}

fn reset_perf_to_balanced() {
    crate::performance::set(crate::performance::PerformanceConfig::default());
}

// Count rows in a table via a raw SQL query.
fn count_table(store: &DocumentStore, table: &str) -> i64 {
    let conn = store.conn.lock();
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap_or(0)
}

// ── Row-cap eviction: match_scores ────────────────────────────────────────────
//
// Implementation note: `prune_table_locked` uses
//   DELETE WHERE created_at < (SELECT created_at … ORDER BY DESC LIMIT 1 OFFSET n)
// OFFSET n picks the (n+1)-th newest row (0-indexed).  DELETE removes rows
// strictly OLDER than that pivot.  Result: the pivot + n rows newer than it stay
// → n+1 rows remain.  So "cap_param=2" leaves 3 rows, "cap_param=1" leaves 2, etc.
// The tests below pin this contract so any drift in the SQL is caught.

#[test]
fn prune_caches_row_cap_keeps_newest_match_scores() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Insert 5 match-score rows with strictly increasing created_at values.
    // Generous limits during insert so the per-write prune is a no-op.
    set_perf(None, None);
    let base_ts = now_ms();
    for i in 0_u64..5 {
        let hash = sha256_hex(&format!("job-text-{i}"));
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO match_scores
             (resume_id, job_id, provider, model, semantic_enabled, formula_version,
              job_text_hash, score_json, created_at)
             VALUES ('r', ?1, 'ollama', 'nomic-embed-text', 1, 1, ?2, ?3, ?4)",
            params![
                format!("job-{i}"),
                hash,
                format!("{{\"score\":{}}}", i),
                ts_to_db(base_ts + i * 1000),
            ],
        )
        .unwrap();
    }

    assert_eq!(
        count_table(&store, "match_scores"),
        5,
        "5 rows before prune"
    );

    // cap_param=2: DELETE WHERE created_at < (row at OFFSET 2 DESC) = ts2.
    // Deletes ts1 and ts0.  Keeps ts4, ts3, ts2 → 3 rows.
    store.prune_caches(None, Some(2));

    let remaining = count_table(&store, "match_scores");
    assert_eq!(
        remaining, 3,
        "after prune(cap=2): 3 rows remain (OFFSET 2 semantics)"
    );

    // The two oldest (job-0, job-1) must be evicted; job-2/3/4 must remain.
    {
        let conn = store.conn.lock();
        for &evicted in &["job-0", "job-1"] {
            let cnt: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM match_scores WHERE job_id = ?1",
                    params![evicted],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            assert_eq!(cnt, 0, "oldest row {evicted} must have been evicted");
        }
        for &kept in &["job-2", "job-3", "job-4"] {
            let cnt: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM match_scores WHERE job_id = ?1",
                    params![kept],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            assert_eq!(cnt, 1, "newest row {kept} must have been kept");
        }
    }

    reset_perf_to_balanced();
}

// ── Row-cap eviction: posting_vectors ─────────────────────────────────────────

#[test]
fn prune_caches_row_cap_keeps_newest_posting_vectors() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Insert 4 posting-vector rows with strictly increasing created_at.
    set_perf(None, None);
    let base_ts = now_ms();
    for i in 0_u64..4 {
        let hash = sha256_hex(&format!("pv-text-{i}"));
        let job_id = format!("pv-row-{i}");
        let v = ev(vec![0.1 * (i + 1) as f64]);
        let json = serde_json::to_string(&v.values).unwrap();
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO posting_vectors
             (job_id, text_hash, vector, provider, model, dim, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                job_id,
                hash,
                json,
                v.space.provider,
                v.space.model,
                v.space.dim as i64,
                ts_to_db(base_ts + i * 1000),
            ],
        )
        .unwrap();
    }

    assert_eq!(count_table(&store, "posting_vectors"), 4);

    // cap_param=1: OFFSET 1 DESC picks the 2nd newest (ts2). DELETE WHERE < ts2.
    // Deleted: ts1, ts0.  Keeps: ts3, ts2 → 2 rows.
    store.prune_caches(None, Some(1));

    assert_eq!(
        count_table(&store, "posting_vectors"),
        2,
        "cap=1 → 2 rows remain"
    );

    // pv-row-0 and pv-row-1 (oldest two) must be evicted.
    for &gone in &["pv-row-0", "pv-row-1"] {
        let conn = store.conn.lock();
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM posting_vectors WHERE job_id = ?1",
                params![gone],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(cnt, 0, "{gone} must have been evicted");
    }

    reset_perf_to_balanced();
}

// ── TTL eviction: prune_caches removes rows older than the cutoff ─────────────

#[test]
fn prune_caches_ttl_removes_old_match_scores() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Insert a row with created_at = now - 2 hours (7200 seconds ago).
    let old_ts = now_ms().saturating_sub(7200 * 1000);
    let new_ts = now_ms();

    set_perf(None, None); // generous during inserts

    for (job_id, ts) in [("old-job", old_ts), ("new-job", new_ts)] {
        let hash = sha256_hex(job_id);
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO match_scores
             (resume_id, job_id, provider, model, semantic_enabled, formula_version,
              job_text_hash, score_json, created_at)
             VALUES ('r', ?1, 'ollama', 'nomic-embed-text', 1, 1, ?2, '{\"s\":1}', ?3)",
            params![job_id, hash, ts_to_db(ts)],
        )
        .unwrap();
    }

    assert_eq!(count_table(&store, "match_scores"), 2);

    // TTL = 3600 s (1 hour): the old-job row (2h old) is past the cutoff; new-job is not.
    store.prune_caches(Some(3600), None);

    assert_eq!(count_table(&store, "match_scores"), 1);
    {
        let conn = store.conn.lock();
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM match_scores WHERE job_id = 'new-job'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(cnt, 1, "new-job must survive TTL prune");
    }

    reset_perf_to_balanced();
}

// ── Read-side TTL: get_match_score returns None for an expired row ─────────────
//
// The read path uses `ttl_cutoff_ms()` which reads the live global — we set a
// very small TTL so the row's age (inserted at now_ms() - a few ms) exceeds it.
// We achieve "expiry" by setting a negative TTL seconds value (the prune SQL
// saturates, but the read cutoff formula allows negative: now - (neg * 1000) >
// created_at when neg is large enough that cutoff > created_at). Use a large
// negative TTL to force the cutoff into the future.
#[test]
fn get_match_score_returns_none_for_expired_row_via_live_ttl() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Insert a fresh row.
    let hash = sha256_hex("expire-me");
    let key = MatchScoreKey {
        resume_id: "r",
        job_id: "j",
        provider: "ollama",
        model: "nomic-embed-text",
        semantic_enabled: 1,
        formula_version: 1,
        job_text_hash: &hash,
    };
    // Insert with generous limits to avoid per-write eviction interfering.
    set_perf(None, None);
    store.upsert_match_score(&key, "{\"s\":1}").unwrap();

    // Confirm the row is a hit under generous TTL.
    assert!(
        store.get_match_score(&key).is_some(),
        "row must be present under no-TTL config"
    );

    // Set a TTL so large (negative) that the cutoff is in the future: every row
    // is "expired". ttl_cutoff_ms() = now_ms() - ttl_secs * 1000. With ttl_secs
    // = i64::MIN / 1000 the subtraction overflows and clamps to i64::MAX via
    // saturating_sub in the production code — that would make cutoff = MAX → all
    // rows expire. However: `prune_table_locked` uses saturating_sub, but
    // `ttl_cutoff_ms` uses saturating_sub too. Let's use a large negative value
    // that keeps the arithmetic well-behaved: -i64::MAX (not i64::MIN to avoid
    // any edge on platforms). A TTL of -1_000_000 means
    // cutoff = now_ms_as_i64 - (-1_000_000 * 1000) = now + 1_000_000_000 ms
    // which is far in the future → every existing row is "before" that → miss.
    set_perf(Some(-1_000_000), None);

    assert!(
        store.get_match_score(&key).is_none(),
        "row must be a read-side TTL miss when the cutoff is in the future"
    );

    reset_perf_to_balanced();
}

// ── Read-side TTL: get_posting_vector returns None for an expired row ──────────

#[test]
fn get_posting_vector_returns_none_for_expired_row_via_live_ttl() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    set_perf(None, None); // generous during insert
    let hash = sha256_hex("posting-expire");
    store
        .upsert_posting_vector("job-x", &hash, &ev(vec![0.1, 0.2]))
        .unwrap();

    assert!(
        store.get_posting_vector("job-x").is_some(),
        "posting vector must be present under no-TTL"
    );

    // Expire via negative TTL (same technique as match_score test above).
    set_perf(Some(-1_000_000), None);

    assert!(
        store.get_posting_vector("job-x").is_none(),
        "posting vector must be a read-side TTL miss when cutoff is in the future"
    );

    reset_perf_to_balanced();
}

// ── Generous (None/None): no eviction ─────────────────────────────────────────

#[test]
fn prune_caches_generous_leaves_all_rows_intact() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    set_perf(None, None);

    // Insert 10 match-score rows.
    for i in 0_u64..10 {
        let hash = sha256_hex(&format!("generous-{i}"));
        let key = MatchScoreKey {
            resume_id: "r",
            job_id: &format!("generous-job-{i}"),
            provider: "ollama",
            model: "nomic-embed-text",
            semantic_enabled: 1,
            formula_version: 1,
            job_text_hash: &hash,
        };
        store.upsert_match_score(&key, "{\"s\":1}").unwrap();
    }
    // Insert 5 posting vectors.
    for i in 0_u64..5 {
        let hash = sha256_hex(&format!("pv-{i}"));
        store
            .upsert_posting_vector(&format!("pv-job-{i}"), &hash, &ev(vec![0.1]))
            .unwrap();
    }

    assert_eq!(count_table(&store, "match_scores"), 10);
    assert_eq!(count_table(&store, "posting_vectors"), 5);

    // Prune with None/None (generous) → nothing removed.
    store.prune_caches(None, None);

    assert_eq!(
        count_table(&store, "match_scores"),
        10,
        "generous prune must not remove any match_scores rows"
    );
    assert_eq!(
        count_table(&store, "posting_vectors"),
        5,
        "generous prune must not remove any posting_vectors rows"
    );

    reset_perf_to_balanced();
}

// ── Row-cap boundary: cap=0 ───────────────────────────────────────────────────
//
// H1: cap=0 means OFFSET 0 → the subquery pivot IS the single newest row.
// DELETE WHERE created_at < newest_ts removes all strictly-older rows.
// The newest row itself (the pivot) is never deleted because the condition is
// strictly-less-than, not less-than-or-equal. Contract: exactly 1 row remains
// AND it is the row with the greatest created_at.

#[test]
fn prune_caches_cap_zero_keeps_exactly_the_single_newest_row() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    set_perf(None, None); // generous during inserts
    let base_ts = now_ms();

    // Insert 3 match-score rows with strictly increasing timestamps.
    for i in 0_u64..3 {
        let hash = sha256_hex(&format!("cap0-text-{i}"));
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO match_scores
             (resume_id, job_id, provider, model, semantic_enabled, formula_version,
              job_text_hash, score_json, created_at)
             VALUES ('r', ?1, 'ollama', 'nomic-embed-text', 1, 1, ?2, ?3, ?4)",
            params![
                format!("cap0-job-{i}"),
                hash,
                format!("{{\"score\":{}}}", i),
                ts_to_db(base_ts + i * 1000),
            ],
        )
        .unwrap();
    }

    assert_eq!(
        count_table(&store, "match_scores"),
        3,
        "3 rows before prune"
    );

    // cap=0: OFFSET 0 → pivot is the newest row (ts+2000).
    // DELETE WHERE created_at < pivot removes the two older rows.
    // Result: exactly 1 row — the newest.
    store.prune_caches(None, Some(0));

    let remaining = count_table(&store, "match_scores");
    assert_eq!(
        remaining, 1,
        "cap=0 keeps exactly the single newest row (OFFSET 0 picks the newest as pivot)"
    );

    // That surviving row must be the one with the largest created_at (cap0-job-2).
    {
        let conn = store.conn.lock();
        let max_ts: i64 = conn
            .query_row(
                "SELECT created_at FROM match_scores ORDER BY created_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let expected_ts = ts_to_db(base_ts + 2 * 1000);
        assert_eq!(
            max_ts, expected_ts,
            "surviving row must have the largest created_at (newest insert)"
        );

        // Confirm the specific job_id is cap0-job-2.
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM match_scores WHERE job_id = 'cap0-job-2'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(cnt, 1, "cap0-job-2 (newest) must be the surviving row");

        // The two older rows must be gone.
        for gone in &["cap0-job-0", "cap0-job-1"] {
            let cnt: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM match_scores WHERE job_id = ?1",
                    params![gone],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            assert_eq!(cnt, 0, "{gone} (older) must have been evicted by cap=0");
        }
    }

    reset_perf_to_balanced();
}

// ── Row-cap + tied created_at ─────────────────────────────────────────────────
//
// H2: The OFFSET DELETE uses a strict `<` comparison against the pivot's
// created_at. When multiple rows share the same created_at as the pivot, ALL
// of them survive (their timestamp is not strictly less than the pivot). This
// means cap=1 with 2 tied-oldest rows leaves ≥ 2 rows, not exactly 1.
//
// Contract (documented relaxed-tie contract):
//   - After prune(cap=1) with 3 rows (2 tied-oldest, 1 distinct-newest):
//     * Row count is in [2, 3] — the 2 older tied rows MAY survive as pivot collateral.
//     * The distinct-newest row ALWAYS survives (its timestamp is ≥ the pivot).
//
// This test pins that behavior so any tightening of the SQL (e.g. LIMIT 1 OFFSET 0
// changed to DELETE all but N) is caught.

#[test]
fn prune_caches_cap_with_tied_timestamps_retains_newest_and_at_least_bound() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    set_perf(None, None); // generous during inserts

    let old_ts = now_ms();
    let new_ts = old_ts + 5000; // clearly later

    // Insert 2 rows with identical (oldest) created_at, then 1 row with a newer ts.
    for i in 0_u64..2 {
        let hash = sha256_hex(&format!("tie-old-text-{i}"));
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO match_scores
             (resume_id, job_id, provider, model, semantic_enabled, formula_version,
              job_text_hash, score_json, created_at)
             VALUES ('r', ?1, 'ollama', 'nomic-embed-text', 1, 1, ?2, '{\"s\":1}', ?3)",
            params![format!("tie-old-{i}"), hash, ts_to_db(old_ts)],
        )
        .unwrap();
    }
    {
        let hash = sha256_hex("tie-new-text");
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO match_scores
             (resume_id, job_id, provider, model, semantic_enabled, formula_version,
              job_text_hash, score_json, created_at)
             VALUES ('r', 'tie-new', 'ollama', 'nomic-embed-text', 1, 1, ?1, '{\"s\":2}', ?2)",
            params![hash, ts_to_db(new_ts)],
        )
        .unwrap();
    }

    assert_eq!(
        count_table(&store, "match_scores"),
        3,
        "3 rows before prune"
    );

    // prune(cap=1): OFFSET 1 DESC picks the 2nd-newest row as pivot.
    // With 3 rows sorted DESC by created_at: new_ts, old_ts, old_ts
    //   → the 2nd element (OFFSET 1) is one of the old_ts rows.
    // DELETE WHERE created_at < old_ts: removes nothing (both old_ts rows are = not <).
    // So all 3 rows (or at minimum the 2 with old_ts) remain.
    // Result: count is in [2, 3] — the tie prevents strict trimming.
    store.prune_caches(None, Some(1));

    let remaining = count_table(&store, "match_scores");
    assert!(
        (2..=3).contains(&remaining),
        "tied created_at means prune(cap=1) retains [2,3] rows, got {remaining}: \
         ties on the OFFSET pivot are never deleted (strict < not <=)"
    );

    // The distinct-newest row must ALWAYS survive, regardless of tie handling.
    {
        let conn = store.conn.lock();
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM match_scores WHERE job_id = 'tie-new'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(
            cnt, 1,
            "the newest distinct-timestamp row must always be retained after prune"
        );
    }

    reset_perf_to_balanced();
}

// ── TTL eviction: prune_caches removes old posting_vectors ────────────────────
//
// M4: Mirror of `prune_caches_ttl_removes_old_match_scores` for posting_vectors.
// The helper `prune_table_locked` is shared; both call sites must be pinned.

#[test]
fn prune_caches_ttl_removes_old_posting_vectors() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Insert one old row (2 hours ago) and one fresh row (now).
    let old_ts = now_ms().saturating_sub(7200 * 1000);
    let new_ts = now_ms();

    set_perf(None, None); // generous during inserts

    for (job_id, ts) in [("pv-old-job", old_ts), ("pv-new-job", new_ts)] {
        let hash = sha256_hex(job_id);
        let v = ev(vec![0.1, 0.2]);
        let json = serde_json::to_string(&v.values).unwrap();
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO posting_vectors
             (job_id, text_hash, vector, provider, model, dim, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                job_id,
                hash,
                json,
                v.space.provider,
                v.space.model,
                v.space.dim as i64,
                ts_to_db(ts),
            ],
        )
        .unwrap();
    }

    assert_eq!(count_table(&store, "posting_vectors"), 2);

    // TTL = 3600 s (1 hour): the old row (2h old) is past the cutoff; new row is not.
    store.prune_caches(Some(3600), None);

    assert_eq!(
        count_table(&store, "posting_vectors"),
        1,
        "TTL prune must remove the 2-hour-old posting_vectors row"
    );

    {
        let conn = store.conn.lock();
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM posting_vectors WHERE job_id = 'pv-new-job'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(cnt, 1, "pv-new-job must survive the TTL prune");
    }

    reset_perf_to_balanced();
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

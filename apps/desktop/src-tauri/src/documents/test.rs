use super::*;
use serial_test::serial;
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
            version: EMBEDDING_VECTOR_VERSION,
        },
    }
}

/// A restored-backup vector is an OLD-FORMAT value (pre-chunk-pool, truncated
/// prefix), so `import()` tags it `version: 0` to force a re-embed. The write
/// path used to bind `EMBEDDING_VECTOR_VERSION` literally, silently advancing it
/// to the current version — the row then read as fresh and was never re-embedded,
/// which is exactly the cross-format mixing the version field exists to prevent.
#[test]
fn upsert_vector_persists_an_older_space_version_instead_of_force_advancing_it() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // One real, UNINDEXED document — the "genuinely stale index" baseline.
    store
        .insert(&DocumentRecord {
            id: "doc-real".into(),
            title: "CV".into(),
            name: "CV".into(),
            locale: None,
            text: "rust engineer".into(),
            pages: None,
            created_at: 0,
            indexed: false,
            is_default: false,
            keywords_json: None,
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
fn count_vectors_in_space_excludes_old_format_rows_sharing_the_same_provider_and_model() {
    // Same {provider, model} as `ev(..)`, but tagged with the OLD (pre-bump)
    // vector format — `EmbeddingConfig::matches` rejects these, so the
    // status strip's `indexedInActiveSpace` figure (and therefore its
    // derived `stale` count) must too, or a stale index would report "N/N
    // indexed" with `stale: 0` and the settings warning would never fire.
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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

/// `import` used to call `clear_all()` — which wipes documents, vectors,
/// posting_vectors and match_scores — BEFORE deserializing the rows, so a
/// malformed row partway through destroyed the user's entire existing library
/// and still returned Err, leaving nothing to restore from.
#[test]
fn import_of_a_malformed_bundle_leaves_the_existing_library_intact() {
    use crate::data_store::DataStore;

    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let existing = DocumentRecord {
        id: "doc-keep".to_string(),
        title: "Keep".to_string(),
        name: "keep.pdf".to_string(),
        locale: None,
        text: "precious".to_string(),
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: true,
        keywords_json: None,
    };
    store.insert(&existing).unwrap();
    store
        .upsert_vector("doc-keep", &ev(vec![0.4, 0.5, 0.6]))
        .unwrap();

    // Row 0 is well-formed; row 1 is not (`created_at` is a string, and `title`
    // is missing) — the failure must be detected before anything is deleted.
    let bundle = serde_json::json!([
        {
            "id": "doc-new",
            "title": "New",
            "name": "new.pdf",
            "text": "fresh",
            "createdAt": now_ms(),
            "indexed": false,
            "isDefault": false,
        },
        { "id": "doc-bad", "createdAt": "not-a-number" },
    ]);

    assert!(
        store.import(&bundle).is_err(),
        "a malformed row must fail the import"
    );

    let docs = store.list();
    assert_eq!(
        docs.len(),
        1,
        "the prior library must survive a failed import"
    );
    assert_eq!(docs[0].id, "doc-keep");
    assert_eq!(
        store.get_vector("doc-keep").map(|e| e.values),
        Some(vec![0.4, 0.5, 0.6]),
        "embeddings must survive a failed import"
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
#[serial]
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
#[serial]
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
#[serial]
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
#[serial]
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
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

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

// ── alias_retired_gemini_text_embedding_004 (HIGH-1 fix) ───────────────────────
//
// Any install that persisted `text-embedding-004` before the default changed
// to `gemini-embedding-2` must self-heal on the next `open()`, not keep
// 404-ing forever.

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_rewrites_a_persisted_stale_row() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Simulate an install that saved the now-retired model BEFORE this fix
    // shipped (bypassing `set_embedding_config` — this is exactly the shape
    // the retired code path used to write).
    {
        let conn = store.conn.lock();
        conn.execute(
            "UPDATE embedding_config SET provider = 'gemini', model = 'text-embedding-004'",
            [],
        )
        .unwrap();
    }

    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();

    let cfg = store.embedding_config();
    assert_eq!(cfg.provider, "gemini");
    assert_eq!(cfg.model, "gemini-embedding-2");
}

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_leaves_other_configs_untouched() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // A non-Gemini provider, and a Gemini row already on the current model,
    // must both survive unchanged — the migration is WHERE-scoped to the
    // exact retired (provider, model) pair only.
    {
        let conn = store.conn.lock();
        conn.execute(
            "UPDATE embedding_config SET provider = 'openai', model = 'text-embedding-3-small'",
            [],
        )
        .unwrap();
    }
    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
    let cfg = store.embedding_config();
    assert_eq!(cfg.provider, "openai");
    assert_eq!(cfg.model, "text-embedding-3-small");

    {
        let conn = store.conn.lock();
        conn.execute(
            "UPDATE embedding_config SET provider = 'gemini', model = 'gemini-embedding-2'",
            [],
        )
        .unwrap();
    }
    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
    let cfg = store.embedding_config();
    assert_eq!(cfg.provider, "gemini");
    assert_eq!(cfg.model, "gemini-embedding-2");
}

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_catches_real_stored_variants() {
    // The model column is free text — the Gemini adapter itself strips a
    // leading `models/`, so that form is a real variant users' saved
    // strings can carry, not a hypothetical one. Case and surrounding
    // whitespace are also user-input noise, not signal.
    for stale_model in [
        "models/text-embedding-004",
        "TEXT-EMBEDDING-004",
        " text-embedding-004 ",
        "Models/Text-Embedding-004",
    ] {
        let temp_dir = TempDir::new().unwrap();
        let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
        {
            let conn = store.conn.lock();
            conn.execute(
                "UPDATE embedding_config SET provider = 'gemini', model = ?1",
                params![stale_model],
            )
            .unwrap();
        }
        alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
        let cfg = store.embedding_config();
        assert_eq!(
            cfg.model, "gemini-embedding-2",
            "stored variant {stale_model:?} was not healed"
        );
    }
}

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_evicts_posting_and_match_caches_only_when_it_changes_something(
) {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // Seed a posting-vector row AND a match-score row — the test name claims
    // BOTH caches are evicted, so both must actually be seeded and asserted;
    // asserting only `posting_vectors` would stay green even if the
    // `DELETE FROM match_scores` half of `alias_retired_gemini_text_embedding_004`
    // were ever removed, silently leaving stale scores (computed in the
    // retired embedding space) to keep being served.
    store
        .upsert_posting_vector("job-1", &sha256_hex("job text"), &ev(vec![0.1, 0.2]))
        .unwrap();
    {
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO match_scores
             (resume_id, job_id, provider, model, semantic_enabled, formula_version,
              vector_version, job_text_hash, score_json, created_at)
             VALUES ('r', 'job-1', 'gemini', 'text-embedding-004', 1, 1, 1, ?1, '{\"score\":1}', ?2)",
            params![sha256_hex("job text"), ts_to_db(now_ms())],
        )
        .unwrap();
    }
    assert_eq!(count_table(&store, "posting_vectors"), 1);
    assert_eq!(count_table(&store, "match_scores"), 1);

    // A stale-model row that DOESN'T match the retired model must not evict.
    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
    assert_eq!(count_table(&store, "posting_vectors"), 1);
    assert_eq!(count_table(&store, "match_scores"), 1);

    // Now seed the actual stale model and re-run — this IS a real space
    // change, so it must evict, mirroring `ai_set_embedding_config`'s
    // runtime eviction for the same kind of change.
    {
        let conn = store.conn.lock();
        conn.execute(
            "UPDATE embedding_config SET provider = 'gemini', model = 'text-embedding-004'",
            [],
        )
        .unwrap();
    }
    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
    assert_eq!(count_table(&store, "posting_vectors"), 0);
    assert_eq!(count_table(&store, "match_scores"), 0);
}

// ── Migration WIRING (not just the bare function) ───────────────────────────
//
// The two test groups above call `alias_retired_gemini_text_embedding_004`
// directly — they'd stay green even if its `Migration { .. }` entry were
// deleted from `DocumentStore::MIGRATIONS`. These exercise the REAL
// end-to-end path instead.

#[test]
#[serial]
fn every_registered_migration_actually_applies_on_open() {
    // Sanity check on the migration SYSTEM itself: `user_version` must land
    // exactly at the registered migration count after a fresh `open()` —
    // catches a migration silently failing to run.
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let version: i64 = store
        .conn
        .lock()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, DocumentStore::MIGRATIONS.len() as i64);
}

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_heals_a_pre_seeded_row_through_a_real_open() {
    // Bring a fresh DB up to JUST BEFORE the alias-fix migration (simulating
    // an install created on an older app version), seed the stale row, then
    // open it for REAL — proving the `Migration { .. }` entry is actually
    // registered and reached, not just that the function works standalone.
    // Looked up BY NAME rather than "all but the last" so this stays correct
    // regardless of where later migrations get appended in the array.
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let alias_idx = all
        .iter()
        .position(|m| m.name == "alias_retired_gemini_text_embedding_004")
        .expect("alias_retired_gemini_text_embedding_004 must still be registered");
    {
        let mut conn = crate::db::open(&db_path).unwrap();
        run_migrations(&mut conn, &all[..alias_idx]).unwrap();
        conn.execute(
            "UPDATE embedding_config SET provider = 'gemini', model = 'text-embedding-004'",
            [],
        )
        .unwrap();
    }

    // A REAL open() — must run the remaining migrations, including the
    // alias-fix one.
    let store = DocumentStore::open(&dir).unwrap();
    let cfg = store.embedding_config();
    assert_eq!(cfg.provider, "gemini");
    assert_eq!(cfg.model, "gemini-embedding-2");
}

#[test]
#[serial]
fn evict_posting_vectors_for_embedding_format_v2_heals_a_pre_seeded_row_through_a_real_open() {
    // The real end-to-end path: seed a posting vector for a NON-Gemini
    // provider under an OLD DB (every migration except this one applied),
    // then open it for real — proving the migration is actually registered
    // in `MIGRATIONS` (unlike the Gemini-specific alias migration, this one
    // has no WHERE clause and must wipe the cache for EVERY provider, since
    // the chunk-and-mean-pool format change affects all of them).
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let evict_idx = all
        .iter()
        .position(|m| m.name == "evict_posting_vectors_for_embedding_format_v2")
        .expect("evict_posting_vectors_for_embedding_format_v2 must still be registered");
    {
        let mut conn = crate::db::open(&db_path).unwrap();
        run_migrations(&mut conn, &all[..evict_idx]).unwrap();
        // `posting_vectors` exists by this point (created several migrations
        // earlier) — seed a row directly via raw SQL (no `DocumentStore` yet).
        // Ollama, not Gemini — proves the WHERE-less DELETE isn't scoped.
        conn.execute(
            "INSERT INTO posting_vectors (job_id, text_hash, vector, provider, model, dim, created_at) \
             VALUES ('job-1', 'hash', '[0.1,0.2]', 'ollama', 'nomic-embed-text', 2, 0)",
            [],
        )
        .unwrap();
    }

    let store = DocumentStore::open(&dir).unwrap();
    assert_eq!(count_table(&store, "posting_vectors"), 0);
}

#[test]
#[serial]
fn add_version_to_posting_vectors_heals_a_pre_seeded_row_through_a_real_open() {
    // Bring a fresh DB up to JUST BEFORE the version-column migration
    // (simulating an install created before this fix shipped), seed a row
    // through the OLD (no `version` column) schema, then open it for REAL —
    // proving the `Migration { .. }` entry is actually registered and reached.
    // The slice below already includes `evict_posting_vectors_for_embedding_
    // format_v2` (it runs earlier in the array), so the row inserted below is
    // exactly the shape a real pre-existing row would be: written AFTER the
    // v1->v2 wipe, hence genuinely v2-native — `DEFAULT 2` must recognize
    // that instead of mislabeling it as stale and forcing a wasted re-embed.
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let version_idx = all
        .iter()
        .position(|m| m.name == "add_version_to_posting_vectors")
        .expect("add_version_to_posting_vectors must still be registered");
    {
        let mut conn = crate::db::open(&db_path).unwrap();
        run_migrations(&mut conn, &all[..version_idx]).unwrap();
        // Old (pre-migration) schema has no `version` column yet. `created_at`
        // is "now" (not epoch 0) so the row survives the read-side TTL filter
        // in `get_posting_vector` below — this test is about the version
        // column, not TTL expiry.
        conn.execute(
            "INSERT INTO posting_vectors (job_id, text_hash, vector, provider, model, dim, created_at) \
             VALUES ('job-1', 'hash', '[0.1,0.2]', 'ollama', 'nomic-embed-text', 2, ?1)",
            params![ts_to_db(now_ms())],
        )
        .unwrap();
    }

    // A REAL open() — must run the remaining migrations, including the
    // version-column one, without erroring on the pre-existing row.
    let store = DocumentStore::open(&dir).unwrap();
    let (v, hash) = store
        .get_posting_vector("job-1")
        .expect("a pre-migration row must still round-trip after the column is added");
    assert_eq!(hash, "hash");
    assert_eq!(
        v.space.version, 2,
        "a pre-existing row must default to version 2 (DEFAULT 2) — it necessarily \
         post-dates the earlier v1->v2 wipe migration, so it is provably v2-native"
    );
    assert!(
        cfg_ollama().matches(&v.space),
        "a provably-current pre-existing row must HIT the cache, not be forced through \
         a wasted (billed) re-embed"
    );
}

#[test]
#[serial]
fn add_version_to_posting_vectors_migration_is_idempotent_on_reopen() {
    // `db::run_migrations` skips any migration whose index is <= the stored
    // `PRAGMA user_version` (see `db.rs`), so a PLAIN reopen never actually
    // re-invokes this migration's `up` — its `column_exists` guard would go
    // completely unexercised a second time. Roll `user_version` back to JUST
    // before this migration (looked up BY NAME, not "the last one", so this
    // stays correct regardless of what gets appended after it) so a reopen
    // makes THIS migration the one pending, genuinely re-executing the guard
    // against a schema where the `version` column already exists — proving
    // the repeated `ALTER TABLE` really is a no-op, not just that a reopen
    // is safe.
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    let store = DocumentStore::open(&dir).unwrap();
    let hash = sha256_hex("job text");
    store
        .upsert_posting_vector("job-1", &hash, &ev(vec![0.1, 0.2]))
        .unwrap();
    drop(store);

    let migration_idx = DocumentStore::MIGRATIONS
        .iter()
        .position(|m| m.name == "add_version_to_posting_vectors")
        .expect("add_version_to_posting_vectors must still be registered");
    {
        let conn = crate::db::open(&dir.join("documents.db")).unwrap();
        conn.execute_batch(&format!("PRAGMA user_version = {migration_idx}"))
            .unwrap();
    }

    let reopened = DocumentStore::open(&dir).unwrap();
    let (v, got_hash) = reopened
        .get_posting_vector("job-1")
        .expect("row must survive a reopen that re-runs this migration's guard");
    assert_eq!(got_hash, hash);
    assert_eq!(v.space.version, EMBEDDING_VECTOR_VERSION);

    // The re-run must be a true no-op on `user_version` too — it should land
    // back at exactly the full migration count, not double-advance or stall.
    let final_version: i64 = reopened
        .conn
        .lock()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(final_version, DocumentStore::MIGRATIONS.len() as i64);
}

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_heals_a_pre_seeded_row_through_a_real_open() {
    // Bring a fresh DB up to JUST BEFORE the repair migration (simulating an
    // install that imported a PDF before PR #955 fixed `pdf_text_string`),
    // seed a corrupt row with the EXACT byte shape hex-dumped from the live
    // `documents.db` row plus an unrelated clean row, then open it for REAL —
    // proving the `Migration { .. }` entry is actually registered and
    // reached, and that it never touches a row it shouldn't.
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");

    let corrupt_text = corrupt_mojibake_text();
    let expected_repaired = REPAIRED_MOJIBAKE_TEXT;
    let clean_text = "Software Engineer with 5 years experience".to_string();

    {
        let mut conn = crate::db::open(&db_path).unwrap();
        run_migrations(&mut conn, &all[..repair_idx]).unwrap();
        // Both rows carry a pre-existing `keywords_json` cache and are
        // `indexed = 1` — a real document that was already scored/embedded
        // BEFORE this repair runs. The corrupt row's cache must be
        // invalidated (it was computed from the corrupt text); the clean
        // row's must survive untouched.
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default, keywords_json)
             VALUES ('corrupt', 't', 'n', 'en', ?1, 1, 0, 1, 0, '[\"stale\"]')",
            params![corrupt_text],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default, keywords_json)
             VALUES ('clean', 't', 'n', 'en', ?1, 1, 0, 1, 0, '[\"software\",\"engineer\"]')",
            params![clean_text],
        )
        .unwrap();
    }

    // A REAL open() — must run the remaining migrations, including the repair.
    let store = DocumentStore::open(&dir).unwrap();
    let corrupt_after = store.get("corrupt").expect("corrupt row must still exist");
    let clean_after = store.get("clean").expect("clean row must still exist");

    assert_eq!(corrupt_after.text, expected_repaired);
    assert!(
        !corrupt_after.text.contains('\0') && !corrupt_after.text.contains('\u{FFFD}'),
        "repaired text must carry no NULs or replacement chars; got {:?}",
        corrupt_after.text
    );
    assert_eq!(
        corrupt_after.keywords_json, None,
        "the stale keywords cache (computed from corrupt text) must be invalidated"
    );
    assert!(
        !corrupt_after.indexed,
        "indexed must be cleared so the repaired text gets re-embedded, not left \
         pinned to a vector computed from the corrupt text"
    );
    assert_eq!(
        clean_after.text, clean_text,
        "a row with no embedded NUL must be byte-identical after the migration"
    );
    assert_eq!(
        clean_after.keywords_json,
        Some("[\"software\",\"engineer\"]".to_string()),
        "an unaffected row's cache must survive untouched"
    );
    assert!(
        clean_after.indexed,
        "an unaffected row's indexed flag must survive untouched"
    );
}

/// Build the exact hex-dumped corrupt byte shape used across the mojibake
/// repair tests: `- [` + doubled U+FFFD (the BOM misread as UTF-8) +
/// NUL-interleaved "aijobhunter.app" (UTF-16BE misread as UTF-8) + the
/// never-corrupted `](url)\n` suffix — see
/// `extraction::pdf::repair_utf16_mojibake`.
fn corrupt_mojibake_text() -> String {
    let mut bytes = b"- [".to_vec();
    bytes.extend_from_slice(&[0xEF, 0xBF, 0xBD, 0xEF, 0xBF, 0xBD]);
    for &b in b"aijobhunter.app" {
        bytes.push(0x00);
        bytes.push(b);
    }
    bytes.extend_from_slice(b"](https://aijobhunter.app/)\n");
    String::from_utf8(bytes).unwrap()
}

const REPAIRED_MOJIBAKE_TEXT: &str = "- [aijobhunter.app](https://aijobhunter.app/)\n";

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_snapshots_the_pre_repair_value_before_rewriting() {
    // The repair is an irreversible in-place rewrite of what may be the only
    // remaining copy of the user's résumé — it must back up the exact
    // pre-repair value of every row it is about to touch, in the same
    // transaction, before overwriting it.
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");

    let corrupt_text = corrupt_mojibake_text();

    {
        let mut conn = crate::db::open(&db_path).unwrap();
        run_migrations(&mut conn, &all[..repair_idx]).unwrap();
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
             VALUES ('corrupt', 't', 'n', 'en', ?1, 1, 0, 0, 0)",
            params![corrupt_text],
        )
        .unwrap();
    }

    let store = DocumentStore::open(&dir).unwrap();
    let backed_up_text: String = store
        .conn
        .lock()
        .query_row(
            "SELECT text FROM documents_pre_mojibake_repair WHERE id = 'corrupt'",
            [],
            |r| r.get(0),
        )
        .expect("the backup table must contain the pre-repair row");
    assert_eq!(
        backed_up_text, corrupt_text,
        "the backup must hold the EXACT pre-repair (still-corrupt) value"
    );

    // The live row, meanwhile, must actually be repaired — the backup is a
    // safety net, not a substitute for doing the repair.
    let live_text = store.get("corrupt").unwrap().text;
    assert_ne!(live_text, corrupt_text);
}

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_skips_an_unmappable_row_without_failing_the_migration() {
    // A `text` value that ends up with BLOB storage class (SQLite is
    // dynamically typed; this can happen independently of anything this
    // migration does) fails `row.get::<_, String>`. That must be logged and
    // skipped — not silently dropped, and not allowed to fail the WHOLE
    // migration and take a normal, mappable corrupt row down with it.
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");

    let corrupt_text = corrupt_mojibake_text();

    {
        let mut conn = crate::db::open(&db_path).unwrap();
        run_migrations(&mut conn, &all[..repair_idx]).unwrap();
        // A NUL-bearing BLOB literal — matches the migration's `instr(...)`
        // WHERE clause, but `row.get::<_, String>` cannot map it.
        conn.execute_batch(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
             VALUES ('unmappable', 't', 'n', 'en', X'0000', 1, 0, 0, 0);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
             VALUES ('corrupt', 't', 'n', 'en', ?1, 1, 0, 0, 0)",
            params![corrupt_text],
        )
        .unwrap();
    }

    // Must NOT fail overall, and must still repair the mappable row.
    let store = DocumentStore::open(&dir).unwrap();
    let repaired = store.get("corrupt").unwrap().text;
    assert_eq!(repaired, REPAIRED_MOJIBAKE_TEXT);

    // The unmappable row itself: `store.get()` can't map it either (its
    // `text` column is still BLOB, so `row.get::<_, String>` still fails),
    // so query the raw row count directly. If the migration ever started
    // DELETEing rows it can't map instead of just skipping them, this is
    // the only assertion in the suite that would catch it.
    let unmappable_row_count: i64 = store
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM documents WHERE id = 'unmappable'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        unmappable_row_count, 1,
        "a row the migration can't map must be logged and skipped, not deleted"
    );

    let version: i64 = store
        .conn
        .lock()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        version,
        DocumentStore::MIGRATIONS.len() as i64,
        "the unmappable row must not block the migration from completing"
    );
}

#[test]
#[serial]
fn insert_repairs_pre_pdf_text_string_mojibake_on_write() {
    // Defensive repair on the write path (not just the one-time migration):
    // restoring an old backup bundle re-inserts a corrupt row into an
    // ALREADY fully-migrated store via `insert()` — it must come out clean.
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    store
        .insert(&DocumentRecord {
            id: make_doc_id(),
            title: "Resume".to_string(),
            name: "resume.pdf".to_string(),
            locale: Some("en".to_string()),
            text: corrupt_mojibake_text(),
            pages: Some(1),
            created_at: now_ms(),
            indexed: false,
            is_default: false,
            keywords_json: None,
        })
        .unwrap();

    let stored = &store.list()[0];
    assert_eq!(stored.text, REPAIRED_MOJIBAKE_TEXT);
}

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_leaves_user_version_unadvanced_on_a_failed_row_write() {
    // Regression test for a real defect: on SQLite's "fatal" error classes
    // (SQLITE_FULL, SQLITE_IOERR, SQLITE_NOMEM, SQLITE_BUSY,
    // SQLITE_INTERRUPT — see https://www.sqlite.org/lang_transaction.html),
    // SQLite can silently roll back the WHOLE enclosing transaction and drop
    // the connection back to autocommit. If a per-row UPDATE error here were
    // logged-and-skipped instead of propagated, the very next statement (the
    // `PRAGMA user_version = N` bump in `db::run_migrations`) would then run
    // and commit on its OWN, durably advancing the version even though the
    // row was never repaired — and no future startup would ever retry it
    // (`run_migrations` skips any migration whose version is `<=
    // user_version`). Reproduced here with a REAL `max_page_count` clamp on
    // a real WAL database, not a mock: a large (multi-overflow-page) corrupt
    // row's repair is a SHRINKING update, but SQLite still needs to allocate
    // fresh overflow pages for the new payload before it can free the old
    // ones, so clamping growth to the current page count still forces
    // SQLITE_FULL on it.
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");

    // A large corrupt row — big enough to span several SQLite overflow
    // pages, so even the repair's SHRINKING update still needs fresh
    // overflow pages before the old ones are freed.
    let mut corrupt_bytes = b"- [".to_vec();
    corrupt_bytes.extend_from_slice(&[0xEF, 0xBF, 0xBD, 0xEF, 0xBF, 0xBD]);
    for _ in 0..20_000 {
        for &b in b"aijobhunter.app" {
            corrupt_bytes.push(0x00);
            corrupt_bytes.push(b);
        }
    }
    corrupt_bytes.extend_from_slice(b"](https://aijobhunter.app/)\n");
    let corrupt_text = String::from_utf8(corrupt_bytes).unwrap();

    let mut conn = crate::db::open(&db_path).unwrap();
    run_migrations(&mut conn, &all[..repair_idx]).unwrap();
    conn.execute(
        "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
         VALUES ('corrupt', 't', 'n', 'en', ?1, 1, 0, 0, 0)",
        params![corrupt_text],
    )
    .unwrap();
    // Pre-create the (empty) backup table so the migration's own `CREATE
    // TABLE IF NOT EXISTS documents_pre_mojibake_repair AS SELECT …` is a
    // total no-op (SQLite skips evaluating the SELECT entirely when the
    // table already exists) instead of itself needing fresh pages to copy
    // this large row — isolating the LOOP'S `UPDATE` as the one write this
    // test's clamp can fail, so this test actually exercises the per-row
    // error-propagation policy under test, not the (separately-propagated,
    // already-safe) backup step.
    conn.execute_batch(
        "CREATE TABLE documents_pre_mojibake_repair (id TEXT PRIMARY KEY, text TEXT);",
    )
    .unwrap();

    // Clamp growth to exactly the current page count: any statement that
    // needs even ONE more page than this fails with SQLITE_FULL. `open()`
    // sets WAL, and `max_page_count` is per-CONNECTION — it must be set on,
    // and the migration must be re-run on, THIS SAME connection (a fresh
    // `DocumentStore::open` would get a brand new connection with the
    // default, effectively unlimited, `max_page_count`).
    let page_count: i64 = conn
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap();
    conn.pragma_update(None, "max_page_count", page_count)
        .unwrap();

    let result = run_migrations(&mut conn, all);
    assert!(
        result.is_err(),
        "the clamp must actually induce a write failure for this test to be meaningful"
    );

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        version, repair_idx as i64,
        "user_version must NOT advance past a migration whose row write failed \
         — advancing it would skip the repair forever on every future startup"
    );

    let stored_text: String = conn
        .query_row("SELECT text FROM documents WHERE id = 'corrupt'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        stored_text, corrupt_text,
        "a failed UPDATE must not leave the row partially rewritten"
    );
}

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_evicts_the_active_space_vector_so_the_document_becomes_stale(
) {
    // Regression test for a real defect: the migration used to only flip
    // `indexed = 0`, which is a complete no-op for re-embedding —
    // `stale_documents` (`commands/ai.rs`) decides what to re-embed purely
    // from whether `get_vector` hits in the ACTIVE embedding space, and
    // never reads `indexed`. A pre-repair vector that still matches the
    // active space means the document is never picked up, so the embedding
    // stays permanently derived from the corrupt text. Asserted directly on
    // `vectors` (via `get_vector`), not on `indexed` — asserting only on
    // `indexed` is exactly what made this defect invisible the first time.
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");

    let corrupt_text = corrupt_mojibake_text();
    let clean_text = "Software Engineer with 5 years experience".to_string();

    {
        let mut conn = crate::db::open(&db_path).unwrap();
        run_migrations(&mut conn, &all[..repair_idx]).unwrap();
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
             VALUES ('corrupt', 't', 'n', 'en', ?1, 1, 0, 1, 0)",
            params![corrupt_text],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
             VALUES ('clean', 't', 'n', 'en', ?1, 1, 0, 1, 0)",
            params![clean_text],
        )
        .unwrap();
        // Both docs already have a vector in the CURRENT active embedding
        // space (the default: ollama/nomic-embed-text, seeded by the
        // `create_embedding_config` migration) — a real "already indexed
        // before this repair runs" document.
        conn.execute(
            "INSERT INTO vectors (doc_id, vector, provider, model, dim, version)
             VALUES ('corrupt', '[0.1,0.2]', 'ollama', 'nomic-embed-text', 2, ?1)",
            params![EMBEDDING_VECTOR_VERSION],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO vectors (doc_id, vector, provider, model, dim, version)
             VALUES ('clean', '[0.3,0.4]', 'ollama', 'nomic-embed-text', 2, ?1)",
            params![EMBEDDING_VECTOR_VERSION],
        )
        .unwrap();
    }

    // A REAL open() — must run the remaining migrations, including the repair.
    let store = DocumentStore::open(&dir).unwrap();

    assert!(
        store.get_vector("corrupt").is_none(),
        "the vector derived from the corrupt text must be evicted, or \
         `stale_documents` will never pick this document up for re-embedding"
    );
    assert!(
        store.get_vector("clean").is_some(),
        "an unaffected row's vector must survive — the repair must not evict \
         embeddings for documents whose text was never touched"
    );
}

#[test]
fn import_of_a_repaired_row_does_not_restore_a_vector_derived_from_the_corrupt_text() {
    // The write-path half of the same defect class: a backup bundle
    // exported before PR #955 carries BOTH the corrupt text AND the vector
    // computed from it. `import` -> `insert` repairs the text; restoring
    // the bundle's own vector right afterward would silently reintroduce
    // exactly the stale-vector problem the migration exists to fix.
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let bundle = serde_json::json!([
        {
            "_id": "corrupt",
            "title": "Resume",
            "name": "resume.pdf",
            "text": corrupt_mojibake_text(),
            "createdAt": 1,
            "indexed": true,
            "isDefault": true,
            "vector": [0.1, 0.2],
            "vectorSpace": { "provider": "ollama", "model": "nomic-embed-text", "dim": 2 }
        },
        {
            "_id": "clean",
            "title": "Resume 2",
            "name": "resume2.pdf",
            "text": "Software Engineer with 5 years experience",
            "createdAt": 2,
            "indexed": true,
            "isDefault": false,
            "vector": [0.3, 0.4],
            "vectorSpace": { "provider": "ollama", "model": "nomic-embed-text", "dim": 2 }
        }
    ]);
    let n = crate::data_store::DataStore::import(&store, &bundle).unwrap();
    assert_eq!(n, 2);

    assert_eq!(
        store.get("corrupt").unwrap().text,
        REPAIRED_MOJIBAKE_TEXT,
        "the text must still be repaired on import"
    );
    assert!(
        store.get_vector("corrupt").is_none(),
        "the bundle's vector (derived from the corrupt text) must not be restored \
         for a row whose text was actually repaired"
    );
    assert!(
        store.get_vector("clean").is_some(),
        "a row whose text was NOT changed must keep its restored vector — \
         the fix must not evict embeddings for clean documents"
    );
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
        vector_version: 1,
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
        vector_version: 1,
        job_text_hash,
    }
}

// Real end-to-end path: seed a `match_scores` row under the OLD (7-column, no
// `vector_version`) schema on a DB with every migration except this one
// applied, then open for real. Proves the migration is actually registered in
// `MIGRATIONS` and recreates the table with `vector_version` as a real PK
// column backing it at the SQL layer — not just a field that compiles into
// the Rust struct with nothing enforcing it underneath (SQLite can't `ALTER
// TABLE` a column into an existing `PRIMARY KEY`, hence the drop+recreate).
#[test]
#[serial]
fn add_vector_version_to_match_scores_key_heals_a_pre_seeded_row_through_a_real_open() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let vv_idx = all
        .iter()
        .position(|m| m.name == "add_vector_version_to_match_scores_key")
        .expect("add_vector_version_to_match_scores_key must still be registered");
    {
        let mut conn = crate::db::open(&db_path).unwrap();
        run_migrations(&mut conn, &all[..vv_idx]).unwrap();
        // Old schema: no `vector_version` column yet.
        conn.execute(
            "INSERT INTO match_scores
                (resume_id, job_id, provider, model, semantic_enabled, formula_version,
                 job_text_hash, score_json, created_at)
             VALUES ('r', 'j', 'ollama', 'nomic-embed-text', 1, 1, 'hash', '{}', 0)",
            [],
        )
        .unwrap();
    }

    let store = DocumentStore::open(&dir).unwrap();
    // The recreated table starts empty — a pure result cache, so losing a
    // pre-migration row is safe (it just forces one recompute).
    assert_eq!(count_table(&store, "match_scores"), 0);

    // The new column is real and part of the PK: writes/reads round-trip
    // through the ordinary store API on the recreated schema.
    let key = match_key("r", "j", 1, 1, "hash");
    store.upsert_match_score(&key, "{\"combined\":1}").unwrap();
    assert!(store.get_match_score(&key).is_some());
}

#[test]
#[serial]
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
#[serial]
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
#[serial]
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
#[serial]
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

// The mojibake-repair migration snapshots the pre-repair (still-corrupt)
// résumé text into `documents_pre_mojibake_repair` before rewriting it in
// place (see `mojibake_repair::up`). That snapshot is the user's ORIGINAL
// document text at rest — a full "erase my data" reset must drop the table,
// not just leave an empty shell behind.
#[test]
#[serial]
fn test_clear_all_drops_the_mojibake_repair_snapshot_table() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("documents.db");

    let all = DocumentStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");

    let corrupt_text = corrupt_mojibake_text();

    {
        let mut conn = crate::db::open(&db_path).unwrap();
        run_migrations(&mut conn, &all[..repair_idx]).unwrap();
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
             VALUES ('corrupt', 't', 'n', 'en', ?1, 1, 0, 0, 0)",
            params![corrupt_text],
        )
        .unwrap();
    }

    // A REAL open() — must run the remaining migrations, populating the
    // snapshot table.
    let store = DocumentStore::open(&dir).unwrap();
    let snapshot_rows: i64 = store
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM documents_pre_mojibake_repair",
            [],
            |r| r.get(0),
        )
        .expect("the migration must have created and populated the snapshot table");
    assert_eq!(snapshot_rows, 1, "snapshot must hold the pre-repair row");

    store.clear_all();

    let table_exists: i64 = store
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'documents_pre_mojibake_repair'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        table_exists, 0,
        "clear_all() must DROP the mojibake snapshot table (not just empty it) — \
         it holds the user's original, un-repaired document text"
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
#[serial]
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
              vector_version, job_text_hash, score_json, created_at)
             VALUES ('r', ?1, 'ollama', 'nomic-embed-text', 1, 1, 1, ?2, ?3, ?4)",
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
#[serial]
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
#[serial]
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
              vector_version, job_text_hash, score_json, created_at)
             VALUES ('r', ?1, 'ollama', 'nomic-embed-text', 1, 1, 1, ?2, '{\"s\":1}', ?3)",
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
#[serial]
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
        vector_version: 1,
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
#[serial]
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
#[serial]
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
            vector_version: 1,
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
#[serial]
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
              vector_version, job_text_hash, score_json, created_at)
             VALUES ('r', ?1, 'ollama', 'nomic-embed-text', 1, 1, 1, ?2, ?3, ?4)",
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
#[serial]
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
              vector_version, job_text_hash, score_json, created_at)
             VALUES ('r', ?1, 'ollama', 'nomic-embed-text', 1, 1, 1, ?2, '{\"s\":1}', ?3)",
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
              vector_version, job_text_hash, score_json, created_at)
             VALUES ('r', 'tie-new', 'ollama', 'nomic-embed-text', 1, 1, 1, ?1, '{\"s\":2}', ?2)",
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
#[serial]
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

// ── documents_get_text (command-layer contract) ───────────────────────────────
//
// The command wraps `DocumentStore::get(id).map(|d| d.text).unwrap_or_default()`.
// Tests exercise the store-level equivalent because the Tauri `AppHandle` cannot
// be instantiated in unit tests. The two invariants:
//   1. A stored document's text round-trips unchanged through get().
//   2. A missing id returns an empty string — never an error.
//   (The command wraps this in `Ok(...)` so it can never fail either.)

#[test]
fn documents_get_text_returns_stored_text() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let id = make_doc_id();
    let expected_text = "Experienced Rust developer with 7 years of experience.";

    let doc = DocumentRecord {
        id: id.clone(),
        title: "Resume".to_string(),
        name: "resume.pdf".to_string(),
        locale: Some("en".to_string()),
        text: expected_text.to_string(),
        pages: Some(1),
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    store.insert(&doc).unwrap();

    // Simulate the command body: get → map text → unwrap_or_default.
    let text = store.get(&id).map(|d| d.text).unwrap_or_default();
    assert_eq!(
        text, expected_text,
        "documents_get_text must return the stored text unchanged"
    );
}

#[test]
fn documents_get_text_returns_empty_string_for_missing_id() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    // No documents inserted — any id is missing.
    let text = store
        .get("nonexistent-doc-id")
        .map(|d| d.text)
        .unwrap_or_default();
    assert_eq!(
        text, "",
        "documents_get_text must return an empty string for a missing id, never an error"
    );
}

#[test]
fn documents_get_text_empty_string_when_stored_text_is_empty() {
    // The renderer treats "no text" and "no document" the same; this pins the
    // degenerate case where a document exists but text is empty (e.g. import edge).
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let id = make_doc_id();
    let doc = DocumentRecord {
        id: id.clone(),
        title: "Empty".to_string(),
        name: "empty.txt".to_string(),
        locale: None,
        text: String::new(), // empty text
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    store.insert(&doc).unwrap();

    let text = store.get(&id).map(|d| d.text).unwrap_or_default();
    assert_eq!(text, "");
}

#[test]
fn documents_get_text_returns_text_after_multiple_inserts() {
    // Verify get() returns the right document when multiple docs are in the store.
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let id_a = "doc-text-a".to_string();
    let id_b = "doc-text-b".to_string();

    for (id, text) in [(&id_a, "Resume A text"), (&id_b, "Resume B text")] {
        store
            .insert(&DocumentRecord {
                id: id.clone(),
                title: id.clone(),
                name: format!("{id}.pdf"),
                locale: None,
                text: text.to_string(),
                pages: None,
                created_at: now_ms(),
                indexed: false,
                is_default: false,
                keywords_json: None,
            })
            .unwrap();
    }

    let text_a = store.get(&id_a).map(|d| d.text).unwrap_or_default();
    let text_b = store.get(&id_b).map(|d| d.text).unwrap_or_default();

    assert_eq!(text_a, "Resume A text");
    assert_eq!(text_b, "Resume B text");
    // An unknown id still returns empty.
    let text_c = store.get("doc-text-c").map(|d| d.text).unwrap_or_default();
    assert_eq!(text_c, "");
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

/// A hand-edited bundle carrying a `<namespace>:` document id must not BRICK the
/// restore. `clear_all()` runs before the first insert, so propagating the
/// document-index write guard from here would leave the library half-restored
/// with nothing to retry from — the very failure mode `import`'s up-front
/// validation pass exists to prevent. The one embedding is skipped (it
/// re-embeds on demand); every document still lands.
///
/// Unreachable for a bundle this app produced (`export()` only walks real
/// `documents` rows), which is why it is a robustness guard rather than a fix.
#[test]
fn import_skips_a_synthetic_id_vector_instead_of_aborting_the_restore() {
    use crate::data_store::DataStore;

    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let bundle = serde_json::json!([
        {
            "_id": "autopilot-resume:deadbeef",
            "title": "Hand-edited",
            "name": "x.pdf",
            "text": "first",
            "createdAt": 1,
            "indexed": false,
            "isDefault": false,
            "vector": [0.1, 0.2, 0.3],
            "vectorSpace": { "provider": "ollama", "model": "nomic-embed-text", "dim": 3 },
        },
        {
            "_id": "doc-real",
            "title": "Real",
            "name": "r.pdf",
            "text": "second",
            "createdAt": 2,
            "indexed": false,
            "isDefault": true,
            "vector": [0.4, 0.5, 0.6],
            "vectorSpace": { "provider": "ollama", "model": "nomic-embed-text", "dim": 3 },
        },
    ]);

    let count = store.import(&bundle).expect("restore must not fail");

    assert_eq!(count, 2, "every document is restored");
    assert_eq!(store.list().len(), 2);
    assert!(
        store.get_vector("autopilot-resume:deadbeef").is_none(),
        "the document index still refuses the synthetic id — it is skipped, not written"
    );
    assert_eq!(
        store.get_vector("doc-real").map(|v| v.values),
        Some(vec![0.4, 0.5, 0.6]),
        "…and the rows AFTER it are still restored, embeddings included"
    );
}

// ── one posting-vector row can be dropped with its producer ──────────────────

/// The autopilot re-rank's résumé snapshot lives in this cache, so deleting the
/// autopilot needs a single-row delete (the cache is otherwise bounded only by
/// its TTL and row cap — see `commands::autopilot::drop_orphaned_resume_cache`).
#[test]
#[serial]
fn delete_posting_vector_removes_only_that_row() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
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

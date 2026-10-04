//! Document CRUD, the `documents_get_text` command contract, and the factory-reset wipe.

use super::{support::*, *};

/// Insert two documents, the second newer and titled `CV`; the first becomes the default.
fn insert_two_documents(store: &DocumentStore) -> (DocumentRecord, DocumentRecord) {
    let doc1 = record(&make_doc_id(), "Text 1");
    let doc2 = DocumentRecord {
        title: "CV".to_string(),
        name: "cv.pdf".to_string(),
        created_at: now_ms() + 1000,
        ..record(&make_doc_id(), "Text 2")
    };
    store.insert(&doc1).unwrap();
    store.insert(&doc2).unwrap();
    (doc1, doc2)
}

#[test]
fn test_open_store() {
    let (_dir, store) = open_store();
    let docs = store.list();
    assert!(docs.is_empty());
}

#[test]
fn test_insert_document() {
    let (_dir, store) = open_store();

    let doc = DocumentRecord {
        locale: Some("en".to_string()),
        pages: Some(2),
        ..record(&make_doc_id(), "Software Engineer with 5 years experience")
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
    let (_dir, store) = open_store();

    insert_two_documents(&store);

    let docs = store.list();
    assert_eq!(docs.len(), 2);
    // Should be sorted by created_at desc
    assert_eq!(docs[0].title, "CV");
    assert_eq!(docs[1].title, "Resume");
}

#[test]
fn test_set_indexed() {
    let (_dir, store) = open_store();

    let doc = record(&make_doc_id(), "Text");

    store.insert(&doc).unwrap();
    store.set_indexed(&doc.id).unwrap();

    let docs = store.list();
    assert!(docs[0].indexed);
}

#[test]
fn test_remove_document() {
    let (_dir, store) = open_store();

    let doc = record(&make_doc_id(), "Text");

    store.insert(&doc).unwrap();
    store.remove(&doc.id).unwrap();

    let docs = store.list();
    assert!(docs.is_empty());
}

#[test]
fn test_set_default() {
    let (_dir, store) = open_store();

    let (doc1, doc2) = insert_two_documents(&store);

    // Set doc2 as default
    store.set_default(&doc2.id).unwrap();

    let docs = store.list();
    assert!(!docs.iter().find(|d| d.id == doc1.id).unwrap().is_default);
    assert!(docs.iter().find(|d| d.id == doc2.id).unwrap().is_default);
}

// Verify that keywords_json survives an insert → list → get round-trip without
// any column-position corruption from future migrations.
#[test]
fn test_keywords_json_round_trip() {
    let (_dir, store) = open_store();

    let keywords_payload = Some("[\"rust\",\"typescript\"]".to_string());
    let doc = DocumentRecord {
        keywords_json: keywords_payload.clone(),
        ..record(&make_doc_id(), "Rust and TypeScript developer")
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
fn test_extract_text_plain_and_markdown() {
    for (file, bytes, expected) in [
        ("test.txt", &b"Hello, World!"[..], "Hello, World!"),
        ("test.md", &b"# Heading\nContent"[..], "# Heading\nContent"),
    ] {
        let result = crate::extraction::route(file, bytes).unwrap();
        assert_eq!(result.text, expected, "{file}");
    }
}

#[test]
fn test_extract_text_unsupported() {
    let result = crate::extraction::route("test.xyz", b"content");
    assert!(result.is_err());
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
    let (_dir, store) = open_store();

    let id = make_doc_id();
    let expected_text = "Experienced Rust developer with 7 years of experience.";

    let doc = DocumentRecord {
        locale: Some("en".to_string()),
        pages: Some(1),
        ..record(&id, expected_text)
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
    let (_dir, store) = open_store();

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
    let (_dir, store) = open_store();

    let id = make_doc_id();
    let doc = DocumentRecord {
        title: "Empty".to_string(),
        name: "empty.txt".to_string(),
        ..record(&id, "")
    };
    store.insert(&doc).unwrap();

    let text = store.get(&id).map(|d| d.text).unwrap_or_default();
    assert_eq!(text, "");
}

#[test]
fn documents_get_text_returns_text_after_multiple_inserts() {
    // Verify get() returns the right document when multiple docs are in the store.
    let (_dir, store) = open_store();

    let id_a = "doc-text-a".to_string();
    let id_b = "doc-text-b".to_string();

    for (id, text) in [(&id_a, "Resume A text"), (&id_b, "Resume B text")] {
        store
            .insert(&DocumentRecord {
                title: id.clone(),
                name: format!("{id}.pdf"),
                ..record(id, text)
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

// `clear_all()` (the factory-reset path: `Resettable::reset()` → `clear_all()`)
// must wipe ALL FOUR tables — documents, vectors, posting_vectors, match_scores —
// otherwise a user's "delete all data" leaves résumés, embeddings, and match
// scores at rest. Guards the data-retention contract for the full table set.
#[test]
#[serial]
fn test_clear_all_wipes_posting_vectors_and_match_scores() {
    let (_dir, store) = open_store();

    // Populate all four tables: a document, its résumé vector, a posting vector,
    // and a match score.
    let doc = record("doc-1", "Rust developer");
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

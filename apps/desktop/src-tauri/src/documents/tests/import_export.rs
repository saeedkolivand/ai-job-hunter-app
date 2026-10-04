//! `DataStore` export / import: the round trip, and the restore's validate-before-wipe and
//! skip-don't-brick guarantees.

use super::{support::*, *};

/// `import` used to call `clear_all()` — which wipes documents, vectors,
/// posting_vectors and match_scores — BEFORE deserializing the rows, so a
/// malformed row partway through destroyed the user's entire existing library
/// and still returned Err, leaving nothing to restore from.
#[test]
fn import_of_a_malformed_bundle_leaves_the_existing_library_intact() {
    use crate::data_store::DataStore;

    let (_dir, store) = open_store();

    let existing = DocumentRecord {
        title: "Keep".to_string(),
        name: "keep.pdf".to_string(),
        is_default: true,
        ..record("doc-keep", "precious")
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

    let (_dir, store) = open_store();

    let a = DocumentRecord {
        title: "A".to_string(),
        name: "a.pdf".to_string(),
        ..record("doc-a", "first")
    };
    let b = DocumentRecord {
        title: "B".to_string(),
        name: "b.pdf".to_string(),
        created_at: now_ms() + 1,
        is_default: true,
        ..record("doc-b", "second")
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

    let (_dir, store) = open_store();

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

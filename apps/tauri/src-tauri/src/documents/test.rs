use super::*;
use tempfile::TempDir;

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
    
    store.upsert_vector(doc_id, &vector).unwrap();
    let retrieved = store.get_vector(doc_id);
    assert_eq!(retrieved, Some(vector));
    
    // Update the vector
    let new_vector = vec![0.5, 0.6, 0.7, 0.8];
    store.upsert_vector(doc_id, &new_vector).unwrap();
    let retrieved = store.get_vector(doc_id);
    assert_eq!(retrieved, Some(new_vector));
}

#[test]
fn test_get_vector() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    
    let doc_id = "doc-123";
    let vector = vec![0.1, 0.2, 0.3];
    
    store.upsert_vector(doc_id, &vector).unwrap();
    assert_eq!(store.get_vector(doc_id), Some(vector));
    assert_eq!(store.get_vector("nonexistent"), None);
}

#[test]
fn test_all_vectors() {
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    
    store.upsert_vector("doc-1", &[0.1, 0.2]).unwrap();
    store.upsert_vector("doc-2", &[0.3, 0.4]).unwrap();
    
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
    };
    store.insert(&a).unwrap();
    store.insert(&b).unwrap();
    store.set_default("doc-b").unwrap();
    store.upsert_vector("doc-b", &[0.1, 0.2, 0.3]).unwrap();

    let bundle = store.export();

    // Restore into a fresh store.
    let temp2 = TempDir::new().unwrap();
    let restored = DocumentStore::open(&temp2.path().to_path_buf()).unwrap();
    let count = restored.import(&bundle).unwrap();

    assert_eq!(count, 2);
    let docs = restored.list();
    assert_eq!(docs.len(), 2);
    // The originally-default doc stays default after restore.
    assert_eq!(docs.iter().find(|d| d.is_default).map(|d| d.id.as_str()), Some("doc-b"));
    // Vectors survive the round trip.
    assert_eq!(restored.get_vector("doc-b"), Some(vec![0.1, 0.2, 0.3]));
}

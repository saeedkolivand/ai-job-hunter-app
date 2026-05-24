/// Native document store (SQLite-backed). Holds metadata + embedding vectors.
///
/// Metadata is persisted in SQLite (rusqlite, bundled). Embedding vectors are
/// stored as JSON arrays in the same database — adequate for the small local
/// datasets (≤ hundreds of documents) this app handles.
///
/// Ollama is called for embeddings via reqwest; gracefully degrades when
/// Ollama is not running.
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRecord {
    #[serde(rename = "_id")]
    pub id: String,
    pub title: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<u32>,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    pub indexed: bool,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

// ── DocumentStore ─────────────────────────────────────────────────────────────

pub struct DocumentStore {
    conn: Mutex<Connection>,
}

impl DocumentStore {
    pub fn open(data_dir: &PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let path = data_dir.join("documents.db");
        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL,
                name        TEXT NOT NULL,
                locale      TEXT,
                text        TEXT NOT NULL,
                pages       INTEGER,
                created_at  INTEGER NOT NULL,
                indexed     INTEGER NOT NULL DEFAULT 0,
                is_default  INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS vectors (
                doc_id  TEXT PRIMARY KEY,
                vector  TEXT NOT NULL
            );",
        )
        .map_err(|e| e.to_string())?;
        
        // Migration: add is_default column if it doesn't exist
        // SQLite doesn't support IF NOT EXISTS for ALTER TABLE, so we check first
        let has_column: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('documents') WHERE name = 'is_default'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if has_column == 0 {
            conn.execute("ALTER TABLE documents ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0", [])
                .map_err(|e| e.to_string())?;
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn list(&self) -> Vec<DocumentRecord> {
        let conn = self.conn.lock().unwrap();
        conn.prepare(
            "SELECT id, title, name, locale, text, pages, created_at, indexed, is_default
             FROM documents ORDER BY created_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(DocumentRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    name: row.get(2)?,
                    locale: row.get(3)?,
                    text: row.get(4)?,
                    pages: row.get(5)?,
                    created_at: row.get::<_, i64>(6)? as u64,
                    indexed: row.get::<_, i64>(7)? != 0,
                    is_default: row.get::<_, i64>(8).unwrap_or(0) != 0,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    pub fn insert(&self, rec: &DocumentRecord) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        // If this is the first document, automatically set it as default
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
            .unwrap_or(0);
        let is_default = if count == 0 { true } else { rec.is_default };
        
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                rec.id,
                rec.title,
                rec.name,
                rec.locale,
                rec.text,
                rec.pages,
                rec.created_at as i64,
                rec.indexed as i64,
                is_default as i64,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_indexed(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE documents SET indexed = 1 WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM documents WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM vectors WHERE doc_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_default(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        // Clear all defaults, then set the new one
        conn.execute("UPDATE documents SET is_default = 0", [])
            .map_err(|e| e.to_string())?;
        conn.execute("UPDATE documents SET is_default = 1 WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn upsert_vector(&self, doc_id: &str, vector: &[f64]) -> Result<(), String> {
        let json = serde_json::to_string(vector).map_err(|e| e.to_string())?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO vectors (doc_id, vector) VALUES (?1, ?2)
             ON CONFLICT(doc_id) DO UPDATE SET vector = excluded.vector",
            params![doc_id, json],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_vector(&self, doc_id: &str) -> Option<Vec<f64>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT vector FROM vectors WHERE doc_id = ?1",
            params![doc_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s: String| serde_json::from_str(&s).ok())
    }

    pub fn all_vectors(&self) -> Vec<(String, Vec<f64>)> {
        let conn = self.conn.lock().unwrap();
        conn.prepare("SELECT doc_id, vector FROM vectors")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .ok()
                .map(|rows| {
                    rows.filter_map(|r| r.ok())
                        .filter_map(|(id, json): (String, String)| {
                            serde_json::from_str::<Vec<f64>>(&json).ok().map(|v| (id, v))
                        })
                        .collect()
                })
            })
            .unwrap_or_default()
    }
}

// ── Text extraction ───────────────────────────────────────────────────────────

pub fn extract_text(name: &str, bytes: &[u8]) -> Result<String, String> {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "pdf" => extract_pdf(bytes),
        "docx" => extract_docx(bytes),
        "txt" | "md" | "markdown" => {
            String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string())
        }
        other => Err(format!("unsupported file type: .{other}")),
    }
}

fn extract_pdf(bytes: &[u8]) -> Result<String, String> {
    pdf_extract::extract_text_from_mem(bytes).map_err(|e| e.to_string())
}

fn extract_docx(bytes: &[u8]) -> Result<String, String> {
    use std::io::Read;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| "invalid docx: missing word/document.xml".to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;

    // Strip XML tags with a simple state machine — avoids quick-xml API churn.
    let mut text = String::new();
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag && !c.is_control() => text.push(c),
            _ => {}
        }
    }
    Ok(text.split_whitespace().collect::<Vec<_>>().join(" "))
}

// ── Ollama embedding ──────────────────────────────────────────────────────────

const EMBED_MODEL: &str = "nomic-embed-text";

pub async fn embed(text: &str) -> Option<Vec<f64>> {
    let base = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let truncated = &text[..text.len().min(8000)];
    let body = serde_json::json!({ "model": EMBED_MODEL, "prompt": truncated });
    let resp = client
        .post(format!("{base}/api/embeddings"))
        .json(&body)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let data: serde_json::Value = resp.json().await.ok()?;
    let arr = data.get("embedding")?.as_array()?;
    Some(arr.iter().filter_map(|v| v.as_f64()).collect())
}

// ── Cosine similarity ─────────────────────────────────────────────────────────

pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn make_doc_id() -> String {
    use uuid::Uuid;
    format!("doc-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
}

pub fn strip_extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((base, _)) if !base.is_empty() => base.to_string(),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
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
        
        store.upsert_vector("doc-1", &vec![0.1, 0.2]).unwrap();
        store.upsert_vector("doc-2", &vec![0.3, 0.4]).unwrap();
        
        let vectors = store.all_vectors();
        assert_eq!(vectors.len(), 2);
    }

    #[test]
    fn test_extract_text_plain() {
        let text = extract_text("test.txt", b"Hello, World!").unwrap();
        assert_eq!(text, "Hello, World!");
    }

    #[test]
    fn test_extract_text_markdown() {
        let text = extract_text("test.md", b"# Heading\nContent").unwrap();
        assert_eq!(text, "# Heading\nContent");
    }

    #[test]
    fn test_extract_text_unsupported() {
        let result = extract_text("test.xyz", b"content");
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
}

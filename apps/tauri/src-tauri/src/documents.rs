/// Native document store — replaces the sidecar's DataStore (NeDB + LanceDB).
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
                indexed     INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS vectors (
                doc_id  TEXT PRIMARY KEY,
                vector  TEXT NOT NULL
            );",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn list(&self) -> Vec<DocumentRecord> {
        let conn = self.conn.lock().unwrap();
        conn.prepare(
            "SELECT id, title, name, locale, text, pages, created_at, indexed
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
                    created_at: row.get(6)?,
                    indexed: row.get::<_, i64>(7)? != 0,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    pub fn insert(&self, rec: &DocumentRecord) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                rec.id,
                rec.title,
                rec.name,
                rec.locale,
                rec.text,
                rec.pages,
                rec.created_at,
                rec.indexed as i64,
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
        .and_then(|s| serde_json::from_str(&s).ok())
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
                        .filter_map(|(id, json)| {
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

    let mut text = String::new();
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Text(e)) => {
                if let Ok(t) = e.unescape() {
                    let s = t.trim();
                    if !s.is_empty() {
                        if !text.is_empty() {
                            text.push(' ');
                        }
                        text.push_str(s);
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
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

use parking_lot::Mutex;
/// Native document store (SQLite-backed). Holds metadata + embedding vectors.
///
/// Metadata is persisted in SQLite (rusqlite, bundled). Embedding vectors are
/// stored as JSON arrays in the same database — adequate for the small local
/// datasets (≤ hundreds of documents) this app handles.
///
/// Ollama is called for embeddings via reqwest; gracefully degrades when
/// Ollama is not running.
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector, ProviderId};
use crate::data_store::DataStore;
use crate::db::{column_exists, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::AppResult;

pub mod keywords;

// ── Types ─────────────────────────────────────────────────────────────────────

/// The active embedding configuration. Persisted next to the vectors it governs
/// (in documents.db) because changing it changes the embedding *space* — every
/// stored vector must be re-embedded. Defaults to local Ollama for offline use.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingConfig {
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl EmbeddingConfig {
    /// True when a stored vector's space was produced by this exact config.
    pub fn matches(&self, space: &EmbeddingSpace) -> bool {
        self.provider == space.provider && self.model == space.model
    }
}

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
    /// Cached normalized (un-stemmed) résumé keywords as a sorted JSON array.
    /// Populated at import; the match path stems these at query time.
    #[serde(rename = "keywordsJson", skip_serializing_if = "Option::is_none")]
    pub keywords_json: Option<String>,
}

// ── DocumentStore ─────────────────────────────────────────────────────────────

pub struct DocumentStore {
    conn: Mutex<Connection>,
}

impl DocumentStore {
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
            name: "create_documents_and_vectors",
            up: |conn| {
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
            },
        },
        Migration {
            name: "add_is_default_column",
            up: |conn| {
                if !column_exists(conn, "documents", "is_default") {
                    conn.execute(
                        "ALTER TABLE documents ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0",
                        [],
                    )?;
                }
                Ok(())
            },
        },
        Migration {
            // Tag every vector with the embedding space that produced it so
            // incompatible vectors can never be silently compared. Legacy rows
            // were all Ollama/nomic-embed-text; their `dim` is backfilled in `open`.
            name: "add_vector_space_metadata",
            up: |conn| {
                for (col, ddl) in [
                    (
                        "provider",
                        "ALTER TABLE vectors ADD COLUMN provider TEXT NOT NULL DEFAULT 'ollama'",
                    ),
                    (
                        "model",
                        "ALTER TABLE vectors ADD COLUMN model TEXT NOT NULL DEFAULT 'nomic-embed-text'",
                    ),
                    ("dim", "ALTER TABLE vectors ADD COLUMN dim INTEGER NOT NULL DEFAULT 0"),
                    ("version", "ALTER TABLE vectors ADD COLUMN version INTEGER NOT NULL DEFAULT 1"),
                ] {
                    if !column_exists(conn, "vectors", col) {
                        conn.execute(ddl, [])?;
                    }
                }
                Ok(())
            },
        },
        Migration {
            name: "create_embedding_config",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS embedding_config (
                        id          INTEGER PRIMARY KEY CHECK (id = 1),
                        provider    TEXT NOT NULL,
                        model       TEXT NOT NULL,
                        base_url    TEXT,
                        updated_at  INTEGER NOT NULL
                    );
                    INSERT OR IGNORE INTO embedding_config (id, provider, model, base_url, updated_at)
                    VALUES (1, 'ollama', 'nomic-embed-text', NULL, 0);",
                )
            },
        },
        Migration {
            // Cache normalized (un-stemmed) keywords per document so the match
            // path skips re-tokenizing résumé text. Nullable: legacy rows fall
            // back to live extraction in match_resume.
            name: "cache_document_keywords",
            up: |conn| {
                if !column_exists(conn, "documents", "keywords_json") {
                    conn.execute("ALTER TABLE documents ADD COLUMN keywords_json TEXT", [])?;
                }
                Ok(())
            },
        },
    ];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let path = data_dir.join("documents.db");
        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
        run_migrations(&conn, Self::MIGRATIONS)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.backfill_vector_dims();
        Ok(store)
    }

    /// Fill the `dim` of legacy vectors (rows added before space metadata existed,
    /// stored with `dim = 0`) from their actual JSON length. One-time, idempotent.
    fn backfill_vector_dims(&self) {
        let conn = self.conn.lock();
        let rows: Vec<(String, String)> = conn
            .prepare("SELECT doc_id, vector FROM vectors WHERE dim = 0")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default();
        for (doc_id, json) in rows {
            if let Ok(v) = serde_json::from_str::<Vec<f64>>(&json) {
                conn.execute(
                    "UPDATE vectors SET dim = ?1 WHERE doc_id = ?2",
                    params![v.len() as i64, doc_id],
                )
                .ok();
            }
        }
    }

    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute_batch("DELETE FROM vectors; DELETE FROM documents;")
            .ok();
    }

    pub fn list(&self) -> Vec<DocumentRecord> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT id, title, name, locale, text, pages, created_at, indexed, is_default, keywords_json
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
                    created_at: ts_from_db(row.get::<_, i64>(6)?),
                    indexed: row.get::<_, i64>(7)? != 0,
                    is_default: row.get::<_, i64>(8).unwrap_or(0) != 0,
                    keywords_json: row.get::<_, Option<String>>(9).unwrap_or(None),
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    /// Fetch a single document by id.
    pub fn get(&self, id: &str) -> Option<DocumentRecord> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, title, name, locale, text, pages, created_at, indexed, is_default, keywords_json
             FROM documents WHERE id = ?1",
            params![id],
            |row| {
                Ok(DocumentRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    name: row.get(2)?,
                    locale: row.get(3)?,
                    text: row.get(4)?,
                    pages: row.get(5)?,
                    created_at: ts_from_db(row.get::<_, i64>(6)?),
                    indexed: row.get::<_, i64>(7)? != 0,
                    is_default: row.get::<_, i64>(8).unwrap_or(0) != 0,
                    keywords_json: row.get::<_, Option<String>>(9).unwrap_or(None),
                })
            },
        )
        .ok()
    }

    pub fn insert(&self, rec: &DocumentRecord) -> AppResult<()> {
        let conn = self.conn.lock();
        // If this is the first document, automatically set it as default
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
            .unwrap_or(0);
        let is_default = if count == 0 { true } else { rec.is_default };

        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default, keywords_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                rec.id,
                rec.title,
                rec.name,
                rec.locale,
                rec.text,
                rec.pages,
                ts_to_db(rec.created_at),
                rec.indexed as i64,
                is_default as i64,
                rec.keywords_json,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_indexed(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE documents SET indexed = 1 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM documents WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM vectors WHERE doc_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_default(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        // Clear all defaults, then set the new one
        conn.execute("UPDATE documents SET is_default = 0", [])
            .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE documents SET is_default = 1 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Store a space-tagged vector. The space (`provider`/`model`/`dim`) travels
    /// with the values so comparisons can reject incompatible vectors.
    pub fn upsert_vector(&self, doc_id: &str, v: &EmbeddingVector) -> AppResult<()> {
        let json = serde_json::to_string(&v.values).map_err(|e| e.to_string())?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO vectors (doc_id, vector, provider, model, dim, version)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)
             ON CONFLICT(doc_id) DO UPDATE SET
                vector = excluded.vector, provider = excluded.provider,
                model = excluded.model, dim = excluded.dim, version = excluded.version",
            params![
                doc_id,
                json,
                v.space.provider,
                v.space.model,
                v.space.dim as i64
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_vector(&self, doc_id: &str) -> Option<EmbeddingVector> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT vector, provider, model, dim FROM vectors WHERE doc_id = ?1",
            params![doc_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .ok()
        .and_then(|(json, provider, model, dim)| {
            let values: Vec<f64> = serde_json::from_str(&json).ok()?;
            Some(EmbeddingVector {
                values,
                space: EmbeddingSpace {
                    provider,
                    model,
                    dim: dim as usize,
                },
            })
        })
    }

    pub fn all_vectors(&self) -> Vec<(String, EmbeddingVector)> {
        let conn = self.conn.lock();
        conn.prepare("SELECT doc_id, vector, provider, model, dim FROM vectors")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })
                .ok()
                .map(|rows| {
                    rows.filter_map(|r| r.ok())
                        .filter_map(|(id, json, provider, model, dim)| {
                            serde_json::from_str::<Vec<f64>>(&json).ok().map(|values| {
                                (
                                    id,
                                    EmbeddingVector {
                                        values,
                                        space: EmbeddingSpace {
                                            provider,
                                            model,
                                            dim: dim as usize,
                                        },
                                    },
                                )
                            })
                        })
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    /// Count of stored vectors grouped by embedding space (for the status panel).
    pub fn vector_space_counts(&self) -> Vec<(EmbeddingSpace, usize)> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT provider, model, dim, COUNT(*) FROM vectors GROUP BY provider, model, dim",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok((
                    EmbeddingSpace {
                        provider: row.get::<_, String>(0)?,
                        model: row.get::<_, String>(1)?,
                        dim: row.get::<_, i64>(2)? as usize,
                    },
                    row.get::<_, i64>(3)? as usize,
                ))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    pub fn embedding_config(&self) -> EmbeddingConfig {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT provider, model, base_url FROM embedding_config WHERE id = 1",
            [],
            |row| {
                Ok(EmbeddingConfig {
                    provider: row.get::<_, String>(0)?,
                    model: row.get::<_, String>(1)?,
                    base_url: row.get::<_, Option<String>>(2)?,
                })
            },
        )
        .unwrap_or_else(|_| EmbeddingConfig {
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            base_url: None,
        })
    }

    pub fn set_embedding_config(&self, cfg: &EmbeddingConfig) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO embedding_config (id, provider, model, base_url, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider, model = excluded.model,
                base_url = excluded.base_url, updated_at = excluded.updated_at",
            params![cfg.provider, cfg.model, cfg.base_url, ts_to_db(now_ms())],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

// ── Embedding ───────────────────────────────────────────────────────────────
//
// Routes through the centralized provider layer using the persisted embedding
// config, so embeddings are provider-aware (Ollama / OpenAI / Gemini) and every
// vector is tagged with the space that produced it. No provider/endpoint strings
// live here.

pub async fn embed(app: &AppHandle, text: &str) -> Option<EmbeddingVector> {
    let cfg = app.state::<DocumentStore>().embedding_config();
    let provider = ProviderId::parse(&cfg.provider).ok()?;
    match crate::commands::ai_provider::embed_text(
        app,
        provider,
        &cfg.model,
        cfg.base_url.clone(),
        text,
    )
    .await
    {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("embed failed ({}): {e}", cfg.provider);
            None
        }
    }
}

// ── Cosine similarity ─────────────────────────────────────────────────────────

/// Raw cosine over two same-length vectors. Prefer
/// [`crate::commands::ai_provider::compare`] for stored vectors, which also
/// verifies both share an embedding space.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    crate::commands::ai_provider::cosine(a, b)
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

impl DataStore for DocumentStore {
    fn key(&self) -> &'static str {
        "documents"
    }

    fn export(&self) -> serde_json::Value {
        let docs: Vec<serde_json::Value> = self
            .list()
            .into_iter()
            .map(|rec| {
                let mut obj = serde_json::to_value(&rec).unwrap_or_else(|_| serde_json::json!({}));
                if let Some(ev) = self.get_vector(&rec.id) {
                    obj["vector"] = serde_json::json!(ev.values);
                    obj["vectorSpace"] = serde_json::json!({
                        "provider": ev.space.provider,
                        "model": ev.space.model,
                        "dim": ev.space.dim,
                    });
                }
                obj
            })
            .collect();
        serde_json::json!(docs)
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data.as_array().ok_or("documents: expected an array")?;
        self.clear_all();
        let mut count = 0;
        let mut default_id: Option<String> = None;
        for item in items {
            let record: DocumentRecord =
                serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
            if record.is_default {
                default_id = Some(record.id.clone());
            }
            self.insert(&record)?;
            if let Some(vector) = item.get("vector").and_then(|v| v.as_array()) {
                let vec: Vec<f64> = vector.iter().filter_map(|v| v.as_f64()).collect();
                if !vec.is_empty() {
                    // Restore the vector's space if present; legacy exports without
                    // it predate cloud embeddings and were all Ollama/nomic-embed-text.
                    let dim = vec.len();
                    let space = item
                        .get("vectorSpace")
                        .map(|s| EmbeddingSpace {
                            provider: s
                                .get("provider")
                                .and_then(|v| v.as_str())
                                .unwrap_or("ollama")
                                .to_string(),
                            model: s
                                .get("model")
                                .and_then(|v| v.as_str())
                                .unwrap_or("nomic-embed-text")
                                .to_string(),
                            dim: s
                                .get("dim")
                                .and_then(|v| v.as_u64())
                                .map(|d| d as usize)
                                .unwrap_or(dim),
                        })
                        .unwrap_or_else(|| EmbeddingSpace {
                            provider: "ollama".to_string(),
                            model: "nomic-embed-text".to_string(),
                            dim,
                        });
                    self.upsert_vector(&record.id, &EmbeddingVector { values: vec, space })?;
                }
            }
            count += 1;
        }
        // insert() auto-defaults the first row; restore the originally-default doc.
        if let Some(id) = default_id {
            self.set_default(&id)?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod test;

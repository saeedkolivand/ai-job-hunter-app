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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Whether moving from `old` to `new` is a real embedding-space change — i.e.
/// any field differs (provider, model, or base_url). The posting_vectors /
/// match_scores caches key on provider+model, so their old-space rows become
/// unreachable and must be evicted only when this returns true. Single source of
/// `ai_set_embedding_config`'s eviction gate so a dropped check fails a test.
pub(crate) fn embedding_space_changed(old: &EmbeddingConfig, new: &EmbeddingConfig) -> bool {
    old != new
}

/// Fill the `dim` of legacy vectors (rows added before space metadata existed,
/// stored with `dim = 0`) from their actual JSON length. A `Migration::up`, so it
/// runs exactly once under the `user_version` gate (previously: on every `open()`).
/// Idempotent — only `dim = 0` rows are touched — and runs inside the migration
/// transaction (`conn` is the migration's transaction handle).
fn backfill_vector_dims(conn: &Connection) -> rusqlite::Result<()> {
    let rows: Vec<(String, String)> = {
        let mut stmt = conn.prepare("SELECT doc_id, vector FROM vectors WHERE dim = 0")?;
        let mapped = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        mapped.filter_map(|r| r.ok()).collect()
    };
    for (doc_id, json) in rows {
        if let Ok(v) = serde_json::from_str::<Vec<f64>>(&json) {
            conn.execute(
                "UPDATE vectors SET dim = ?1 WHERE doc_id = ?2",
                params![v.len() as i64, doc_id],
            )?;
        }
    }
    Ok(())
}

/// Full cache key for the `match_scores` result cache (the table PK). Borrowed
/// fields keep it allocation-free at the call site; passed by reference to the
/// store methods. Grouped into a struct because 7 positional args read poorly.
pub struct MatchScoreKey<'a> {
    pub resume_id: &'a str,
    pub job_id: &'a str,
    pub provider: &'a str,
    pub model: &'a str,
    /// 1 when semantic scoring ran, 0 when it was skipped.
    pub semantic_enabled: i64,
    pub formula_version: i64,
    /// SHA-256 of the post-translation job text (see [`sha256_hex`]).
    pub job_text_hash: &'a str,
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
    /// Monotonic count of `upsert_posting_vector` writes, used to amortize the
    /// posting-vector cache prune onto a cheap cadence (every
    /// [`Self::POSTING_PRUNE_EVERY`] writes) instead of running two DELETEs under
    /// the held connection lock on *every* write. A re-embed (`ai_reembed_all`)
    /// upserts hundreds of postings back-to-back, so the old per-write prune was
    /// pure overhead — the cache can briefly exceed its bound between prunes,
    /// which is fine for a best-effort cache.
    posting_writes: std::sync::atomic::AtomicU64,
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
        Migration {
            // Persisted, translation-aware job-vector cache. Keyed by job_id
            // (one row per posting); `text_hash` pins the row to the exact text
            // that was embedded (post-translation) and the provider/model pin the
            // embedding space, so a stale or wrong-language row is a natural miss.
            name: "create_posting_vectors",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS posting_vectors (
                        job_id     TEXT PRIMARY KEY,
                        text_hash  TEXT NOT NULL,
                        vector     TEXT NOT NULL,
                        provider   TEXT NOT NULL,
                        model      TEXT NOT NULL,
                        dim        INTEGER NOT NULL,
                        created_at INTEGER NOT NULL
                    );",
                )
            },
        },
        Migration {
            // Persisted, self-invalidating match-result cache. The full PK is the
            // cache key: resume/job ids, embedding space (provider/model), whether
            // semantic scoring ran, the formula version, and a hash of the
            // post-translation job text. Any change to those is a fresh key (miss).
            name: "create_match_scores",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS match_scores (
                        resume_id        TEXT NOT NULL,
                        job_id           TEXT NOT NULL,
                        provider         TEXT NOT NULL,
                        model            TEXT NOT NULL,
                        semantic_enabled INTEGER NOT NULL,
                        formula_version  INTEGER NOT NULL,
                        job_text_hash    TEXT NOT NULL,
                        score_json       TEXT NOT NULL,
                        created_at       INTEGER NOT NULL,
                        PRIMARY KEY (resume_id, job_id, provider, model, semantic_enabled, formula_version, job_text_hash)
                    );",
                )
            },
        },
        Migration {
            // Index `created_at` on both result caches so the per-write TTL prune
            // and the row-cap eviction (an ORDER BY created_at threshold delete)
            // run index-backed instead of full-table sorts. Hot path: batch
            // match-scoring upserts once per row under the held connection lock.
            name: "index_cache_created_at",
            up: |conn| {
                conn.execute_batch(
                    "CREATE INDEX IF NOT EXISTS idx_match_scores_created_at ON match_scores(created_at);
                     CREATE INDEX IF NOT EXISTS idx_posting_vectors_created_at ON posting_vectors(created_at);",
                )
            },
        },
        Migration {
            // One-time `dim` backfill for legacy vectors (rows added before the
            // space metadata existed, stored with `dim = 0`), filling each from its
            // actual JSON length. Previously this scanned on EVERY `open()`; folding
            // it into a `user_version`-gated migration makes it run exactly once.
            // Idempotent (only touches `dim = 0` rows) and runs inside the migration
            // transaction.
            name: "backfill_vector_dims",
            up: backfill_vector_dims,
        },
    ];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("documents.db");
        let mut conn = crate::db::open(&path)?;
        // The legacy `dim` backfill now runs as a one-time, `user_version`-gated
        // migration (see `backfill_vector_dims` migration above) instead of on
        // every `open()`.
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
            posting_writes: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// Prune the posting-vector cache once per this many writes (see
    /// [`DocumentStore::posting_writes`]). 64 keeps re-embed batches (hundreds of
    /// upserts) cheap while still bounding the cache regularly under steady use.
    const POSTING_PRUNE_EVERY: u64 = 64;

    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute_batch(
            "DELETE FROM vectors; DELETE FROM documents; DELETE FROM posting_vectors; DELETE FROM match_scores;",
        )
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

    // ── Posting-vector cache (translation-aware job embeddings) ───────────────
    //
    // A persisted, single-row-per-job cache of the job-text embedding. Distinct
    // from `vectors` (résumé/document embeddings) and from the in-memory
    // `PostingsCache` (which holds RAW-text vectors for hybrid search): this
    // table stores the vector for the EXACT text that was embedded, which may be
    // a translation. Reads are guarded by both the embedding space and a
    // `text_hash` of that exact text, so a stale or wrong-language row misses.

    /// Fetch a cached posting vector plus the `text_hash` it was stored under.
    /// The caller compares the space (`EmbeddingConfig::matches`) and the hash
    /// before trusting it. Mirrors `get_vector`'s read+deserialize shape.
    pub fn get_posting_vector(&self, job_id: &str) -> Option<(EmbeddingVector, String)> {
        let conn = self.conn.lock();
        // Read-side TTL: an expired-but-not-yet-evicted row is a miss. None ttl = no expiry.
        let cutoff = ttl_cutoff_ms();
        conn.query_row(
            "SELECT vector, provider, model, dim, text_hash FROM posting_vectors WHERE job_id = ?1 AND created_at >= ?2",
            params![job_id, cutoff],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .ok()
        .and_then(|(json, provider, model, dim, text_hash)| {
            let values: Vec<f64> = serde_json::from_str(&json).ok()?;
            Some((
                EmbeddingVector {
                    values,
                    space: EmbeddingSpace {
                        provider,
                        model,
                        dim: dim as usize,
                    },
                },
                text_hash,
            ))
        })
    }

    /// Store (or replace) the cached vector for `job_id`, tagged with the
    /// `text_hash` of the exact text embedded and its embedding space.
    pub fn upsert_posting_vector(
        &self,
        job_id: &str,
        text_hash: &str,
        v: &EmbeddingVector,
    ) -> AppResult<()> {
        let json = serde_json::to_string(&v.values).map_err(|e| e.to_string())?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO posting_vectors (job_id, text_hash, vector, provider, model, dim, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(job_id) DO UPDATE SET
                text_hash = excluded.text_hash, vector = excluded.vector,
                provider = excluded.provider, model = excluded.model,
                dim = excluded.dim, created_at = excluded.created_at",
            params![
                job_id,
                text_hash,
                json,
                v.space.provider,
                v.space.model,
                v.space.dim as i64,
                ts_to_db(now_ms()),
            ],
        )
        .map_err(|e| e.to_string())?;
        // Amortized eviction: prune only once per `POSTING_PRUNE_EVERY` writes,
        // reusing the held lock (must NOT re-lock). A re-embed upserts hundreds of
        // postings back-to-back, so the previous per-write prune ran two DELETEs
        // on every call for no benefit; the cache may briefly exceed its bound
        // between prunes, which is fine for a best-effort cache.
        let n = self
            .posting_writes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        if n.is_multiple_of(Self::POSTING_PRUNE_EVERY) {
            let cfg = crate::performance::current();
            Self::prune_table_locked(
                &conn,
                "posting_vectors",
                cfg.cache_ttl_secs,
                cfg.cache_max_rows,
            );
        }
        Ok(())
    }

    /// Drop the entire posting-vector cache (e.g. on embedding-config change).
    pub fn clear_posting_vectors(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM posting_vectors", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Bound BOTH result caches: expire rows older than `ttl_secs` and cap each
    /// table to the newest `max_rows`. `None` for a knob disables that bound
    /// (today's unbounded behavior). Best-effort — a failed prune never blocks
    /// the caller. Pure of its inputs (does not read the live global), so the
    /// command can pass the exact tier it just applied.
    pub fn prune_caches(&self, ttl_secs: Option<i64>, max_rows: Option<i64>) {
        let conn = self.conn.lock();
        Self::prune_table_locked(&conn, "posting_vectors", ttl_secs, max_rows);
        Self::prune_table_locked(&conn, "match_scores", ttl_secs, max_rows);
    }

    /// Prune one cache table to the given TTL + row cap, reusing an already-held
    /// connection lock (callers hold `self.conn`; parking_lot Mutex is NOT
    /// reentrant, so we must never call a `self.*` method that re-locks). No-op
    /// when a knob is `None`. Table names are hardcoded literals (not user
    /// input), so formatting them into the SQL is safe.
    fn prune_table_locked(
        conn: &Connection,
        table: &str,
        ttl_secs: Option<i64>,
        max_rows: Option<i64>,
    ) {
        if let Some(ttl) = ttl_secs {
            // created_at is epoch-MILLIS; ttl is seconds.
            let cutoff = ts_to_db(now_ms()).saturating_sub(ttl.saturating_mul(1000));
            let _ = conn.execute(
                &format!("DELETE FROM {table} WHERE created_at < ?1"),
                params![cutoff],
            );
        }
        if let Some(n) = max_rows {
            // Index-friendly row cap: delete everything older than the n-th newest
            // row. The subquery uses idx_*_created_at (ORDER BY created_at DESC
            // LIMIT 1 OFFSET n) instead of an unindexed full-table NOT IN sort.
            // ≤ n rows → subquery is NULL → `created_at < NULL` deletes nothing.
            // Ties on created_at may retain slightly more than n rows — fine for a
            // cache bound. `{table}` is a hardcoded literal; `n` is bound.
            let _ = conn.execute(
                &format!(
                    "DELETE FROM {table} WHERE created_at < \
                     (SELECT created_at FROM {table} ORDER BY created_at DESC LIMIT 1 OFFSET ?1)"
                ),
                params![n],
            );
        }
    }

    // ── Match-result cache (self-invalidating) ────────────────────────────────
    //
    // Caches the full `match_resume` JSON result. The cache key (the table PK)
    // captures every input that can change the score: the resume/job ids, the
    // embedding space, whether semantic scoring ran, the formula version, and a
    // hash of the post-translation job text. A change to any of those is a new
    // key — so the cache self-invalidates without explicit eviction.

    /// Fetch a cached match-score JSON result for the given key, if present.
    pub fn get_match_score(&self, key: &MatchScoreKey) -> Option<serde_json::Value> {
        let conn = self.conn.lock();
        // Read-side TTL: an expired-but-not-yet-evicted row is a miss. None ttl = no expiry.
        let cutoff = ttl_cutoff_ms();
        conn.query_row(
            "SELECT score_json FROM match_scores
             WHERE resume_id = ?1 AND job_id = ?2 AND provider = ?3 AND model = ?4
               AND semantic_enabled = ?5 AND formula_version = ?6 AND job_text_hash = ?7
               AND created_at >= ?8",
            params![
                key.resume_id,
                key.job_id,
                key.provider,
                key.model,
                key.semantic_enabled,
                key.formula_version,
                key.job_text_hash,
                cutoff,
            ],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
    }

    /// Store (or replace) the cached match-score JSON result for the given key.
    pub fn upsert_match_score(&self, key: &MatchScoreKey, score_json: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO match_scores
                (resume_id, job_id, provider, model, semantic_enabled, formula_version,
                 job_text_hash, score_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(resume_id, job_id, provider, model, semantic_enabled, formula_version, job_text_hash)
             DO UPDATE SET score_json = excluded.score_json, created_at = excluded.created_at",
            params![
                key.resume_id,
                key.job_id,
                key.provider,
                key.model,
                key.semantic_enabled,
                key.formula_version,
                key.job_text_hash,
                score_json,
                ts_to_db(now_ms()),
            ],
        )
        .map_err(|e| e.to_string())?;
        // Lazy per-write eviction, reusing the held lock (must NOT re-lock).
        let cfg = crate::performance::current();
        Self::prune_table_locked(
            &conn,
            "match_scores",
            cfg.cache_ttl_secs,
            cfg.cache_max_rows,
        );
        Ok(())
    }

    /// Drop the entire match-result cache (e.g. on embedding-config change).
    pub fn clear_match_scores(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM match_scores", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Count of stored vectors in one embedding space, by SQL `COUNT(*)` — never
    /// deserializes the float-array blobs. Powers `ai_embedding_status`'s
    /// indexed-in-active-space figure (the old path loaded every vector via
    /// `all_vectors()` just to count the matching ones). Matches the same
    /// provider+model identity as [`EmbeddingConfig::matches`].
    pub fn count_vectors_in_space(&self, provider: &str, model: &str) -> usize {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM vectors WHERE provider = ?1 AND model = ?2",
            params![provider, model],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n as usize)
        .unwrap_or(0)
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

/// Lowercase-hex SHA-256 of `text`. Deterministic and stable across process
/// restarts (unlike `DefaultHasher`/`RandomState`), so it is safe as the
/// cross-session cache guard for both `posting_vectors.text_hash` and
/// `match_scores.job_text_hash`. Single source of the hash for both caches.
pub(crate) fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    h.finalize()
        .iter()
        .fold(String::with_capacity(64), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

/// Cache-precedence predicate for the posting-vector cache: a cached row is a
/// HIT iff its embedding space matches the `active` config AND it was stored for
/// the exact text we're requesting (`requested_hash == cached.text_hash`). A
/// `None` row (no cached vector) is always a miss. This is the single source of
/// the resolver's cache-hit decision — [`posting_vector_or_embed`] calls it so a
/// reverted/loosened check fails a unit test (see documents/test.rs).
pub(crate) fn posting_vector_is_fresh(
    active: &EmbeddingConfig,
    requested_hash: &str,
    cached: Option<&(EmbeddingVector, String)>,
) -> bool {
    match cached {
        Some((v, stored_hash)) => active.matches(&v.space) && stored_hash == requested_hash,
        None => false,
    }
}

/// Resolve the embedding for a job posting's (possibly translated) `text`,
/// using the persisted `posting_vectors` cache. A hit avoids the embed call
/// entirely. The cache is guarded by BOTH the active embedding space and a
/// `text_hash` of the exact `text` passed here, so a stale or wrong-language
/// row is a natural miss. Does NOT touch `PostingsCache` (raw-text vectors).
pub async fn posting_vector_or_embed(
    app: &AppHandle,
    job_id: &str,
    text: &str,
) -> Option<EmbeddingVector> {
    // Snapshot everything from the store before any await — the store methods
    // each take/release the lock internally and return owned values, so no DB
    // lock is held across the embed call below.
    let active = app.state::<DocumentStore>().embedding_config();
    let hash = sha256_hex(text);
    let cached = app.state::<DocumentStore>().get_posting_vector(job_id);
    // Single cache-hit decision (space + text_hash), shared with its unit test.
    if posting_vector_is_fresh(&active, &hash, cached.as_ref()) {
        return cached.map(|(v, _)| v); // cache hit — no embed
    }
    let v = embed(app, text).await?;
    app.state::<DocumentStore>()
        .upsert_posting_vector(job_id, &hash, &v)
        .ok();
    Some(v)
}

// ── Cosine similarity ─────────────────────────────────────────────────────────

/// Raw cosine over two same-length vectors. Prefer
/// [`crate::commands::ai_provider::compare`] for stored vectors, which also
/// verifies both share an embedding space.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    crate::commands::ai_provider::cosine(a, b)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Millisecond cutoff for a read-side TTL miss from the live performance config.
/// `None` TTL → `i64::MIN` (no row is ever excluded). created_at is epoch-MILLIS.
fn ttl_cutoff_ms() -> i64 {
    match crate::performance::current().cache_ttl_secs {
        Some(ttl) => ts_to_db(now_ms()).saturating_sub(ttl.saturating_mul(1000)),
        None => i64::MIN,
    }
}

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

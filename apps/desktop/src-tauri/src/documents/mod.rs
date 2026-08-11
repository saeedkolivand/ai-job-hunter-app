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
use std::sync::Arc;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::ai_provider::{
    EmbeddingSpace, EmbeddingVector, ProviderId, EMBEDDING_VECTOR_VERSION,
};
use crate::data_store::DataStore;
use crate::db::{column_exists, now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::AppResult;

use sql::{
    get_match_score_with_conn, get_vector_with_conn, prune_due, prune_table_locked,
    spawn_blocking_db, upsert_match_score_with_conn, upsert_vector_with_conn,
};

pub mod evidence;
pub mod keywords;
mod mojibake_repair;
mod sql;

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
    /// True when a stored vector's space was produced by this exact config
    /// AND the current [`EMBEDDING_VECTOR_VERSION`] — a version bump (e.g.
    /// replacing naive truncation with chunk-and-mean-pool) makes an
    /// old-format vector a miss even though its provider/model tag is
    /// unchanged, so a re-embed picks up the new format instead of silently
    /// comparing across formats.
    pub fn matches(&self, space: &EmbeddingSpace) -> bool {
        self.provider == space.provider
            && self.model == space.model
            && space.version == EMBEDDING_VECTOR_VERSION
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

/// One-time migration: `text-embedding-004` was retired by Google (shutdown
/// Jan 14, 2026). Any install that had already persisted it as the active
/// Gemini embedding model would keep 404-ing forever even after the code
/// default changed to `gemini-embedding-2` — `embed_text` only falls back to
/// `AiProvider::default_embedding_model()` when the STORED model is empty
/// (`ai_provider/mod.rs`), so a non-empty retired id is never revisited on
/// its own. Idempotent and self-healing: rewrites the persisted row
/// directly, so the Settings UI (which mirrors `status.active.model`
/// verbatim) shows the corrected model with no additional read-time
/// special-casing anywhere.
///
/// The model column is free text (whatever the user typed/pasted into the
/// Settings model field), so the `WHERE` matches on `trim(lower(model))`
/// against BOTH the bare id and its `models/`-prefixed form — the Gemini
/// adapter (`gemini.rs`) itself deliberately strips a leading `models/`, so
/// that form is a real, deliberately-accepted variant, not a hypothetical
/// one. An exact-only match would miss `models/text-embedding-004`,
/// `TEXT-EMBEDDING-004`, and a trailing-space paste — leaving those installs
/// 404-ing forever, the exact failure mode this migration exists to fix.
///
/// This IS a real embedding-space change (retired model → current model),
/// exactly the case `ai_set_embedding_config` evicts `posting_vectors` /
/// `match_scores` for at runtime (see `embedding_space_changed`) — so this
/// migration performs the SAME eviction, only when the `UPDATE` actually
/// touched a row (an install that never had the stale model is left alone,
/// no needless cache wipe).
fn alias_retired_gemini_text_embedding_004(conn: &Connection) -> rusqlite::Result<()> {
    let rows_changed = conn.execute(
        "UPDATE embedding_config SET model = 'gemini-embedding-2' \
         WHERE provider = 'gemini' \
           AND trim(lower(model)) IN ('text-embedding-004', 'models/text-embedding-004')",
        [],
    )?;
    if rows_changed > 0 {
        conn.execute_batch("DELETE FROM posting_vectors; DELETE FROM match_scores;")?;
    }
    Ok(())
}

/// Full cache key for the `match_scores` result cache (the table PK). Borrowed
/// fields keep it allocation-free at the call site; passed by reference to the
/// store methods. Grouped into a struct because 8 positional args read poorly.
pub struct MatchScoreKey<'a> {
    pub resume_id: &'a str,
    pub job_id: &'a str,
    pub provider: &'a str,
    pub model: &'a str,
    /// 1 when semantic scoring ran, 0 when it was skipped.
    pub semantic_enabled: i64,
    pub formula_version: i64,
    /// [`EMBEDDING_VECTOR_VERSION`] at score-compute time. A semantic score is
    /// derived from embedding vectors, so a vector-format bump changes what the
    /// cached score MEANS even when neither `formula_version` nor the job text
    /// changes — without this field a bump could only self-invalidate by
    /// accident (a coincidental `formula_version` bump, or a maintainer
    /// remembering to add a `DELETE FROM match_scores` to that release's
    /// migration). Carrying it in the key makes invalidation structural: a new
    /// `EMBEDDING_VECTOR_VERSION` is a new key, so it's a miss by construction.
    pub vector_version: i64,
    /// SHA-256 of the post-translation job text (see [`sha256_hex`]).
    pub job_text_hash: &'a str,
}

impl MatchScoreKey<'_> {
    /// Copy the borrowed key into an owned form that can cross into a
    /// `spawn_blocking` (`'static`) closure for the async store methods.
    pub fn to_owned_key(&self) -> OwnedMatchScoreKey {
        OwnedMatchScoreKey {
            resume_id: self.resume_id.to_string(),
            job_id: self.job_id.to_string(),
            provider: self.provider.to_string(),
            model: self.model.to_string(),
            semantic_enabled: self.semantic_enabled,
            formula_version: self.formula_version,
            vector_version: self.vector_version,
            job_text_hash: self.job_text_hash.to_string(),
        }
    }
}

/// Owned twin of [`MatchScoreKey`]. The borrowed key keeps the hot call site
/// allocation-free, but a `spawn_blocking` closure must be `Send + 'static`, so
/// the async store methods take this owned form and borrow it back inside the
/// closure via [`OwnedMatchScoreKey::as_ref`].
pub struct OwnedMatchScoreKey {
    pub resume_id: String,
    pub job_id: String,
    pub provider: String,
    pub model: String,
    pub semantic_enabled: i64,
    pub formula_version: i64,
    pub vector_version: i64,
    pub job_text_hash: String,
}

impl OwnedMatchScoreKey {
    /// Borrow back into a [`MatchScoreKey`] so the shared SQL helper takes one
    /// key type for both the sync and async paths.
    pub fn as_ref(&self) -> MatchScoreKey<'_> {
        MatchScoreKey {
            resume_id: &self.resume_id,
            job_id: &self.job_id,
            provider: &self.provider,
            model: &self.model,
            semantic_enabled: self.semantic_enabled,
            formula_version: self.formula_version,
            vector_version: self.vector_version,
            job_text_hash: &self.job_text_hash,
        }
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
    /// Shared so a clone can move into a `spawn_blocking` closure on the hot
    /// match path (the closure must be `Send + 'static`). `parking_lot::Mutex`
    /// is not reentrant — never re-lock while a guard is held, and never hold a
    /// guard across an `.await`.
    conn: Arc<Mutex<Connection>>,
    /// Monotonic count of `upsert_posting_vector` writes, used to amortize that
    /// cache's prune onto a cheap cadence — see [`sql::prune_due`], which owns
    /// the rationale and the cadence both counters share.
    posting_writes: std::sync::atomic::AtomicU64,
    /// The same, for `match_scores`. Its own counter, not a shared one: the two
    /// tables are written by different paths at wildly different rates (a
    /// re-embed batch touches only postings; a scoring run writes both), and one
    /// counter would let the busy path drag its quiet sibling's prune along.
    match_score_writes: std::sync::atomic::AtomicU64,
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
        Migration {
            // text-embedding-004 was retired by Google (shutdown Jan 14, 2026 —
            // the exact "model or endpoint not found" error this fixes). Any
            // install that had already persisted it as the active embedding
            // model would keep 404-ing FOREVER even after the code default
            // changed to gemini-embedding-2: `embed_text` only falls back to
            // `AiProvider::default_embedding_model()` when the STORED model
            // string is empty, so a non-empty retired id is never revisited.
            // One-time, idempotent (WHERE-scoped), self-healing alias — chosen
            // over a read-time alias in `embedding_config()` so the fix is a
            // single UPDATE rather than special-casing every reader forever,
            // and so the Settings UI mirrors the corrected model immediately
            // (it reads the persisted `model` verbatim).
            name: "alias_retired_gemini_text_embedding_004",
            up: alias_retired_gemini_text_embedding_004,
        },
        Migration {
            // `EMBEDDING_VECTOR_VERSION` bumped 1 -> 2 when embeddings moved
            // from a naive single truncation to chunk-and-mean-pool. The
            // résumé/document `vectors` table carries its own `version`
            // column, so a stale row there is already caught by
            // `EmbeddingConfig::matches` on the next read. `posting_vectors`
            // (job-posting embeddings) has NO version column — its only
            // built-in staleness guard is the TTL/row-cap prune, and the TTL
            // is NOT a guarantee: the top preference tier sets
            // `cacheTtlSecs: null`, which `ttl_cutoff_ms()` reads as "never
            // expires" (only the row cap still applies). Without this, a
            // pre-upgrade (truncated-prefix) posting vector can be cosined
            // against a post-upgrade (mean-pooled) résumé vector under the
            // identical space tag for however long the row cap allows.
            // Unconditional and applies to EVERY provider (unlike the
            // Gemini-specific migration above) — the format change affects
            // every provider's embeddings, not just Gemini's.
            name: "evict_posting_vectors_for_embedding_format_v2",
            up: |conn| conn.execute_batch("DELETE FROM posting_vectors;"),
        },
        Migration {
            // `match_scores`' PK didn't carry the embedding vector version, only
            // `formula_version` — so a semantic score computed from an old-format
            // vector stayed a valid cache hit against new-format vectors unless a
            // `MATCH_FORMULA_VERSION` bump happened to coincide with the
            // `EMBEDDING_VECTOR_VERSION` bump (true of the v1->v2 migration above
            // only by accident — CodeRabbit #933 follow-up). Adds `vector_version`
            // to the PK so a future format bump invalidates by construction
            // instead of relying on someone remembering to evict this table too.
            // Recreated rather than `ALTER TABLE ADD COLUMN` because SQLite can't
            // add a column to an existing PRIMARY KEY; `match_scores` is a pure
            // result cache, so dropping its rows only forces a recompute, not a
            // real data loss.
            name: "add_vector_version_to_match_scores_key",
            up: |conn| {
                conn.execute_batch(
                    "DROP TABLE IF EXISTS match_scores;
                     CREATE TABLE match_scores (
                        resume_id        TEXT NOT NULL,
                        job_id           TEXT NOT NULL,
                        provider         TEXT NOT NULL,
                        model            TEXT NOT NULL,
                        semantic_enabled INTEGER NOT NULL,
                        formula_version  INTEGER NOT NULL,
                        vector_version   INTEGER NOT NULL,
                        job_text_hash    TEXT NOT NULL,
                        score_json       TEXT NOT NULL,
                        created_at       INTEGER NOT NULL,
                        PRIMARY KEY (resume_id, job_id, provider, model, semantic_enabled,
                                     formula_version, vector_version, job_text_hash)
                     );
                     CREATE INDEX IF NOT EXISTS idx_match_scores_created_at ON match_scores(created_at);",
                )
            },
        },
        Migration {
            // `posting_vectors` had no persisted `version` column — every read
            // synthesized the CURRENT `EMBEDDING_VECTOR_VERSION` on the fly
            // (see `get_posting_vector`), so `EmbeddingConfig::matches` could
            // structurally never reject a row here on format version. Appended
            // at the END of the array (not inserted earlier) — migrations are
            // position-indexed via `PRAGMA user_version`, so an insertion
            // mid-array would make an already-migrated install skip it
            // entirely.
            //
            // `DEFAULT 2`, not 0 and not a live `EMBEDDING_VECTOR_VERSION`
            // reference: this migration runs strictly AFTER
            // `evict_posting_vectors_for_embedding_format_v2` above
            // (migrations are position-indexed, so the ordering is fixed),
            // which unconditionally wipes the table. So by the time this ADD
            // COLUMN runs, every surviving row was necessarily written
            // afterward, under the format that was current at that point —
            // `EMBEDDING_VECTOR_VERSION == 2` when this migration was
            // authored. `DEFAULT 0` would mislabel every one of those
            // provably-current rows as stale, forcing a real (billed)
            // re-embed of the entire cache for zero correctness gain. The
            // literal must stay `2` even after a future
            // `EMBEDDING_VECTOR_VERSION` bump — it records a historical fact
            // about rows as of migration time, not the live constant.
            name: "add_version_to_posting_vectors",
            up: |conn| {
                if !column_exists(conn, "posting_vectors", "version") {
                    conn.execute(
                        "ALTER TABLE posting_vectors ADD COLUMN version INTEGER NOT NULL DEFAULT 2",
                        [],
                    )?;
                }
                Ok(())
            },
        },
        Migration {
            // See `mojibake_repair` module doc for the full corruption shape
            // and the error-policy rationale (why a per-row failure
            // propagates instead of being logged-and-skipped).
            name: "repair_pre_pdf_text_string_mojibake",
            up: mojibake_repair::up,
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
            conn: Arc::new(Mutex::new(conn)),
            posting_writes: std::sync::atomic::AtomicU64::new(0),
            match_score_writes: std::sync::atomic::AtomicU64::new(0),
        })
    }

    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        // The `repair_pre_pdf_text_string_mojibake` migration (see
        // `mojibake_repair::up`) snapshots every affected row's pre-repair,
        // still-corrupt text into `documents_pre_mojibake_repair` as a
        // safety net for its in-place rewrite. That snapshot holds the
        // user's ORIGINAL document text, so a full "erase my data" reset
        // must drop it too, not just the live tables — a one-shot migration
        // artifact, not a table the app writes going forward, so `DROP`
        // (not `DELETE`) is correct: `user_version` is already past this
        // migration, so it will not be recreated.
        conn.execute_batch(
            "DELETE FROM vectors; DELETE FROM documents; DELETE FROM posting_vectors; DELETE FROM match_scores; \
             DROP TABLE IF EXISTS documents_pre_mojibake_repair;",
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

        // Defensive, not just the migration: a restored backup exported
        // before `repair_pre_pdf_text_string_mojibake` would otherwise
        // re-inject the mojibake via `import` -> `insert` (`serde_json`
        // round-trips an embedded NUL intact). No-op scan on clean input.
        let text = crate::extraction::pdf::repair_utf16_mojibake(&rec.text);
        let text_was_repaired = matches!(text, std::borrow::Cow::Owned(_));

        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default, keywords_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                rec.id,
                rec.title,
                rec.name,
                rec.locale,
                text.as_ref(),
                rec.pages,
                ts_to_db(rec.created_at),
                rec.indexed as i64,
                is_default as i64,
                rec.keywords_json,
            ],
        )
        .map_err(|e| e.to_string())?;

        if text_was_repaired {
            // The text just changed under this id — any `vectors` row for
            // it (a stale leftover, or one `import` below is about to
            // restore) is now derived from the WRONG text. `stale_documents`
            // (`commands/ai.rs`) decides what to re-embed purely from
            // `get_vector` presence in the active space, never `indexed`.
            conn.execute("DELETE FROM vectors WHERE doc_id = ?1", params![rec.id])
                .map_err(|e| e.to_string())?;
        }
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
        let conn = self.conn.lock();
        upsert_vector_with_conn(&conn, doc_id, v)
    }

    /// Async variant of [`upsert_vector`] that runs the blocking lock + write on
    /// a `spawn_blocking` thread, keeping the Tokio worker free on the hot match
    /// path (`score_one` may call this up to once per job in a 1000-job batch).
    /// Same write, same return type — callers in async contexts use this.
    pub async fn upsert_vector_async(&self, doc_id: &str, v: &EmbeddingVector) -> AppResult<()> {
        let conn = Arc::clone(&self.conn);
        let doc_id = doc_id.to_string();
        let v = v.clone();
        spawn_blocking_db(move || {
            let conn = conn.lock();
            upsert_vector_with_conn(&conn, &doc_id, &v)
        })
        .await
    }

    pub fn get_vector(&self, doc_id: &str) -> Option<EmbeddingVector> {
        let conn = self.conn.lock();
        get_vector_with_conn(&conn, doc_id)
    }

    /// Async variant of [`get_vector`] — runs the blocking lock + read off the
    /// async worker via `spawn_blocking`. A `JoinError` (closure panicked)
    /// degrades to `None`, matching the sync read's "row missing → None" shape.
    pub async fn get_vector_async(&self, doc_id: &str) -> Option<EmbeddingVector> {
        let conn = Arc::clone(&self.conn);
        let doc_id = doc_id.to_string();
        tauri::async_runtime::spawn_blocking(move || {
            let conn = conn.lock();
            get_vector_with_conn(&conn, &doc_id)
        })
        .await
        .ok()
        .flatten()
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
            "SELECT vector, provider, model, dim, version, text_hash FROM posting_vectors WHERE job_id = ?1 AND created_at >= ?2",
            params![job_id, cutoff],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .ok()
        .and_then(|(json, provider, model, dim, version, text_hash)| {
            let values: Vec<f64> = serde_json::from_str(&json).ok()?;
            Some((
                EmbeddingVector {
                    values,
                    space: EmbeddingSpace {
                        provider,
                        model,
                        dim: dim as usize,
                        // Persisted at write time (`upsert_posting_vector`), not
                        // re-derived here — `EmbeddingConfig::matches` compares
                        // it against `EMBEDDING_VECTOR_VERSION`, so a stale-format
                        // row (including a pre-migration row, defaulted to 0) is
                        // a real cache miss instead of a hand-maintained invariant.
                        version,
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
            "INSERT INTO posting_vectors (job_id, text_hash, vector, provider, model, dim, version, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(job_id) DO UPDATE SET
                text_hash = excluded.text_hash, vector = excluded.vector,
                provider = excluded.provider, model = excluded.model,
                dim = excluded.dim, version = excluded.version, created_at = excluded.created_at",
            params![
                job_id,
                text_hash,
                json,
                v.space.provider,
                v.space.model,
                v.space.dim as i64,
                v.space.version,
                ts_to_db(now_ms()),
            ],
        )
        .map_err(|e| e.to_string())?;
        // Amortized eviction on the shared cadence, reusing the held lock (must
        // NOT re-lock) — see `sql::prune_due`.
        if prune_due(&self.posting_writes) {
            let cfg = crate::performance::current();
            prune_table_locked(
                &conn,
                "posting_vectors",
                cfg.cache_ttl_secs,
                cfg.cache_max_rows,
            );
        }
        Ok(())
    }

    /// Drop ONE cached posting vector.
    ///
    /// The cache is otherwise bounded only by its TTL and row cap, which is the
    /// right discipline for a derived row whose producer still exists. It is
    /// the wrong one for a row derived from user CONTENT that has just been
    /// deleted (an Autopilot's résumé snapshot, `autopilot-resume:<sha>`): that
    /// row must go with its producer, not linger for the TTL. Idempotent — a
    /// missing row is not an error.
    pub fn delete_posting_vector(&self, job_id: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM posting_vectors WHERE job_id = ?1",
            params![job_id],
        )
        .map_err(|e| e.to_string())?;
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
    /// command can pass the exact tier it just applied. Unlike the amortized
    /// per-write prune, this one always runs: its caller is the settings change.
    pub fn prune_caches(&self, ttl_secs: Option<i64>, max_rows: Option<i64>) {
        let conn = self.conn.lock();
        prune_table_locked(&conn, "posting_vectors", ttl_secs, max_rows);
        prune_table_locked(&conn, "match_scores", ttl_secs, max_rows);
    }

    // ── Match-result cache (self-invalidating) ────────────────────────────────
    //
    // Caches the full `match_resume` JSON result. The cache key (the table PK)
    // captures every input that can change the score: the resume/job ids, the
    // embedding space, whether semantic scoring ran, the formula version, the
    // embedding vector version, and a hash of the post-translation job text. A
    // change to any of those is a new key — so the cache self-invalidates
    // without explicit eviction.

    /// Fetch a cached match-score JSON result for the given key, if present.
    pub fn get_match_score(&self, key: &MatchScoreKey) -> Option<serde_json::Value> {
        let conn = self.conn.lock();
        get_match_score_with_conn(&conn, key)
    }

    /// Async variant of [`get_match_score`] — runs the blocking lock + read off
    /// the async worker. Takes an owned [`OwnedMatchScoreKey`] because the
    /// borrowed [`MatchScoreKey`] can't cross into a `'static` closure. A
    /// `JoinError` degrades to `None` (a cache miss), so a panicking blocking
    /// task never poisons the result — `score_one` recomputes the score.
    pub async fn get_match_score_async(
        &self,
        key: OwnedMatchScoreKey,
    ) -> Option<serde_json::Value> {
        let conn = Arc::clone(&self.conn);
        tauri::async_runtime::spawn_blocking(move || {
            let conn = conn.lock();
            get_match_score_with_conn(&conn, &key.as_ref())
        })
        .await
        .ok()
        .flatten()
    }

    /// Store (or replace) the cached match-score JSON result for the given key.
    ///
    /// The amortized-prune decision is taken HERE, not in the SQL helper: the
    /// counter belongs to the store and the helper only holds a `&Connection`.
    pub fn upsert_match_score(&self, key: &MatchScoreKey, score_json: &str) -> AppResult<()> {
        let prune = prune_due(&self.match_score_writes);
        let conn = self.conn.lock();
        upsert_match_score_with_conn(&conn, key, score_json, prune)
    }

    /// Async variant of [`upsert_match_score`] — runs the blocking write + lazy
    /// eviction off the async worker. Takes owned key + json so the closure is
    /// `'static`; the prune decision is likewise resolved before the move, since
    /// the counter lives on `self`. The TTL/row-cap prune reads
    /// `performance::current()` *inside* the closure, reusing the held lock
    /// (never re-locks → no deadlock).
    pub async fn upsert_match_score_async(
        &self,
        key: OwnedMatchScoreKey,
        score_json: String,
    ) -> AppResult<()> {
        let conn = Arc::clone(&self.conn);
        let prune = prune_due(&self.match_score_writes);
        spawn_blocking_db(move || {
            let conn = conn.lock();
            upsert_match_score_with_conn(&conn, &key.as_ref(), &score_json, prune)
        })
        .await
    }

    /// Drop every cached match score computed FOR one résumé id.
    ///
    /// A `match_scores` row is résumé-derived content — its gaps,
    /// recommendations and explanation all describe that résumé — so it must
    /// die with the résumé, not at the TTL. Sibling of
    /// [`Self::delete_posting_vector`] for the other half of an Autopilot
    /// snapshot's cache footprint. Idempotent.
    pub fn delete_match_scores_for_resume(&self, resume_id: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM match_scores WHERE resume_id = ?1",
            params![resume_id],
        )
        .map_err(|e| e.to_string())?;
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
    /// indexed-in-active-space figure (the old path loaded every vector via a
    /// full vector scan just to count the matching ones). Matches the SAME
    /// identity [`EmbeddingConfig::matches`] uses — provider + model AND the
    /// current [`EMBEDDING_VECTOR_VERSION`] — so a version bump (an old-format
    /// row every real match-check now rejects) is also reflected here: without
    /// the version filter, the status strip would report `stale: 0` and "N/N
    /// indexed" over an index where every row is actually stale.
    pub fn count_vectors_in_space(&self, provider: &str, model: &str) -> usize {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM vectors WHERE provider = ?1 AND model = ?2 AND version = ?3",
            params![provider, model, EMBEDDING_VECTOR_VERSION],
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
                        // Display-only aggregate (grouped by provider/model/dim,
                        // not version) — never fed into `.matches()`/`compare()`,
                        // so a placeholder is fine here.
                        version: EMBEDDING_VECTOR_VERSION,
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

pub async fn embed(app: &AppHandle, text: &str) -> AppResult<EmbeddingVector> {
    let cfg = app.state::<DocumentStore>().embedding_config();
    let provider = ProviderId::parse(&cfg.provider).map_err(|e| {
        tracing::warn!(
            "embed failed: unknown embedding provider '{}': {e}",
            cfg.provider
        );
        e
    })?;
    crate::commands::ai_provider::embed_text(app, provider, &cfg.model, cfg.base_url.clone(), text)
        .await
        .map_err(|e| {
            tracing::warn!("embed failed ({}): {e}", cfg.provider);
            e
        })
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

/// Whether `id` belongs to a synthetic, content-addressed SCORING identity
/// (`adhoc:…` from the extension bridge, `autopilot:…` / `autopilot-resume:…`
/// from the headless re-rank) rather than a real `documents` row.
///
/// The app-wide convention for such an id is a `<namespace>:` prefix; a real
/// document id is `doc-<millis>-<uuid8>` (see [`make_doc_id`]) and never
/// contains a colon. Used by [`upsert_vector_with_conn`] to keep the DOCUMENT
/// index free of rows that have no document: nothing would ever delete them
/// (document delete and re-embed both iterate real documents; `prune_caches`
/// only touches `posting_vectors`/`match_scores`), and
/// [`DocumentStore::count_vectors_in_space`] counts every row — so one orphan
/// makes the Embeddings panel report "N/N indexed, stale 0" over an index that
/// is genuinely stale.
pub(crate) fn is_synthetic_scoring_id(id: &str) -> bool {
    id.contains(':')
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

/// ONE embedding round-trip, behind a seam.
///
/// The scoring kernel's only reach into the provider layer. It is a trait (not
/// a bare `AppHandle` call) because this crate has no `tauri::test` mock-app
/// harness: with the round-trip behind a seam, "how many provider calls did
/// this score make, on what bytes, and what did each one cost" is a plain unit
/// test over a real [`DocumentStore`] instead of a hand-retyped mirror of the
/// kernel's own cache logic — which is exactly the shape that let an
/// untranslated-hash charge predicate pass review.
/// `pub(crate)`, not `pub`: the choke point is only a guarantee while every
/// implementor is in this crate and reachable from `embed_charged`.
#[async_trait::async_trait]
pub(crate) trait Embedder: Send + Sync {
    /// `None` on any failure — the caller degrades to keyword-only scoring.
    async fn embed_one(&self, text: &str) -> Option<EmbeddingVector>;
}

/// A budget consulted immediately before each ACTUAL embedding round-trip.
///
/// The charge belongs HERE, at the call, evaluated on the exact bytes the call
/// consumes — never in a caller that predicts the call from earlier state. The
/// two answers diverge in practice: a caller sees the pre-translation blob (so
/// its hash never matches the cached row for a translated posting → it charges
/// on every total cache hit), and it cannot see the résumé-side embed at all
/// (an evicted résumé vector then makes a real, uncharged round-trip). Same
/// rule as `extension_bridge::answer_assist`: charge immediately before the
/// work that reaches the provider, once per round-trip, and not at all when a
/// cached path short-circuits.
///
/// `Err` means the ceiling refused: the round-trip must NOT happen.
/// `pub(crate)` for the same reason as [`Embedder`]: "charged exactly once" is
/// only a guarantee while every implementor is in this crate.
pub(crate) trait EmbedBudget: Send + Sync {
    fn charge_one_embed(&self) -> AppResult<()>;
}

/// Charge (if a budget is present) and then make one embedding round-trip.
///
/// The single choke point for every provider call the scoring kernel makes, so
/// "charged exactly once per actual embed" is a property of one function rather
/// than a convention each call site has to remember. A refused charge returns
/// `None` — the same degrade-to-keyword-only signal as a failed embed.
pub(crate) async fn embed_charged<E: Embedder + ?Sized>(
    embedder: &E,
    budget: Option<&dyn EmbedBudget>,
    text: &str,
) -> Option<EmbeddingVector> {
    if let Some(budget) = budget {
        if let Err(e) = budget.charge_one_embed() {
            log::info!("[embed] round-trip refused by the daily ceiling: {e}");
            return None;
        }
    }
    embedder.embed_one(text).await
}

/// Production [`Embedder`]: the app's configured embedding provider.
///
/// `embed` already logs its own failure (see its doc), so `.ok()` here only
/// discards the error from this `Option`-returning seam.
pub(crate) struct AppEmbedder<'a>(pub &'a AppHandle);

#[async_trait::async_trait]
impl Embedder for AppEmbedder<'_> {
    async fn embed_one(&self, text: &str) -> Option<EmbeddingVector> {
        embed(self.0, text).await.ok()
    }
}

/// Resolve the embedding for a job posting's (possibly translated) `text`,
/// using the persisted `posting_vectors` cache. A hit avoids the embed call
/// entirely. The cache is guarded by BOTH the active embedding space and a
/// `text_hash` of the exact `text` passed here, so a stale or wrong-language
/// row is a natural miss. Does NOT touch `PostingsCache` (raw-text vectors).
///
/// `budget` is charged only on the miss path, immediately before the call —
/// see [`EmbedBudget`]. `active` comes from the caller (which already resolved
/// it for its own cache key) so the hit decision and the score are computed
/// against one snapshot of the embedding space.
pub(crate) async fn posting_vector_or_embed<E: Embedder + ?Sized>(
    store: &DocumentStore,
    active: &EmbeddingConfig,
    embedder: &E,
    budget: Option<&dyn EmbedBudget>,
    job_id: &str,
    text: &str,
) -> Option<EmbeddingVector> {
    // Snapshot everything from the store before any await — the store methods
    // each take/release the lock internally and return owned values, so no DB
    // lock is held across the embed call below.
    let hash = sha256_hex(text);
    let cached = store.get_posting_vector(job_id);
    // Single cache-hit decision (space + text_hash), shared with its unit test.
    if posting_vector_is_fresh(active, &hash, cached.as_ref()) {
        return cached.map(|(v, _)| v); // cache hit — no embed, no charge
    }
    let v = embed_charged(embedder, budget, text).await?;
    // Best-effort cache write: the embed already succeeded, so a failed upsert
    // must not fail the score — it only means the NEXT call re-embeds (and
    // re-charges) this posting. Logged rather than dropped, because a
    // persistently failing write is invisible otherwise and reads downstream as
    // "the cache never hits". Content-free: the id and the error, never the text.
    if let Err(e) = store.upsert_posting_vector(job_id, &hash, &v) {
        log::warn!("[documents] posting vector not cached for {job_id}: {e}");
    }
    Some(v)
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

pub fn make_doc_id() -> String {
    use uuid::Uuid;
    format!("doc-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
}

/// Read an exported document's optional embedding vector out of its JSON row.
///
/// Legacy exports carry no `vectorSpace` — they predate cloud embeddings and were
/// all Ollama/nomic-embed-text, which is what the fallback records. The export
/// JSON never carries a `version` (see `export()` below), so every imported
/// vector is tagged `version: 0` — never [`EMBEDDING_VECTOR_VERSION`] — so
/// `EmbeddingConfig::matches` always treats a just-imported vector as stale
/// and re-embeds it. Conservative on purpose: we genuinely don't know which
/// format version produced a vector from another install/app version.
fn parse_exported_vector(item: &serde_json::Value) -> Option<EmbeddingVector> {
    let values: Vec<f64> = item
        .get("vector")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_f64())
        .collect();
    if values.is_empty() {
        return None;
    }
    let dim = values.len();
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
            version: 0,
        })
        .unwrap_or_else(|| EmbeddingSpace {
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            dim,
            version: 0,
        });
    Some(EmbeddingVector { values, space })
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
        // Deserialize EVERY row before mutating the store. `clear_all` wipes
        // documents, vectors, posting_vectors AND match_scores, so a malformed
        // row (hand-edited bundle, newer schema, corruption) reached after that
        // call used to destroy the user's whole document library + embeddings and
        // still return Err — nothing left to restore from. The sibling stores
        // (applications, ai_generations, dedup, discovered) all validate up-front
        // for exactly this reason; documents was the lone outlier.
        let parsed: Vec<(DocumentRecord, Option<EmbeddingVector>)> = items
            .iter()
            .map(|item| {
                let record: DocumentRecord =
                    serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
                Ok((record, parse_exported_vector(item)))
            })
            .collect::<AppResult<_>>()?;

        self.clear_all();
        let mut count = 0;
        let mut default_id: Option<String> = None;
        for (record, vector) in &parsed {
            if record.is_default {
                default_id = Some(record.id.clone());
            }
            // A pre-#955 bundle carries BOTH the corrupt text and the vector
            // derived from it. `insert()` repairs the text and, if it
            // changed, deletes any latent vector for this id — but that
            // happens BEFORE the `upsert_vector` below would restore the
            // bundle's own (corrupt-derived) one, so it must be skipped
            // here too or it would just get written right back.
            let text_was_repaired = matches!(
                crate::extraction::pdf::repair_utf16_mojibake(&record.text),
                std::borrow::Cow::Owned(_)
            );
            self.insert(record)?;
            if let Some(vector) = vector {
                if !text_was_repaired {
                    // A `<namespace>:` id is refused by the document-index write
                    // guard (`is_synthetic_scoring_id`). Unreachable for a bundle
                    // this app produced — `export()` only walks real `documents`
                    // rows — but a hand-edited backup must not BRICK the restore:
                    // `clear_all()` has already run, so propagating here would
                    // leave the library half-restored with nothing to retry from.
                    // Skip the one vector (it re-embeds on demand) and keep going.
                    if let Err(e) = self.upsert_vector(&record.id, vector) {
                        log::warn!(
                            "[documents] import: skipping the embedding of one restored document ({e})"
                        );
                    }
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

//! Native document store (SQLite-backed). Holds metadata + embedding vectors.
//!
//! Metadata is persisted in SQLite (rusqlite, bundled). Embedding vectors are
//! stored as JSON arrays in the same database — adequate for the small local
//! datasets (≤ hundreds of documents) this app handles.
//!
//! Ollama is called for embeddings via reqwest; gracefully degrades when
//! Ollama is not running.
//!
//! This file holds the store handle and the document CRUD; every other slice of
//! the same store lives in a sibling module, split by responsibility (R8):
//! `migrations` (schema), `vectors` (embedding vectors + active config),
//! `caches` (posting-vector + match-score caches), `async_ops` (the
//! `spawn_blocking` variants), `backup` (export / import), `sql` (shared
//! connection-bound queries).

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db};
use crate::error::AppResult;

mod async_ops;
mod backup;
mod caches;
mod embedding;
pub mod evidence;
// Inherent `impl DocumentStore` only (the `help_vectors` cache), so there is
// nothing to re-export — see help_vectors.rs's own doc for why it is a
// separate file rather than more of mod.rs.
mod help_vectors;
pub mod keywords;
mod migrations;
mod mojibake_repair;
mod sql;
mod vectors;

pub use caches::{MatchScoreKey, OwnedMatchScoreKey};
// Re-exported flat at `documents::` so this split is invisible to every
// existing `crate::documents::X` call site (`embed` alone has ~5 external
// callers, `sha256_hex`/`EmbedBudget` several more) — see embedding.rs's doc.
pub use embedding::embed;
pub(crate) use embedding::{
    embed_charged, embed_with_config, is_synthetic_scoring_id, posting_vector_or_embed, sha256_hex,
    AppEmbedder, EmbedBudget, Embedder,
};
// `posting_vector_is_fresh` has no caller outside `embedding.rs` itself in a
// non-test build — only the `documents::tests` unit tests reach it through
// this re-export, so it is unused (and clippy `-D warnings` fails on it)
// outside `#[cfg(test)]`.
#[cfg(test)]
pub(crate) use embedding::posting_vector_is_fresh;
pub(crate) use vectors::embedding_space_changed;
pub use vectors::EmbeddingConfig;

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

/// Map one `documents` row, in the column order of the `SELECT`s in
/// [`DocumentStore::list`] and [`DocumentStore::get`].
fn record_from_row(row: &rusqlite::Row) -> rusqlite::Result<DocumentRecord> {
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
}

impl DocumentStore {
    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("documents.db");
        let mut conn = crate::db::open(&path)?;
        // The legacy `dim` backfill now runs as a one-time, `user_version`-gated
        // migration (see `backfill_vector_dims` in `migrations.rs`) instead of on
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
             DELETE FROM help_vectors; \
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
            stmt.query_map([], record_from_row)
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
            record_from_row,
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
            // (`commands/ai/embeddings.rs`) decides what to re-embed purely from
            // `get_vector` presence in the active space, never `indexed`.
            conn.execute("DELETE FROM vectors WHERE doc_id = ?1", params![rec.id])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Run ONE statement under the connection lock, mapping the SQLite error to the
    /// store's string error — the shape every single-statement write shares (also
    /// used by the sibling modules' cache deletes).
    fn exec(&self, sql: &str, params: impl rusqlite::Params) -> AppResult<()> {
        self.conn
            .lock()
            .execute(sql, params)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_indexed(&self, id: &str) -> AppResult<()> {
        self.exec(
            "UPDATE documents SET indexed = 1 WHERE id = ?1",
            params![id],
        )
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

#[cfg(test)]
mod tests;

//! Embedding vectors and the active embedding config.
//!
//! Split out of `documents/mod.rs` (R8's hard LOC cap); everything here moved
//! verbatim. Owns the space-tagged document `vectors` table's sync reads/writes
//! (the shared SQL lives in `documents/sql.rs`), the per-space counts the
//! Embeddings status panel reads, and the persisted [`EmbeddingConfig`] that
//! decides which space those vectors must be in.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector, EMBEDDING_VECTOR_VERSION};
use crate::db::{now_ms, ts_to_db};
use crate::error::AppResult;
use crate::observability::sanitize_reason;

use super::sql::{get_vector_with_conn, upsert_vector_with_conn};
use super::DocumentStore;

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

impl DocumentStore {
    /// Store a space-tagged vector. The space (`provider`/`model`/`dim`) travels
    /// with the values so comparisons can reject incompatible vectors.
    pub fn upsert_vector(&self, doc_id: &str, v: &EmbeddingVector) -> AppResult<()> {
        let conn = self.conn.lock();
        upsert_vector_with_conn(&conn, doc_id, v)
    }

    pub fn get_vector(&self, doc_id: &str) -> Option<EmbeddingVector> {
        let conn = self.conn.lock();
        get_vector_with_conn(&conn, doc_id)
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
        let result = conn.query_row(
            "SELECT provider, model, base_url FROM embedding_config WHERE id = 1",
            [],
            |row| {
                Ok(EmbeddingConfig {
                    provider: row.get::<_, String>(0)?,
                    model: row.get::<_, String>(1)?,
                    base_url: row.get::<_, Option<String>>(2)?,
                })
            },
        );
        result.unwrap_or_else(|e| {
            // A missing row is the ordinary unseeded default; anything else is
            // a real fault silently substituting a different embedding model
            // — this used to swallow both cases identically, with no log line.
            if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                tracing::debug!(
                    "embedding_config: unseeded, defaulting to ollama/nomic-embed-text"
                );
            } else {
                // `e` is `rusqlite::Error` — `InvalidPath` can carry the
                // absolute DB path, so this must never interpolate it raw
                // (the `check-log-error-leaks` guard only scans `log::`
                // macros, not `tracing::`, so it can't catch this itself).
                tracing::warn!(
                    "embedding_config: read failed ({}), defaulting to ollama/nomic-embed-text",
                    sanitize_reason(&e.to_string())
                );
            }
            EmbeddingConfig {
                provider: "ollama".to_string(),
                model: "nomic-embed-text".to_string(),
                base_url: None,
            }
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

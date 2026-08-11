//! Connection-bound SQL for the hot match path, split out of `documents/mod.rs`
//! (which is at R8's LOC cap). Pure move — the functions, their SQL and their
//! comments are unchanged; only their visibility widened to `pub(super)` so the
//! parent's sync + async wrappers keep sharing one copy of each query.
//!
//! A child module sees the parent's private items, so `DocumentStore::
//! prune_table_locked` and `ttl_cutoff_ms` are still reachable from here.

use rusqlite::{params, Connection};

use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector};
use crate::db::{now_ms, ts_to_db};
use crate::error::AppResult;

use super::{is_synthetic_scoring_id, ttl_cutoff_ms, DocumentStore, MatchScoreKey};

// ── Connection-bound SQL helpers ───────────────────────────────────────────────
//
// The hot match-path methods (`*_vector` / `*_match_score`) have both a sync
// form (used by the synchronous `DataStore` trait + tests) and an async form
// that offloads the blocking lock + query onto `spawn_blocking`. Both share the
// SQL through these `&Connection`-bound free functions so the query lives in
// exactly one place. They take `&Connection` (the caller already holds the
// lock), so they never re-lock — `parking_lot::Mutex` is not reentrant.

pub(super) fn upsert_vector_with_conn(
    conn: &Connection,
    doc_id: &str,
    v: &EmbeddingVector,
) -> AppResult<()> {
    // The guard lives HERE, at the write, not at each call site: a future
    // ephemeral scoring path would otherwise re-introduce the orphan by simply
    // not knowing the rule. See [`is_synthetic_scoring_id`]. Loud (an `Err`),
    // because every legitimate caller writes a real document id.
    if is_synthetic_scoring_id(doc_id) {
        return Err(format!(
            "vectors is the document index: refusing a synthetic scoring id ({doc_id})"
        )
        .into());
    }
    let json = serde_json::to_string(&v.values)?;
    // Persists the vector's OWN `space.version` rather than force-advancing to
    // `EMBEDDING_VECTOR_VERSION`. Every caller that produces a *fresh* vector
    // already builds it at the current version (`embed_text`), so binding the
    // field is equivalent for them — but `import()` deliberately tags a restored
    // backup vector `version: 0` (it is an old-format, pre-chunk-pool value), and
    // force-advancing here silently overwrote that to the current version, so the
    // restored vector read as fresh and was never re-embedded. A write path must
    // persist an identity field, not re-derive it.
    conn.execute(
        "INSERT INTO vectors (doc_id, vector, provider, model, dim, version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(doc_id) DO UPDATE SET
            vector = excluded.vector, provider = excluded.provider,
            model = excluded.model, dim = excluded.dim, version = excluded.version",
        params![
            doc_id,
            json,
            v.space.provider,
            v.space.model,
            v.space.dim as i64,
            v.space.version,
        ],
    )?;
    Ok(())
}

pub(super) fn get_vector_with_conn(conn: &Connection, doc_id: &str) -> Option<EmbeddingVector> {
    conn.query_row(
        "SELECT vector, provider, model, dim, version FROM vectors WHERE doc_id = ?1",
        params![doc_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        },
    )
    .ok()
    .and_then(|(json, provider, model, dim, version)| {
        let values: Vec<f64> = serde_json::from_str(&json).ok()?;
        Some(EmbeddingVector {
            values,
            space: EmbeddingSpace {
                provider,
                model,
                dim: dim as usize,
                version,
            },
        })
    })
}

pub(super) fn get_match_score_with_conn(
    conn: &Connection,
    key: &MatchScoreKey,
) -> Option<serde_json::Value> {
    // Read-side TTL: an expired-but-not-yet-evicted row is a miss. None ttl = no expiry.
    let cutoff = ttl_cutoff_ms();
    conn.query_row(
        "SELECT score_json FROM match_scores
         WHERE resume_id = ?1 AND job_id = ?2 AND provider = ?3 AND model = ?4
           AND semantic_enabled = ?5 AND formula_version = ?6 AND vector_version = ?7
           AND job_text_hash = ?8
           AND created_at >= ?9",
        params![
            key.resume_id,
            key.job_id,
            key.provider,
            key.model,
            key.semantic_enabled,
            key.formula_version,
            key.vector_version,
            key.job_text_hash,
            cutoff,
        ],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|json| serde_json::from_str(&json).ok())
}

pub(super) fn upsert_match_score_with_conn(
    conn: &Connection,
    key: &MatchScoreKey,
    score_json: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO match_scores
            (resume_id, job_id, provider, model, semantic_enabled, formula_version,
             vector_version, job_text_hash, score_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(resume_id, job_id, provider, model, semantic_enabled, formula_version,
                     vector_version, job_text_hash)
         DO UPDATE SET score_json = excluded.score_json, created_at = excluded.created_at",
        params![
            key.resume_id,
            key.job_id,
            key.provider,
            key.model,
            key.semantic_enabled,
            key.formula_version,
            key.vector_version,
            key.job_text_hash,
            score_json,
            ts_to_db(now_ms()),
        ],
    )
    .map_err(|e| e.to_string())?;
    // Lazy per-write eviction, reusing the held lock (must NOT re-lock).
    let cfg = crate::performance::current();
    DocumentStore::prune_table_locked(conn, "match_scores", cfg.cache_ttl_secs, cfg.cache_max_rows);
    Ok(())
}

/// Run a fallible blocking DB closure on the `spawn_blocking` pool and flatten
/// the `JoinError` into the typed [`AppError`] hierarchy. A panic in the closure
/// (or pool shutdown) surfaces as an `AppError::Storage`, matching how every
/// other rusqlite failure on this store is categorized. Used by the write-side
/// async methods, where the `?`-propagated result must distinguish failure.
pub(super) async fn spawn_blocking_db<F>(f: F) -> AppResult<()>
where
    F: FnOnce() -> AppResult<()> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| crate::error::AppError::Storage(format!("documents db task failed: {e}")))?
}

//! Connection-bound SQL for the hot match path, split out of `documents/mod.rs`
//! (which is at R8's LOC cap), together with the lazy cache eviction that runs
//! under the same held lock. Visibility widened to `pub(super)` so the parent's
//! sync + async wrappers keep sharing one copy of each query.
//!
//! Everything here was moved verbatim except two things, both documented where
//! they live: the synthetic-scoring-id guard in [`upsert_vector_with_conn`]
//! (added with the split, because the guard belongs AT the write rather than at
//! each call site), and the amortized [`prune_due`] cadence that
//! `upsert_match_score_with_conn` now shares with `upsert_posting_vector`
//! instead of running two DELETEs on every write.
//!
//! A child module sees the parent's private items, so `ttl_cutoff_ms` is still
//! reachable from here.

use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{params, Connection};

use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector};
use crate::db::{now_ms, ts_to_db};
use crate::error::AppResult;

use super::{is_synthetic_scoring_id, ttl_cutoff_ms, MatchScoreKey};

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

/// `prune` carries the amortized-eviction decision from the caller, which owns
/// the write counter (see [`prune_due`]); this function only has a
/// `&Connection`. Passing the decision in — rather than pruning here on every
/// write — is what puts `match_scores` on the same cadence as its
/// `posting_vectors` sibling. The match path is the BIGGER batch of the two: one
/// Autopilot re-rank writes a row per job on top of whatever the Jobs page
/// scores, so a per-write prune was up to ~2000 extra DELETEs per run.
pub(super) fn upsert_match_score_with_conn(
    conn: &Connection,
    key: &MatchScoreKey,
    score_json: &str,
    prune: bool,
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
    // Lazy amortized eviction, reusing the held lock (must NOT re-lock).
    if prune {
        let cfg = crate::performance::current();
        prune_table_locked(conn, "match_scores", cfg.cache_ttl_secs, cfg.cache_max_rows);
    }
    Ok(())
}

// ── Lazy cache eviction ────────────────────────────────────────────────────────

/// How many writes to a cache table share ONE eviction pass.
///
/// Pruning under the held connection lock on every write is two DELETEs of pure
/// overhead in the batches this store actually sees: `ai_reembed_all` upserts
/// hundreds of posting vectors back-to-back, and one Autopilot re-rank writes a
/// `match_scores` row per scored job. 64 keeps those batches cheap while still
/// bounding the caches regularly under steady use; a table may briefly exceed
/// its row cap between prunes, which is fine for a best-effort cache (the
/// read-side TTL in [`get_match_score_with_conn`] is what keeps a STALE row from
/// being served, and it does not depend on eviction having run).
pub(super) const CACHE_PRUNE_EVERY: u64 = 64;

/// Whether THIS write carries the amortized prune: true once every
/// [`CACHE_PRUNE_EVERY`] writes of `counter`.
///
/// Bumping and testing in one place is what keeps the two cache tables on one
/// documented cadence — the alternative (each write site rolling its own) is how
/// `posting_vectors` ended up amortized while its `match_scores` sibling still
/// ran two DELETEs per write.
pub(super) fn prune_due(counter: &AtomicU64) -> bool {
    let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
    n.is_multiple_of(CACHE_PRUNE_EVERY)
}

/// Prune one cache table to the given TTL + row cap, reusing an already-held
/// connection lock (callers hold `DocumentStore::conn`; parking_lot Mutex is NOT
/// reentrant, so we must never call a `self.*` method that re-locks). No-op
/// when a knob is `None`. Table names are hardcoded literals (not user input),
/// so formatting them into the SQL is safe.
pub(super) fn prune_table_locked(
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

/// Run a fallible blocking DB closure on the `spawn_blocking` pool and flatten
/// the `JoinError` into the typed [`AppError`] hierarchy. A panic in the closure
/// (or pool shutdown) surfaces as an `AppError::Storage`, matching how every
/// other rusqlite failure on this store is categorized. Used by the write-side
/// async methods, where the `?`-propagated result must distinguish failure.
///
/// Tokio's `spawn_blocking`, not `tauri::async_runtime`'s: the latter is a thin
/// forward to exactly this call, and taking it directly keeps the whole file
/// Tauri-free (architecture R2 — it is an L1 store, and its allowlist entry is
/// gone). Every caller awaits from inside the runtime Tauri itself runs on, so
/// there is no context to lose.
pub(super) async fn spawn_blocking_db<F>(f: F) -> AppResult<()>
where
    F: FnOnce() -> AppResult<()> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| crate::error::AppError::Storage(format!("documents db task failed: {e}")))?
}

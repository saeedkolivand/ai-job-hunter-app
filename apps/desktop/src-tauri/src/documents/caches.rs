//! The derived caches — `posting_vectors` (translation-aware job embeddings) and
//! `match_scores` (the self-invalidating match-result cache) — and the TTL +
//! row-cap sweep that bounds them.
//!
//! Split out of `documents/mod.rs` (R8's hard LOC cap); everything here moved
//! verbatim. Both caches are pure derived data: losing a row costs a recompute,
//! never user content.

use rusqlite::params;

use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector};
use crate::db::{now_ms, ts_to_db};
use crate::error::AppResult;

use super::sql::{
    get_match_score_with_conn, prune_due, prune_table_locked, upsert_match_score_with_conn,
};
use super::{ttl_cutoff_ms, DocumentStore};

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

impl DocumentStore {
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
        self.exec(
            "DELETE FROM posting_vectors WHERE job_id = ?1",
            params![job_id],
        )
    }

    /// Drop the entire posting-vector cache (e.g. on embedding-config change).
    pub fn clear_posting_vectors(&self) -> AppResult<()> {
        self.exec("DELETE FROM posting_vectors", [])
    }

    /// Bound EVERY derived cache table: expire rows older than `ttl_secs` and
    /// cap each to the newest `max_rows`. `None` for a knob disables that bound
    /// (today's unbounded behavior). Best-effort — a failed prune never blocks
    /// the caller. Pure of its inputs (does not read the live global), so the
    /// command can pass the exact tier it just applied. Unlike the amortized
    /// per-write prune, this one always runs: its caller is the settings change.
    pub fn prune_caches(&self, ttl_secs: Option<i64>, max_rows: Option<i64>) {
        let conn = self.conn.lock();
        prune_table_locked(&conn, "posting_vectors", ttl_secs, max_rows);
        prune_table_locked(&conn, "match_scores", ttl_secs, max_rows);
        // `help_vectors` is swept on the same tier, for the same reason: its
        // producer (`commands::help`) takes its entries from the REQUEST, so
        // the shipped corpus does not bound the table — see its own module
        // doc. Losing a row costs one re-embed, never user content.
        prune_table_locked(&conn, "help_vectors", ttl_secs, max_rows);
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

    /// Store (or replace) the cached match-score JSON result for the given key.
    ///
    /// The amortized-prune decision is taken HERE, not in the SQL helper: the
    /// counter belongs to the store and the helper only holds a `&Connection`.
    pub fn upsert_match_score(&self, key: &MatchScoreKey, score_json: &str) -> AppResult<()> {
        let prune = prune_due(&self.match_score_writes);
        let conn = self.conn.lock();
        upsert_match_score_with_conn(&conn, key, score_json, prune)
    }

    /// Drop every cached match score computed FOR one résumé id.
    ///
    /// A `match_scores` row is résumé-derived content — its gaps,
    /// recommendations and explanation all describe that résumé — so it must
    /// die with the résumé, not at the TTL. Sibling of
    /// [`Self::delete_posting_vector`] for the other half of an Autopilot
    /// snapshot's cache footprint. Idempotent.
    pub fn delete_match_scores_for_resume(&self, resume_id: &str) -> AppResult<()> {
        self.exec(
            "DELETE FROM match_scores WHERE resume_id = ?1",
            params![resume_id],
        )
    }

    /// Drop the entire match-result cache (e.g. on embedding-config change).
    pub fn clear_match_scores(&self) -> AppResult<()> {
        self.exec("DELETE FROM match_scores", [])
    }
}

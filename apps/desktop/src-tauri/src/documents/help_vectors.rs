//! The help-corpus vector cache (`help_vectors`) — one row per help ENTRY
//! TEXT, keyed by `sha256_hex(body)`.
//!
//! Split out of `documents/mod.rs` for the same reason `documents/sql.rs` and
//! `documents/embedding.rs` were (R8's hard LOC cap), not because it is a
//! second store: every query below runs on the SAME `DocumentStore`
//! connection, inside the same domain store, and the table is created by the
//! same `DocumentStore::MIGRATIONS` list.
//!
//! **Keyed by text hash, never by entry id or locale.** The corpus lives in
//! the translation bundles (see `commands::help`), so the same answer reaches
//! Rust under a `<section>Questions.<leaf>` id that could be renamed, in
//! whichever locale the user has active. Hashing the BODY makes the cache
//! locale-agnostic *and* self-invalidating: an edited answer is a different
//! hash, so it is a natural miss and re-embeds itself, while an unchanged
//! answer costs at most ONE embed per embedding space **once the cache is
//! warm**.
//!
//! **The concurrency caveat, stated rather than engineered away.** That is a
//! steady-state claim, not an exactly-once guarantee. There is no in-flight
//! registry, and the connection lock is taken separately by
//! [`DocumentStore::get_help_vector`] and
//! [`DocumentStore::upsert_help_vector`] (never held across the embed, which
//! is a network round trip), so two `help_search` calls racing on a COLD
//! entry each see a miss and each embed it; the upserts are idempotent
//! (`ON CONFLICT(text_hash) DO UPDATE`), so the last writer simply wins and
//! the cache CONVERGES after the first one lands. A registry would trade
//! those few duplicate embeds for a cross-request lock on a degradable arm,
//! which is the worse deal: the duplicates are bounded on both axes that
//! matter — each racing call is capped at `commands::help`'s
//! `HELP_EMBED_MISSES_MAX` misses, and every embed is charged against the
//! provider's daily ceiling (`limits::PROVIDER_DAILY_MAX`) before it is
//! dispatched, concurrent or not.
//!
//! **What actually bounds this table** (corrected — an earlier version of
//! this doc claimed the SHIPPED corpus did, which is wrong: `commands::help`
//! embeds the entries of the REQUEST, and a `help_search` is reachable from
//! the agent CLI and the extension bridge with a hand-written body, so the
//! corpus the app ships bounds nothing here):
//!
//! 1. **Per request** — `commands::help::HELP_EMBED_MISSES_MAX` cache-miss
//!    embeds, so one call can add at most that many rows.
//! 2. **Over time** — [`DocumentStore::prune_caches`]' TTL + row-cap sweep,
//!    the same one `posting_vectors` and `match_scores` are on. Rows may
//!    exceed the cap between sweeps; that is the bound every cache table here
//!    carries.
//! 3. **Whole-table** — `clear_help_vectors` (the embedding-space change) and
//!    `clear_all` (factory reset).
//!
//! Under normal use it stays small (~51 entries per locale, a few hundred
//! rows at the extreme), and a pruned row costs one re-embed, never user
//! content.

use rusqlite::params;

use super::{DocumentStore, EmbeddingConfig};
use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector};
use crate::db::{now_ms, ts_to_db};
use crate::error::AppResult;

impl DocumentStore {
    /// The cached vector for `text_hash`, or `None` on a miss.
    ///
    /// The embedding-space check lives HERE, against the caller's `active`
    /// snapshot, rather than being left to each call site: a vector produced
    /// by a different provider/model — or by an older
    /// [`crate::commands::ai_provider::EMBEDDING_VECTOR_VERSION`] format —
    /// is not comparable to a query vector from the current space, and a
    /// cache that returned it anyway would silently score across spaces.
    /// `active` is a PARAMETER, not a fresh `self.embedding_config()` read,
    /// so the hit decision is made against the exact same snapshot the
    /// caller charged and dispatched with (the #1087 rule — see
    /// `documents::embedding::embed_with_config`).
    pub fn get_help_vector(
        &self,
        text_hash: &str,
        active: &EmbeddingConfig,
    ) -> Option<EmbeddingVector> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT vector, provider, model, dim, version FROM help_vectors WHERE text_hash = ?1",
            params![text_hash],
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
            let space = EmbeddingSpace {
                provider,
                model,
                dim: dim as usize,
                // Persisted at write time, never re-derived here — same
                // reasoning as `get_posting_vector`'s own note.
                version,
            };
            // A row from another provider/model, or in an older vector
            // FORMAT, is a miss — not a hit the caller has to re-check.
            if !active.matches(&space) {
                return None;
            }
            Some(EmbeddingVector { values, space })
        })
    }

    /// Store (or replace) the cached vector for `text_hash`, tagged with the
    /// embedding space that produced it. Same storage shape as
    /// `upsert_posting_vector`: the values as a JSON array of `f64` plus the
    /// space's provider/model/dim/version as their own columns, so a read can
    /// decide comparability without deserialising the vector first.
    pub fn upsert_help_vector(&self, text_hash: &str, v: &EmbeddingVector) -> AppResult<()> {
        let json = serde_json::to_string(&v.values).map_err(|e| e.to_string())?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO help_vectors (text_hash, provider, model, dim, version, vector, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(text_hash) DO UPDATE SET
                provider = excluded.provider, model = excluded.model,
                dim = excluded.dim, version = excluded.version,
                vector = excluded.vector, created_at = excluded.created_at",
            params![
                text_hash,
                v.space.provider,
                v.space.model,
                v.space.dim as i64,
                v.space.version,
                json,
                ts_to_db(now_ms()),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Drop the whole help-vector cache — the embedding space changed, so
    /// every row in it is unreachable by [`Self::get_help_vector`] anyway.
    /// Called from `ai_set_embedding_config`'s `space_changed` branch next to
    /// `clear_posting_vectors`/`clear_match_scores`, for the same reason:
    /// a space flip with no read in between would otherwise leave the rows
    /// sitting there indefinitely.
    pub fn clear_help_vectors(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM help_vectors", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

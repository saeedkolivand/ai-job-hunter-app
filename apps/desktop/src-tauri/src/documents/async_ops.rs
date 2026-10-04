//! The async variants of the hot match-path store methods.
//!
//! Split out of `documents/mod.rs` (R8's hard LOC cap); everything here moved
//! verbatim. Each runs the blocking lock + query on a `spawn_blocking` thread so
//! a 1000-job scoring batch never parks a Tokio worker on the connection mutex;
//! the SQL itself is the shared `&Connection`-bound code in `documents/sql.rs`.

use std::sync::Arc;

use crate::commands::ai_provider::EmbeddingVector;
use crate::error::AppResult;

use super::sql::{
    get_match_score_with_conn, get_vector_with_conn, prune_due, spawn_blocking_db,
    upsert_match_score_with_conn, upsert_vector_with_conn,
};
use super::{DocumentStore, OwnedMatchScoreKey};

impl DocumentStore {
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
}

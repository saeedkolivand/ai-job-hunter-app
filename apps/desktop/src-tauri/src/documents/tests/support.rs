//! Fixtures shared by the `documents` store tests.

use super::*;

/// A fresh store in a temp dir. Hold the guard for as long as the store is used.
pub(super) fn open_store() -> (TempDir, DocumentStore) {
    let dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&dir.path().to_path_buf()).unwrap();
    (dir, store)
}

/// A vector tagged with an arbitrary OTHER embedding space.
pub(super) fn ev_in(provider: &str, model: &str, values: Vec<f64>) -> EmbeddingVector {
    let dim = values.len();
    EmbeddingVector {
        values,
        space: EmbeddingSpace {
            provider: provider.to_string(),
            model: model.to_string(),
            dim,
            version: EMBEDDING_VECTOR_VERSION,
        },
    }
}

/// Build a space-tagged vector for the default (Ollama/nomic) space in tests.
pub(super) fn ev(values: Vec<f64>) -> EmbeddingVector {
    ev_in("ollama", "nomic-embed-text", values)
}

/// The default (Ollama/nomic) embedding config — the space `ev` builds vectors in.
pub(super) fn cfg_ollama() -> EmbeddingConfig {
    EmbeddingConfig {
        provider: "ollama".to_string(),
        model: "nomic-embed-text".to_string(),
        base_url: None,
    }
}

/// A plain, un-indexed, non-default `Resume` / `resume.pdf` record created now;
/// every field a test cares about is an explicit `..record(..)` override.
pub(super) fn record(id: &str, text: &str) -> DocumentRecord {
    DocumentRecord {
        id: id.to_string(),
        title: "Resume".to_string(),
        name: "resume.pdf".to_string(),
        locale: None,
        text: text.to_string(),
        pages: None,
        created_at: now_ms(),
        indexed: false,
        is_default: false,
        keywords_json: None,
    }
}

// `PerformanceConfig` lives in a process-global `OnceLock<ArcSwap>`. A test that depends on it sets
// it explicitly first, then restores the balanced default after so it cannot bleed into the others;
// every such test is `#[serial]`.

pub(super) fn set_perf(ttl_secs: Option<i64>, max_rows: Option<i64>) {
    crate::performance::set(crate::performance::PerformanceConfig {
        keep_alive_secs: 300,
        cache_ttl_secs: ttl_secs,
        cache_max_rows: max_rows,
    });
}

pub(super) fn reset_perf_to_balanced() {
    crate::performance::set(crate::performance::PerformanceConfig::default());
}

// Count rows in a table via a raw SQL query.
pub(super) fn count_table(store: &DocumentStore, table: &str) -> i64 {
    let conn = store.conn.lock();
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap_or(0)
}

/// Rows of `table` whose `col` equals `val`.
pub(super) fn count_where(store: &DocumentStore, table: &str, col: &str, val: &str) -> i64 {
    let conn = store.conn.lock();
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE {col} = ?1"),
        params![val],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

/// Insert one `match_scores` row straight into the table, bypassing the store, so a test
/// controls `created_at` and spends none of the store's write counter.
pub(super) fn seed_match_score(
    store: &DocumentStore,
    job_id: &str,
    hash: &str,
    score_json: &str,
    created_at_ms: u64,
) {
    let conn = store.conn.lock();
    conn.execute(
        "INSERT OR REPLACE INTO match_scores
         (resume_id, job_id, provider, model, semantic_enabled, formula_version,
          vector_version, job_text_hash, score_json, created_at)
         VALUES ('r', ?1, 'ollama', 'nomic-embed-text', 1, 1, 1, ?2, ?3, ?4)",
        params![job_id, hash, score_json, ts_to_db(created_at_ms)],
    )
    .unwrap();
}

/// Insert one `posting_vectors` row straight into the table (same reason as
/// [`seed_match_score`]).
pub(super) fn seed_posting_vector(
    store: &DocumentStore,
    job_id: &str,
    hash: &str,
    v: &EmbeddingVector,
    created_at_ms: u64,
) {
    let json = serde_json::to_string(&v.values).unwrap();
    let conn = store.conn.lock();
    conn.execute(
        "INSERT OR REPLACE INTO posting_vectors
         (job_id, text_hash, vector, provider, model, dim, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            job_id,
            hash,
            json,
            v.space.provider,
            v.space.model,
            v.space.dim as i64,
            ts_to_db(created_at_ms),
        ],
    )
    .unwrap();
}

// ── Match-result cache keys ─────────────────────────────────────────────────

pub(super) fn match_key<'a>(
    resume_id: &'a str,
    job_id: &'a str,
    semantic_enabled: i64,
    formula_version: i64,
    job_text_hash: &'a str,
) -> MatchScoreKey<'a> {
    MatchScoreKey {
        resume_id,
        job_id,
        provider: "ollama",
        model: "nomic-embed-text",
        semantic_enabled,
        formula_version,
        vector_version: 1,
        job_text_hash,
    }
}

/// Like [`match_key`] but with the embedding space (provider/model) parameterized,
/// so tests can vary the space axis of the cache PK.
pub(super) fn match_key_in_space<'a>(
    resume_id: &'a str,
    job_id: &'a str,
    provider: &'a str,
    model: &'a str,
    semantic_enabled: i64,
    formula_version: i64,
    job_text_hash: &'a str,
) -> MatchScoreKey<'a> {
    MatchScoreKey {
        resume_id,
        job_id,
        provider,
        model,
        semantic_enabled,
        formula_version,
        vector_version: 1,
        job_text_hash,
    }
}

// ── Migration wiring ────────────────────────────────────────────────────────

/// A temp dir plus a connection to its `documents.db`, migrated up to JUST BEFORE the
/// migration named `name` — an install created on an older app version. Looked up BY NAME
/// rather than "all but the last" so it stays correct wherever later migrations get appended.
/// Returns the guard, the dir, the connection and the migration's index.
pub(super) fn migrated_before(name: &str) -> (TempDir, PathBuf, Connection, usize) {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let all = DocumentStore::MIGRATIONS;
    let idx = all
        .iter()
        .position(|m| m.name == name)
        .unwrap_or_else(|| panic!("{name} must still be registered"));
    let mut conn = crate::db::open(&dir.join("documents.db")).unwrap();
    run_migrations(&mut conn, &all[..idx]).unwrap();
    (tmp, dir, conn, idx)
}

/// `PRAGMA user_version` — how many migrations have been applied.
pub(super) fn user_version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}

/// [`migrated_before`], with `seed` writing rows through the OLD schema before the
/// connection is closed — the caller then does a REAL `DocumentStore::open(&dir)`.
pub(super) fn seeded_before(name: &str, seed: impl FnOnce(&Connection)) -> (TempDir, PathBuf) {
    let (tmp, dir, conn, _) = migrated_before(name);
    seed(&conn);
    (tmp, dir)
}

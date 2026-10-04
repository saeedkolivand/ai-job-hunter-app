//! The `documents.db` schema as a position-indexed migration list, plus the two
//! migration bodies that are named functions rather than inline closures.
//!
//! Split out of `documents/mod.rs` (R8's hard LOC cap); everything here moved
//! verbatim. `run_migrations` is position-indexed via `PRAGMA user_version`, so a
//! new migration is ALWAYS appended at the end of [`DocumentStore::MIGRATIONS`] —
//! never inserted earlier.

use rusqlite::{params, Connection};

use crate::db::{column_exists, Migration};

use super::{mojibake_repair, DocumentStore};

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
pub(super) fn alias_retired_gemini_text_embedding_004(conn: &Connection) -> rusqlite::Result<()> {
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

impl DocumentStore {
    pub(super) const MIGRATIONS: &'static [Migration] = &[
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
        Migration {
            // The help-corpus vector cache (`documents::help_vectors`). Keyed
            // by `sha256_hex(entry body)` — NOT by entry id or locale — so an
            // edited answer misses by itself and an unchanged one costs at
            // most one embed per embedding space once the cache is warm (the
            // concurrency caveat on that claim lives in `help_vectors`' own
            // module doc).
            //
            // The `created_at` index ships in the SAME migration as the table,
            // not a later one: `prune_caches` sweeps `help_vectors` on the
            // same tier as its two siblings, and `sql::prune_table_locked`'s
            // row-cap delete is WRITTEN for that index (`ORDER BY created_at
            // DESC LIMIT 1 OFFSET ?`) — without it the cap degrades to a
            // full-table sort on every pruning write. Same
            // `idx_<table>_created_at` name shape as
            // `index_cache_created_at`'s two.
            //
            // Forward-safe and safe to drop: `CREATE TABLE IF NOT EXISTS` on
            // a table no earlier migration reads, holding nothing but derived
            // data (losing it costs a re-embed, never user content). APPENDED
            // at the END — `run_migrations` is position-indexed, so a new
            // migration must never be inserted earlier in this list.
            name: "create_help_vectors",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS help_vectors (
                        text_hash  TEXT PRIMARY KEY,
                        provider   TEXT NOT NULL,
                        model      TEXT NOT NULL,
                        dim        INTEGER NOT NULL,
                        version    INTEGER NOT NULL,
                        vector     TEXT NOT NULL,
                        created_at INTEGER NOT NULL
                    );
                     CREATE INDEX IF NOT EXISTS idx_help_vectors_created_at ON help_vectors(created_at);",
                )
            },
        },
    ];
}

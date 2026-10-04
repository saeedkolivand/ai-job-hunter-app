//! `DocumentStore::MIGRATIONS` end to end: the retired-Gemini-model alias, and every migration
//! whose registration is proven through a REAL `open()` over a pre-seeded older schema.

use crate::documents::migrations::alias_retired_gemini_text_embedding_004;

use super::{support::*, *};

/// Write the persisted embedding-config row directly, bypassing `set_embedding_config` — the
/// shape the retired code path used to write.
fn force_embedding_row(store: &DocumentStore, provider: &str, model: &str) {
    store
        .conn
        .lock()
        .execute(
            "UPDATE embedding_config SET provider = ?1, model = ?2",
            params![provider, model],
        )
        .unwrap();
}

// ── alias_retired_gemini_text_embedding_004 (HIGH-1 fix) ───────────────────────
//
// Any install that persisted `text-embedding-004` before the default changed
// to `gemini-embedding-2` must self-heal on the next `open()`, not keep
// 404-ing forever.

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_rewrites_a_persisted_stale_row() {
    let (_dir, store) = open_store();

    // Simulate an install that saved the now-retired model BEFORE this fix
    // shipped (bypassing `set_embedding_config` — this is exactly the shape
    // the retired code path used to write).
    force_embedding_row(&store, "gemini", "text-embedding-004");

    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();

    let cfg = store.embedding_config();
    assert_eq!(cfg.provider, "gemini");
    assert_eq!(cfg.model, "gemini-embedding-2");
}

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_leaves_other_configs_untouched() {
    let (_dir, store) = open_store();

    // A non-Gemini provider, and a Gemini row already on the current model,
    // must both survive unchanged — the migration is WHERE-scoped to the
    // exact retired (provider, model) pair only.
    force_embedding_row(&store, "openai", "text-embedding-3-small");
    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
    let cfg = store.embedding_config();
    assert_eq!(cfg.provider, "openai");
    assert_eq!(cfg.model, "text-embedding-3-small");

    force_embedding_row(&store, "gemini", "gemini-embedding-2");
    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
    let cfg = store.embedding_config();
    assert_eq!(cfg.provider, "gemini");
    assert_eq!(cfg.model, "gemini-embedding-2");
}

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_catches_real_stored_variants() {
    // The model column is free text — the Gemini adapter itself strips a
    // leading `models/`, so that form is a real variant users' saved
    // strings can carry, not a hypothetical one. Case and surrounding
    // whitespace are also user-input noise, not signal.
    for stale_model in [
        "models/text-embedding-004",
        "TEXT-EMBEDDING-004",
        " text-embedding-004 ",
        "Models/Text-Embedding-004",
    ] {
        let (_dir, store) = open_store();
        force_embedding_row(&store, "gemini", stale_model);
        alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
        let cfg = store.embedding_config();
        assert_eq!(
            cfg.model, "gemini-embedding-2",
            "stored variant {stale_model:?} was not healed"
        );
    }
}

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_evicts_posting_and_match_caches_only_when_it_changes_something(
) {
    let (_dir, store) = open_store();

    // Seed a posting-vector row AND a match-score row — the test name claims
    // BOTH caches are evicted, so both must actually be seeded and asserted;
    // asserting only `posting_vectors` would stay green even if the
    // `DELETE FROM match_scores` half of `alias_retired_gemini_text_embedding_004`
    // were ever removed, silently leaving stale scores (computed in the
    // retired embedding space) to keep being served.
    store
        .upsert_posting_vector("job-1", &sha256_hex("job text"), &ev(vec![0.1, 0.2]))
        .unwrap();
    {
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO match_scores
             (resume_id, job_id, provider, model, semantic_enabled, formula_version,
              vector_version, job_text_hash, score_json, created_at)
             VALUES ('r', 'job-1', 'gemini', 'text-embedding-004', 1, 1, 1, ?1, '{\"score\":1}', ?2)",
            params![sha256_hex("job text"), ts_to_db(now_ms())],
        )
        .unwrap();
    }
    assert_eq!(count_table(&store, "posting_vectors"), 1);
    assert_eq!(count_table(&store, "match_scores"), 1);

    // A stale-model row that DOESN'T match the retired model must not evict.
    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
    assert_eq!(count_table(&store, "posting_vectors"), 1);
    assert_eq!(count_table(&store, "match_scores"), 1);

    // Now seed the actual stale model and re-run — this IS a real space
    // change, so it must evict, mirroring `ai_set_embedding_config`'s
    // runtime eviction for the same kind of change.
    force_embedding_row(&store, "gemini", "text-embedding-004");
    alias_retired_gemini_text_embedding_004(&store.conn.lock()).unwrap();
    assert_eq!(count_table(&store, "posting_vectors"), 0);
    assert_eq!(count_table(&store, "match_scores"), 0);
}

// ── Migration WIRING (not just the bare function) ───────────────────────────
//
// The two test groups above call `alias_retired_gemini_text_embedding_004`
// directly — they'd stay green even if its `Migration { .. }` entry were
// deleted from `DocumentStore::MIGRATIONS`. These exercise the REAL
// end-to-end path instead.

#[test]
#[serial]
fn every_registered_migration_actually_applies_on_open() {
    // Sanity check on the migration SYSTEM itself: `user_version` must land
    // exactly at the registered migration count after a fresh `open()` —
    // catches a migration silently failing to run.
    let (_dir, store) = open_store();
    let version = user_version(&store.conn.lock());
    assert_eq!(version, DocumentStore::MIGRATIONS.len() as i64);
}

#[test]
#[serial]
fn alias_retired_gemini_text_embedding_004_heals_a_pre_seeded_row_through_a_real_open() {
    // Bring a fresh DB up to JUST BEFORE the alias-fix migration (simulating
    // an install created on an older app version), seed the stale row, then
    // open it for REAL — proving the `Migration { .. }` entry is actually
    // registered and reached, not just that the function works standalone.
    // Looked up BY NAME rather than "all but the last" so this stays correct
    // regardless of where later migrations get appended in the array.
    let (_tmp, dir) = seeded_before("alias_retired_gemini_text_embedding_004", |conn| {
        conn.execute(
            "UPDATE embedding_config SET provider = 'gemini', model = 'text-embedding-004'",
            [],
        )
        .unwrap();
    });

    // A REAL open() — must run the remaining migrations, including the
    // alias-fix one.
    let store = DocumentStore::open(&dir).unwrap();
    let cfg = store.embedding_config();
    assert_eq!(cfg.provider, "gemini");
    assert_eq!(cfg.model, "gemini-embedding-2");
}

#[test]
#[serial]
fn evict_posting_vectors_for_embedding_format_v2_heals_a_pre_seeded_row_through_a_real_open() {
    // The real end-to-end path: seed a posting vector for a NON-Gemini
    // provider under an OLD DB (every migration except this one applied),
    // then open it for real — proving the migration is actually registered
    // in `MIGRATIONS` (unlike the Gemini-specific alias migration, this one
    // has no WHERE clause and must wipe the cache for EVERY provider, since
    // the chunk-and-mean-pool format change affects all of them).
    let (_tmp, dir) = seeded_before("evict_posting_vectors_for_embedding_format_v2", |conn| {
        // `posting_vectors` exists by this point (created several migrations
        // earlier) — seed a row directly via raw SQL (no `DocumentStore` yet).
        // Ollama, not Gemini — proves the WHERE-less DELETE isn't scoped.
        conn.execute(
            "INSERT INTO posting_vectors (job_id, text_hash, vector, provider, model, dim, created_at) \
             VALUES ('job-1', 'hash', '[0.1,0.2]', 'ollama', 'nomic-embed-text', 2, 0)",
            [],
        )
        .unwrap();
    });

    let store = DocumentStore::open(&dir).unwrap();
    assert_eq!(count_table(&store, "posting_vectors"), 0);
}

#[test]
#[serial]
fn add_version_to_posting_vectors_heals_a_pre_seeded_row_through_a_real_open() {
    // Bring a fresh DB up to JUST BEFORE the version-column migration
    // (simulating an install created before this fix shipped), seed a row
    // through the OLD (no `version` column) schema, then open it for REAL —
    // proving the `Migration { .. }` entry is actually registered and reached.
    // The slice below already includes `evict_posting_vectors_for_embedding_
    // format_v2` (it runs earlier in the array), so the row inserted below is
    // exactly the shape a real pre-existing row would be: written AFTER the
    // v1->v2 wipe, hence genuinely v2-native — `DEFAULT 2` must recognize
    // that instead of mislabeling it as stale and forcing a wasted re-embed.
    let (_tmp, dir) = seeded_before("add_version_to_posting_vectors", |conn| {
        // Old (pre-migration) schema has no `version` column yet. `created_at`
        // is "now" (not epoch 0) so the row survives the read-side TTL filter
        // in `get_posting_vector` below — this test is about the version
        // column, not TTL expiry.
        conn.execute(
            "INSERT INTO posting_vectors (job_id, text_hash, vector, provider, model, dim, created_at) \
             VALUES ('job-1', 'hash', '[0.1,0.2]', 'ollama', 'nomic-embed-text', 2, ?1)",
            params![ts_to_db(now_ms())],
        )
        .unwrap();
    });

    // A REAL open() — must run the remaining migrations, including the
    // version-column one, without erroring on the pre-existing row.
    let store = DocumentStore::open(&dir).unwrap();
    let (v, hash) = store
        .get_posting_vector("job-1")
        .expect("a pre-migration row must still round-trip after the column is added");
    assert_eq!(hash, "hash");
    assert_eq!(
        v.space.version, 2,
        "a pre-existing row must default to version 2 (DEFAULT 2) — it necessarily \
         post-dates the earlier v1->v2 wipe migration, so it is provably v2-native"
    );
    assert!(
        cfg_ollama().matches(&v.space),
        "a provably-current pre-existing row must HIT the cache, not be forced through \
         a wasted (billed) re-embed"
    );
}

#[test]
#[serial]
fn add_version_to_posting_vectors_migration_is_idempotent_on_reopen() {
    // `db::run_migrations` skips any migration whose index is <= the stored
    // `PRAGMA user_version` (see `db.rs`), so a PLAIN reopen never actually
    // re-invokes this migration's `up` — its `column_exists` guard would go
    // completely unexercised a second time. Roll `user_version` back to JUST
    // before this migration (looked up BY NAME, not "the last one", so this
    // stays correct regardless of what gets appended after it) so a reopen
    // makes THIS migration the one pending, genuinely re-executing the guard
    // against a schema where the `version` column already exists — proving
    // the repeated `ALTER TABLE` really is a no-op, not just that a reopen
    // is safe.
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    let store = DocumentStore::open(&dir).unwrap();
    let hash = sha256_hex("job text");
    store
        .upsert_posting_vector("job-1", &hash, &ev(vec![0.1, 0.2]))
        .unwrap();
    drop(store);

    let migration_idx = DocumentStore::MIGRATIONS
        .iter()
        .position(|m| m.name == "add_version_to_posting_vectors")
        .expect("add_version_to_posting_vectors must still be registered");
    {
        let conn = crate::db::open(&dir.join("documents.db")).unwrap();
        conn.execute_batch(&format!("PRAGMA user_version = {migration_idx}"))
            .unwrap();
    }

    let reopened = DocumentStore::open(&dir).unwrap();
    let (v, got_hash) = reopened
        .get_posting_vector("job-1")
        .expect("row must survive a reopen that re-runs this migration's guard");
    assert_eq!(got_hash, hash);
    assert_eq!(v.space.version, EMBEDDING_VECTOR_VERSION);

    // The re-run must be a true no-op on `user_version` too — it should land
    // back at exactly the full migration count, not double-advance or stall.
    let final_version = user_version(&reopened.conn.lock());
    assert_eq!(final_version, DocumentStore::MIGRATIONS.len() as i64);
}

// Real end-to-end path: seed a `match_scores` row under the OLD (7-column, no
// `vector_version`) schema on a DB with every migration except this one
// applied, then open for real. Proves the migration is actually registered in
// `MIGRATIONS` and recreates the table with `vector_version` as a real PK
// column backing it at the SQL layer — not just a field that compiles into
// the Rust struct with nothing enforcing it underneath (SQLite can't `ALTER
// TABLE` a column into an existing `PRIMARY KEY`, hence the drop+recreate).
#[test]
#[serial]
fn add_vector_version_to_match_scores_key_heals_a_pre_seeded_row_through_a_real_open() {
    let (_tmp, dir) = seeded_before("add_vector_version_to_match_scores_key", |conn| {
        // Old schema: no `vector_version` column yet.
        conn.execute(
            "INSERT INTO match_scores
                (resume_id, job_id, provider, model, semantic_enabled, formula_version,
                 job_text_hash, score_json, created_at)
             VALUES ('r', 'j', 'ollama', 'nomic-embed-text', 1, 1, 'hash', '{}', 0)",
            [],
        )
        .unwrap();
    });

    let store = DocumentStore::open(&dir).unwrap();
    // The recreated table starts empty — a pure result cache, so losing a
    // pre-migration row is safe (it just forces one recompute).
    assert_eq!(count_table(&store, "match_scores"), 0);

    // The new column is real and part of the PK: writes/reads round-trip
    // through the ordinary store API on the recreated schema.
    let key = match_key("r", "j", 1, 1, "hash");
    store.upsert_match_score(&key, "{\"combined\":1}").unwrap();
    assert!(store.get_match_score(&key).is_some());
}

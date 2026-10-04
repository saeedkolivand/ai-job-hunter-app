//! The `repair_pre_pdf_text_string_mojibake` migration and its write-path twin: the in-place repair,
//! the pre-repair snapshot, the error policy, vector eviction, and the factory-reset wipe.

use super::{support::*, *};

/// Build the exact hex-dumped corrupt byte shape used across the mojibake
/// repair tests: `- [` + doubled U+FFFD (the BOM misread as UTF-8) +
/// NUL-interleaved "aijobhunter.app" (UTF-16BE misread as UTF-8) + the
/// never-corrupted `](url)\n` suffix — see
/// `extraction::pdf::repair_utf16_mojibake`. `repeats` stretches the NUL-interleaved run for a
/// row big enough to span several SQLite overflow pages.
fn corrupt_mojibake_text_repeated(repeats: usize) -> String {
    let mut bytes = b"- [".to_vec();
    bytes.extend_from_slice(&[0xEF, 0xBF, 0xBD, 0xEF, 0xBF, 0xBD]);
    for _ in 0..repeats {
        for &b in b"aijobhunter.app" {
            bytes.push(0x00);
            bytes.push(b);
        }
    }
    bytes.extend_from_slice(b"](https://aijobhunter.app/)\n");
    String::from_utf8(bytes).unwrap()
}

fn corrupt_mojibake_text() -> String {
    corrupt_mojibake_text_repeated(1)
}

const REPAIRED_MOJIBAKE_TEXT: &str = "- [aijobhunter.app](https://aijobhunter.app/)\n";

/// A row written through the pre-repair schema (no `keywords_json` write), `indexed` as given.
fn seed_old_document(conn: &Connection, id: &str, text: &str, indexed: i64) {
    conn.execute(
        "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
         VALUES (?1, 't', 'n', 'en', ?2, 1, 0, ?3, 0)",
        params![id, text, indexed],
    )
    .unwrap();
}

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_heals_a_pre_seeded_row_through_a_real_open() {
    // Bring a fresh DB up to JUST BEFORE the repair migration (simulating an
    // install that imported a PDF before PR #955 fixed `pdf_text_string`),
    // seed a corrupt row with the EXACT byte shape hex-dumped from the live
    // `documents.db` row plus an unrelated clean row, then open it for REAL —
    // proving the `Migration { .. }` entry is actually registered and
    // reached, and that it never touches a row it shouldn't.

    let corrupt_text = corrupt_mojibake_text();
    let expected_repaired = REPAIRED_MOJIBAKE_TEXT;
    let clean_text = "Software Engineer with 5 years experience".to_string();

    let (_tmp, dir) = seeded_before("repair_pre_pdf_text_string_mojibake", |conn| {
        // Both rows carry a pre-existing `keywords_json` cache and are
        // `indexed = 1` — a real document that was already scored/embedded
        // BEFORE this repair runs. The corrupt row's cache must be
        // invalidated (it was computed from the corrupt text); the clean
        // row's must survive untouched.
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default, keywords_json)
             VALUES ('corrupt', 't', 'n', 'en', ?1, 1, 0, 1, 0, '[\"stale\"]')",
            params![corrupt_text],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default, keywords_json)
             VALUES ('clean', 't', 'n', 'en', ?1, 1, 0, 1, 0, '[\"software\",\"engineer\"]')",
            params![clean_text],
        )
        .unwrap();
    });

    // A REAL open() — must run the remaining migrations, including the repair.
    let store = DocumentStore::open(&dir).unwrap();
    let corrupt_after = store.get("corrupt").expect("corrupt row must still exist");
    let clean_after = store.get("clean").expect("clean row must still exist");

    assert_eq!(corrupt_after.text, expected_repaired);
    assert!(
        !corrupt_after.text.contains('\0') && !corrupt_after.text.contains('\u{FFFD}'),
        "repaired text must carry no NULs or replacement chars; got {:?}",
        corrupt_after.text
    );
    assert_eq!(
        corrupt_after.keywords_json, None,
        "the stale keywords cache (computed from corrupt text) must be invalidated"
    );
    assert!(
        !corrupt_after.indexed,
        "indexed must be cleared so the repaired text gets re-embedded, not left \
         pinned to a vector computed from the corrupt text"
    );
    assert_eq!(
        clean_after.text, clean_text,
        "a row with no embedded NUL must be byte-identical after the migration"
    );
    assert_eq!(
        clean_after.keywords_json,
        Some("[\"software\",\"engineer\"]".to_string()),
        "an unaffected row's cache must survive untouched"
    );
    assert!(
        clean_after.indexed,
        "an unaffected row's indexed flag must survive untouched"
    );
}

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_snapshots_the_pre_repair_value_before_rewriting() {
    // The repair is an irreversible in-place rewrite of what may be the only
    // remaining copy of the user's résumé — it must back up the exact
    // pre-repair value of every row it is about to touch, in the same
    // transaction, before overwriting it.

    let corrupt_text = corrupt_mojibake_text();

    let (_tmp, dir) = seeded_before("repair_pre_pdf_text_string_mojibake", |conn| {
        seed_old_document(conn, "corrupt", &corrupt_text, 0);
    });

    let store = DocumentStore::open(&dir).unwrap();
    let backed_up_text: String = store
        .conn
        .lock()
        .query_row(
            "SELECT text FROM documents_pre_mojibake_repair WHERE id = 'corrupt'",
            [],
            |r| r.get(0),
        )
        .expect("the backup table must contain the pre-repair row");
    assert_eq!(
        backed_up_text, corrupt_text,
        "the backup must hold the EXACT pre-repair (still-corrupt) value"
    );

    // The live row, meanwhile, must actually be repaired — the backup is a
    // safety net, not a substitute for doing the repair.
    let live_text = store.get("corrupt").unwrap().text;
    assert_ne!(live_text, corrupt_text);
}

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_skips_an_unmappable_row_without_failing_the_migration() {
    // A `text` value that ends up with BLOB storage class (SQLite is
    // dynamically typed; this can happen independently of anything this
    // migration does) fails `row.get::<_, String>`. That must be logged and
    // skipped — not silently dropped, and not allowed to fail the WHOLE
    // migration and take a normal, mappable corrupt row down with it.

    let corrupt_text = corrupt_mojibake_text();

    let (_tmp, dir) = seeded_before("repair_pre_pdf_text_string_mojibake", |conn| {
        // A NUL-bearing BLOB literal — matches the migration's `instr(...)`
        // WHERE clause, but `row.get::<_, String>` cannot map it.
        conn.execute_batch(
            "INSERT INTO documents (id, title, name, locale, text, pages, created_at, indexed, is_default)
             VALUES ('unmappable', 't', 'n', 'en', X'0000', 1, 0, 0, 0);",
        )
        .unwrap();
        seed_old_document(conn, "corrupt", &corrupt_text, 0);
    });

    // Must NOT fail overall, and must still repair the mappable row.
    let store = DocumentStore::open(&dir).unwrap();
    let repaired = store.get("corrupt").unwrap().text;
    assert_eq!(repaired, REPAIRED_MOJIBAKE_TEXT);

    // The unmappable row itself: `store.get()` can't map it either (its
    // `text` column is still BLOB, so `row.get::<_, String>` still fails),
    // so query the raw row count directly. If the migration ever started
    // DELETEing rows it can't map instead of just skipping them, this is
    // the only assertion in the suite that would catch it.
    let unmappable_row_count: i64 = store
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM documents WHERE id = 'unmappable'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        unmappable_row_count, 1,
        "a row the migration can't map must be logged and skipped, not deleted"
    );

    let version = user_version(&store.conn.lock());
    assert_eq!(
        version,
        DocumentStore::MIGRATIONS.len() as i64,
        "the unmappable row must not block the migration from completing"
    );
}

#[test]
#[serial]
fn insert_repairs_pre_pdf_text_string_mojibake_on_write() {
    // Defensive repair on the write path (not just the one-time migration):
    // restoring an old backup bundle re-inserts a corrupt row into an
    // ALREADY fully-migrated store via `insert()` — it must come out clean.
    let (_dir, store) = open_store();

    store
        .insert(&DocumentRecord {
            locale: Some("en".to_string()),
            pages: Some(1),
            ..record(&make_doc_id(), &corrupt_mojibake_text())
        })
        .unwrap();

    let stored = &store.list()[0];
    assert_eq!(stored.text, REPAIRED_MOJIBAKE_TEXT);
}

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_leaves_user_version_unadvanced_on_a_failed_row_write() {
    // Regression test for a real defect: on SQLite's "fatal" error classes
    // (SQLITE_FULL, SQLITE_IOERR, SQLITE_NOMEM, SQLITE_BUSY,
    // SQLITE_INTERRUPT — see https://www.sqlite.org/lang_transaction.html),
    // SQLite can silently roll back the WHOLE enclosing transaction and drop
    // the connection back to autocommit. If a per-row UPDATE error here were
    // logged-and-skipped instead of propagated, the very next statement (the
    // `PRAGMA user_version = N` bump in `db::run_migrations`) would then run
    // and commit on its OWN, durably advancing the version even though the
    // row was never repaired — and no future startup would ever retry it
    // (`run_migrations` skips any migration whose version is `<=
    // user_version`). Reproduced here with a REAL `max_page_count` clamp on
    // a real WAL database, not a mock: a large (multi-overflow-page) corrupt
    // row's repair is a SHRINKING update, but SQLite still needs to allocate
    // fresh overflow pages for the new payload before it can free the old
    // ones, so clamping growth to the current page count still forces
    // SQLITE_FULL on it.
    let (_tmp, _dir, mut conn, repair_idx) = migrated_before("repair_pre_pdf_text_string_mojibake");

    // A large corrupt row — big enough to span several SQLite overflow
    // pages, so even the repair's SHRINKING update still needs fresh
    // overflow pages before the old ones are freed.
    let corrupt_text = corrupt_mojibake_text_repeated(20_000);

    seed_old_document(&conn, "corrupt", &corrupt_text, 0);
    // Pre-create the (empty) backup table so the migration's own `CREATE
    // TABLE IF NOT EXISTS documents_pre_mojibake_repair AS SELECT …` is a
    // total no-op (SQLite skips evaluating the SELECT entirely when the
    // table already exists) instead of itself needing fresh pages to copy
    // this large row — isolating the LOOP'S `UPDATE` as the one write this
    // test's clamp can fail, so this test actually exercises the per-row
    // error-propagation policy under test, not the (separately-propagated,
    // already-safe) backup step.
    conn.execute_batch(
        "CREATE TABLE documents_pre_mojibake_repair (id TEXT PRIMARY KEY, text TEXT);",
    )
    .unwrap();

    // Clamp growth to exactly the current page count: any statement that
    // needs even ONE more page than this fails with SQLITE_FULL. `open()`
    // sets WAL, and `max_page_count` is per-CONNECTION — it must be set on,
    // and the migration must be re-run on, THIS SAME connection (a fresh
    // `DocumentStore::open` would get a brand new connection with the
    // default, effectively unlimited, `max_page_count`).
    let page_count: i64 = conn
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap();
    conn.pragma_update(None, "max_page_count", page_count)
        .unwrap();

    let result = run_migrations(&mut conn, DocumentStore::MIGRATIONS);
    assert!(
        result.is_err(),
        "the clamp must actually induce a write failure for this test to be meaningful"
    );

    let version = user_version(&conn);
    assert_eq!(
        version, repair_idx as i64,
        "user_version must NOT advance past a migration whose row write failed \
         — advancing it would skip the repair forever on every future startup"
    );

    let stored_text: String = conn
        .query_row("SELECT text FROM documents WHERE id = 'corrupt'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        stored_text, corrupt_text,
        "a failed UPDATE must not leave the row partially rewritten"
    );
}

#[test]
#[serial]
fn repair_pre_pdf_text_string_mojibake_evicts_the_active_space_vector_so_the_document_becomes_stale(
) {
    // Regression test for a real defect: the migration used to only flip
    // `indexed = 0`, which is a complete no-op for re-embedding —
    // `stale_documents` (`commands/ai.rs`) decides what to re-embed purely
    // from whether `get_vector` hits in the ACTIVE embedding space, and
    // never reads `indexed`. A pre-repair vector that still matches the
    // active space means the document is never picked up, so the embedding
    // stays permanently derived from the corrupt text. Asserted directly on
    // `vectors` (via `get_vector`), not on `indexed` — asserting only on
    // `indexed` is exactly what made this defect invisible the first time.

    let corrupt_text = corrupt_mojibake_text();
    let clean_text = "Software Engineer with 5 years experience".to_string();

    let (_tmp, dir) = seeded_before("repair_pre_pdf_text_string_mojibake", |conn| {
        seed_old_document(conn, "corrupt", &corrupt_text, 1);
        seed_old_document(conn, "clean", &clean_text, 1);
        // Both docs already have a vector in the CURRENT active embedding
        // space (the default: ollama/nomic-embed-text, seeded by the
        // `create_embedding_config` migration) — a real "already indexed
        // before this repair runs" document.
        conn.execute(
            "INSERT INTO vectors (doc_id, vector, provider, model, dim, version)
             VALUES ('corrupt', '[0.1,0.2]', 'ollama', 'nomic-embed-text', 2, ?1)",
            params![EMBEDDING_VECTOR_VERSION],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO vectors (doc_id, vector, provider, model, dim, version)
             VALUES ('clean', '[0.3,0.4]', 'ollama', 'nomic-embed-text', 2, ?1)",
            params![EMBEDDING_VECTOR_VERSION],
        )
        .unwrap();
    });

    // A REAL open() — must run the remaining migrations, including the repair.
    let store = DocumentStore::open(&dir).unwrap();

    assert!(
        store.get_vector("corrupt").is_none(),
        "the vector derived from the corrupt text must be evicted, or \
         `stale_documents` will never pick this document up for re-embedding"
    );
    assert!(
        store.get_vector("clean").is_some(),
        "an unaffected row's vector must survive — the repair must not evict \
         embeddings for documents whose text was never touched"
    );
}

#[test]
fn import_of_a_repaired_row_does_not_restore_a_vector_derived_from_the_corrupt_text() {
    // The write-path half of the same defect class: a backup bundle
    // exported before PR #955 carries BOTH the corrupt text AND the vector
    // computed from it. `import` -> `insert` repairs the text; restoring
    // the bundle's own vector right afterward would silently reintroduce
    // exactly the stale-vector problem the migration exists to fix.
    let (_dir, store) = open_store();

    let bundle = serde_json::json!([
        {
            "_id": "corrupt",
            "title": "Resume",
            "name": "resume.pdf",
            "text": corrupt_mojibake_text(),
            "createdAt": 1,
            "indexed": true,
            "isDefault": true,
            "vector": [0.1, 0.2],
            "vectorSpace": { "provider": "ollama", "model": "nomic-embed-text", "dim": 2 }
        },
        {
            "_id": "clean",
            "title": "Resume 2",
            "name": "resume2.pdf",
            "text": "Software Engineer with 5 years experience",
            "createdAt": 2,
            "indexed": true,
            "isDefault": false,
            "vector": [0.3, 0.4],
            "vectorSpace": { "provider": "ollama", "model": "nomic-embed-text", "dim": 2 }
        }
    ]);
    let n = crate::data_store::DataStore::import(&store, &bundle).unwrap();
    assert_eq!(n, 2);

    assert_eq!(
        store.get("corrupt").unwrap().text,
        REPAIRED_MOJIBAKE_TEXT,
        "the text must still be repaired on import"
    );
    assert!(
        store.get_vector("corrupt").is_none(),
        "the bundle's vector (derived from the corrupt text) must not be restored \
         for a row whose text was actually repaired"
    );
    assert!(
        store.get_vector("clean").is_some(),
        "a row whose text was NOT changed must keep its restored vector — \
         the fix must not evict embeddings for clean documents"
    );
}

// The mojibake-repair migration snapshots the pre-repair (still-corrupt)
// résumé text into `documents_pre_mojibake_repair` before rewriting it in
// place (see `mojibake_repair::up`). That snapshot is the user's ORIGINAL
// document text at rest — a full "erase my data" reset must drop the table,
// not just leave an empty shell behind.
#[test]
#[serial]
fn test_clear_all_drops_the_mojibake_repair_snapshot_table() {
    let corrupt_text = corrupt_mojibake_text();

    let (_tmp, dir) = seeded_before("repair_pre_pdf_text_string_mojibake", |conn| {
        seed_old_document(conn, "corrupt", &corrupt_text, 0);
    });

    // A REAL open() — must run the remaining migrations, populating the
    // snapshot table.
    let store = DocumentStore::open(&dir).unwrap();
    let snapshot_rows: i64 = store
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM documents_pre_mojibake_repair",
            [],
            |r| r.get(0),
        )
        .expect("the migration must have created and populated the snapshot table");
    assert_eq!(snapshot_rows, 1, "snapshot must hold the pre-repair row");

    store.clear_all();

    let table_exists: i64 = store
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'documents_pre_mojibake_repair'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        table_exists, 0,
        "clear_all() must DROP the mojibake snapshot table (not just empty it) — \
         it holds the user's original, un-repaired document text"
    );
}

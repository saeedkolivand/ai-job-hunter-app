use super::{support::*, *};

/// Bring a fresh DB up to JUST BEFORE the repair migration (simulating an
/// install that generated a résumé/cover letter from a PDF import before PR
/// #955 fixed `pdf_text_string`), seed a corrupt row with the EXACT byte
/// shape hex-dumped from a live `ai_generations` row (in BOTH `resume_text`
/// and `cover_letter_text`) plus an unrelated clean row, then open it for
/// REAL — proving the `Migration { .. }` entry is actually registered and
/// reached, and that it never touches a row it shouldn't.
#[test]
fn repair_pre_pdf_text_string_mojibake_heals_a_pre_seeded_row_through_a_real_open() {
    let dir = TempDir::new().unwrap();

    let corrupt_text = corrupt_mojibake_text();
    let expected_repaired = REPAIRED_MOJIBAKE_TEXT;
    let clean_cover_letter = "Dear Hiring Manager, I'm excited to apply.".to_string();

    {
        let conn = conn_before_repair(&dir);
        conn.execute(
            "INSERT INTO ai_generations (id, created_at, resume_text, cover_letter_text)
             VALUES ('corrupt', 0, ?1, ?1)",
            params![corrupt_text],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ai_generations (id, created_at, resume_text, cover_letter_text)
             VALUES ('clean', 0, 'resume text', ?1)",
            params![clean_cover_letter],
        )
        .unwrap();
    }

    // A REAL open() — must run the remaining migrations, including the repair.
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let list = store.list();
    let corrupt = list.iter().find(|r| r.id == "corrupt").unwrap();
    let clean = list.iter().find(|r| r.id == "clean").unwrap();

    assert_eq!(corrupt.resume_text, expected_repaired);
    assert_eq!(corrupt.cover_letter_text, expected_repaired);
    assert!(
        !corrupt.resume_text.contains('\0') && !corrupt.cover_letter_text.contains('\0'),
        "repaired text must carry no NULs; got resume={:?} cover={:?}",
        corrupt.resume_text,
        corrupt.cover_letter_text
    );
    assert_eq!(
        clean.resume_text, "resume text",
        "a row with no embedded NUL must be byte-identical after the migration"
    );
    assert_eq!(
        clean.cover_letter_text, clean_cover_letter,
        "a row with no embedded NUL must be byte-identical after the migration"
    );
}

/// [`conn_before`] the repair migration.
fn conn_before_repair(dir: &TempDir) -> Connection {
    conn_before(dir, "repair_pre_pdf_text_string_mojibake").0
}

/// A store opened for REAL over a DB frozen just before the repair migration holding one
/// corrupt row (`'corrupt'`, with a clean cover letter), so `open` runs the repair and
/// takes the pre-repair snapshot. Also returns the corrupt text for comparison.
fn open_over_a_corrupt_row() -> (TempDir, AiGenerationStore, String) {
    let dir = TempDir::new().unwrap();
    let corrupt_text = corrupt_mojibake_text();
    conn_before_repair(&dir)
        .execute(
            "INSERT INTO ai_generations (id, created_at, resume_text, cover_letter_text)
             VALUES ('corrupt', 0, ?1, 'clean cover letter')",
            params![corrupt_text],
        )
        .unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    (dir, store, corrupt_text)
}

/// Build the exact hex-dumped corrupt byte shape used across the mojibake
/// repair tests: `- [` + doubled U+FFFD (the BOM misread as UTF-8) +
/// NUL-interleaved "aijobhunter.app" (UTF-16BE misread as UTF-8) + the
/// never-corrupted `](url)\n` suffix.
fn corrupt_mojibake_text() -> String {
    corrupt_mojibake_text_repeated(1)
}

/// [`corrupt_mojibake_text`] with the NUL-interleaved run repeated `repeats` times — a
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

const REPAIRED_MOJIBAKE_TEXT: &str = "- [aijobhunter.app](https://aijobhunter.app/)\n";

#[test]
fn repair_pre_pdf_text_string_mojibake_snapshots_the_pre_repair_value_before_rewriting() {
    // An irreversible in-place rewrite of what may be the only remaining
    // copy of the user's résumé/cover letter needs a safety net.
    let (_dir, store, corrupt_text) = open_over_a_corrupt_row();
    let (backed_up_resume, backed_up_cover): (String, String) = store
        .conn
        .lock()
        .query_row(
            "SELECT resume_text, cover_letter_text FROM ai_generations_pre_mojibake_repair \
             WHERE id = 'corrupt'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the backup table must contain the pre-repair row");
    assert_eq!(backed_up_resume, corrupt_text);
    assert_eq!(backed_up_cover, "clean cover letter");
}

// The mojibake-repair migration snapshots the pre-repair (still-corrupt)
// résumé/cover-letter text into `ai_generations_pre_mojibake_repair` before
// rewriting it in place. That snapshot is the user's ORIGINAL document text
// at rest — a full "erase my data" reset must drop the table, not just leave
// an empty shell behind.
#[test]
fn clear_all_drops_the_mojibake_repair_snapshot_table() {
    // A REAL open() — must run the remaining migrations, populating the
    // snapshot table.
    let (_dir, store, _corrupt_text) = open_over_a_corrupt_row();
    let snapshot_rows: i64 = store
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM ai_generations_pre_mojibake_repair",
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
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ai_generations_pre_mojibake_repair'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        table_exists, 0,
        "clear_all() must DROP the mojibake snapshot table (not just empty it) — \
         it holds the user's original, un-repaired résumé/cover-letter text"
    );
}

#[test]
fn repair_pre_pdf_text_string_mojibake_skips_an_unmappable_row_without_failing_the_migration() {
    // A `resume_text`/`cover_letter_text` value that ends up with BLOB
    // storage class (SQLite is dynamically typed) fails `row.get::<_,
    // String>`. That must be logged and skipped, not allowed to fail the
    // whole migration and take a normal, mappable corrupt row down with it.
    let dir = TempDir::new().unwrap();
    let corrupt_text = corrupt_mojibake_text();
    {
        let conn = conn_before_repair(&dir);
        // A NUL-bearing BLOB literal in resume_text — matches the
        // migration's `instr(...)` WHERE clause, but `row.get::<_, String>`
        // cannot map it.
        conn.execute_batch(
            "INSERT INTO ai_generations (id, created_at, resume_text, cover_letter_text)
             VALUES ('unmappable', 0, X'0000', '');",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ai_generations (id, created_at, resume_text, cover_letter_text)
             VALUES ('corrupt', 0, ?1, '')",
            params![corrupt_text],
        )
        .unwrap();
    }

    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let list = store.list();
    let corrupt = list.iter().find(|r| r.id == "corrupt").unwrap();
    assert_eq!(corrupt.resume_text, REPAIRED_MOJIBAKE_TEXT);

    let version: i64 = store
        .conn
        .lock()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        version,
        migrations::MIGRATIONS.len() as i64,
        "the unmappable row must not block the migration from completing"
    );
}

#[test]
fn repair_pre_pdf_text_string_mojibake_leaves_user_version_unadvanced_on_a_failed_row_write() {
    // Regression test for a real defect: on SQLite's "fatal" error classes
    // (SQLITE_FULL and friends), SQLite can silently roll back the WHOLE
    // enclosing transaction and drop the connection back to autocommit — see
    // the identical, more detailed comment on the sibling `documents` test
    // of the same name. Reproduced here with a real `max_page_count` clamp
    // against a real WAL database.
    let dir = TempDir::new().unwrap();

    // A large corrupt row — big enough to span several SQLite overflow
    // pages, so even the repair's SHRINKING update still needs fresh
    // overflow pages before the old ones are freed.
    let corrupt_text = corrupt_mojibake_text_repeated(20_000);

    let (mut conn, repair_idx) = conn_before(&dir, "repair_pre_pdf_text_string_mojibake");
    conn.execute(
        "INSERT INTO ai_generations (id, created_at, resume_text, cover_letter_text)
         VALUES ('corrupt', 0, ?1, '')",
        params![corrupt_text],
    )
    .unwrap();
    // Pre-create the (empty) backup table so the migration's own `CREATE
    // TABLE IF NOT EXISTS ai_generations_pre_mojibake_repair AS SELECT …` is
    // a total no-op instead of itself needing fresh pages to copy this large
    // row — isolating the LOOP'S `UPDATE` as the one write this test's
    // clamp can fail.
    conn.execute_batch(
        "CREATE TABLE ai_generations_pre_mojibake_repair (id TEXT PRIMARY KEY, resume_text TEXT, cover_letter_text TEXT);",
    )
    .unwrap();

    let page_count: i64 = conn
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap();
    conn.pragma_update(None, "max_page_count", page_count)
        .unwrap();

    let result = crate::db::run_migrations(&mut conn, migrations::MIGRATIONS);
    assert!(
        result.is_err(),
        "the clamp must actually induce a write failure for this test to be meaningful"
    );

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        version, repair_idx as i64,
        "user_version must NOT advance past a migration whose row write failed \
         — advancing it would skip the repair forever on every future startup"
    );

    let stored_text: String = conn
        .query_row(
            "SELECT resume_text FROM ai_generations WHERE id = 'corrupt'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        stored_text, corrupt_text,
        "a failed UPDATE must not leave the row partially rewritten"
    );
}

#[test]
fn insert_repairs_pre_pdf_text_string_mojibake_on_write() {
    // Defensive repair on the write path (not just the one-time migration):
    // `save_application` calls `insert()` for a brand-new job aggregate on
    // an ALREADY fully-migrated store — it must come out clean.
    let (_dir, store) = open_store();

    let mut rec = record("g1", "https://acme.com/job/1");
    rec.resume_text = corrupt_mojibake_text();
    rec.cover_letter_text = corrupt_mojibake_text();
    store.insert(&rec).unwrap();

    let list = store.list();
    assert_eq!(list[0].resume_text, REPAIRED_MOJIBAKE_TEXT);
    assert_eq!(list[0].cover_letter_text, REPAIRED_MOJIBAKE_TEXT);
}

#[test]
fn save_application_update_path_repairs_pre_pdf_text_string_mojibake_on_write() {
    // `save_application`'s merge-upsert calls the private `update()` for an
    // EXISTING per-job aggregate — that write path needs the same repair.
    let (_dir, store) = open_store();
    insert(&store, "g1", "https://acme.com/job/1");

    let mut regenerated = record("g2", "https://acme.com/job/1");
    regenerated.resume_text = corrupt_mojibake_text();
    regenerated.cover_letter_text = corrupt_mojibake_text();
    store.save_application(regenerated).unwrap();

    let list = store.list();
    assert_eq!(
        list.len(),
        1,
        "the per-job aggregate must have merged into one row"
    );
    assert_eq!(list[0].resume_text, REPAIRED_MOJIBAKE_TEXT);
    assert_eq!(list[0].cover_letter_text, REPAIRED_MOJIBAKE_TEXT);
}

#[test]
fn import_repairs_pre_pdf_text_string_mojibake_on_restore() {
    // The specific scenario this defends against: restoring an old backup
    // bundle (exported BEFORE PR #955, still carrying the corruption —
    // `serde_json` round-trips an embedded NUL intact) into an
    // ALREADY-migrated store must not re-inject the mojibake. `import`
    // bulk-inserts via its own loop (it does not call `insert()`), so this
    // exercises that loop's repair specifically.
    let (_dir, store) = open_store();
    let corrupt_text = corrupt_mojibake_text();

    let legacy = serde_json::json!([{
        "id": "old-1",
        "createdAt": 1,
        "candidateName": "Jane",
        "jobTitle": "Engineer",
        "companyName": "Acme",
        "resumeLanguage": "en",
        "jobAdLanguage": "en",
        "targetLanguage": "en",
        "mismatch": false,
        "topRequirements": [],
        "mode": "ats",
        "resumeText": corrupt_text,
        "coverLetterText": corrupt_text,
        "jobAd": ""
    }]);
    let n = store.import(&legacy).unwrap();
    assert_eq!(n, 1);

    let list = store.list();
    assert_eq!(list[0].resume_text, REPAIRED_MOJIBAKE_TEXT);
    assert_eq!(list[0].cover_letter_text, REPAIRED_MOJIBAKE_TEXT);
}

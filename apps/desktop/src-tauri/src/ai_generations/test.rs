use super::*;
use tempfile::TempDir;

fn record(id: &str, job_url: &str) -> AiGenerationRecord {
    AiGenerationRecord {
        id: id.into(),
        created_at: now_ms(),
        candidate_name: "Jane".into(),
        job_title: "Engineer".into(),
        company_name: "Acme".into(),
        resume_language: "en".into(),
        job_ad_language: "en".into(),
        target_language: "en".into(),
        mismatch: false,
        top_requirements: vec!["rust".into()],
        mode: "ats".into(),
        resume_text: "R".into(),
        cover_letter_text: "C".into(),
        job_ad: "JD".into(),
        job_url: job_url.into(),
        board: "linkedin".into(),
        application_answers: vec![],
        company_brief: String::new(),
        interview_questions: vec![],
        email_subject: String::new(),
        email_body: String::new(),
        application_id: None,
    }
}

fn answer(id: &str) -> ApplicationAnswer {
    ApplicationAnswer {
        id: id.into(),
        question: format!("Q-{id}"),
        answer: format!("A-{id}"),
    }
}

fn interview_question(id: &str) -> InterviewQuestion {
    InterviewQuestion {
        id: id.into(),
        question: format!("Q-{id}"),
        why: format!("why-{id}"),
        audience: "recruiter".into(),
    }
}

#[test]
fn insert_round_trips_the_job_link() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store
        .insert(&record("g1", "https://acme.com/job/1"))
        .unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].job_url, "https://acme.com/job/1");
    assert_eq!(list[0].board, "linkedin");
}

#[test]
fn applied_job_urls_returns_only_non_empty_links() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store
        .insert(&record("g1", "https://acme.com/job/1"))
        .unwrap();
    store
        .insert(&record("g2", "https://acme.com/job/2"))
        .unwrap();
    store.insert(&record("g3", "")).unwrap(); // manual generation, no job link

    let urls = store.applied_job_urls();
    assert_eq!(urls.len(), 2);
    assert!(urls.contains("https://acme.com/job/1"));
    assert!(urls.contains("https://acme.com/job/2"));
    assert!(!urls.contains(""));
}

#[test]
fn migration_defaults_link_fields_for_legacy_records() {
    // A record exported before the link columns existed (no jobUrl/board) must
    // import via serde defaults, not fail.
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
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
        "resumeText": "",
        "coverLetterText": "",
        "jobAd": ""
    }]);
    let n = crate::data_store::DataStore::import(&store, &legacy).unwrap();
    assert_eq!(n, 1);
    let list = store.list();
    assert_eq!(list[0].job_url, "");
    assert_eq!(list[0].board, "");
    assert!(list[0].application_answers.is_empty());
    assert_eq!(list[0].company_brief, "");
    assert!(list[0].interview_questions.is_empty());
    assert_eq!(list[0].email_subject, "");
    assert_eq!(list[0].email_body, "");
}

/// A backup round-trip must carry the email draft — otherwise restoring a
/// backup silently drops the user's saved application email.
#[test]
fn export_import_round_trip_preserves_the_email_draft() {
    let src_dir = TempDir::new().unwrap();
    let src = AiGenerationStore::open(&src_dir.path().to_path_buf()).unwrap();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.email_subject = "Application: Engineer".into();
    rec.email_body = "Hello,\n\nI'd like to apply.".into();
    src.insert(&rec).unwrap();

    let exported = crate::data_store::DataStore::export(&src);

    let dst_dir = TempDir::new().unwrap();
    let dst = AiGenerationStore::open(&dst_dir.path().to_path_buf()).unwrap();
    assert_eq!(
        crate::data_store::DataStore::import(&dst, &exported).unwrap(),
        1
    );

    let list = dst.list();
    assert_eq!(list[0].email_subject, "Application: Engineer");
    assert_eq!(list[0].email_body, "Hello,\n\nI'd like to apply.");
}

#[test]
fn insert_round_trips_answers_and_brief() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.application_answers = vec![answer("why-company"), answer("strengths")];
    rec.company_brief = "Acme builds payment rails.".into();
    store.insert(&rec).unwrap();

    let list = store.list();
    assert_eq!(list[0].application_answers, rec.application_answers);
    assert_eq!(list[0].company_brief, "Acme builds payment rails.");
}

#[test]
fn insert_round_trips_interview_questions() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.interview_questions = vec![interview_question("iq-1"), interview_question("iq-2")];
    store.insert(&rec).unwrap();

    let list = store.list();
    assert_eq!(list[0].interview_questions, rec.interview_questions);
}

#[test]
fn insert_round_trips_the_email_draft() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.email_subject = "Application: Engineer".into();
    rec.email_body = "Hello,\n\nI'd like to apply.".into();
    store.insert(&rec).unwrap();

    let list = store.list();
    assert_eq!(list[0].email_subject, "Application: Engineer");
    assert_eq!(list[0].email_body, "Hello,\n\nI'd like to apply.");
}

/// Re-opening the same data dir must re-run `run_migrations` as a no-op: the
/// `add_email_draft` `ALTER TABLE` would fail with "duplicate column" if the
/// `PRAGMA user_version` guard did not skip it, and any row written before the
/// re-open must still be readable through the new projection.
#[test]
fn reopening_the_store_reapplies_no_migration_and_keeps_email_data() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    {
        let store = AiGenerationStore::open(&path).unwrap();
        let mut rec = record("g1", "https://acme.com/job/1");
        rec.email_subject = "S".into();
        rec.email_body = "B".into();
        store.insert(&rec).unwrap();
    }

    // Second open on the SAME file — migrations must be skipped, not replayed.
    let reopened = AiGenerationStore::open(&path).unwrap();
    let list = reopened.list();
    assert_eq!(list.len(), 1, "the prior row must survive the re-open");
    assert_eq!(list[0].email_subject, "S");
    assert_eq!(list[0].email_body, "B");
}

/// A DB created before `add_email_draft` (schema at the previous migration) must
/// gain the columns with '' defaults for every existing row, not lose data.
#[test]
fn add_email_draft_migration_backfills_legacy_rows_with_empty_strings() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ai_generations.db");
    {
        // Build the store at the PREVIOUS schema version by running every
        // migration up to (not including) `add_email_draft`. Looked up BY
        // NAME rather than "all but the last" so this stays correct
        // regardless of what gets appended after it.
        let all = AiGenerationStore::MIGRATIONS;
        let add_email_draft_idx = all
            .iter()
            .position(|m| m.name == "add_email_draft")
            .expect("add_email_draft must still be registered");
        let mut conn = crate::db::open(&path).unwrap();
        crate::db::run_migrations(&mut conn, &all[..add_email_draft_idx]).unwrap();
        assert!(
            !crate::db::column_exists(&conn, "ai_generations", "email_subject"),
            "precondition: the legacy schema has no email columns"
        );
        conn.execute(
            "INSERT INTO ai_generations (id, created_at, resume_text, cover_letter_text)
             VALUES ('legacy', 1000, 'R', 'C')",
            [],
        )
        .unwrap();
    }

    // Opening with the full migration list applies `add_email_draft` in place.
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let list = store.list();
    assert_eq!(list.len(), 1, "the legacy row must survive the migration");
    assert_eq!(list[0].id, "legacy");
    assert_eq!(list[0].resume_text, "R", "existing data is untouched");
    assert_eq!(list[0].email_subject, "", "new column defaults to empty");
    assert_eq!(list[0].email_body, "");
}

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
    let path = dir.path().join("ai_generations.db");

    let all = AiGenerationStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");

    // "- [" + doubled U+FFFD (the BOM misread as UTF-8) + NUL-interleaved
    // "aijobhunter.app" (UTF-16BE misread as UTF-8) + the never-corrupted
    // "](url)\n" suffix — see `extraction::pdf::repair_utf16_mojibake`.
    let mut corrupt_bytes = b"- [".to_vec();
    corrupt_bytes.extend_from_slice(&[0xEF, 0xBF, 0xBD, 0xEF, 0xBF, 0xBD]);
    for &b in b"aijobhunter.app" {
        corrupt_bytes.push(0x00);
        corrupt_bytes.push(b);
    }
    corrupt_bytes.extend_from_slice(b"](https://aijobhunter.app/)\n");
    let corrupt_text = String::from_utf8(corrupt_bytes).unwrap();
    let expected_repaired = "- [aijobhunter.app](https://aijobhunter.app/)\n";
    let clean_cover_letter = "Dear Hiring Manager, I'm excited to apply.".to_string();

    {
        let mut conn = crate::db::open(&path).unwrap();
        crate::db::run_migrations(&mut conn, &all[..repair_idx]).unwrap();
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

/// Build the exact hex-dumped corrupt byte shape used across the mojibake
/// repair tests: `- [` + doubled U+FFFD (the BOM misread as UTF-8) +
/// NUL-interleaved "aijobhunter.app" (UTF-16BE misread as UTF-8) + the
/// never-corrupted `](url)\n` suffix.
fn corrupt_mojibake_text() -> String {
    let mut bytes = b"- [".to_vec();
    bytes.extend_from_slice(&[0xEF, 0xBF, 0xBD, 0xEF, 0xBF, 0xBD]);
    for &b in b"aijobhunter.app" {
        bytes.push(0x00);
        bytes.push(b);
    }
    bytes.extend_from_slice(b"](https://aijobhunter.app/)\n");
    String::from_utf8(bytes).unwrap()
}

const REPAIRED_MOJIBAKE_TEXT: &str = "- [aijobhunter.app](https://aijobhunter.app/)\n";

#[test]
fn repair_pre_pdf_text_string_mojibake_snapshots_the_pre_repair_value_before_rewriting() {
    // An irreversible in-place rewrite of what may be the only remaining
    // copy of the user's résumé/cover letter needs a safety net.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ai_generations.db");
    let corrupt_text = corrupt_mojibake_text();

    let all = AiGenerationStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");
    {
        let mut conn = crate::db::open(&path).unwrap();
        crate::db::run_migrations(&mut conn, &all[..repair_idx]).unwrap();
        conn.execute(
            "INSERT INTO ai_generations (id, created_at, resume_text, cover_letter_text)
             VALUES ('corrupt', 0, ?1, 'clean cover letter')",
            params![corrupt_text],
        )
        .unwrap();
    }

    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
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

#[test]
fn repair_pre_pdf_text_string_mojibake_skips_an_unmappable_row_without_failing_the_migration() {
    // A `resume_text`/`cover_letter_text` value that ends up with BLOB
    // storage class (SQLite is dynamically typed) fails `row.get::<_,
    // String>`. That must be logged and skipped, not allowed to fail the
    // whole migration and take a normal, mappable corrupt row down with it.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ai_generations.db");
    let corrupt_text = corrupt_mojibake_text();

    let all = AiGenerationStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");
    {
        let mut conn = crate::db::open(&path).unwrap();
        crate::db::run_migrations(&mut conn, &all[..repair_idx]).unwrap();
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
        AiGenerationStore::MIGRATIONS.len() as i64,
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
    let path = dir.path().join("ai_generations.db");

    let all = AiGenerationStore::MIGRATIONS;
    let repair_idx = all
        .iter()
        .position(|m| m.name == "repair_pre_pdf_text_string_mojibake")
        .expect("repair_pre_pdf_text_string_mojibake must still be registered");

    // A large corrupt row — big enough to span several SQLite overflow
    // pages, so even the repair's SHRINKING update still needs fresh
    // overflow pages before the old ones are freed.
    let mut corrupt_bytes = b"- [".to_vec();
    corrupt_bytes.extend_from_slice(&[0xEF, 0xBF, 0xBD, 0xEF, 0xBF, 0xBD]);
    for _ in 0..20_000 {
        for &b in b"aijobhunter.app" {
            corrupt_bytes.push(0x00);
            corrupt_bytes.push(b);
        }
    }
    corrupt_bytes.extend_from_slice(b"](https://aijobhunter.app/)\n");
    let corrupt_text = String::from_utf8(corrupt_bytes).unwrap();

    let mut conn = crate::db::open(&path).unwrap();
    crate::db::run_migrations(&mut conn, &all[..repair_idx]).unwrap();
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

    let result = crate::db::run_migrations(&mut conn, all);
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
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

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
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store
        .insert(&record("g1", "https://acme.com/job/1"))
        .unwrap();

    let mut regenerated = record("g2", "https://acme.com/job/1");
    regenerated.resume_text = corrupt_mojibake_text();
    regenerated.cover_letter_text = corrupt_mojibake_text();
    store.save_application(regenerated).unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1, "the per-job aggregate must have merged into one row");
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
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
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
    let n = crate::data_store::DataStore::import(&store, &legacy).unwrap();
    assert_eq!(n, 1);

    let list = store.list();
    assert_eq!(list[0].resume_text, REPAIRED_MOJIBAKE_TEXT);
    assert_eq!(list[0].cover_letter_text, REPAIRED_MOJIBAKE_TEXT);
}

/// The email fields merge exactly like `cover_letter_text` (`pick`): a save that
/// carries a draft OVERWRITES the stored one (so re-generating replaces it),
/// while a save from another surface leaves it alone.
#[test]
fn merge_email_draft_overwrites_on_regeneration_and_survives_other_saves() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.email_subject = "OLD SUBJECT".into();
    existing.email_body = "OLD BODY".into();

    // A regenerated email carries both fields → both are replaced.
    let mut regenerated = record("g2", "https://acme.com/job/1");
    regenerated.email_subject = "NEW SUBJECT".into();
    regenerated.email_body = "NEW BODY".into();
    let merged = merge_application(existing.clone(), regenerated);
    assert_eq!(merged.email_subject, "NEW SUBJECT");
    assert_eq!(merged.email_body, "NEW BODY");
    assert_eq!(merged.cover_letter_text, "C", "cover is untouched");

    // A résumé/answers save carries no email → the stored draft is preserved.
    let mut answers_only = record("g3", "https://acme.com/job/1");
    answers_only.application_answers = vec![answer("why-company")];
    let merged = merge_application(existing.clone(), answers_only);
    assert_eq!(merged.email_subject, "OLD SUBJECT", "draft is not wiped");
    assert_eq!(merged.email_body, "OLD BODY", "draft is not wiped");

    // Subject and body merge ATOMICALLY: a save that carries a body but an empty
    // subject owns the whole draft, so the stale subject must NOT survive glued
    // onto the new body. This is the model breaking the `Subject:` line contract
    // (the renderer's `splitEmail` then yields subject == "" and body == the raw
    // output) — the one case that actually reaches here, since the email surface
    // always writes both fields together.
    let mut preamble_output = record("g4", "https://acme.com/job/1");
    preamble_output.email_subject = String::new();
    preamble_output.email_body = "Sure! Here is your email: Hello,".into();
    let merged = merge_application(existing.clone(), preamble_output);
    assert_eq!(
        merged.email_subject, "",
        "a stale subject must not be glued onto a newly generated body"
    );
    assert_eq!(merged.email_body, "Sure! Here is your email: Hello,");

    // Mirror case: a subject with an empty body also owns the whole draft, so
    // the stale body must not survive under a newly generated subject.
    let mut subject_only = record("g5", "https://acme.com/job/1");
    subject_only.email_subject = "NEW SUBJECT".into();
    subject_only.email_body = String::new();
    let merged = merge_application(existing, subject_only);
    assert_eq!(merged.email_subject, "NEW SUBJECT");
    assert_eq!(
        merged.email_body, "",
        "a stale body must not survive under a newly generated subject"
    );
}

/// End-to-end through the store: an email save must land on the SAME aggregate
/// row the tailor flow wrote (dedupe key = normalized job_url), not fork a
/// second row, and must not disturb the résumé/cover already stored there.
#[test]
fn save_application_merges_an_email_draft_onto_the_existing_aggregate() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    // Tailor flow first: résumé + cover for the job (raw url with tracking params).
    store
        .save_application(record("g1", "https://acme.com/job/1?utm_source=indeed"))
        .unwrap();

    // Then the email tab saves its draft — no résumé/cover, different raw url.
    let mut email_save = record("g2", "https://acme.com/job/1#apply");
    email_save.resume_text = String::new();
    email_save.cover_letter_text = String::new();
    email_save.email_subject = "Application: Engineer".into();
    email_save.email_body = "Hello,".into();
    store.save_application(email_save).unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1, "one aggregate row per job");
    assert_eq!(list[0].id, "g1", "merged into the tailor-flow row");
    assert_eq!(list[0].resume_text, "R", "résumé preserved");
    assert_eq!(list[0].cover_letter_text, "C", "cover preserved");
    assert_eq!(list[0].email_subject, "Application: Engineer");
    assert_eq!(list[0].email_body, "Hello,");
}

/// The UNIQUE index is partial on `job_url != ''`, so unlinked email saves (an
/// unusable url normalizes to '') must still insert as separate rows.
#[test]
fn save_application_keeps_empty_job_url_email_saves_separate() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let mut first = record("g1", "");
    first.email_subject = "A".into();
    let mut second = record("g2", "");
    second.email_subject = "B".into();
    store.save_application(first).unwrap();
    store.save_application(second).unwrap();

    let list = store.list();
    assert_eq!(list.len(), 2, "empty-url saves must not collide");
    let subjects: std::collections::HashSet<&str> =
        list.iter().map(|r| r.email_subject.as_str()).collect();
    assert!(subjects.contains("A") && subjects.contains("B"));
}

#[test]
fn merge_layers_interview_questions_without_clobbering_other_fields() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.cover_letter_text = "COVER".into();
    existing.interview_questions = vec![];

    // An interview-questions-only save: empty résumé/cover, carries questions.
    let mut incoming = record("g2", "https://acme.com/job/1");
    incoming.resume_text = String::new();
    incoming.cover_letter_text = String::new();
    incoming.interview_questions = vec![interview_question("iq-1")];

    let merged = merge_application(existing, incoming);

    assert_eq!(merged.cover_letter_text, "COVER", "cover is not wiped");
    assert_eq!(merged.interview_questions, vec![interview_question("iq-1")]);
}

/// A corrected regeneration must be able to CLEAR a stale language-mismatch
/// warning, and a content-less (answers/interview-only) save must NOT clobber a
/// real prior verdict. The old `incoming || existing` made a `true` permanent.
#[test]
fn merge_mismatch_follows_a_language_bearing_save_and_ignores_a_content_less_one() {
    // Prior verdict: mismatch (a German résumé generated against an English ad).
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.mismatch = true;

    // A corrected regeneration carries the language pair and says "no mismatch".
    let mut fixed = record("g2", "https://acme.com/job/1");
    fixed.resume_language = "en".into();
    fixed.job_ad_language = "en".into();
    fixed.mismatch = false;
    assert!(
        !merge_application(existing.clone(), fixed).mismatch,
        "a language-bearing save with mismatch=false must clear the stale warning"
    );

    // An answers-only save carries no language pair; its default mismatch=false
    // must not wipe the existing verdict.
    let mut answers_only = record("g3", "https://acme.com/job/1");
    answers_only.resume_language = String::new();
    answers_only.job_ad_language = String::new();
    answers_only.mismatch = false;
    assert!(
        merge_application(existing, answers_only).mismatch,
        "a content-less save must not clobber a real prior mismatch verdict"
    );
}

#[test]
fn merge_layers_answers_onto_an_existing_cover_without_clobbering() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.cover_letter_text = "COVER".into();
    existing.application_answers = vec![];

    // An answers-only save: empty résumé/cover, but carries answers + brief.
    let mut incoming = record("g2", "https://acme.com/job/1");
    incoming.resume_text = String::new();
    incoming.cover_letter_text = String::new();
    incoming.application_answers = vec![answer("why-company")];
    incoming.company_brief = "brief".into();

    let merged = merge_application(existing, incoming);

    assert_eq!(merged.id, "g1", "keeps the existing row id");
    assert_eq!(merged.cover_letter_text, "COVER", "cover is not wiped");
    assert_eq!(merged.application_answers, vec![answer("why-company")]);
    assert_eq!(merged.company_brief, "brief");
}

#[test]
fn save_application_upserts_by_job_url_into_one_aggregate() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let url = "https://acme.com/job/1";

    // First the tailor flow saves a résumé/cover for the job.
    store.save_application(record("g1", url)).unwrap();

    // Then the questions assistant saves answers (no résumé/cover) for the same job.
    let mut answers_save = record("g2", url);
    answers_save.resume_text = String::new();
    answers_save.cover_letter_text = String::new();
    answers_save.application_answers = vec![answer("why-company")];
    store.save_application(answers_save).unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1, "one aggregate row per job");
    assert_eq!(list[0].cover_letter_text, "C", "cover preserved");
    assert_eq!(list[0].application_answers, vec![answer("why-company")]);
}

/// The aggregate is "one row per job", but it used to key on the RAW url, so the
/// same job reached under different tracking params (the norm on query-id boards
/// like Indeed) missed its own row and split into a second aggregate.
#[test]
fn save_application_merges_the_same_job_across_tracking_params() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    store
        .save_application(record("g1", "https://acme.com/job/1?utm_source=indeed"))
        .unwrap();
    let mut second = record("g2", "https://acme.com/job/1#apply");
    second.resume_text = String::new();
    second.cover_letter_text = String::new();
    second.application_answers = vec![answer("why-company")];
    store.save_application(second).unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1, "one aggregate row per job");
    assert_eq!(list[0].id, "g1", "merged into the first row");
    assert_eq!(list[0].cover_letter_text, "C", "cover preserved");
    assert_eq!(list[0].application_answers, vec![answer("why-company")]);
    assert_eq!(
        list[0].job_url, "https://acme.com/job/1",
        "the aggregate is keyed on the normalized url"
    );
}

/// A row written before the normalization carries its raw url; a later save must
/// still find it (and migrate it onto the normalized key) rather than fork.
#[test]
fn save_application_still_merges_a_legacy_raw_url_row() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    // Written directly, bypassing save_application's normalization.
    store
        .insert(&record("legacy", "https://acme.com/job/2?utm_source=old"))
        .unwrap();

    let mut incoming = record("g2", "https://acme.com/job/2?utm_source=old");
    incoming.application_answers = vec![answer("why-company")];
    store.save_application(incoming).unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1, "must merge, not fork off the legacy row");
    assert_eq!(list[0].id, "legacy");
    assert_eq!(
        list[0].job_url, "https://acme.com/job/2",
        "the legacy row is migrated onto the normalized key"
    );
}

#[test]
fn save_application_inserts_separate_rows_when_unlinked() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store.save_application(record("g1", "")).unwrap();
    store.save_application(record("g2", "")).unwrap();
    assert_eq!(
        store.list().len(),
        2,
        "manual (unlinked) saves stay separate"
    );
}

// ── unique-aggregate index (#816 follow-up) ────────────────────────────────────

/// The UNIQUE(job_url) partial index rejects a second row for the same non-empty
/// job — the DB-level guarantee that a concurrent double-insert can't fork the
/// aggregate. `save_application` recovers from this by merging (see the
/// concurrency test); a raw `insert` surfaces the error.
#[test]
fn unique_index_rejects_a_direct_duplicate_job_url() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let url = "https://acme.com/job/unique";
    store.insert(&record("g1", url)).unwrap();
    assert!(
        store.insert(&record("g2", url)).is_err(),
        "a second row for one non-empty job_url must be rejected"
    );
    assert_eq!(store.list().len(), 1, "only the first row persists");
}

/// The index is PARTIAL (`WHERE job_url != ''`): unusable raw urls normalize to
/// '' and are deliberately stored as separate unlinked rows, so many empty-url
/// rows must coexist without tripping the constraint.
#[test]
fn empty_job_url_rows_survive_the_unique_index() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store.insert(&record("g1", "")).unwrap();
    store.insert(&record("g2", "")).unwrap();
    store.insert(&record("g3", "")).unwrap();
    assert_eq!(store.list().len(), 3, "empty-url rows are not collapsed");
}

/// Many concurrent saves for the SAME job: each may miss `find_by_job_url` and
/// race to insert, but the UNIQUE index turns every loser's insert into a
/// conflict `save_application` recovers from by merging. Exactly one aggregate
/// survives and every call succeeds (the `.unwrap()`s).
#[test]
fn concurrent_saves_for_one_job_keep_a_single_aggregate() {
    let dir = TempDir::new().unwrap();
    let store = std::sync::Arc::new(AiGenerationStore::open(&dir.path().to_path_buf()).unwrap());
    let url = "https://acme.com/job/concurrent";

    let threads: Vec<_> = (0..6)
        .map(|i| {
            let store = store.clone();
            std::thread::spawn(move || {
                store
                    .save_application(record(&format!("g{i}"), url))
                    .unwrap();
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }

    assert_eq!(
        store.list().len(),
        1,
        "concurrent saves for one job must not fork the aggregate"
    );
}

// ── remove_many tests ─────────────────────────────────────────────────────────

#[test]
fn remove_many_deletes_subset_and_returns_count() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store.insert(&record("g1", "")).unwrap();
    store.insert(&record("g2", "")).unwrap();
    store.insert(&record("g3", "")).unwrap();

    let deleted = store.remove_many(&["g1".into(), "g3".into()]).unwrap();

    assert_eq!(deleted, 2, "should report 2 deleted rows");
    let remaining: Vec<_> = store.list().iter().map(|r| r.id.clone()).collect();
    assert_eq!(remaining, vec!["g2"], "only g2 should remain");
}

#[test]
fn remove_many_empty_input_is_noop() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store.insert(&record("g1", "")).unwrap();

    let deleted = store.remove_many(&[]).unwrap();

    assert_eq!(deleted, 0);
    assert_eq!(store.list().len(), 1, "row must not be touched");
}

// ── update_texts tests (F1 edit-before-export) ────────────────────────────────

#[test]
fn update_texts_resume_only_leaves_cover_untouched() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store.insert(&record("g1", "")).unwrap();

    store
        .update_texts("g1", Some("new resume".into()), None)
        .unwrap();

    let list = store.list();
    assert_eq!(list[0].resume_text, "new resume");
    assert_eq!(list[0].cover_letter_text, "C", "cover must be untouched");
}

#[test]
fn update_texts_cover_only_leaves_resume_untouched() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store.insert(&record("g1", "")).unwrap();

    store
        .update_texts("g1", None, Some("new cover".into()))
        .unwrap();

    let list = store.list();
    assert_eq!(list[0].resume_text, "R", "resume must be untouched");
    assert_eq!(list[0].cover_letter_text, "new cover");
}

#[test]
fn update_texts_both_fields_updates_both() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store.insert(&record("g1", "")).unwrap();

    store
        .update_texts(
            "g1",
            Some("updated resume".into()),
            Some("updated cover".into()),
        )
        .unwrap();

    let list = store.list();
    assert_eq!(list[0].resume_text, "updated resume");
    assert_eq!(list[0].cover_letter_text, "updated cover");
}

#[test]
fn update_texts_both_none_is_a_noop_and_returns_ok() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store.insert(&record("g1", "")).unwrap();

    // Both-None must succeed without issuing an UPDATE — no rows-changed path.
    store.update_texts("g1", None, None).unwrap();

    let list = store.list();
    assert_eq!(list[0].resume_text, "R", "resume unchanged");
    assert_eq!(list[0].cover_letter_text, "C", "cover unchanged");
}

#[test]
fn update_texts_unknown_id_resume_only_returns_err() {
    // (Some(resume), None) arm — rows==0 guard must surface an Err.
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let result = store.update_texts("does-not-exist", Some("x".into()), None);
    assert!(
        result.is_err(),
        "must return Err for an unknown id (resume-only arm)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("does-not-exist"),
        "error message must include the id: {msg}"
    );
}

#[test]
fn update_texts_unknown_id_cover_only_returns_err() {
    // (None, Some(cover)) arm — each arm has its own rows==0 guard.
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let result = store.update_texts("does-not-exist", None, Some("y".into()));
    assert!(
        result.is_err(),
        "must return Err for an unknown id (cover-only arm)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("does-not-exist"),
        "error message must include the id: {msg}"
    );
}

#[test]
fn update_texts_unknown_id_both_fields_returns_err() {
    // (Some(resume), Some(cover)) arm — rows==0 guard must also fire here.
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let result = store.update_texts("does-not-exist", Some("x".into()), Some("y".into()));
    assert!(
        result.is_err(),
        "must return Err for an unknown id (both-fields arm)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("does-not-exist"),
        "error message must include the id: {msg}"
    );
}

// ── R1 — Import rollback regression guard ────────────────────────────────────
//
// The C1 fix added a transaction around clear + repopulate in `DataStore::import`.
// These tests pin that fix: a malformed LATER record must abort the import and
// leave the store's PRIOR data fully intact (neither wiped nor half-restored).

/// A valid generation serialised with all required camelCase fields. Used as the
/// "good" payload in rollback tests.
fn valid_generation_json(id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "createdAt": 1_000_000u64,
        "candidateName": "Jane",
        "jobTitle": "Engineer",
        "companyName": "Acme",
        "resumeLanguage": "en",
        "jobAdLanguage": "en",
        "targetLanguage": "en",
        "mismatch": false,
        "topRequirements": ["rust"],
        "mode": "ats",
        "resumeText": "My resume",
        "coverLetterText": "My cover letter",
        "jobAd": "Job description"
    })
}

#[test]
fn import_with_invalid_later_record_returns_err_and_prior_data_intact() {
    // R1 — Build a store with one known record, then attempt an import whose
    // LAST element has an invalid type for `mismatch` (must be bool, not "bad").
    // Assert: import returns Err AND the prior record is still present.
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    // Seed the store with prior data.
    store.insert(&record("prior-1", "")).unwrap();
    assert_eq!(store.list().len(), 1, "precondition: one prior record");

    // Build a bundle: first element is valid, second is malformed (`mismatch` is
    // a string instead of a bool). The malformed record must abort the whole import.
    let bundle = serde_json::json!([
        valid_generation_json("new-1"),
        {
            "id": "bad-2",
            "createdAt": 2_000_000u64,
            "candidateName": "Jane",
            "jobTitle": "Engineer",
            "companyName": "Acme",
            "resumeLanguage": "en",
            "jobAdLanguage": "en",
            "targetLanguage": "en",
            "mismatch": "not-a-bool",   // ← wrong type: must be bool
            "topRequirements": [],
            "mode": "ats",
            "resumeText": "",
            "coverLetterText": "",
            "jobAd": ""
        }
    ]);

    let result = crate::data_store::DataStore::import(&store, &bundle);
    assert!(
        result.is_err(),
        "import of a malformed bundle must return Err; got Ok"
    );

    // Prior data must be fully intact — not wiped, not partially replaced.
    let remaining = store.list();
    assert_eq!(
        remaining.len(),
        1,
        "import rollback must leave the prior 1 record intact; got {} records",
        remaining.len()
    );
    assert_eq!(
        remaining[0].id, "prior-1",
        "the surviving record must be the original 'prior-1', not a partial import"
    );
}

#[test]
fn import_with_all_valid_records_replaces_prior_data() {
    // R1 happy-path companion: confirms the import transaction DOES commit when
    // the bundle is fully valid — prior data is replaced, not preserved.
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    store.insert(&record("prior-1", "")).unwrap();
    store.insert(&record("prior-2", "")).unwrap();
    assert_eq!(store.list().len(), 2, "precondition: two prior records");

    let bundle = serde_json::json!([
        valid_generation_json("new-1"),
        valid_generation_json("new-2"),
        valid_generation_json("new-3"),
    ]);

    let n = crate::data_store::DataStore::import(&store, &bundle).unwrap();
    assert_eq!(n, 3, "import must report 3 records restored");

    let list = store.list();
    assert_eq!(list.len(), 3, "store must now hold the 3 imported records");
    let ids: Vec<&str> = list.iter().map(|r| r.id.as_str()).collect();
    assert!(
        ids.contains(&"new-1") && ids.contains(&"new-2") && ids.contains(&"new-3"),
        "imported ids must be present; got {ids:?}"
    );
    assert!(
        !ids.contains(&"prior-1"),
        "prior record 'prior-1' must not survive a successful import"
    );
}

#[test]
fn import_non_array_returns_err_and_prior_data_intact() {
    // R1 edge-case: the top-level value is not an array at all. The store must
    // return Err before touching any data.
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    store.insert(&record("prior-1", "")).unwrap();

    let result = crate::data_store::DataStore::import(&store, &serde_json::json!({"bad": true}));
    assert!(result.is_err(), "non-array input must be rejected");

    let remaining = store.list();
    assert_eq!(
        remaining.len(),
        1,
        "prior data must be intact after non-array import rejection"
    );
    assert_eq!(remaining[0].id, "prior-1");
}

// ── Finding 7 — application_id survives export → import round-trip ────────────
//
// `application_id` is the parent Application FK. Before the fix, export/import
// dropped it, so a backup round-trip orphaned every linked generation
// (`remove_for_application` stopped matching the restored rows). This pins the
// FK through the round-trip.

#[test]
fn export_import_round_trip_preserves_application_id() {
    let app_id = "app-123";

    // Source store: one generation linked to an application (the FK that
    // `applications::ApplicationStore::open` would set via its backfill UPDATE).
    let src_dir = TempDir::new().unwrap();
    let src = AiGenerationStore::open(&src_dir.path().to_path_buf()).unwrap();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.application_id = Some(app_id.to_string());
    src.insert(&rec).unwrap();

    // Export the backup (non-destructive — reads via `list`).
    let exported = crate::data_store::DataStore::export(&src);

    // Fresh store in a NEW temp dir imports the backup.
    let dst_dir = TempDir::new().unwrap();
    let dst = AiGenerationStore::open(&dst_dir.path().to_path_buf()).unwrap();
    let n = crate::data_store::DataStore::import(&dst, &exported).unwrap();
    assert_eq!(n, 1, "one record restored");

    // The FK survived the round-trip: the restored row is still linked, so
    // `remove_for_application` matches it (== 1). Before the fix the FK was
    // dropped on export/import and this returned 0 (orphaned generation).
    assert_eq!(
        dst.remove_for_application(app_id).unwrap(),
        1,
        "application_id must survive export/import so the link still matches"
    );
}

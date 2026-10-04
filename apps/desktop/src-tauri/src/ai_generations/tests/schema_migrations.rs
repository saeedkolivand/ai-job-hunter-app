use super::{support::*, *};

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
    {
        // Build the store at the PREVIOUS schema version by running every
        // migration up to (not including) `add_email_draft`.
        let (conn, _) = conn_before(&dir, "add_email_draft");
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

/// A DB created before `add_quality_report` (schema at the previous migration)
/// must gain the column with a `''` default for every existing row, and a
/// later save must be able to merge onto that backfilled row: a content-less
/// save leaves it alone, a save carrying a fresh wrapper replaces it outright
/// (nothing to merge a key onto — see `merge_quality_report`).
#[test]
fn add_quality_report_migration_backfills_legacy_rows_and_a_later_save_replaces_it() {
    let dir = TempDir::new().unwrap();
    let job_url = "https://acme.com/job/legacy";
    {
        // Build the store at the PREVIOUS schema version by running every
        // migration up to (not including) `add_quality_report`.
        let (conn, _) = conn_before(&dir, "add_quality_report");
        assert!(
            !crate::db::column_exists(&conn, "ai_generations", "quality_report"),
            "precondition: the legacy schema has no quality_report column"
        );
        conn.execute(
            "INSERT INTO ai_generations (id, created_at, resume_text, cover_letter_text, job_url)
             VALUES ('legacy', 1000, 'R', 'C', ?1)",
            params![job_url],
        )
        .unwrap();
    }

    // Opening with the full migration list applies `add_quality_report` in
    // place — the legacy row backfills to ''.
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let list = store.list();
    assert_eq!(list.len(), 1, "the legacy row must survive the migration");
    assert_eq!(list[0].id, "legacy");
    assert_eq!(list[0].quality_report, "", "new column defaults to empty");

    // A content-less (answers-only) save merges into the legacy row and must
    // leave the backfilled empty report alone.
    let mut answers_only = content_less("g-answers", job_url);
    answers_only.quality_report = String::new();
    answers_only.application_answers = vec![answer("why-company")];
    store.save_application(answers_only).unwrap();
    assert_eq!(
        store.list()[0].quality_report,
        "",
        "a content-less save must leave the backfilled empty report alone"
    );

    // A résumé save carrying a fresh wrapper report replaces the (unparseable,
    // empty) existing value outright.
    let mut resume_save = record("g-resume", job_url);
    resume_save.cover_letter_text = String::new();
    resume_save.quality_report =
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":42,"resume":{"ok":true}}"#.into();
    store.save_application(resume_save).unwrap();
    assert_eq!(
        store.list()[0].quality_report,
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":42,"resume":{"ok":true}}"#,
        "a résumé save must replace the backfilled empty report with its wrapper"
    );
}

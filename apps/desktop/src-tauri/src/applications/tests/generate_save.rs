use super::{support::*, *};

// ── Gap 1: generate-save demotion behaviour ───────────────────────────────────
//
// The command `ai_generations_save` (ADR 0001) calls:
//   1. ApplicationStore::upsert_for_origin(…, Generate, …)  → Application row
//   2. AiGenerationStore::save_application(rec)             → generation row
//
// These tests mirror that two-step call at the store level (the Tauri command
// wrapper cannot be unit-tested without a live AppHandle).

#[test]
fn generate_save_creates_one_saved_application() {
    // Calling upsert_for_origin with Generate origin for the first time must
    // produce exactly ONE Application row with status `saved` and no
    // `applied_at` (generating documents is not a submission).
    let (_dir, app_store) = open_store();

    let app_id = upsert(
        &app_store,
        "https://acme.com/job/42",
        "linkedin",
        &meta("Acme", "Engineer"),
        ApplicationOrigin::Generate,
    );

    let apps = app_store.list();
    assert_eq!(apps.len(), 1, "exactly one Application must be created");
    let app = app_store.get(&app_id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Saved,
        "Generate origin must yield status=saved"
    );
    assert!(
        app.applied_at.is_none(),
        "applied_at must stay unset for Generate origin"
    );
}

#[test]
fn generate_save_second_generation_same_url_merge_into_one_gen_row_and_one_application() {
    // Saving two generations (e.g. résumé then cover) for the same normalized
    // url must produce ONE Application and TWO generation rows — the aggregate
    // stays single while the child document table grows.
    let (dir, app_store) = open_store();
    // Open gen store after app_store so the backfill migration has already run
    // and the application_id column exists.
    let gen_store = open_gen_store(dir.path());

    let url = "https://acme.com/job/42";

    // First save: résumé generation.
    let app_id_1 = upsert(
        &app_store,
        url,
        "linkedin",
        &meta("Acme", "Engineer"),
        ApplicationOrigin::Generate,
    );
    // The two saves differ only in id and which text they carry.
    let record = |id: &str, resume_text: &str, cover_letter_text: &str| {
        crate::ai_generations::AiGenerationRecord {
            id: id.into(),
            created_at: crate::db::now_ms(),
            candidate_name: "Jane".into(),
            job_title: "Engineer".into(),
            company_name: "Acme".into(),
            resume_language: "en".into(),
            job_ad_language: "en".into(),
            target_language: "en".into(),
            mismatch: false,
            top_requirements: vec![],
            mode: "ats".into(),
            resume_text: resume_text.into(),
            cover_letter_text: cover_letter_text.into(),
            job_ad: "JD".into(),
            job_url: url.into(),
            board: "linkedin".into(),
            application_answers: vec![],
            company_brief: String::new(),
            interview_questions: vec![],
            email_subject: String::new(),
            email_body: String::new(),
            application_id: None,
            quality_report: String::new(),
        }
    };
    let rec1 = record("gen-resume", "RESUME", "");
    gen_store.save_application(rec1).unwrap();

    // Second save: cover-letter generation for the same url.
    let app_id_2 = upsert(
        &app_store,
        url,
        "linkedin",
        &meta("Acme", "Engineer"),
        ApplicationOrigin::Generate,
    );
    let rec2 = record("gen-cover", "", "COVER");
    // AiGenerationStore::save_application merges same-url into one gen row.
    // Both upsert_for_origin calls must return the SAME Application id.
    gen_store.save_application(rec2).unwrap();

    assert_eq!(
        app_id_1, app_id_2,
        "both generate-saves for the same url must resolve to the same Application id"
    );

    let apps = app_store.list();
    assert_eq!(apps.len(), 1, "still exactly one Application for the url");
    assert_eq!(
        apps[0].status,
        ApplicationStatus::Saved,
        "Application status must remain saved"
    );

    // AiGenerationStore merges same-url into one aggregate gen row (existing
    // save_application_upserts_by_job_url test covers this); what we assert
    // here is that the Application aggregate is unaffected (still one row).
    let gen_list = gen_store.list();
    assert_eq!(
        gen_list.len(),
        1,
        "same-url generations merge into one gen row (per save_application semantics)"
    );
}

#[test]
fn generate_never_demotes_or_rewrites_an_applied_row() {
    let (_dir, store) = open_store();
    let url = "https://acme.com/job/7";
    let id = upsert(
        &store,
        url,
        "linkedin",
        &meta("Acme", "Engineer"),
        ApplicationOrigin::Manual,
    );
    let before = store.get(&id).unwrap();
    assert_eq!(before.status, ApplicationStatus::Applied);

    let again = upsert(
        &store,
        url,
        "linkedin",
        &meta("Acme", "Engineer"),
        ApplicationOrigin::Generate,
    );
    assert_eq!(again, id);
    let after = store.get(&id).unwrap();
    assert_eq!(after.status, ApplicationStatus::Applied);
    assert_eq!(
        after.applied_at, before.applied_at,
        "original applied_at kept"
    );
}

#[test]
fn generate_with_explicit_applied_true_applies() {
    let (_dir, store) = open_store();
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/8",
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Generate,
            Some(true),
        )
        .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Applied);
    assert!(app.applied_at.is_some());
}

#[test]
fn generate_then_explicit_set_status_applied_records_the_submission() {
    let (_dir, store) = open_store();
    let id = upsert(
        &store,
        "https://acme.com/job/9",
        "linkedin",
        &meta("Acme", "Engineer"),
        ApplicationOrigin::Generate,
    );
    store
        .set_status(&id, ApplicationStatus::Applied, "")
        .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Applied);
    assert!(app.applied_at.is_some());
}

#[test]
fn manual_and_backfill_origins_still_apply() {
    let (_dir, store) = open_store();
    for (url, origin) in [
        ("https://acme.com/job/10", ApplicationOrigin::Manual),
        ("https://acme.com/job/11", ApplicationOrigin::Backfill),
    ] {
        let id = upsert(&store, url, "linkedin", &meta("Acme", "Engineer"), origin);
        let app = store.get(&id).unwrap();
        assert_eq!(app.status, ApplicationStatus::Applied, "{origin:?}");
        assert!(app.applied_at.is_some(), "{origin:?}");
    }
}

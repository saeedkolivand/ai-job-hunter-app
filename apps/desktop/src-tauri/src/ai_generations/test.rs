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

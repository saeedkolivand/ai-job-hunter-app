use super::{support::*, *};

#[test]
fn insert_round_trips_the_job_link() {
    let (_dir, store) = open_store();
    insert(&store, "g1", "https://acme.com/job/1");

    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].job_url, "https://acme.com/job/1");
    assert_eq!(list[0].board, "linkedin");
}

#[test]
fn insert_round_trips_answers_and_brief() {
    let (_dir, store) = open_store();
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
    let (_dir, store) = open_store();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.interview_questions = vec![interview_question("iq-1"), interview_question("iq-2")];
    store.insert(&rec).unwrap();

    let list = store.list();
    assert_eq!(list[0].interview_questions, rec.interview_questions);
}

#[test]
fn insert_round_trips_the_email_draft() {
    let (_dir, store) = open_store();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.email_subject = "Application: Engineer".into();
    rec.email_body = "Hello,\n\nI'd like to apply.".into();
    store.insert(&rec).unwrap();

    let list = store.list();
    assert_eq!(list[0].email_subject, "Application: Engineer");
    assert_eq!(list[0].email_body, "Hello,\n\nI'd like to apply.");
}

/// End-to-end through the store: an email save must land on the SAME aggregate
/// row the tailor flow wrote (dedupe key = normalized job_url), not fork a
/// second row, and must not disturb the résumé/cover already stored there.
#[test]
fn save_application_merges_an_email_draft_onto_the_existing_aggregate() {
    let (_dir, store) = open_store();

    // Tailor flow first: résumé + cover for the job (raw url with tracking params).
    store
        .save_application(record("g1", "https://acme.com/job/1?utm_source=indeed"))
        .unwrap();

    // Then the email tab saves its draft — no résumé/cover, different raw url.
    let mut email_save = content_less("g2", "https://acme.com/job/1#apply");
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
    let (_dir, store) = open_store();

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
fn save_application_upserts_by_job_url_into_one_aggregate() {
    let (_dir, store) = open_store();
    let url = "https://acme.com/job/1";

    // First the tailor flow saves a résumé/cover for the job.
    store.save_application(record("g1", url)).unwrap();

    // Then the questions assistant saves answers (no résumé/cover) for the same job.
    let mut answers_save = content_less("g2", url);
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
    let (_dir, store) = open_store();

    store
        .save_application(record("g1", "https://acme.com/job/1?utm_source=indeed"))
        .unwrap();
    let mut second = content_less("g2", "https://acme.com/job/1#apply");
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
    let (_dir, store) = open_store();

    // Written directly, bypassing save_application's normalization.
    insert(&store, "legacy", "https://acme.com/job/2?utm_source=old");

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
    let (_dir, store) = open_store();
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
    let (_dir, store) = open_store();
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
    let (_dir, store) = open_store();
    insert(&store, "g1", "");
    insert(&store, "g2", "");
    insert(&store, "g3", "");
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

/// The other half of the mid-run resurrection: `save_application` is a
/// merge-UPSERT, so a run that finishes after its posting was deleted
/// re-creates the aggregate row rather than failing or no-opping.
///
/// Pinned here because it is the reason the guard in
/// `commands::resume_pipeline::execute` has to skip `persist_document` too —
/// suppressing only the run row would still put the résumé back in the
/// Documents list.
///
/// Mutation check: none needed on this file — it documents existing store
/// behaviour that the CALLER must now avoid; the caller's guard is pinned by
/// `a_run_whose_posting_was_deleted_mid_flight_does_not_resurrect_it`.
#[test]
fn saving_a_generation_for_a_deleted_posting_re_creates_the_aggregate() {
    let (_dir, store) = open_store();
    let url = "https://acme.test/job/1";

    store.insert(&record("g1", url)).unwrap();
    assert!(store.find_for_job(url).is_some());

    // The user deletes it mid-run.
    store.remove("g1").unwrap();
    assert!(store.find_for_job(url).is_none(), "the premise: it is gone");

    // The run finishes and persists what it produced.
    store.save_application(record("g2", url)).unwrap();
    assert!(
        store.find_for_job(url).is_some(),
        "a merge-upsert INSERTS when nothing is there — the deleted document comes back"
    );
}

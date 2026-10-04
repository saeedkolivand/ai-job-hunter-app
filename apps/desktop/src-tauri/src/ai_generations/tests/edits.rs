use super::support::*;

#[test]
fn applied_job_urls_returns_only_non_empty_links() {
    let (_dir, store) = open_store();
    insert(&store, "g1", "https://acme.com/job/1");
    insert(&store, "g2", "https://acme.com/job/2");
    insert(&store, "g3", ""); // manual generation, no job link

    let urls = store.applied_job_urls();
    assert_eq!(urls.len(), 2);
    assert!(urls.contains("https://acme.com/job/1"));
    assert!(urls.contains("https://acme.com/job/2"));
    assert!(!urls.contains(""));
}

// ── remove_many tests ─────────────────────────────────────────────────────────

#[test]
fn remove_many_deletes_subset_and_returns_count() {
    let (_dir, store) = open_store();
    insert(&store, "g1", "");
    insert(&store, "g2", "");
    insert(&store, "g3", "");

    let deleted = store.remove_many(&["g1".into(), "g3".into()]).unwrap();

    assert_eq!(deleted, 2, "should report 2 deleted rows");
    let remaining: Vec<_> = store.list().iter().map(|r| r.id.clone()).collect();
    assert_eq!(remaining, vec!["g2"], "only g2 should remain");
}

#[test]
fn remove_many_empty_input_is_noop() {
    let (_dir, store) = open_store();
    insert(&store, "g1", "");

    let deleted = store.remove_many(&[]).unwrap();

    assert_eq!(deleted, 0);
    assert_eq!(store.list().len(), 1, "row must not be touched");
}

// ── update_texts tests (F1 edit-before-export) ────────────────────────────────

#[test]
fn update_texts_resume_only_leaves_cover_untouched() {
    let (_dir, store) = open_store();
    insert(&store, "g1", "");

    store
        .update_texts("g1", Some("new resume".into()), None)
        .unwrap();

    let list = store.list();
    assert_eq!(list[0].resume_text, "new resume");
    assert_eq!(list[0].cover_letter_text, "C", "cover must be untouched");
}

#[test]
fn update_texts_cover_only_leaves_resume_untouched() {
    let (_dir, store) = open_store();
    insert(&store, "g1", "");

    store
        .update_texts("g1", None, Some("new cover".into()))
        .unwrap();

    let list = store.list();
    assert_eq!(list[0].resume_text, "R", "resume must be untouched");
    assert_eq!(list[0].cover_letter_text, "new cover");
}

#[test]
fn update_texts_both_fields_updates_both() {
    let (_dir, store) = open_store();
    insert(&store, "g1", "");

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
    let (_dir, store) = open_store();
    insert(&store, "g1", "");

    // Both-None must succeed without issuing an UPDATE — no rows-changed path.
    store.update_texts("g1", None, None).unwrap();

    let list = store.list();
    assert_eq!(list[0].resume_text, "R", "resume unchanged");
    assert_eq!(list[0].cover_letter_text, "C", "cover unchanged");
}

/// Each `(resume, cover)` arm of `update_texts` has its own `rows == 0` guard, so an
/// unknown id must surface an `Err` that names the id from every one of them.
#[test]
fn update_texts_unknown_id_returns_err_from_every_arm() {
    let (_dir, store) = open_store();

    for (resume, cover, arm) in [
        // (Some(resume), None) arm — rows==0 guard must surface an Err.
        (Some("x"), None, "resume-only"),
        // (None, Some(cover)) arm — each arm has its own rows==0 guard.
        (None, Some("y"), "cover-only"),
        // (Some(resume), Some(cover)) arm — rows==0 guard must also fire here.
        (Some("x"), Some("y"), "both-fields"),
    ] {
        let result = store.update_texts(
            "does-not-exist",
            resume.map(Into::into),
            cover.map(Into::into),
        );
        assert!(
            result.is_err(),
            "must return Err for an unknown id ({arm} arm)"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("does-not-exist"),
            "error message must include the id: {msg}"
        );
    }
}

/// **Deleting a generated résumé has to reach the pipeline run trail, and this
/// is the read that makes that possible.**
///
/// The Documents page's delete is the PRIMARY one — `applications_delete` is
/// the secondary path — and it removed the aggregate row while leaving a
/// max-depth run's full strategy (the whole employment history) and full
/// evidence map (verbatim résumé quotes) in `pipeline_run_events` with no
/// owner, no UI, no eviction, and a one-way ticket into every backup. The
/// cascade joins on `job_url`, so the url has to be read BEFORE the delete —
/// afterwards nothing can answer which posting the row belonged to.
///
/// Mutation check: drop the `job_url != ''` filter and the unlinked generation
/// contributes an empty url (which would ask the run store to match every
/// empty-url run); return the ids instead of the urls and the assertion fails.
#[test]
fn job_urls_for_reads_the_postings_a_delete_must_cascade_into() {
    let (_dir, store) = open_store();
    insert(&store, "g1", "https://acme.test/job/1");
    insert(&store, "g2", "https://acme.test/job/2");
    insert(&store, "g3", "");

    let mut urls = store.job_urls_for(&["g1".to_string(), "g2".to_string()]);
    urls.sort();
    assert_eq!(
        urls,
        vec![
            "https://acme.test/job/1".to_string(),
            "https://acme.test/job/2".to_string()
        ]
    );

    // An UNLINKED generation contributes nothing: it has no posting, and an
    // empty url handed to `delete_for_job` would be a request to match every
    // empty-url run in the store.
    assert!(store.job_urls_for(&["g3".to_string()]).is_empty());
    assert!(store.job_urls_for(&[]).is_empty());
    // An id that is not there is not an error.
    assert!(store.job_urls_for(&["nope".to_string()]).is_empty());
    // …and the read does not itself delete anything.
    assert_eq!(store.list().len(), 3);
}

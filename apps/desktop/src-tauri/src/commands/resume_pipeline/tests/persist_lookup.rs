use super::support::persist_document_source;
use crate::ai_generations::{AiGenerationRecord, AiGenerationStore};

/// **Delete-cascade guard, independent of how the id got there.** Once
/// `persist_document`'s (now read-only) lookup finds an id and lands it on
/// `AiGenerationRecord.application_id`, `AiGenerationStore::save_application`
/// persists it, and `remove_for_application` — what
/// `applications_delete(keepDocuments=false)` calls — must actually find and
/// delete the row through it. `upsert_for_origin` here is only this test's
/// OWN setup (synthesizing an Application the way the ordinary Save/Apply
/// flow would, before a staged run ever starts) — not a claim about what
/// `persist_document` itself calls; see
/// [`persist_document_lookup_links_to_an_existing_application`] below for the
/// guard on `persist_document`'s actual lookup.
///
/// Mutation check: force `application_id: None` on the record below (the
/// pre-fix `..empty_record()` shape this FK exists to prevent) and `deleted`
/// reddens from `1` to `0`.
#[test]
fn upsert_for_origin_id_on_the_record_is_what_remove_for_application_finds() {
    let dir = tempfile::TempDir::new().unwrap();
    let app_store = crate::applications::ApplicationStore::open(dir.path()).unwrap();
    let gen_store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let job_url = "https://acme.com/jobs/fk-1";
    let app_id = app_store
        .upsert_for_origin(
            job_url,
            "linkedin",
            &crate::applications::ApplicationMeta {
                company: "Acme".into(),
                title: "Staff Engineer".into(),
                ..Default::default()
            },
            crate::applications::ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    // The shape `persist_document` builds once its lookup finds an id: that
    // id landing on the record's `application_id` field.
    let record = AiGenerationRecord {
        id: "gen-fk-1".into(),
        job_url: job_url.to_string(),
        resume_text: "Staff Engineer résumé".into(),
        application_id: Some(app_id.clone()),
        ..super::super::empty_record()
    };
    gen_store.save_application(record).unwrap();

    let deleted = gen_store.remove_for_application(&app_id).unwrap();
    assert_eq!(
        deleted, 1,
        "the id persist_document writes onto application_id must be the SAME id \
         remove_for_application deletes by"
    );
}

/// **Store-level no-resurrect guard.** Drives the exact chain
/// `persist_document`'s lookup now runs (`normalize_job_url` +
/// `find_by_job_url`) against a REAL `ApplicationStore`, reproducing the
/// user-visible sequence with **no concurrency involved**: an Application
/// exists (created the ordinary way — a staged run is always launched FROM
/// an Application's own page) → the run is in flight for minutes → the user
/// deletes it → the run lands and the lookup runs. Asserts no Application
/// was created and the id comes back `None`.
///
/// Mutation check: swap this test's own `find_by_job_url` call for
/// `upsert_for_origin` (the pre-fix shape) and `list()` goes from empty to
/// `1` — the deleted Application comes back.
#[test]
fn persist_document_lookup_never_resurrects_a_deleted_application() {
    let dir = tempfile::TempDir::new().unwrap();
    let app_store = crate::applications::ApplicationStore::open(dir.path()).unwrap();

    let job_url = "https://acme.com/jobs/resurrect-1";
    let app_id = app_store
        .upsert_for_origin(
            job_url,
            "linkedin",
            &crate::applications::ApplicationMeta {
                company: "Acme".into(),
                title: "Staff Engineer".into(),
                ..Default::default()
            },
            crate::applications::ApplicationOrigin::Generate,
            None,
        )
        .unwrap();
    assert_eq!(
        app_store.list().len(),
        1,
        "the Application the run was launched from exists before the delete"
    );

    // "the run is in flight, then the user deletes it" — the deletion just
    // has to happen before the lookup below runs; no race needed to trigger
    // the hazard.
    app_store.delete(&app_id, false).unwrap();
    assert!(
        app_store.get(&app_id).is_none(),
        "deletion actually took effect"
    );

    // Exactly the two lines `persist_document`'s lookup runs.
    let normalized = crate::applications::normalize_job_url(job_url);
    let application_id = app_store.find_by_job_url(&normalized).map(|found| found.id);

    assert!(
        application_id.is_none(),
        "a deleted Application must not resolve — Some(_) here would mean the \
         delete got silently resurrected"
    );
    assert!(
        app_store.list().is_empty(),
        "the lookup itself must never CREATE an Application on a miss"
    );
}

/// **Store-level normal-path guard — the fix must not have simply disabled
/// linking.** Same two-line chain as the guard above, but the Application
/// still exists when the lookup runs — the common case, since every staged
/// run is launched FROM an Application's own page. Confirms the id resolves
/// and actually lands on the persisted generation row, which is the behavior
/// `28f5833d` existed to add; narrowing create-on-miss must not regress it.
///
/// Mutation check: look the wrong url up (`find_by_job_url(job_url)` instead
/// of the normalized one) and this reddens for any url normalization
/// changes (e.g. a tracking query param).
#[test]
fn persist_document_lookup_links_to_an_existing_application() {
    let dir = tempfile::TempDir::new().unwrap();
    let app_store = crate::applications::ApplicationStore::open(dir.path()).unwrap();
    let gen_store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let job_url = "https://acme.com/jobs/fk-2?utm_source=li";
    let app_id = app_store
        .upsert_for_origin(
            job_url,
            "linkedin",
            &crate::applications::ApplicationMeta {
                company: "Acme".into(),
                title: "Staff Engineer".into(),
                ..Default::default()
            },
            crate::applications::ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    // Exactly the two lines `persist_document`'s lookup runs.
    let normalized = crate::applications::normalize_job_url(job_url);
    let application_id = app_store.find_by_job_url(&normalized).map(|found| found.id);
    assert_eq!(
        application_id,
        Some(app_id.clone()),
        "the Application still exists, so the read-only lookup must still find it \
         — the resurrection fix narrows create-on-miss, it does not stop finding"
    );

    let record = AiGenerationRecord {
        id: "gen-fk-2".into(),
        job_url: job_url.to_string(),
        resume_text: "Staff Engineer résumé".into(),
        application_id,
        ..super::super::empty_record()
    };
    gen_store.save_application(record).unwrap();

    let saved = gen_store
        .find_for_job(job_url)
        .expect("the saved row is findable by job_url");
    assert_eq!(
        saved.application_id.as_deref(),
        Some(app_id.as_str()),
        "the FK the lookup found must persist onto the row"
    );
}

/// **A completed run exposes two DISTINCT documents — pinned by SOURCE
/// against the exact two lines that build them, not by feeding two
/// already-distinct literals through the store and asserting they differ.**
/// A store-round-trip test shaped that way could never fail for what it
/// claims: it would never call `persist_document` at all, so a regression
/// INSIDE it — `cover_letter_text: ctx.draft.clone()`, say — would sail
/// through clean, both fields still non-empty, which is exactly the "both
/// non-empty" shape that let both the fence-tag leak (BUG-A) and this FK
/// orphaning (this bug) ship undetected. (`ai_generations`'s own store-level
/// merge tests already cover the downstream round trip thoroughly; what was
/// never pinned is the two lines in `persist_document` that decide which
/// `QualityCtx` field feeds which DB column.)
///
/// Pins that `resume_text` and `cover_letter_text` are built from two
/// DIFFERENT `QualityCtx` fields — `ctx.draft` ("the résumé body") and
/// `ctx.letter` ("the letter `cover_letter` generated"), per that struct's
/// own field docs — never the same field read twice.
///
/// Mutation check, both applied and reverted: change `cover_letter_text:
/// ctx.letter.clone()` to `cover_letter_text: ctx.draft.clone()` in
/// `persist_document` and the second assertion fails.
#[test]
fn persist_document_builds_the_two_documents_from_different_ctx_fields() {
    let body = persist_document_source();
    assert!(
        body.contains("resume_text: ctx.draft.clone(),"),
        "the résumé slot must come from ctx.draft — QualityCtx's own \"the résumé \
         body\" field"
    );
    assert!(
        body.contains("cover_letter_text: ctx.letter.clone(),"),
        "the letter slot must come from ctx.letter, NEVER ctx.draft — QualityCtx's \
         own \"the letter `cover_letter` generated\" field"
    );
}

/// **`persist_document` must save a run that validated only a letter, and must
/// hand `report::build` an OPTION for the résumé.**
///
/// Two silent failures live in this function, and both look like success:
///
/// * the gate used to be `let report = ctx.report.as_ref()?;` — the RÉSUMÉ's
///   report as the precondition for the whole save. A cover-letter-only run has
///   no résumé report by design (`stages::validate`), so a fully generated,
///   validated, humanized letter was dropped on the floor with nothing anywhere
///   saying why: the run still reported `completed`, and the letter simply was
///   not there when the user came back;
/// * the wrapper argument used to be an unconditional `Some((report, …))`.
///   Passing an empty-but-present slot is not a smaller save — the store's
///   merge overlays whole top-level keys, so it ERASES the posting's stored
///   résumé report and every Keep/Remove verdict recorded against it.
///
/// A source pin, for the reason
/// `persist_document_looks_up_the_application_read_only_never_creates_one`
/// above already states: this function needs an `AppHandle`, which no test in
/// this crate can build. The BEHAVIOUR either side of it is pinned by real
/// tests — `a_letter_only_wrapper_omits_the_resume_key_so_the_merge_cannot_erase_it`
/// for `report::build`, and `max_test`'s cover-only `save_verdict` cases for the
/// gate — so what is left here is the wiring between them.
///
/// Mutation check: restore either the `ctx.report.as_ref()?` gate or the
/// unconditional `Some((report, &ctx.draft))` and the matching assertion fails.
#[test]
fn persist_document_saves_a_run_that_validated_only_the_letter() {
    let body = persist_document_source()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        !body.contains("let report = ctx.report.as_ref()?;"),
        "the pre-fix gate — the résumé's report as the precondition for the \
         whole save — discarded a cover-letter-only run's letter entirely"
    );
    assert!(
        body.contains("if ctx.report.is_none() && ctx.letter_report.is_none() { return None; }"),
        "the save must be gated on having validated AT LEAST ONE document, not \
         on the résumé specifically"
    );
    assert!(
        body.contains("ctx.report .as_ref() .map(|report| (report, ctx.draft.as_str())),"),
        "report::build's résumé slot must be an Option — an empty-but-present \
         slot erases the posting's stored one on merge"
    );
    assert!(
        body.contains("ctx.input.include_resume,"),
        "the save gate must be told which documents this run was asked to write"
    );
}

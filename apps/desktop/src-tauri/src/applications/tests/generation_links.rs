use super::{support::*, *};

// ── `link_orphaned_generations` — the FK backfill for existing installs ──────
//
// The FK fix (`commands::resume_pipeline::persist::persist_document`) stops NEW
// rows being orphaned; it does nothing for rows the staged pipeline already
// wrote before that fix shipped. Those rows are the user-visible defect:
// `applications_delete(keepDocuments=false)` calls `remove_for_application`,
// which matches by `application_id` — an orphaned row is invisible to it, so
// the user asks the app to delete their documents and the documents stay.
//
// Each test below proves something `link_orphaned_generations` returning
// `Ok(_)` alone cannot: the exact COUNT it claims to have linked, and — for
// the delete case — the actual downstream behaviour the user experiences,
// not just the FK column's value.

/// **The happy path, and the one the whole backfill exists for**: an orphaned
/// row whose Application already exists (created independently, by the apply
/// flow, before the run that orphaned this row) gets linked, keyed by
/// NORMALIZED `job_url` — the same key every live save merges on. Asserts the
/// returned COUNT, not just that the call succeeded: a migration that runs
/// clean while linking nothing is the worst outcome named in this backfill's
/// own doc, and a bare `Ok(())`/`is_ok()` check cannot tell the two apart.
///
/// Mutation check: make `link_orphaned_generations` an immediate `Ok(0)` (the
/// no-op it must never regress to) and both assertions fail — applied and
/// reverted.
#[test]
fn link_orphaned_generations_links_a_row_whose_application_already_exists() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());
    let app_store = ApplicationStore::open(dir.path()).unwrap();

    // The Application: created by the apply flow, exactly like production —
    // `upsert_for_origin` normalizes the raw url internally.
    let raw_url = "https://acme.com/jobs/42?utm_source=newsletter";
    let app_id = upsert(
        &app_store,
        raw_url,
        "linkedin",
        &meta("Acme", "Dev"),
        ApplicationOrigin::Generate,
    );

    // The orphan: a staged-pipeline row saved BEFORE `persist_document`
    // established the FK. `save_application` always normalizes `job_url`
    // before writing it, so a REAL orphaned row carries the NORMALIZED form —
    // this seeds the same one, not the raw string, to match production.
    let normalized_url = normalize_job_url(raw_url);
    insert_gen(&gen_conn, "gen-orphan", &normalized_url, None);
    assert_eq!(
        gen_application_id(&gen_conn, "gen-orphan"),
        None,
        "precondition: the row is orphaned"
    );

    let linked = app_store.link_orphaned_generations(dir.path()).unwrap();
    assert_eq!(linked, 1, "exactly the one orphan must be linked");
    assert_eq!(
        gen_application_id(&gen_conn, "gen-orphan"),
        Some(app_id),
        "the row must now reference the Application that already existed for its posting"
    );
}

/// **Proves `link_orphaned_generations` itself normalizes `job_url` before
/// looking it up** — every other link test in this file seeds the orphan row
/// with an already-normalized url (matching what production actually writes),
/// so none of them can tell the internal `normalize_job_url` call apart from
/// simply being a no-op on an already-normalized string. This one seeds the
/// RAW, un-normalized url instead — what a genuinely pre-fix row on disk
/// would carry — so only the real normalization call can make the lookup hit.
///
/// Mutation check: delete the `normalize_job_url(&job_url)` call inside
/// `link_orphaned_generations` (looking up the raw string directly) and
/// `linked` becomes `0` — applied and reverted.
#[test]
fn link_orphaned_generations_normalizes_a_raw_unnormalized_orphan_url() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());
    let app_store = ApplicationStore::open(dir.path()).unwrap();

    // `utm_source` is dropped and the host is lowercased by normalization —
    // this raw form is NOT equal to its own normalized form.
    let raw_url = "https://ACME.com/jobs/42?utm_source=newsletter";
    assert_ne!(
        raw_url,
        normalize_job_url(raw_url),
        "test precondition: raw_url must actually differ from its normalized form"
    );
    let app_id = upsert(
        &app_store,
        raw_url,
        "linkedin",
        &meta("Acme", "Dev"),
        ApplicationOrigin::Generate,
    );

    // The orphan row carries the RAW url, unlike every other test in this file.
    insert_gen(&gen_conn, "gen-orphan-raw", raw_url, None);

    let linked = app_store.link_orphaned_generations(dir.path()).unwrap();
    assert_eq!(
        linked, 1,
        "a raw, un-normalized orphan url must still resolve to its Application"
    );
    assert_eq!(
        gen_application_id(&gen_conn, "gen-orphan-raw"),
        Some(app_id)
    );
}

/// **The actual user-visible defect, reproduced end to end and then closed.**
/// BEFORE the backfill, `remove_for_application` — what `applications_delete`
/// calls for `keepDocuments=false` — matches nothing for an orphaned row: the
/// user asks the app to delete their documents and the documents stay. AFTER
/// the backfill, the SAME delete call actually removes it. Proves the
/// downstream BEHAVIOUR, not merely the `application_id` column's value.
///
/// This cannot pass against a no-op migration: `deleted_before` is asserted
/// `0` (reproducing the defect) and `deleted_after` is asserted `1` — a
/// backfill that linked nothing would leave `deleted_after` at `0` too, and
/// the test would fail on that assertion, not merely on an unchecked seed.
///
/// Mutation check: make `link_orphaned_generations` an immediate `Ok(0)` and
/// the `deleted_after`/`total_after` assertions fail — applied and reverted.
#[test]
fn a_backfilled_row_is_then_actually_removed_by_delete_keep_documents_false() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());
    let app_store = ApplicationStore::open(dir.path()).unwrap();
    let gen_store = open_gen_store(dir.path());

    let url = normalize_job_url("https://acme.com/jobs/43");
    let app_id = upsert(
        &app_store,
        &url,
        "linkedin",
        &meta("Acme", "Dev"),
        ApplicationOrigin::Generate,
    );
    insert_gen(&gen_conn, "gen-orphan-2", &url, None);

    // THE defect, reproduced: `applications_delete`'s own `remove_for_application`
    // call matches nothing for an orphaned row, and the row survives untouched.
    let deleted_before = gen_store.remove_for_application(&app_id).unwrap();
    assert_eq!(
        deleted_before, 0,
        "reproduces the defect: an orphaned row is invisible to remove_for_application"
    );
    let survives: i64 = gen_conn
        .query_row(
            "SELECT COUNT(*) FROM ai_generations WHERE id = 'gen-orphan-2'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        survives, 1,
        "the user asked to delete this document and it is still here — the defect"
    );

    // The fix: back-link it.
    let linked = app_store.link_orphaned_generations(dir.path()).unwrap();
    assert_eq!(linked, 1);

    // The SAME delete call, now reaching it.
    let deleted_after = gen_store.remove_for_application(&app_id).unwrap();
    assert_eq!(
        deleted_after, 1,
        "the backfilled row must now be deleted along with its application"
    );
    let total_after: i64 = gen_conn
        .query_row(
            "SELECT COUNT(*) FROM ai_generations WHERE id = 'gen-orphan-2'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        total_after, 0,
        "the user's delete must actually remove the document now"
    );
}

/// **Never guess.** A row whose posting has no resolvable Application — the
/// user deleted it, or the row was never linked to begin with — must stay
/// NULL. A DIFFERENT, unrelated Application exists in the same store so this
/// cannot pass merely because the `applications` table happened to be empty;
/// a wrong link here would put someone's résumé under an unrelated
/// Application and delete it along with THAT one.
///
/// Mutation check: have `link_orphaned_generations` fall back to the first
/// Application it finds instead of `find_by_job_url`'s exact match, and the
/// final assertion fails — applied and reverted.
#[test]
fn link_orphaned_generations_never_guesses_a_link_for_an_unmatched_row() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());
    let app_store = ApplicationStore::open(dir.path()).unwrap();

    // A real Application exists in the store — for a DIFFERENT posting.
    upsert(
        &app_store,
        "https://other.com/jobs/1",
        "linkedin",
        &meta("Other Co", "Role"),
        ApplicationOrigin::Generate,
    );

    // No Application anywhere matches this posting.
    let url = normalize_job_url("https://nomatch.example.com/jobs/999");
    insert_gen(&gen_conn, "gen-unmatched", &url, None);

    let linked = app_store.link_orphaned_generations(dir.path()).unwrap();
    assert_eq!(linked, 0, "nothing resolvable was linked");
    assert_eq!(
        gen_application_id(&gen_conn, "gen-unmatched"),
        None,
        "an unresolvable row must stay NULL rather than being guessed onto an unrelated Application"
    );
}

/// **Boot-path regression guard.** The three tests above call
/// `link_orphaned_generations` DIRECTLY — which passed even while `open()`'s
/// original call order ran `backfill_from_generations` FIRST and left every
/// resolvable orphan already linked (via `upsert_internal`'s wide merge)
/// before `link_orphaned_generations` ever ran, making it dead in the real
/// boot path despite passing in isolation. This drives `ApplicationStore::
/// open` itself — a REAL reboot on an existing data dir, not a direct call to
/// either private backfill method — so a regression in `open()`'s own call
/// ORDER is caught here even if both functions individually still pass their
/// own tests.
///
/// The two paths are distinguishable by more than the row's `application_id`:
/// `upsert_internal`'s merge (`pick(incoming, existing)`) overwrites the
/// Application's OTHER fields with whatever the orphaned generation row
/// carries, while `link_orphaned_generations` only ever writes the
/// generation's FK column. So `company` is set here to a value the orphan
/// row's hardcoded `'Acme'` (see `insert_gen`) does NOT carry —
/// if `open()` ever let the wide backfill resolve this row instead of the
/// lookup-only pass, `company` would be clobbered back to `'Acme'`.
///
/// Mutation check: revert `open()`'s call order (`backfill_from_generations`
/// before `link_orphaned_generations`) and the `company` assertion fails —
/// applied and reverted.
#[test]
fn open_links_an_orphan_through_the_lookup_only_path_not_the_wide_backfill() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());

    let raw_url = "https://acme.com/jobs/77";
    let normalized_url = normalize_job_url(raw_url);

    let app_id = {
        let store = ApplicationStore::open(dir.path()).unwrap();
        upsert(
            &store,
            raw_url,
            "linkedin",
            &meta("Acme", "Dev"),
            ApplicationOrigin::Generate,
        )
        // `store` drops here, releasing the connection before the reboot below.
    };

    // A later correction — a value the stale orphan row below does NOT carry.
    {
        let apps_conn = Connection::open(dir.path().join("applications.db")).unwrap();
        apps_conn
            .execute(
                "UPDATE applications SET company = 'Verified Later Value' WHERE id = ?1",
                params![app_id],
            )
            .unwrap();
    }

    // The orphan: `insert_gen` hardcodes company_name = 'Acme',
    // the STALE value from before the correction above.
    insert_gen(&gen_conn, "gen-orphan-boot", &normalized_url, None);

    // A REAL reboot: `ApplicationStore::open`, not a direct call to either
    // private backfill method.
    let store2 = ApplicationStore::open(dir.path()).unwrap();

    assert_eq!(
        store2.list().len(),
        1,
        "the orphan must link to the pre-existing Application, not spawn a duplicate"
    );
    assert_eq!(
        gen_application_id(&gen_conn, "gen-orphan-boot"),
        Some(app_id.clone()),
        "open() must link the orphan to the SAME pre-existing Application"
    );
    let app = store2.get(&app_id).unwrap();
    assert_eq!(
        app.company, "Verified Later Value",
        "open() must resolve this row through the lookup-only path — a boot that let \
         the wide backfill's merge reach it first would clobber this field with the \
         orphan's stale 'Acme'"
    );
}

use super::{support::*, *};

// ── Gap 3: delete(keepDocuments) cross-store semantics ────────────────────────
//
// `applications_delete` (the Tauri command) does two separate store operations:
//   • keepDocuments=false → gen_store.remove_for_application(&id)   → rows gone
//   • keepDocuments=true  → gen_store.detach_application(&id)        → rows stay, FK nulled
// then ApplicationStore::delete in both cases.
//
// These tests call each store method directly (matching what the command does)
// and assert the exact generation-row counts before and after.

#[test]
fn delete_keep_documents_false_removes_child_generations() {
    let dir = TempDir::new().unwrap();
    // Create the gen DB with the application_id column before opening ApplicationStore
    // so the backfill migration finds it already present.
    let gen_conn = open_gen_db(dir.path());

    let app_store = ApplicationStore::open(dir.path()).unwrap();
    let gen_store = open_gen_store(dir.path());

    // Create an Application.
    let app_id = upsert(
        &app_store,
        "https://acme.com/job/99",
        "linkedin",
        &meta("Acme", "Dev"),
        ApplicationOrigin::Generate,
    );

    // Pre-link two generation rows to this Application (simulates what a live
    // session would have after the FK write-back). They carry DISTINCT urls: the
    // per-job UNIQUE(job_url) index forbids two rows sharing one non-empty url,
    // and `remove_for_application` must still delete every linked row regardless.
    insert_gen(&gen_conn, "gen-a", "https://acme.com/job/99", Some(&app_id));
    insert_gen(
        &gen_conn,
        "gen-b",
        "https://acme.com/job/99b",
        Some(&app_id),
    );

    assert_eq!(
        gen_count_for_app(&gen_conn, &app_id),
        2,
        "precondition: two child generations linked"
    );

    // Simulate keepDocuments=false: delete child gens first, then the Application.
    let deleted = gen_store.remove_for_application(&app_id).unwrap();
    assert_eq!(deleted, 2, "remove_for_application must delete both rows");

    app_store.delete(&app_id, false).unwrap();

    // Application and its history are gone.
    assert!(
        app_store.get(&app_id).is_none(),
        "Application row must be deleted"
    );
    assert!(
        app_store.events(&app_id).is_empty(),
        "status events must be deleted"
    );

    // Generation rows are gone.
    assert_eq!(
        gen_count_for_app(&gen_conn, &app_id),
        0,
        "child generations must be deleted when keepDocuments=false"
    );
    // The actual rows no longer exist at all.
    let total: i64 = gen_conn
        .query_row(
            "SELECT COUNT(*) FROM ai_generations WHERE id IN ('gen-a','gen-b')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(total, 0, "generation rows gen-a and gen-b must be gone");
}

#[test]
fn delete_keep_documents_true_detaches_child_generations_but_keeps_rows() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());

    let app_store = ApplicationStore::open(dir.path()).unwrap();
    let gen_store = open_gen_store(dir.path());

    let app_id = upsert(
        &app_store,
        "https://acme.com/job/100",
        "linkedin",
        &meta("Acme", "Dev"),
        ApplicationOrigin::Generate,
    );

    // Distinct urls per the per-job UNIQUE(job_url) index (see the sibling test);
    // `detach_application` must still null the FK on every linked row.
    insert_gen(
        &gen_conn,
        "gen-c",
        "https://acme.com/job/100",
        Some(&app_id),
    );
    insert_gen(
        &gen_conn,
        "gen-d",
        "https://acme.com/job/100b",
        Some(&app_id),
    );

    assert_eq!(
        gen_count_for_app(&gen_conn, &app_id),
        2,
        "precondition: two child generations linked"
    );

    // Simulate keepDocuments=true: detach (null FK), then delete the Application.
    let detached = gen_store.detach_application(&app_id).unwrap();
    assert_eq!(detached, 2, "detach_application must update both rows");

    app_store.delete(&app_id, true).unwrap();

    // Application is gone.
    assert!(
        app_store.get(&app_id).is_none(),
        "Application row must be deleted"
    );

    // Generation rows SURVIVE — they are now orphaned (application_id = NULL).
    let total: i64 = gen_conn
        .query_row(
            "SELECT COUNT(*) FROM ai_generations WHERE id IN ('gen-c','gen-d')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        total, 2,
        "generation rows must survive when keepDocuments=true"
    );

    // FK is now NULL on both rows (detached).
    assert_eq!(
        gen_application_id(&gen_conn, "gen-c"),
        None,
        "gen-c application_id must be NULL after detach"
    );
    assert_eq!(
        gen_application_id(&gen_conn, "gen-d"),
        None,
        "gen-d application_id must be NULL after detach"
    );

    // No longer linked to the deleted Application.
    assert_eq!(
        gen_count_for_app(&gen_conn, &app_id),
        0,
        "no generation rows should still reference the deleted Application id"
    );
}

// ── `APPLICATIONS_FEATURE_EPOCH_MS` — the resurrection fix ───────────────────
//
// `backfill_from_generations` used to CREATE an Application for any
// unmatched row, unconditionally, on every boot. Deleting an Application
// (either arm) does not — cannot — reach into the sibling `ai_generations.db`
// atomically, so a surviving orphaned row with the deleted posting's
// `job_url` made the NEXT boot silently re-create it: the user's delete was
// undone without any signal that it happened. These tests drive the actual
// user-visible sequence (create → delete → reboot) through `ApplicationStore::
// open`, not the private backfill methods directly.

/// **`keepDocuments=true`: detach, then delete, then reboot — must not come
/// back.** Mirrors `commands::applications::applications_delete`'s own two
/// calls (`AiGenerationStore::detach_application` then `ApplicationStore::
/// delete`) for that arm.
///
/// Mutation check: comment out the `created_at >= APPLICATIONS_FEATURE_EPOCH_MS`
/// guard in `backfill_from_generations` and the `list().len()` assertion
/// after reboot fails (1, not 0) — applied and reverted.
#[test]
fn a_keep_documents_true_delete_does_not_resurrect_the_application_on_reboot() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());
    let gen_store = open_gen_store(dir.path());

    let raw_url = "https://acme.com/jobs/resurrect-1";
    let normalized = normalize_job_url(raw_url);

    // ONE long-lived store instance for the whole create→detach→delete
    // sequence — exactly production's shape (`applications_delete` runs
    // against the already-open `tauri::State<ApplicationStore>`, it never
    // reopens the store mid-delete). A second `ApplicationStore::open` call
    // BEFORE the delete would itself run a boot-repair pass while the
    // Application still exists and re-link the just-detached row, masking
    // the very defect this test exists to catch.
    let store = ApplicationStore::open(dir.path()).unwrap();
    let app_id = upsert(
        &store,
        raw_url,
        "linkedin",
        &meta("Acme", "Dev"),
        ApplicationOrigin::Generate,
    );
    // A MODERN generation, linked to the Application about to be deleted —
    // exactly the shape a real staged-pipeline run produces.
    insert_gen_at(
        &gen_conn,
        "gen-r1",
        &normalized,
        Some(&app_id),
        MODERN_CREATED_AT,
    );

    // The `keepDocuments=true` delete sequence.
    gen_store.detach_application(&app_id).unwrap();
    store.delete(&app_id, true).unwrap();
    assert_eq!(
        store.list().len(),
        0,
        "precondition: the Application is gone"
    );
    drop(store); // release the connection before the reboot below.

    let rebooted = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(
        rebooted.list().len(),
        0,
        "a deletion the user asked for must survive a restart"
    );
    assert_eq!(
        gen_application_id(&gen_conn, "gen-r1"),
        None,
        "the detached generation must stay unlinked, not get a freshly created Application"
    );
}

/// **`keepDocuments=false`: a row already orphaned BEFORE the delete — the
/// exact pre-fix state every existing install is in right now — must not
/// resurrect the Application either.** `remove_for_application` only deletes
/// rows CURRENTLY linked by id, so a row that was never linked to this
/// Application (already NULL) survives the delete untouched, sharing its
/// `job_url` — precisely the shape a reboot must not re-link into existence.
///
/// Mutation check: same as above — remove the vintage guard and this reddens.
#[test]
fn a_keep_documents_false_delete_does_not_resurrect_from_a_pre_orphaned_row() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());
    let gen_store = open_gen_store(dir.path());

    let raw_url = "https://acme.com/jobs/resurrect-2";
    let normalized = normalize_job_url(raw_url);
    // ONE long-lived store instance, same reasoning as the keepDocuments=true
    // test above — a second `open` before the delete would itself relink the
    // pre-orphaned row to the still-alive Application via `link_orphaned_
    // generations`, masking the defect this test exists to catch.
    let store = ApplicationStore::open(dir.path()).unwrap();
    let app_id = upsert(
        &store,
        raw_url,
        "linkedin",
        &meta("Acme", "Dev"),
        ApplicationOrigin::Generate,
    );

    // Already orphaned BEFORE the delete — never linked to `app_id`.
    insert_gen_at(&gen_conn, "gen-r2", &normalized, None, MODERN_CREATED_AT);

    let deleted = gen_store.remove_for_application(&app_id).unwrap();
    assert_eq!(
        deleted, 0,
        "precondition: the pre-orphaned row was never linked to this id"
    );
    store.delete(&app_id, false).unwrap();
    drop(store); // release the connection before the reboot below.

    let rebooted = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(
        rebooted.list().len(),
        0,
        "must not resurrect the Application from a row that was already orphaned before the delete"
    );
    assert_eq!(gen_application_id(&gen_conn, "gen-r2"), None);
}

/// **Direct proof of the vintage gate itself**, isolated from the delete
/// sequence above: a MODERN row with no matching Application ANYWHERE (never
/// had one, or it was deleted) must never get one CREATED by
/// `backfill_from_generations` — only [`link_orphaned_generations`] may ever
/// resolve it, and only by linking to one that already exists.
///
/// **The epoch boundary is inclusive** — a row created at EXACTLY
/// `APPLICATIONS_FEATURE_EPOCH_MS` must be treated as modern (`>=`, not
/// `>`). The first case seeds [`MODERN_CREATED_AT`], which sits comfortably after
/// the boundary and so cannot distinguish `>` from `>=`; the second seeds the
/// boundary value itself.
///
/// Mutation check (first case): drop the `created_at >= APPLICATIONS_FEATURE_EPOCH_MS`
/// guard and `list().len()` becomes 1 — applied and reverted. Mutation check
/// (second case): change the guard from `created_at >= APPLICATIONS_FEATURE_
/// EPOCH_MS` to `created_at > APPLICATIONS_FEATURE_EPOCH_MS` and
/// `list().len()` becomes 1 — applied and reverted.
#[test]
fn modern_and_epoch_boundary_orphans_never_get_an_application_created_by_backfill() {
    for (gen_id, job_url, created_at, message) in [
        (
            "gen-modern-orphan",
            "https://acme.com/jobs/never-had-one",
            MODERN_CREATED_AT,
            "a MODERN orphan must never get a freshly created Application",
        ),
        (
            "gen-boundary",
            "https://acme.com/jobs/exactly-at-epoch",
            APPLICATIONS_FEATURE_EPOCH_MS as i64,
            "a row created exactly at the epoch boundary must be treated as modern, not legacy",
        ),
    ] {
        let dir = TempDir::new().unwrap();
        let gen_conn = open_gen_db(dir.path());
        insert_gen_at(&gen_conn, gen_id, job_url, None, created_at);

        let store = ApplicationStore::open(dir.path()).unwrap();
        assert_eq!(store.list().len(), 0, "{message}");
        assert_eq!(gen_application_id(&gen_conn, gen_id), None);
    }
}

/// **The genuinely-pre-epoch resurrection, closed.** The epoch guard above
/// only protects a MODERN row; a row that predates `APPLICATIONS_FEATURE_
/// EPOCH_MS` stays pre-epoch forever, so it alone cannot stop this sequence:
/// backfill creates an Application from a legacy row → the user deletes it
/// (which detaches the generation's FK back to `NULL`, same as the
/// `keepDocuments=true` command path) → the row is STILL pre-epoch and STILL
/// unlinked, so an unguarded backfill would recreate the very Application the
/// user just deleted on the next boot. `LEGACY_BACKFILL_MARKER` is what
/// actually closes this: the FIRST boot's backfill sets it, so the SECOND
/// boot's `backfill_from_generations` short-circuits before it ever looks at
/// `ai_generations.db` again.
///
/// Mutation check: make `legacy_backfill_done` always return `false` (i.e.
/// restore the old unconditional re-scan) and `rebooted.list().len()`
/// becomes 1 — applied and reverted.
#[test]
fn a_legacy_backfilled_application_stays_deleted_across_a_second_reboot() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());

    let raw_url = "https://acme.com/jobs/legacy-resurrect";
    let normalized = normalize_job_url(raw_url);
    // `insert_gen` hardcodes `created_at = 1000` — provably before
    // `APPLICATIONS_FEATURE_EPOCH_MS`, i.e. a genuinely legacy row.
    insert_gen(&gen_conn, "gen-legacy-resurrect", &normalized, None);

    // Boot 1: the legacy row has no Application anywhere, so
    // `backfill_from_generations` creates one and links it.
    let store = ApplicationStore::open(dir.path()).unwrap();
    let apps = store.list();
    assert_eq!(apps.len(), 1, "precondition: the legacy row was backfilled");
    let app_id = apps[0].id.clone();
    assert_eq!(
        gen_application_id(&gen_conn, "gen-legacy-resurrect"),
        Some(app_id.clone()),
        "precondition: the backfill linked the generation to the new Application"
    );

    // The user deletes it — same two calls `applications_delete` makes for
    // `keepDocuments=true`: detach the FK, then remove the Application row.
    let gen_store = open_gen_store(dir.path());
    gen_store.detach_application(&app_id).unwrap();
    store.delete(&app_id, true).unwrap();
    assert_eq!(
        store.list().len(),
        0,
        "precondition: the Application is gone"
    );
    assert_eq!(
        gen_application_id(&gen_conn, "gen-legacy-resurrect"),
        None,
        "precondition: the delete detached the generation's FK back to NULL"
    );
    drop(store); // release the connection before the reboot below.

    // Boot 2: the row is STILL pre-epoch and STILL unlinked — exactly the
    // shape an unguarded backfill would recreate an Application from.
    let rebooted = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(
        rebooted.list().len(),
        0,
        "the user's delete must survive a second reboot, not just the first"
    );
    assert_eq!(
        gen_application_id(&gen_conn, "gen-legacy-resurrect"),
        None,
        "must stay unlinked, not get a freshly re-created Application"
    );
}

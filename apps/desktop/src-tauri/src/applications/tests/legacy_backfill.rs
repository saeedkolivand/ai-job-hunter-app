use super::{support::*, *};

/// Seed legacy (pre-migration) generation rows using the OLD ai_generations
/// schema that existed before the `application_id` column was added.
///
/// IMPORTANT: this helper deliberately uses the OLD schema and must NOT be
/// updated to match the live schema.  Its purpose is to verify that the
/// backfill migration runs correctly against data that predates the migration.
fn seed_legacy_generations(dir: &Path, rows: &[(&str, &str, &str)]) {
    let conn = Connection::open(dir.join("ai_generations.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE ai_generations (
            id TEXT PRIMARY KEY, created_at INTEGER NOT NULL,
            candidate_name TEXT NOT NULL DEFAULT '', job_title TEXT NOT NULL DEFAULT '',
            company_name TEXT NOT NULL DEFAULT '', resume_language TEXT NOT NULL DEFAULT 'en',
            job_ad_language TEXT NOT NULL DEFAULT 'en', target_language TEXT NOT NULL DEFAULT 'en',
            mismatch INTEGER NOT NULL DEFAULT 0, top_requirements TEXT NOT NULL DEFAULT '[]',
            mode TEXT NOT NULL DEFAULT 'ats', resume_text TEXT NOT NULL DEFAULT '',
            cover_letter_text TEXT NOT NULL DEFAULT '', job_ad TEXT NOT NULL DEFAULT '',
            job_url TEXT NOT NULL DEFAULT '', board TEXT NOT NULL DEFAULT '',
            application_answers TEXT NOT NULL DEFAULT '[]', company_brief TEXT NOT NULL DEFAULT ''
        );",
    )
    .unwrap();
    for (id, job_url, company) in rows {
        conn.execute(
            "INSERT INTO ai_generations (id, created_at, company_name, job_url, board)
             VALUES (?1, ?2, ?3, ?4, 'linkedin')",
            params![id, 1000_i64, company, job_url],
        )
        .unwrap();
    }
}

#[test]
fn backfill_creates_one_application_per_generation_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    seed_legacy_generations(
        dir.path(),
        &[
            ("g1", "https://acme.com/job/1", "Acme"),
            ("g2", "https://www.acme.com/job/1/", "Acme"),
            ("g3", "", "NoLink"),
        ],
    );

    let store = ApplicationStore::open(dir.path()).unwrap();
    let apps = store.list();
    assert_eq!(
        apps.len(),
        2,
        "shared-url gens merge; url-less gen stands alone"
    );
    assert!(apps.iter().all(|a| a.status == ApplicationStatus::Applied));
    assert!(apps.iter().all(|a| a.applied_at == Some(1000)));

    let gen_conn = Connection::open(dir.path().join("ai_generations.db")).unwrap();
    let linked: i64 = gen_conn
        .query_row(
            "SELECT COUNT(*) FROM ai_generations WHERE application_id IS NOT NULL AND application_id != ''",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(linked, 3, "every generation is linked");

    drop(store);
    let store2 = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(store2.list().len(), 2, "re-run backfill is idempotent");
}

#[test]
fn backfill_no_generations_db_is_noop() {
    let (_dir, store) = open_store();
    assert!(store.list().is_empty());
}

/// **FIX-2 mutation guard: a transient read failure must NOT read as "not
/// done".** Before FIX-2, `legacy_backfill_done` swallowed EVERY
/// `query_row` error into `false` — indistinguishable from the genuine "no
/// row yet" case — so a glitch reading `backfill_state` re-ran the whole
/// one-shot scan. Manually dropping `backfill_state` AFTER its own migration
/// already ran (`run_migrations`'s versioned skip — own doc — never
/// recreates an already-applied migration's table) forces
/// `legacy_backfill_done`'s `query_row` into a genuine SQL error
/// (`no such table`) rather than `QueryReturnedNoRows`.
///
/// Mutation check: revert `legacy_backfill_done` to `-> bool` / `.is_ok()`
/// (swallowing the "no such table" error into `false`) and
/// `rebooted.list().len()` becomes 1 (the swallow reads as "not done", so
/// the scan runs and backfills the legacy row) instead of 0 — applied and
/// reverted.
#[test]
fn a_transient_marker_read_failure_does_not_re_run_the_scan() {
    let dir = TempDir::new().unwrap();

    // Boot 1: creates `applications.db` (and `backfill_state`, via
    // migration) with nothing yet to find.
    {
        let store = ApplicationStore::open(dir.path()).unwrap();
        assert_eq!(store.list().len(), 0, "precondition: nothing to find yet");
        assert!(
            legacy_backfill_marker_set(dir.path()),
            "precondition: boot 1 sets the marker (nothing to find, ever)"
        );
    }

    // Break `legacy_backfill_done`'s read: drop the table its own migration
    // already created and versioned past.
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute_batch("DROP TABLE backfill_state;").unwrap();
    }

    // A legacy row boot 1 never saw (it didn't exist yet).
    let gen_conn = open_gen_db(dir.path());
    insert_gen(
        &gen_conn,
        "gen-glitch",
        "https://acme.com/jobs/glitch",
        None,
    );

    // Boot 2: `legacy_backfill_done` now hits a genuine SQL error reading the
    // (missing) marker table. `open()` still swallows the resulting backfill
    // error non-fatally at its own call site (own doc), but the scan itself
    // must never have run.
    let rebooted = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(
        rebooted.list().len(),
        0,
        "a genuine read error must abort before the scan runs, not be read as \"not done\""
    );
    assert_eq!(gen_application_id(&gen_conn, "gen-glitch"), None);
}

/// **FIX-1 mutation guard.** `relink_legacy_generations_after_restore` must
/// bypass `LEGACY_BACKFILL_MARKER` entirely — that is the whole point of it
/// existing as a SEPARATE method from `backfill_from_generations`. Drives the
/// exact sequence `data_import` produces: `ApplicationStore::open` runs once
/// on an empty data dir (no `ai_generations.db` yet, so the ordinary backfill
/// finds nothing and sets the marker immediately — the common case), THEN a
/// legacy row lands in `ai_generations.db` out of band, exactly as
/// `AiGenerationStore::import` replacing the table would produce for a
/// pre-`ApplicationStore`-era bundle. A second `ApplicationStore::open` alone
/// could never link it (the marker is already set); only the restore-specific
/// call can.
///
/// Mutation check: change `relink_legacy_generations_after_restore` to call
/// `self.backfill_from_generations` instead of `self.scan_legacy_generations`
/// (i.e. reintroduce the marker gate) and `list().len()` after the call stays
/// 0 — applied and reverted.
#[test]
fn relink_after_restore_bypasses_the_already_set_marker() {
    let dir = TempDir::new().unwrap();

    // Boot on an empty data dir: nothing to find, so the ordinary backfill
    // sets the marker immediately (own doc on `backfill_from_generations`).
    let store = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(store.list().len(), 0, "precondition: nothing to find yet");
    assert!(
        legacy_backfill_marker_set(dir.path()),
        "precondition: the marker is set by the first boot, before the row below ever exists"
    );

    // A legacy row lands afterward — the shape a bundle-replace import
    // produces, out of band from any `ApplicationStore::open` call.
    let gen_conn = open_gen_db(dir.path());
    insert_gen(
        &gen_conn,
        "gen-restored",
        "https://acme.com/jobs/restored",
        None,
    );

    store
        .relink_legacy_generations_after_restore(dir.path())
        .unwrap();

    assert_eq!(
        store.list().len(),
        1,
        "the restore-specific pass must create the Application the boot-time \
         marker now permanently blocks"
    );
    assert!(
        gen_application_id(&gen_conn, "gen-restored").is_some(),
        "the restored generation must end up linked"
    );
}

/// **FIX-4 mutation guard: the marker is set LAST, not on partial
/// progress.** `backfill_from_generations`'s own doc promises a run that
/// errors PARTWAY through is retried on the next boot, not silently marked
/// done — only the success path was covered before this test. A `BEFORE
/// UPDATE` trigger poisons the SECOND row's link-back write only, after the
/// FIRST row's Application already committed on the applications
/// connection — a genuine partial failure, not a total pre-loop I/O failure
/// (which wouldn't prove "partway").
///
/// `gen-fail` still gets an Application CREATED before its own write fails —
/// `upsert_internal` (applications.db) runs before the link-back `UPDATE`
/// (ai_generations.db) inside the loop body, so the poisoned row's failure
/// lands strictly between the two, not before either.
///
/// Mutation check: move `self.mark_legacy_backfill_done()` in
/// `backfill_from_generations` to run unconditionally (e.g. before the `?`
/// on `scan_legacy_generations`) and the first `legacy_backfill_marker_set`
/// assertion (expected `false`) fails — applied and reverted.
///
/// (FIX-2's own swallowed-error regression has a dedicated test above,
/// `a_transient_marker_read_failure_does_not_re_run_the_scan` — this test's
/// trigger never makes `legacy_backfill_done`'s OWN read fail, only the
/// later link-back write, so it cannot mutation-test FIX-2 on its own.)
#[test]
fn a_run_that_errors_partway_does_not_set_the_marker() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db(dir.path());

    // Two legacy (pre-epoch) rows, distinct `created_at` so `ORDER BY
    // created_at ASC` deterministically processes `gen-ok` first.
    insert_gen_at(
        &gen_conn,
        "gen-ok",
        "https://acme.com/jobs/partial-ok",
        None,
        1_000,
    );
    insert_gen_at(
        &gen_conn,
        "gen-fail",
        "https://acme.com/jobs/partial-fail",
        None,
        2_000,
    );

    // Poison ONLY `gen-fail`'s link-back write.
    gen_conn
        .execute_batch(
            "CREATE TRIGGER trg_poison BEFORE UPDATE OF application_id ON ai_generations
             WHEN NEW.id = 'gen-fail'
             BEGIN SELECT RAISE(ABORT, 'simulated failure'); END;",
        )
        .unwrap();

    // `open()` swallows the backfill error non-fatally (own doc) and still
    // returns `Ok` — matches the real boot path exactly.
    let store = ApplicationStore::open(dir.path()).unwrap();
    assert!(
        !legacy_backfill_marker_set(dir.path()),
        "a run that errored partway through must not set the one-shot marker"
    );
    assert_eq!(
        store.list().len(),
        2,
        "both rows reach `upsert_internal` before either write can fail — \
         gen-fail's Application IS created, only its link-back write fails"
    );
    assert!(
        gen_application_id(&gen_conn, "gen-ok").is_some(),
        "gen-ok's link-back write completed before gen-fail's poisoned one ran"
    );
    assert_eq!(
        gen_application_id(&gen_conn, "gen-fail"),
        None,
        "gen-fail's own link-back write is the one that failed"
    );
    drop(store); // release the connection before the reboot below.

    // Un-poison, then reboot: the retry must finish the row it never reached.
    gen_conn.execute_batch("DROP TRIGGER trg_poison;").unwrap();
    let rebooted = ApplicationStore::open(dir.path()).unwrap();
    assert!(
        legacy_backfill_marker_set(dir.path()),
        "the retry must succeed once every row is processed and set the marker"
    );
    assert_eq!(
        rebooted.list().len(),
        2,
        "still 2 — the retry must merge gen-fail into ITS ALREADY-CREATED \
         Application (job_url lookup in `upsert_internal`), not spawn a \
         duplicate, while skipping the already-linked gen-ok entirely"
    );
    assert!(
        gen_application_id(&gen_conn, "gen-fail").is_some(),
        "gen-fail must end up linked once the retry succeeds"
    );
}

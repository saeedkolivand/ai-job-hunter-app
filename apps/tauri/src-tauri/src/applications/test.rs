use super::*;
use rusqlite::Connection;
use tempfile::TempDir;

// ── helpers shared by the new gap tests ──────────────────────────────────────

/// Insert a bare generation row directly into `ai_generations.db`, setting
/// `application_id` to the supplied value (or NULL when None).  Used by the
/// delete/detach cross-store tests so they don't depend on the backfill path.
fn insert_gen_with_app_id(
    gen_conn: &Connection,
    id: &str,
    job_url: &str,
    application_id: Option<&str>,
) {
    gen_conn
        .execute(
            "INSERT INTO ai_generations
             (id, created_at, company_name, job_url, board, application_id)
             VALUES (?1, 1000, 'Acme', ?2, 'linkedin', ?3)",
            rusqlite::params![id, job_url, application_id],
        )
        .unwrap();
}

/// Return the `application_id` column for a generation row (None when NULL).
fn gen_application_id(gen_conn: &Connection, gen_id: &str) -> Option<String> {
    gen_conn
        .query_row(
            "SELECT application_id FROM ai_generations WHERE id = ?1",
            rusqlite::params![gen_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .unwrap()
}

/// Return the number of rows in `ai_generations` matching an `application_id`.
fn gen_count_for_app(gen_conn: &Connection, application_id: &str) -> i64 {
    gen_conn
        .query_row(
            "SELECT COUNT(*) FROM ai_generations WHERE application_id = ?1",
            rusqlite::params![application_id],
            |r| r.get(0),
        )
        .unwrap()
}

/// Open (or create) the `ai_generations.db` in `dir` and run
/// `AiGenerationStore`'s own migrations so all columns — including
/// `application_id` — exist before the test inserts rows.
/// Returns an open `Connection` for direct SQL assertions.
///
/// We let the store migrations run rather than hand-rolling the schema so that
/// future schema additions don't break these tests silently, and so we never
/// hit "duplicate column" errors from a CREATE TABLE that already includes
/// columns the migrations try to ADD.
fn open_gen_db_with_app_id_col(dir: &std::path::Path) -> Connection {
    // Opening the store runs all migrations (including add_application_id).
    // We then drop it immediately; the DB file stays on disk.
    {
        let _store = crate::ai_generations::AiGenerationStore::open(&dir.to_path_buf()).unwrap();
    }
    // Re-open raw for direct SQL reads/writes in the test.
    Connection::open(dir.join("ai_generations.db")).unwrap()
}

fn meta(company: &str, title: &str) -> ApplicationMeta {
    ApplicationMeta {
        company: company.into(),
        title: title.into(),
        candidate: "Jane".into(),
        brief: String::new(),
        answers: vec![],
    }
}

#[test]
fn normalize_strips_www_query_fragment_and_trailing_slash() {
    assert_eq!(
        normalize_job_url("https://WWW.Example.com/Jobs/123/?utm=x#frag"),
        "https://example.com/jobs/123"
    );
    assert_eq!(
        normalize_job_url("https://example.com/"),
        "https://example.com"
    );
    assert_eq!(normalize_job_url("  "), "");
    assert_eq!(
        normalize_job_url("https://www.acme.io/job/9/"),
        normalize_job_url("https://acme.io/job/9?ref=foo")
    );
}

#[test]
fn rejects_dangerous_url_schemes_to_empty() {
    // Explicit non-http(s) schemes are neutralized to "" (treated as "no url")
    // so an import-borne or Track-modal payload is never stored as an openable link.
    // `javascript:` has a scheme but no `://` — the `scheme:` form must be caught.
    assert_eq!(normalize_job_url("javascript:alert(1)"), "");
    assert_eq!(
        normalize_job_url("data:text/html,<script>alert(1)</script>"),
        ""
    );
    assert_eq!(normalize_job_url("file:///etc/passwd"), "");
    assert_eq!(normalize_job_url("vbscript:msgbox(1)"), "");
    assert_eq!(normalize_job_url("blob:https://evil.example/uuid"), "");
    // Case-insensitive scheme detection: mixed-case dangerous scheme still rejected.
    assert_eq!(normalize_job_url("JavaScript:alert(1)"), "");
}

#[test]
fn allows_http_and_https_including_mixed_case_scheme() {
    // http(s) round-trips with the exact prior normalization; mixed-case scheme is
    // lowercased like before and is NOT rejected by the dangerous-scheme guard.
    assert_eq!(
        normalize_job_url("HTTP://Example.com/Job/1/"),
        "http://example.com/job/1"
    );
    assert_eq!(
        normalize_job_url("HTTPS://WWW.Acme.io/job/9?ref=foo"),
        "https://acme.io/job/9"
    );
}

#[test]
fn scheme_less_input_with_colon_in_path_is_not_misclassified() {
    // A `:` inside the path/query must NOT look like a scheme — scheme-less input
    // keeps its exact prior behavior (host/path preserved, query dropped).
    assert_eq!(
        normalize_job_url("example.com/job/9?x=a:b"),
        "example.com/job/9"
    );
    assert_eq!(
        normalize_job_url("www.example.com/jobs/123/"),
        "example.com/jobs/123"
    );
}

#[test]
fn status_from_id_is_relaxed_and_round_trips() {
    for &s in ApplicationStatus::ALL {
        assert_eq!(ApplicationStatus::from_id(s.as_id()), s);
    }
    assert_eq!(
        ApplicationStatus::from_id("some_future_stage"),
        ApplicationStatus::Saved
    );
}

#[test]
fn terminal_and_pre_apply_classification() {
    assert!(ApplicationStatus::Accepted.is_terminal());
    assert!(ApplicationStatus::Rejected.is_terminal());
    assert!(ApplicationStatus::Withdrawn.is_terminal());
    assert!(!ApplicationStatus::Ghosted.is_terminal());
    assert!(!ApplicationStatus::Applied.is_terminal());
    assert!(ApplicationStatus::Saved.is_pre_apply());
    assert!(!ApplicationStatus::Applied.is_pre_apply());
}

#[test]
fn save_then_generate_merges_into_one_application() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();

    let saved_id = store
        .upsert_for_origin(
            "https://acme.com/job/1?x=1",
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    let gen_id = store
        .upsert_for_origin(
            "https://www.acme.com/job/1/",
            "linkedin",
            &meta("", "Senior Engineer"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    assert_eq!(saved_id, gen_id, "same normalized url must merge");
    let all = store.list();
    assert_eq!(all.len(), 1);
    let app = &all[0];
    assert_eq!(app.status, ApplicationStatus::Applied);
    assert!(app.applied_at.is_some());
    assert_eq!(app.title, "Senior Engineer");
    assert_eq!(app.company, "Acme");
}

#[test]
fn applied_job_urls_excludes_saved() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    store
        .upsert_for_origin(
            "https://a.com/1",
            "b",
            &meta("A", "T"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    store
        .upsert_for_origin(
            "https://b.com/2",
            "b",
            &meta("B", "T"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();
    let applied = store.applied_job_urls();
    assert!(applied.contains("https://b.com/2"));
    assert!(!applied.contains("https://a.com/1"), "saved is not applied");
}

#[test]
fn set_status_appends_event_and_sets_applied_at() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .upsert_for_origin("", "", &meta("C", "T"), ApplicationOrigin::Saved, None)
        .unwrap();
    assert_eq!(store.get(&id).unwrap().status, ApplicationStatus::Saved);
    assert!(store.get(&id).unwrap().applied_at.is_none());

    store
        .set_status(&id, ApplicationStatus::Interviewing, "phone screen")
        .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Interviewing);
    assert!(app.applied_at.is_some(), "leaving saved sets applied_at");

    let events = store.events(&id);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].from_status, "saved");
    assert_eq!(events[1].to_status, "interviewing");
    assert_eq!(events[1].note, "phone screen");
}

#[test]
fn update_fields_patches_only_provided() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store.track_manual("", "", &meta("C", "T")).unwrap();
    store
        .update_fields(
            &id,
            Some("call back Tuesday".into()),
            Some(Some(123)),
            None,
            Some("Recruiter".into()),
            None,
        )
        .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.notes, "call back Tuesday");
    assert_eq!(app.next_action_at, Some(123));
    assert_eq!(app.contact_name, "Recruiter");
    assert_eq!(app.comp, "");
}

#[test]
fn delete_removes_application_and_events() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store.track_manual("", "", &meta("C", "T")).unwrap();
    store.delete(&id, true).unwrap();
    assert!(store.get(&id).is_none());
    assert!(store.events(&id).is_empty());
}

/// Seed legacy (pre-migration) generation rows using the OLD ai_generations
/// schema that existed before the `application_id` column was added.
///
/// IMPORTANT: this helper deliberately uses the OLD schema and must NOT be
/// updated to match the live schema.  Its purpose is to verify that the
/// backfill migration runs correctly against data that predates the migration.
fn seed_legacy_generations(dir: &std::path::Path, rows: &[(&str, &str, &str)]) {
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
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    assert!(store.list().is_empty());
}

#[test]
fn export_import_round_trips() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    store
        .upsert_for_origin(
            "https://x.com/1",
            "b",
            &meta("X", "T"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();
    let bundle = store.export();

    let dir2 = TempDir::new().unwrap();
    let store2 = ApplicationStore::open(dir2.path()).unwrap();
    let n = store2.import(&bundle).unwrap();
    assert_eq!(n, 1);
    assert_eq!(store2.list().len(), 1);
    assert_eq!(store2.list()[0].company, "X");
}

/// HIGH blocker fix: `DataStore::import` must return `Err(AppError::Parse(…))` when
/// the supplied JSON value is not a JSON array.  The production path at mod.rs
/// line ~825 calls `.as_array().ok_or_else(|| AppError::Parse(…))`.
#[test]
fn import_non_array_returns_parse_error() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();

    // Passing an object instead of an array must be rejected.
    let result = store.import(&serde_json::json!({"key": "value"}));
    assert!(result.is_err(), "non-array input must return Err");

    // Check it is specifically the Parse variant.
    match result.unwrap_err() {
        AppError::Parse(msg) => {
            assert!(
                msg.contains("applications"),
                "error message should mention 'applications', got: {msg}"
            );
        }
        other => panic!("expected AppError::Parse, got: {other:?}"),
    }

    // Sanity: the store is still empty — the failed import must not have written anything.
    assert!(
        store.list().is_empty(),
        "store must be empty after a failed import"
    );
}

/// Happy-path companion: import a valid array after the error-path test to
/// confirm the store is still operational.
#[test]
fn import_non_array_does_not_corrupt_subsequent_happy_path() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();

    // First call fails.
    assert!(store.import(&serde_json::json!(42)).is_err());

    // Subsequent valid import still works.
    store
        .upsert_for_origin(
            "https://x.com/1",
            "b",
            &meta("X", "T"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();
    let bundle = store.export();

    let dir2 = TempDir::new().unwrap();
    let store2 = ApplicationStore::open(dir2.path()).unwrap();
    let n = store2.import(&bundle).unwrap();
    assert_eq!(n, 1, "valid import after failed import must succeed");
    assert_eq!(store2.list()[0].company, "X");
}

/// MEDIUM: `update_fields` null-vs-absent semantics.
///
/// - `Some(None)` for `next_action_at` must CLEAR the field to `None`.
/// - `None` for `next_action_at` must leave the prior value UNCHANGED.
#[test]
fn update_fields_next_action_at_null_clears_and_absent_preserves() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store.track_manual("", "", &meta("C", "T")).unwrap();

    // Set a value.
    store
        .update_fields(&id, None, Some(Some(999)), None, None, None)
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        Some(999),
        "precondition: value set"
    );

    // Passing `Some(None)` must CLEAR the value.
    store
        .update_fields(&id, None, Some(None), None, None, None)
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        None,
        "Some(None) must clear next_action_at"
    );

    // Set value again.
    store
        .update_fields(&id, None, Some(Some(456)), None, None, None)
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        Some(456),
        "precondition: value re-set"
    );

    // Passing `None` (field absent) must PRESERVE the prior value.
    store
        .update_fields(&id, None, None, None, None, None)
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        Some(456),
        "None must leave next_action_at unchanged"
    );
}

/// MEDIUM: `set_status` must advance `updated_at` — assert `>=` old value while
/// also confirming a status_event was appended, which together proves the call
/// was not a no-op.  We avoid `>` because ms-resolution clocks can tick the same
/// value; the event-count assertion is the correctness proof.
#[test]
fn set_status_bumps_updated_at_and_appends_event() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store.track_manual("", "", &meta("C", "T")).unwrap();

    let before = store.get(&id).unwrap().updated_at;
    let events_before = store.events(&id).len();

    store
        .set_status(&id, ApplicationStatus::Screening, "moved to screening")
        .unwrap();

    let after = store.get(&id).unwrap();
    // updated_at must not go backwards.
    assert!(
        after.updated_at >= before,
        "updated_at must advance after set_status (before={before}, after={})",
        after.updated_at
    );
    // The status event is the hard proof that set_status actually ran.
    let events_after = store.events(&id).len();
    assert_eq!(
        events_after,
        events_before + 1,
        "set_status must append exactly one new status event"
    );
    let last_event = store.events(&id).into_iter().last().unwrap();
    assert_eq!(last_event.to_status, "screening");
    assert_eq!(last_event.note, "moved to screening");
}

/// Parity guard: the Rust stage registry order/ids must match the shared-TS
/// `APPLICATION_STAGES`. The expected list is HARD-CODED from the TS `as const`
/// so any drift on either side fails the build (see
/// packages/shared/src/types/index.ts → APPLICATION_STAGES).
#[test]
fn rust_stage_registry_matches_shared_ts() {
    let expected = [
        "saved",
        "applied",
        "screening",
        "interviewing",
        "offer",
        "accepted",
        "rejected",
        "ghosted",
        "withdrawn",
    ];
    let actual: Vec<&str> = ApplicationStatus::ALL.iter().map(|s| s.as_id()).collect();
    assert_eq!(
        actual, expected,
        "ApplicationStatus::ALL drifted from shared-TS APPLICATION_STAGES"
    );
}

// ── Gap 1: generate-save demotion behaviour ───────────────────────────────────
//
// The command `ai_generations_save` (ADR 0001) calls:
//   1. ApplicationStore::upsert_for_origin(…, Generate, …)  → Application row
//   2. AiGenerationStore::save_application(rec)             → generation row
//
// These tests mirror that two-step call at the store level (the Tauri command
// wrapper cannot be unit-tested without a live AppHandle).

#[test]
fn generate_save_creates_one_application_with_applied_status() {
    // Calling upsert_for_origin with Generate origin for the first time must
    // produce exactly ONE Application row with status `applied` and a set
    // `applied_at`.
    let dir = TempDir::new().unwrap();
    let app_store = ApplicationStore::open(dir.path()).unwrap();

    let app_id = app_store
        .upsert_for_origin(
            "https://acme.com/job/42",
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    let apps = app_store.list();
    assert_eq!(apps.len(), 1, "exactly one Application must be created");
    let app = app_store.get(&app_id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Applied,
        "Generate origin must yield status=applied"
    );
    assert!(
        app.applied_at.is_some(),
        "applied_at must be set for Generate origin"
    );
}

#[test]
fn generate_save_second_generation_same_url_merge_into_one_gen_row_and_one_application() {
    // Saving two generations (e.g. résumé then cover) for the same normalized
    // url must produce ONE Application and TWO generation rows — the aggregate
    // stays single while the child document table grows.
    let dir = TempDir::new().unwrap();
    let app_store = ApplicationStore::open(dir.path()).unwrap();
    // Open gen store after app_store so the backfill migration has already run
    // and the application_id column exists.
    let gen_store =
        crate::ai_generations::AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let url = "https://acme.com/job/42";

    // First save: résumé generation.
    let app_id_1 = app_store
        .upsert_for_origin(
            url,
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();
    let rec1 = crate::ai_generations::AiGenerationRecord {
        id: "gen-resume".into(),
        created_at: crate::ai_generations::now_ms(),
        candidate_name: "Jane".into(),
        job_title: "Engineer".into(),
        company_name: "Acme".into(),
        resume_language: "en".into(),
        job_ad_language: "en".into(),
        target_language: "en".into(),
        mismatch: false,
        top_requirements: vec![],
        mode: "ats".into(),
        resume_text: "RESUME".into(),
        cover_letter_text: String::new(),
        job_ad: "JD".into(),
        job_url: url.into(),
        board: "linkedin".into(),
        application_answers: vec![],
        company_brief: String::new(),
        interview_questions: vec![],
        application_id: None,
    };
    gen_store.save_application(rec1).unwrap();

    // Second save: cover-letter generation for the same url.
    let app_id_2 = app_store
        .upsert_for_origin(
            url,
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();
    let rec2 = crate::ai_generations::AiGenerationRecord {
        id: "gen-cover".into(),
        created_at: crate::ai_generations::now_ms(),
        candidate_name: "Jane".into(),
        job_title: "Engineer".into(),
        company_name: "Acme".into(),
        resume_language: "en".into(),
        job_ad_language: "en".into(),
        target_language: "en".into(),
        mismatch: false,
        top_requirements: vec![],
        mode: "ats".into(),
        resume_text: String::new(),
        cover_letter_text: "COVER".into(),
        job_ad: "JD".into(),
        job_url: url.into(),
        board: "linkedin".into(),
        application_answers: vec![],
        company_brief: String::new(),
        interview_questions: vec![],
        application_id: None,
    };
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
        ApplicationStatus::Applied,
        "Application status must remain applied"
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

// ── Gap 2: applied_job_urls excludes saved, includes any non-saved status ─────
//
// The existing `applied_job_urls_excludes_saved` test only checks `saved` vs
// `applied`.  This test also checks that after a `saved` Application is advanced
// to a non-saved status it IS included, covering the transition edge.

#[test]
fn applied_job_urls_includes_application_after_status_leaves_saved() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();

    // Create a saved Application.
    let id = store
        .upsert_for_origin(
            "https://beta.com/job/1",
            "linkedin",
            &meta("Beta", "Dev"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    // Must NOT be in applied_job_urls while still saved.
    assert!(
        !store.applied_job_urls().contains("https://beta.com/job/1"),
        "saved Application must not appear in applied_job_urls"
    );

    // Advance to Screening (a non-saved, non-applied status).
    store
        .set_status(&id, ApplicationStatus::Screening, "phone screen booked")
        .unwrap();

    // NOW it must appear.
    assert!(
        store.applied_job_urls().contains("https://beta.com/job/1"),
        "Application must appear in applied_job_urls after leaving saved"
    );
}

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
    let gen_conn = open_gen_db_with_app_id_col(dir.path());

    let app_store = ApplicationStore::open(dir.path()).unwrap();
    let gen_store =
        crate::ai_generations::AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    // Create an Application.
    let app_id = app_store
        .upsert_for_origin(
            "https://acme.com/job/99",
            "linkedin",
            &meta("Acme", "Dev"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    // Pre-link two generation rows to this Application (simulates what a
    // live session would have after the FK write-back).
    insert_gen_with_app_id(&gen_conn, "gen-a", "https://acme.com/job/99", Some(&app_id));
    insert_gen_with_app_id(&gen_conn, "gen-b", "https://acme.com/job/99", Some(&app_id));

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
    let gen_conn = open_gen_db_with_app_id_col(dir.path());

    let app_store = ApplicationStore::open(dir.path()).unwrap();
    let gen_store =
        crate::ai_generations::AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let app_id = app_store
        .upsert_for_origin(
            "https://acme.com/job/100",
            "linkedin",
            &meta("Acme", "Dev"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    insert_gen_with_app_id(
        &gen_conn,
        "gen-c",
        "https://acme.com/job/100",
        Some(&app_id),
    );
    insert_gen_with_app_id(
        &gen_conn,
        "gen-d",
        "https://acme.com/job/100",
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

// ── R1 — ApplicationStore::import rollback regression guard ──────────────────
//
// `DataStore::import` for ApplicationStore runs clear+repopulate in ONE
// transaction. These tests pin that contract: a malformed LATER record must
// abort the import and leave PRIOR data fully intact.

/// Minimal valid Application JSON, compatible with the `Application` serde shape.
fn valid_application_json(id: &str, status: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "status": status,
        "appliedAt": null,
        "createdAt": 1_000_000u64,
        "updatedAt": 1_000_000u64,
        "jobUrl": "",
        "board": "linkedin",
        "company": "Acme",
        "title": "Engineer",
        "candidate": "Jane",
        "answers": [],
        "brief": "",
        "notes": "",
        "nextActionAt": null,
        "comp": "",
        "contactName": "",
        "contactEmail": ""
    })
}

#[test]
fn application_import_malformed_later_record_rolls_back_prior_data() {
    // R1 — Seed store with prior data, then import a bundle whose LAST element
    // has `status` as a number (must be a string). Import must fail and prior
    // data must be fully intact.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();

    // Seed with a known application.
    let prior_id = store
        .track_manual("", "", &meta("Prior Corp", "Prior Role"))
        .unwrap();
    let prior_count = store.list().len();
    assert_eq!(prior_count, 1, "precondition: one prior record");

    // Bundle: first element is valid, second has a numeric status (invalid type).
    let bundle = serde_json::json!([
        valid_application_json("new-1", "applied"),
        {
            "id": "bad-2",
            "status": 42,           // ← wrong type: must be string
            "appliedAt": null,
            "createdAt": 2_000_000u64,
            "updatedAt": 2_000_000u64,
            "jobUrl": "",
            "board": "",
            "company": "Bad Corp",
            "title": "Bad Role",
            "candidate": "",
            "answers": [],
            "brief": "",
            "notes": "",
            "nextActionAt": null,
            "comp": "",
            "contactName": "",
            "contactEmail": ""
        }
    ]);

    let result = crate::data_store::DataStore::import(&store, &bundle);
    assert!(
        result.is_err(),
        "import of a bundle with a malformed record must return Err; got Ok"
    );

    // PRIOR data must be fully intact — the transaction must have rolled back.
    let remaining = store.list();
    assert_eq!(
        remaining.len(),
        1,
        "import rollback must leave prior records intact; got {} records (expected 1)",
        remaining.len()
    );
    assert_eq!(
        remaining[0].id, prior_id,
        "the surviving record must be the original prior application, not a partial import"
    );
    // Status events for the prior record must also still be present.
    assert!(
        !store.events(&prior_id).is_empty(),
        "status events for the prior application must survive a rolled-back import"
    );
}

#[test]
fn application_import_all_valid_records_replaces_prior_data() {
    // R1 happy-path: confirms the import transaction commits when the bundle is
    // fully valid — prior data is replaced with the imported records.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();

    store
        .track_manual("", "", &meta("Old Corp", "Old Role"))
        .unwrap();
    assert_eq!(store.list().len(), 1, "precondition: one prior record");

    let bundle = serde_json::json!([
        valid_application_json("new-1", "applied"),
        valid_application_json("new-2", "saved"),
    ]);

    let n = crate::data_store::DataStore::import(&store, &bundle).unwrap();
    assert_eq!(n, 2, "import must report 2 records restored");

    let list = store.list();
    assert_eq!(list.len(), 2, "store must hold the 2 imported records");
    let ids: Vec<&str> = list.iter().map(|a| a.id.as_str()).collect();
    assert!(
        ids.contains(&"new-1") && ids.contains(&"new-2"),
        "both imported ids must be present; got {ids:?}"
    );
    // Prior record must be gone.
    assert!(
        list.iter().all(|a| a.company != "Old Corp"),
        "prior record 'Old Corp' must not survive a successful import"
    );
}

use super::*;
use rusqlite::Connection;
use tempfile::TempDir;

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
    assert_eq!(normalize_job_url("data:text/html,<script>alert(1)</script>"), "");
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

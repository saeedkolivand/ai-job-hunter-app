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

/// Same as [`insert_gen_with_app_id`], but with an explicit `created_at` —
/// for the `APPLICATIONS_FEATURE_EPOCH_MS` vintage-gate tests, which need a
/// row on a specific side of that boundary rather than the hardcoded legacy
/// `1000` every other caller of `insert_gen_with_app_id` relies on.
fn insert_gen_with_app_id_and_created_at(
    gen_conn: &Connection,
    id: &str,
    job_url: &str,
    application_id: Option<&str>,
    created_at: i64,
) {
    gen_conn
        .execute(
            "INSERT INTO ai_generations
             (id, created_at, company_name, job_url, board, application_id)
             VALUES (?1, ?2, 'Acme', ?3, 'linkedin', ?4)",
            rusqlite::params![id, created_at, job_url, application_id],
        )
        .unwrap();
}

/// 2026-07-01T00:00:00Z — safely after
/// [`super::APPLICATIONS_FEATURE_EPOCH_MS`], an arbitrary fixed value rather
/// than a wall-clock read so these tests are deterministic.
const MODERN_CREATED_AT: i64 = 1_782_864_000_000;

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

/// Named-field view of [`ApplicationStore::update_fields`]' ten positional
/// arguments, for tests.
///
/// `update_fields(&id, None, None, None, None, None, None, None, Some(x), None)`
/// is unreadable and silently wrong if an argument shifts; `Patch { recipient_name:
/// Some(x), ..Default::default() }` names what the test actually means and is
/// checked by the compiler. Adding a field to `update_fields` breaks
/// [`patch`] once, here, instead of every call site.
#[derive(Default)]
struct Patch {
    notes: Option<String>,
    /// Outer `None` = leave the reminder alone; `Some(None)` = clear it.
    next_action_at: Option<Option<u64>>,
    comp: Option<String>,
    contact_name: Option<String>,
    contact_email: Option<String>,
    job_description: Option<String>,
    job_summary: Option<String>,
    recipient_name: Option<String>,
    recipient_email: Option<String>,
}

/// Forward a [`Patch`] to [`ApplicationStore::update_fields`] in the one place
/// the positional order has to be spelled out.
fn patch(store: &ApplicationStore, id: &str, p: Patch) -> AppResult<()> {
    store.update_fields(
        id,
        p.notes,
        p.next_action_at,
        p.comp,
        p.contact_name,
        p.contact_email,
        p.job_description,
        p.job_summary,
        p.recipient_name,
        p.recipient_email,
    )
}

/// Shorthand for the overwhelmingly common single-field case: set/clear the
/// follow-up reminder.
fn set_reminder(store: &ApplicationStore, id: &str, at: Option<u64>) {
    patch(
        store,
        id,
        Patch {
            next_action_at: Some(at),
            ..Default::default()
        },
    )
    .unwrap();
}

fn meta(company: &str, title: &str) -> ApplicationMeta {
    ApplicationMeta {
        company: company.into(),
        title: title.into(),
        candidate: "Jane".into(),
        brief: String::new(),
        job_description: String::new(),
        answers: vec![],
        job_summary: String::new(),
        salary_min: None,
        salary_max: None,
        salary_currency: None,
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
fn embedded_control_characters_cannot_smuggle_a_scheme_past_the_guard() {
    // HTML and the WHATWG URL parser REMOVE embedded tab/CR/LF before parsing, so
    // this string is `javascript:` to any consumer — while a raw-byte scheme scan
    // sees the scheme-less `"java"` and would store the payload verbatim.
    // Stripping C0 controls first makes the guard see what a consumer sees.
    assert_eq!(normalize_job_url("java\nscript:alert(1)"), "");
    assert_eq!(normalize_job_url("java\tscript:alert(1)"), "");
    assert_eq!(normalize_job_url("java\rscript:alert(1)"), "");
    assert_eq!(normalize_job_url("ja\u{0}vascript:alert(1)"), "");
    assert_eq!(normalize_job_url("da\u{7F}ta:text/html,x"), "");

    // A control character anywhere else is removed too, so nothing unprintable is
    // ever stored as part of the dedup key.
    assert_eq!(
        normalize_job_url("https://example.com/job/\u{0}1"),
        "https://example.com/job/1"
    );
    // Leading whitespace exposed by the removal is still trimmed away.
    assert_eq!(
        normalize_job_url("\u{0} https://example.com/job/1"),
        "https://example.com/job/1"
    );
    // A control-only input degrades to "no url" rather than a bare host.
    assert_eq!(normalize_job_url("\u{0}\u{1}"), "");
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
fn retains_per_host_identifying_query_params_for_dedup() {
    // Indeed carries the job id in the query (`?jk=<id>`). Two DISTINCT ids must
    // yield two DISTINCT keys — the collision bug was that every Indeed job
    // normalized to a single `/viewjob` key and merged onto one Application.
    assert_ne!(
        normalize_job_url("https://indeed.com/viewjob?jk=aaa"),
        normalize_job_url("https://indeed.com/viewjob?jk=bbb")
    );
    // The identifying param survives verbatim (lowercased with the rest of the url).
    assert_eq!(
        normalize_job_url("https://www.indeed.com/viewjob?jk=abc123"),
        "https://indeed.com/viewjob?jk=abc123"
    );
    // Country TLD (de.indeed.com) is covered by the `.indeed.com` suffix match.
    assert_eq!(
        normalize_job_url("https://de.indeed.com/viewjob?jk=xyz"),
        "https://de.indeed.com/viewjob?jk=xyz"
    );
    // Same job, tracking-only query differences → SAME key (jk retained, the rest
    // dropped). Param ORDER must not matter — allowlist order is authoritative.
    assert_eq!(
        normalize_job_url("https://indeed.com/viewjob?jk=abc&from=serp&utm_source=x"),
        normalize_job_url("https://indeed.com/viewjob?utm_campaign=y&jk=abc")
    );
    assert_eq!(
        normalize_job_url("https://indeed.com/viewjob?jk=abc&from=serp&utm_source=x"),
        "https://indeed.com/viewjob?jk=abc"
    );
    // Path-based URL on a non-allowlisted host is UNCHANGED: the whole query is still
    // dropped (LinkedIn puts the id in the path, so it is unaffected by this fix).
    assert_eq!(
        normalize_job_url("https://www.linkedin.com/jobs/view/12345?trk=abc&refId=z"),
        "https://linkedin.com/jobs/view/12345"
    );
    // A non-Indeed host never retains a query param, even a `jk` lookalike.
    assert_eq!(
        normalize_job_url("https://acme.example/viewjob?jk=abc"),
        "https://acme.example/viewjob"
    );
}

/// Companion to the `canonical_xing_*`/`canonical_stepstone_*` tests in
/// `scraping::scrape_url::test` (PR 7): both hosts put the job id in the PATH, so
/// neither has an entry in `identifying_query_params` — the whole query must be
/// dropped, not just the tracking param, via `retain_identifying_params` seeing an
/// empty allowlist for the host. Pinned against the real detail-URL shapes
/// (including their tracking params, Xing `?ijt=`/StepStone `?rltr=`) captured
/// live during PR 7's browser probe, so this fails if either host ever gains a
/// query-param allowlist entry without an accompanying deliberate decision, or if
/// `retain_identifying_params`/`identifying_query_params` regress to leaking an
/// unlisted host's query through.
#[test]
fn xing_and_stepstone_tracking_query_is_dropped_entirely() {
    assert_eq!(
        normalize_job_url(
            "https://www.xing.com/jobs/berlin-senior-software-engineer-155853218?ijt=jb_55"
        ),
        "https://xing.com/jobs/berlin-senior-software-engineer-155853218"
    );
    assert_eq!(
        normalize_job_url(
            "https://www.stepstone.de/stellenangebote--Software-Engineer-m-w-d-Distribution-Berlin-GEMA-Gesellschaft-fuer-musik-Auffuehrungs-und-mechan-Vervielfaeltigungsrechte--14009455-inline.html?rltr=1_1_25_seorl_m_0_0_0_0_0_0"
        ),
        "https://stepstone.de/stellenangebote--software-engineer-m-w-d-distribution-berlin-gema-gesellschaft-fuer-musik-auffuehrungs-und-mechan-vervielfaeltigungsrechte--14009455-inline.html"
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

/// Documents WHY `ApplyByEmailTab::persistDraft` refuses to save for a URL-less
/// Application. `row_by_job_url_conn` returns `None` unconditionally for an
/// empty url, so `upsert_for_origin` can never merge and mints a BRAND-NEW
/// `applied` Application on every call. Any renderer surface that saves
/// per-keystroke/per-action for a URL-less job therefore forks a duplicate
/// application each time — the guard in the email tab is what prevents that.
#[test]
fn upsert_for_origin_forks_a_new_application_for_every_empty_url_save() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();

    let first = store
        .upsert_for_origin(
            "",
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();
    let second = store
        .upsert_for_origin(
            "",
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    assert_ne!(
        first, second,
        "an empty url can never match an existing row, so each save mints a new id"
    );
    let all = store.list();
    assert_eq!(all.len(), 2, "two phantom Applications, not one merged row");
    assert!(
        all.iter().all(|a| a.status == ApplicationStatus::Applied),
        "each phantom is created already `applied`, so it sorts to the top of the list"
    );
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
            None,
            None,
            None,
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
        .update_fields(
            &id,
            None,
            Some(Some(999)),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        Some(999),
        "precondition: value set"
    );

    // Passing `Some(None)` must CLEAR the value.
    store
        .update_fields(
            &id,
            None,
            Some(None),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        None,
        "Some(None) must clear next_action_at"
    );

    // Set value again.
    store
        .update_fields(
            &id,
            None,
            Some(Some(456)),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        Some(456),
        "precondition: value re-set"
    );

    // Passing `None` (field absent) must PRESERVE the prior value.
    store
        .update_fields(&id, None, None, None, None, None, None, None, None, None)
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

/// `transition_status_if` (the CAS the extension bridge's `status.update`
/// guard relies on): a matching `from` transitions the row, appends exactly
/// one status event, and sets `applied_at` — same field semantics as
/// `set_status`.
#[test]
fn transition_status_if_matches_from_transitions_and_appends_event() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .upsert_for_origin(
            "https://x.com/1",
            "b",
            &meta("C", "T"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    let events_before = store.events(&id).len();

    let ok = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap();
    assert!(ok, "a matching `from` must transition and return true");

    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Applied);
    assert!(
        app.applied_at.is_some(),
        "applied_at must be set on saved -> applied"
    );

    let events_after = store.events(&id);
    assert_eq!(
        events_after.len(),
        events_before + 1,
        "exactly one event appended"
    );
    let last = events_after.last().unwrap();
    assert_eq!(last.from_status, "saved");
    assert_eq!(last.to_status, "applied");
    assert_eq!(last.note, "via extension");
}

/// A `from` that does NOT match the row's current status is refused
/// (`Ok(false)`) — no write, no event appended. This is the compare-and-set
/// guard itself: it must never transition an unexpected starting status.
#[test]
fn transition_status_if_refuses_when_from_does_not_match_current_status() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store.track_manual("", "", &meta("C", "T")).unwrap(); // starts `applied`
    let events_before = store.events(&id).len();

    let ok = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap();
    assert!(!ok, "a from mismatch must refuse (Ok(false)), never write");

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Applied,
        "status must be unchanged"
    );
    assert_eq!(
        store.events(&id).len(),
        events_before,
        "no event appended on refusal"
    );
}

/// The lost-race scenario the review flagged: two callers race the same
/// saved->applied transition. Only the FIRST succeeds; the SECOND must see
/// `Ok(false)` (the guard lost the race) — never a second event, never a
/// re-bumped `applied_at`.
#[test]
fn transition_status_if_second_racing_call_loses_and_appends_nothing() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .upsert_for_origin(
            "https://race.example/1",
            "b",
            &meta("C", "T"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let first = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap();
    assert!(first, "the first call wins the race");
    let applied_at_after_first = store.get(&id).unwrap().applied_at;
    let events_after_first = store.events(&id).len();

    // Simulate the lost race: a second concurrent caller attempts the exact
    // same saved -> applied transition after the first already committed.
    let second = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap();
    assert!(!second, "the second call must lose the race (Ok(false))");

    assert_eq!(
        store.events(&id).len(),
        events_after_first,
        "the losing call must not append a second status event"
    );
    assert_eq!(
        store.get(&id).unwrap().applied_at,
        applied_at_after_first,
        "the losing call must not bump applied_at again"
    );
}

/// A `saved` row CAN already carry a prior `applied_at` — from an earlier
/// `applied -> saved` demotion via the stage picker (`set_status` never clears
/// `applied_at` on a demotion to a pre-apply status). Re-transitioning that row
/// back to `applied` through `transition_status_if` (the extension bridge's
/// guard) must preserve the ORIGINAL `applied_at`, not stamp a fresh `now()` —
/// first-applied-wins, same semantics as `set_status`.
#[test]
fn transition_status_if_preserves_prior_applied_at_after_demotion_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .upsert_for_origin(
            "https://demote.example/1",
            "b",
            &meta("C", "T"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    // Simulate the applied -> saved round-trip via `set_status` (the stage
    // picker's path): leaving saved sets applied_at, then demoting back to
    // saved must NOT clear it.
    store
        .set_status(&id, ApplicationStatus::Applied, "applied")
        .unwrap();
    let original_applied_at = store.get(&id).unwrap().applied_at;
    assert!(
        original_applied_at.is_some(),
        "leaving saved must set applied_at"
    );

    store
        .set_status(&id, ApplicationStatus::Saved, "demoted back to saved")
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().applied_at,
        original_applied_at,
        "a saved demotion must not clear the prior applied_at"
    );

    // Pin an unmistakably distinct sentinel directly on the row so the final
    // assertion can't pass by clock-resolution coincidence with `now()` — it
    // must come from a genuine COALESCE preservation, not a lucky same-ms read.
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute(
            "UPDATE applications SET applied_at = 12345 WHERE id = ?1",
            rusqlite::params![id],
        )
        .unwrap();
    }

    let ok = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap();
    assert!(ok, "saved -> applied must still transition");

    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Applied);
    assert_eq!(
        app.applied_at,
        Some(12345),
        "transition_status_if must preserve the prior applied_at (first-applied-wins), not stamp a fresh now()"
    );
}

/// If the status-event INSERT fails, the whole transaction must roll back —
/// no status flip with a missing history row. Forces the failure by dropping
/// `status_events` out from under the store via a second raw connection to
/// the same db file (same trick the demotion round-trip test above uses to
/// poke the row directly), then asserts the row is UNCHANGED afterward.
#[test]
fn transition_status_if_rolls_back_status_when_event_insert_fails() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .upsert_for_origin(
            "https://rollback.example/1",
            "b",
            &meta("C", "T"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute("DROP TABLE status_events", []).unwrap();
    }

    let err = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .expect_err("the event insert must fail (no such table) and propagate");
    let _ = err; // exact AppError variant isn't the contract here, only that it's Err

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Saved,
        "the status UPDATE must roll back together with the failed event insert"
    );
    assert!(
        app.applied_at.is_none(),
        "applied_at must not be stamped when the whole transaction rolled back"
    );
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
        resume_text: "RESUME".into(),
        cover_letter_text: String::new(),
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
        resume_text: String::new(),
        cover_letter_text: "COVER".into(),
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

    // Pre-link two generation rows to this Application (simulates what a live
    // session would have after the FK write-back). They carry DISTINCT urls: the
    // per-job UNIQUE(job_url) index forbids two rows sharing one non-empty url,
    // and `remove_for_application` must still delete every linked row regardless.
    insert_gen_with_app_id(&gen_conn, "gen-a", "https://acme.com/job/99", Some(&app_id));
    insert_gen_with_app_id(
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

    // Distinct urls per the per-job UNIQUE(job_url) index (see the sibling test);
    // `detach_application` must still null the FK on every linked row.
    insert_gen_with_app_id(
        &gen_conn,
        "gen-c",
        "https://acme.com/job/100",
        Some(&app_id),
    );
    insert_gen_with_app_id(
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

// ── `link_orphaned_generations` — the FK backfill for existing installs ──────
//
// The FK fix (`commands::resume_pipeline::mod::persist_document`) stops NEW
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
    let gen_conn = open_gen_db_with_app_id_col(dir.path());
    let app_store = ApplicationStore::open(dir.path()).unwrap();

    // The Application: created by the apply flow, exactly like production —
    // `upsert_for_origin` normalizes the raw url internally.
    let raw_url = "https://acme.com/jobs/42?utm_source=newsletter";
    let app_id = app_store
        .upsert_for_origin(
            raw_url,
            "linkedin",
            &meta("Acme", "Dev"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    // The orphan: a staged-pipeline row saved BEFORE `persist_document`
    // established the FK. `save_application` always normalizes `job_url`
    // before writing it, so a REAL orphaned row carries the NORMALIZED form —
    // this seeds the same one, not the raw string, to match production.
    let normalized_url = normalize_job_url(raw_url);
    insert_gen_with_app_id(&gen_conn, "gen-orphan", &normalized_url, None);
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
    let gen_conn = open_gen_db_with_app_id_col(dir.path());
    let app_store = ApplicationStore::open(dir.path()).unwrap();
    let gen_store =
        crate::ai_generations::AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let url = normalize_job_url("https://acme.com/jobs/43");
    let app_id = app_store
        .upsert_for_origin(
            &url,
            "linkedin",
            &meta("Acme", "Dev"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();
    insert_gen_with_app_id(&gen_conn, "gen-orphan-2", &url, None);

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
    let gen_conn = open_gen_db_with_app_id_col(dir.path());
    let app_store = ApplicationStore::open(dir.path()).unwrap();

    // A real Application exists in the store — for a DIFFERENT posting.
    app_store
        .upsert_for_origin(
            "https://other.com/jobs/1",
            "linkedin",
            &meta("Other Co", "Role"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    // No Application anywhere matches this posting.
    let url = normalize_job_url("https://nomatch.example.com/jobs/999");
    insert_gen_with_app_id(&gen_conn, "gen-unmatched", &url, None);

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
/// row's hardcoded `'Acme'` (see `insert_gen_with_app_id`) does NOT carry —
/// if `open()` ever let the wide backfill resolve this row instead of the
/// lookup-only pass, `company` would be clobbered back to `'Acme'`.
///
/// Mutation check: revert `open()`'s call order (`backfill_from_generations`
/// before `link_orphaned_generations`) and the `company` assertion fails —
/// applied and reverted.
#[test]
fn open_links_an_orphan_through_the_lookup_only_path_not_the_wide_backfill() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db_with_app_id_col(dir.path());

    let raw_url = "https://acme.com/jobs/77";
    let normalized_url = normalize_job_url(raw_url);

    let app_id = {
        let store = ApplicationStore::open(dir.path()).unwrap();
        store
            .upsert_for_origin(
                raw_url,
                "linkedin",
                &meta("Acme", "Dev"),
                ApplicationOrigin::Generate,
                None,
            )
            .unwrap()
        // `store` drops here, releasing the connection before the reboot below.
    };

    // A later correction — a value the stale orphan row below does NOT carry.
    {
        let apps_conn = Connection::open(dir.path().join("applications.db")).unwrap();
        apps_conn
            .execute(
                "UPDATE applications SET company = 'Verified Later Value' WHERE id = ?1",
                rusqlite::params![app_id],
            )
            .unwrap();
    }

    // The orphan: `insert_gen_with_app_id` hardcodes company_name = 'Acme',
    // the STALE value from before the correction above.
    insert_gen_with_app_id(&gen_conn, "gen-orphan-boot", &normalized_url, None);

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
    let gen_conn = open_gen_db_with_app_id_col(dir.path());
    let gen_store =
        crate::ai_generations::AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

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
    let app_id = store
        .upsert_for_origin(
            raw_url,
            "linkedin",
            &meta("Acme", "Dev"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();
    // A MODERN generation, linked to the Application about to be deleted —
    // exactly the shape a real staged-pipeline run produces.
    insert_gen_with_app_id_and_created_at(
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
    let gen_conn = open_gen_db_with_app_id_col(dir.path());
    let gen_store =
        crate::ai_generations::AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();

    let raw_url = "https://acme.com/jobs/resurrect-2";
    let normalized = normalize_job_url(raw_url);
    // ONE long-lived store instance, same reasoning as the keepDocuments=true
    // test above — a second `open` before the delete would itself relink the
    // pre-orphaned row to the still-alive Application via `link_orphaned_
    // generations`, masking the defect this test exists to catch.
    let store = ApplicationStore::open(dir.path()).unwrap();
    let app_id = store
        .upsert_for_origin(
            raw_url,
            "linkedin",
            &meta("Acme", "Dev"),
            ApplicationOrigin::Generate,
            None,
        )
        .unwrap();

    // Already orphaned BEFORE the delete — never linked to `app_id`.
    insert_gen_with_app_id_and_created_at(
        &gen_conn,
        "gen-r2",
        &normalized,
        None,
        MODERN_CREATED_AT,
    );

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
/// Mutation check: drop the `created_at >= APPLICATIONS_FEATURE_EPOCH_MS`
/// guard and `list().len()` becomes 1 — applied and reverted.
#[test]
fn a_modern_unmatched_orphan_never_gets_an_application_created_by_backfill() {
    let dir = TempDir::new().unwrap();
    let gen_conn = open_gen_db_with_app_id_col(dir.path());
    insert_gen_with_app_id_and_created_at(
        &gen_conn,
        "gen-modern-orphan",
        "https://acme.com/jobs/never-had-one",
        None,
        MODERN_CREATED_AT,
    );

    let store = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(
        store.list().len(),
        0,
        "a MODERN orphan must never get a freshly created Application"
    );
    assert_eq!(gen_application_id(&gen_conn, "gen-modern-orphan"), None);
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
        "jobDescription": "",
        "notes": "",
        "nextActionAt": null,
        "comp": "",
        "contactName": "",
        "contactEmail": "",
        "jobSummary": ""
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
            "jobDescription": "",
            "notes": "",
            "nextActionAt": null,
            "comp": "",
            "contactName": "",
            "contactEmail": "",
            "jobSummary": ""
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

// ── job_description column: migration, persistence, and merge-preserve ────────
//
// Three behaviours pinned in ONE test function:
//   1. Additive migration applies cleanly on top of a populated old-schema DB
//      (no job_description column) → existing row survives with DEFAULT ''.
//   2. upsert_for_origin with a non-empty JD persists it (mirrors the import path).
//   3. Merge-preserve: empty incoming JD keeps the stored JD; non-empty incoming
//      JD overwrites it.  One Application throughout (no accidental duplicates).

#[test]
fn job_description_migrates_persists_and_merge_preserves() {
    let dir = TempDir::new().unwrap();

    // ── Step 1: seed a legacy DB (migrations 1+2 applied, migration 3 not yet) ─
    //
    // We hand-create applications.db with the pre-job_description schema and set
    // PRAGMA user_version = 2 so ApplicationStore::open applies only migration 3
    // (ALTER TABLE … ADD COLUMN job_description …) when it opens.
    let legacy_id = "app-legacy-001";
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE applications (
                id              TEXT PRIMARY KEY,
                status          TEXT NOT NULL DEFAULT 'saved',
                applied_at      INTEGER,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL,
                job_url         TEXT NOT NULL DEFAULT '',
                board           TEXT NOT NULL DEFAULT '',
                company         TEXT NOT NULL DEFAULT '',
                title           TEXT NOT NULL DEFAULT '',
                candidate       TEXT NOT NULL DEFAULT '',
                answers         TEXT NOT NULL DEFAULT '[]',
                brief           TEXT NOT NULL DEFAULT '',
                notes           TEXT NOT NULL DEFAULT '',
                next_action_at  INTEGER,
                comp            TEXT NOT NULL DEFAULT '',
                contact_name    TEXT NOT NULL DEFAULT '',
                contact_email   TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_applications_job_url
                ON applications(job_url);
            CREATE TABLE status_events (
                application_id  TEXT NOT NULL,
                from_status     TEXT NOT NULL DEFAULT '',
                to_status       TEXT NOT NULL,
                at              INTEGER NOT NULL,
                note            TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_status_events_app
                ON status_events(application_id);
            PRAGMA user_version = 2;",
        )
        .unwrap();
        // Insert one row without job_description (column doesn't exist yet).
        conn.execute(
            "INSERT INTO applications
             (id, status, created_at, updated_at)
             VALUES (?1, 'applied', 1000, 1000)",
            rusqlite::params![legacy_id],
        )
        .unwrap();
    }

    // Open the store — migration 3 (ADD COLUMN job_description … DEFAULT '')
    // must apply without error and the pre-existing row must survive intact.
    let store = ApplicationStore::open(dir.path()).unwrap();

    let legacy_app = store
        .get(legacy_id)
        .expect("legacy row must be readable after migration");
    assert_eq!(
        legacy_app.job_description, "",
        "legacy row must get DEFAULT '' for job_description after additive migration"
    );
    assert_eq!(
        legacy_app.id, legacy_id,
        "legacy row id must be unchanged after migration"
    );

    // ── Step 2: import path — upsert with a non-empty JD persists it ──────────
    let jd = "Senior Rust role. Async, Tokio.";
    let m_with_jd = ApplicationMeta {
        job_description: jd.into(),
        ..meta("Acme", "Engineer")
    };
    let app_id = store
        .upsert_for_origin(
            "https://acme.com/job/import/1",
            "linkedin",
            &m_with_jd,
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();

    assert_eq!(
        store.get(&app_id).unwrap().job_description,
        jd,
        "upsert_for_origin must persist the supplied job_description"
    );

    // ── Step 3a: merge-preserve — empty incoming JD keeps the stored JD ───────
    store
        .upsert_for_origin(
            "https://acme.com/job/import/1",
            "linkedin",
            &meta("Acme", "Engineer"), // job_description: String::new()
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();

    assert_eq!(
        store.get(&app_id).unwrap().job_description,
        jd,
        "empty incoming job_description must NOT overwrite the stored JD"
    );

    // Still exactly ONE Application for this URL — no duplicate created.
    assert_eq!(
        store
            .list()
            .iter()
            .filter(|a| a.job_url == "https://acme.com/job/import/1")
            .count(),
        1,
        "merge must never duplicate the Application"
    );

    // ── Step 3b: non-empty incoming JD overwrites the stored JD ───────────────
    let updated_jd = "Updated JD";
    let m_updated = ApplicationMeta {
        job_description: updated_jd.into(),
        ..meta("Acme", "Engineer")
    };
    store
        .upsert_for_origin(
            "https://acme.com/job/import/1",
            "linkedin",
            &m_updated,
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();

    assert_eq!(
        store.get(&app_id).unwrap().job_description,
        updated_jd,
        "non-empty incoming job_description must overwrite the stored JD"
    );

    // Final sanity: still one Application for the URL.
    assert_eq!(
        store
            .list()
            .iter()
            .filter(|a| a.job_url == "https://acme.com/job/import/1")
            .count(),
        1,
        "store must hold exactly one Application after all upserts"
    );
}

// ── Security: server-side job_description cap (the real trust boundary) ───────
//
// The renderer Zod cap is UX-only; the extension import path persists
// attacker-influenced page HTML that never passes through it. The store must
// clamp the JD to MAX_JOB_DESCRIPTION_BYTES on a UTF-8 char boundary (truncate,
// never reject) on BOTH write entry points: upsert_for_origin and update_fields.

#[test]
fn job_description_is_clamped_on_char_boundary_via_both_write_paths() {
    // Over-cap (~250 KB) JD whose 4-byte 'U+1F600' STARTS at byte MAX-1, so a
    // naive byte-cut at MAX lands mid-char and must be walked back to MAX-1.
    // After the walk-back the emoji and everything after it is dropped → stored
    // is exactly MAX-1 'a's.
    let jd = "a".repeat(MAX_JOB_DESCRIPTION_BYTES - 1) + "\u{1F600}" + &"b".repeat(1000);
    let expected = "a".repeat(MAX_JOB_DESCRIPTION_BYTES - 1);
    assert!(
        jd.len() > MAX_JOB_DESCRIPTION_BYTES,
        "precondition: input is over-cap"
    );

    // Direct helper assertion: an under-cap string is returned unchanged.
    let small = "short JD".to_string();
    assert_eq!(
        clamp_job_description(small.clone()),
        small,
        "under-cap input must pass through unchanged"
    );

    // ── Path A — upsert_for_origin (import funnel + every creation trigger) ────
    let dir_a = TempDir::new().unwrap();
    let store_a = ApplicationStore::open(dir_a.path()).unwrap();
    let id_a = store_a
        .upsert_for_origin(
            "https://acme.com/job/clamp/a",
            "linkedin",
            &ApplicationMeta {
                job_description: jd.clone(),
                ..meta("Acme", "Eng")
            },
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();
    let stored_a = store_a.get(&id_a).unwrap().job_description;
    assert!(
        stored_a.len() <= MAX_JOB_DESCRIPTION_BYTES,
        "upsert_for_origin must clamp JD to <= MAX (got {})",
        stored_a.len()
    );
    assert!(
        std::str::from_utf8(stored_a.as_bytes()).is_ok(),
        "stored JD must be valid UTF-8 (char-boundary cut)"
    );
    assert_eq!(
        stored_a.len(),
        MAX_JOB_DESCRIPTION_BYTES - 1,
        "cut must walk back off the 4-byte char to MAX-1"
    );
    assert_eq!(
        stored_a, expected,
        "the multibyte char and everything after it must be dropped"
    );

    // ── Path B — update_fields (applications_update IPC; attacker-reachable) ───
    let dir_b = TempDir::new().unwrap();
    let store_b = ApplicationStore::open(dir_b.path()).unwrap();
    let id_b = store_b.track_manual("", "", &meta("C", "T")).unwrap();
    store_b
        .update_fields(
            &id_b,
            None,
            None,
            None,
            None,
            None,
            Some(jd.clone()),
            None,
            None,
            None,
        )
        .unwrap();
    let stored_b = store_b.get(&id_b).unwrap().job_description;
    assert!(
        stored_b.len() <= MAX_JOB_DESCRIPTION_BYTES,
        "update_fields must clamp JD to <= MAX (got {})",
        stored_b.len()
    );
    assert!(
        std::str::from_utf8(stored_b.as_bytes()).is_ok(),
        "stored JD must be valid UTF-8 (char-boundary cut)"
    );
    assert_eq!(
        stored_b.len(),
        MAX_JOB_DESCRIPTION_BYTES - 1,
        "update_fields cut must walk back off the 4-byte char to MAX-1"
    );
    assert_eq!(stored_b, expected);

    // None must leave the (now-clamped) JD untouched.
    store_b
        .update_fields(
            &id_b,
            Some("note".into()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        store_b.get(&id_b).unwrap().job_description,
        expected,
        "None job_description must preserve the existing (clamped) JD"
    );
}

/// Old-schema applications.db (no job_summary column) must gain it via the
/// additive migration with NO data loss, then accept/return a summary.
#[test]
fn job_summary_migration_adds_column_without_data_loss() {
    let dir = TempDir::new().unwrap();
    // Hand-build the PRE-job_summary applications table (the create_applications
    // shape) and seed one row, simulating a DB from before this migration.
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE applications (
                id TEXT PRIMARY KEY, status TEXT NOT NULL DEFAULT 'saved',
                applied_at INTEGER, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                job_url TEXT NOT NULL DEFAULT '', board TEXT NOT NULL DEFAULT '',
                company TEXT NOT NULL DEFAULT '', title TEXT NOT NULL DEFAULT '',
                candidate TEXT NOT NULL DEFAULT '', answers TEXT NOT NULL DEFAULT '[]',
                brief TEXT NOT NULL DEFAULT '', notes TEXT NOT NULL DEFAULT '',
                next_action_at INTEGER, comp TEXT NOT NULL DEFAULT '',
                contact_name TEXT NOT NULL DEFAULT '', contact_email TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE status_events (
                application_id TEXT NOT NULL, from_status TEXT NOT NULL DEFAULT '',
                to_status TEXT NOT NULL, at INTEGER NOT NULL, note TEXT NOT NULL DEFAULT ''
            );
            INSERT INTO applications (id, status, created_at, updated_at, company)
                VALUES ('old-1', 'applied', 1000, 1000, 'Legacy Corp');",
        )
        .unwrap();
    }
    // Opening the store runs migrations (incl. add_applications_job_summary).
    let store = ApplicationStore::open(dir.path()).unwrap();
    let app = store
        .get("old-1")
        .expect("legacy row must survive migration");
    assert_eq!(app.company, "Legacy Corp", "no data loss on migrated row");
    assert_eq!(app.job_summary, "", "new column defaults to empty");
}

/// An upsert with a non-empty job_summary persists it; a follow-up upsert with an
/// EMPTY summary must NOT clobber the stored value (merge-preserve, like `brief`).
#[test]
fn job_summary_upsert_persists_and_merge_preserves() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let url = "https://acme.com/job/777";

    let mut m = meta("Acme", "Engineer");
    m.job_summary = "A concise role summary.".into();
    let id = store
        .upsert_for_origin(url, "linkedin", &m, ApplicationOrigin::Generate, None)
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().job_summary,
        "A concise role summary."
    );

    // Re-upsert the same url with an EMPTY summary — must keep the stored one.
    let m2 = meta("Acme", "Engineer"); // job_summary == ""
    let id2 = store
        .upsert_for_origin(url, "linkedin", &m2, ApplicationOrigin::Generate, None)
        .unwrap();
    assert_eq!(id, id2, "same url merges into one Application");
    assert_eq!(
        store.get(&id).unwrap().job_summary,
        "A concise role summary.",
        "empty incoming summary must not clobber the stored one"
    );
}

/// `update_fields` can set the summary, and the 50 KB server cap truncates an
/// oversize value on a UTF-8 char boundary (no panic, no split char).
#[test]
fn job_summary_update_and_50kb_clamp_truncates_on_char_boundary() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store.track_manual("", "", &meta("C", "T")).unwrap();

    // Normal update path persists a summary.
    store
        .update_fields(
            &id,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("hello".into()),
            None,
            None,
        )
        .unwrap();
    assert_eq!(store.get(&id).unwrap().job_summary, "hello");

    // >50 KB of a 2-byte char ('é' = U+00E9). 50_000 is even and every boundary in
    // an all-'é' string is even, so exactly 25_000 whole chars (50_000 bytes) fit.
    let big = "é".repeat(40_000); // 80_000 bytes
    store
        .update_fields(
            &id,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(big),
            None,
            None,
        )
        .unwrap();
    let stored = store.get(&id).unwrap().job_summary;
    assert!(
        stored.len() <= 50_000,
        "must be capped at 50 KB, got {}",
        stored.len()
    );
    assert!(
        stored.chars().all(|c| c == 'é'),
        "no split/garbage char at the cut"
    );
    assert_eq!(
        stored.chars().count(),
        25_000,
        "exactly the whole chars that fit"
    );
}

/// Migration round-trip: seed a DB at user_version=4 (has job_summary column,
/// no recipient columns), open the store, verify migration 5 adds them, and
/// confirm pre-existing rows survive intact with DEFAULT '' values.
#[test]
fn recipient_columns_migrate_from_pre_recipient_schema() {
    let dir = TempDir::new().unwrap();
    let legacy_id = "app-legacy-recip-001";
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE applications (
                id              TEXT PRIMARY KEY,
                status          TEXT NOT NULL DEFAULT 'saved',
                applied_at      INTEGER,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL,
                job_url         TEXT NOT NULL DEFAULT '',
                board           TEXT NOT NULL DEFAULT '',
                company         TEXT NOT NULL DEFAULT '',
                title           TEXT NOT NULL DEFAULT '',
                candidate       TEXT NOT NULL DEFAULT '',
                answers         TEXT NOT NULL DEFAULT '[]',
                brief           TEXT NOT NULL DEFAULT '',
                notes           TEXT NOT NULL DEFAULT '',
                next_action_at  INTEGER,
                comp            TEXT NOT NULL DEFAULT '',
                contact_name    TEXT NOT NULL DEFAULT '',
                contact_email   TEXT NOT NULL DEFAULT '',
                job_description TEXT NOT NULL DEFAULT '',
                job_summary     TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_applications_job_url
                ON applications(job_url);
            CREATE TABLE status_events (
                application_id  TEXT NOT NULL,
                from_status     TEXT NOT NULL DEFAULT '',
                to_status       TEXT NOT NULL,
                at              INTEGER NOT NULL,
                note            TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_status_events_app
                ON status_events(application_id);
            PRAGMA user_version = 4;",
        )
        .unwrap();
        // Insert one row — no recipient columns yet.
        conn.execute(
            "INSERT INTO applications (id, status, created_at, updated_at)
             VALUES (?1, 'applied', 1000, 1000)",
            rusqlite::params![legacy_id],
        )
        .unwrap();
    }

    // Opening the store runs migration 5 (ADD COLUMN recipient_name/email).
    let store = ApplicationStore::open(dir.path()).unwrap();
    let app = store
        .get(legacy_id)
        .expect("legacy row must be readable after migration");
    assert_eq!(
        app.recipient_name, "",
        "legacy row must get DEFAULT '' for recipient_name after migration"
    );
    assert_eq!(
        app.recipient_email, "",
        "legacy row must get DEFAULT '' for recipient_email after migration"
    );
    assert_eq!(app.id, legacy_id, "row id must be unchanged");

    // Write recipient fields and confirm they round-trip.
    store
        .update_fields(
            legacy_id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("Jane Smith".into()),
            Some("jane@acme.com".into()),
        )
        .unwrap();
    let updated = store.get(legacy_id).unwrap();
    assert_eq!(updated.recipient_name, "Jane Smith");
    assert_eq!(updated.recipient_email, "jane@acme.com");
}

/// Recipient fields persist and round-trip through update_fields and export/import.
#[test]
fn recipient_fields_persist_and_export_import_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .track_manual("", "", &meta("Acme", "Engineer"))
        .unwrap();

    // Set both fields.
    store
        .update_fields(
            &id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("Jane Smith".into()),
            Some("jane@acme.com".into()),
        )
        .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.recipient_name, "Jane Smith");
    assert_eq!(app.recipient_email, "jane@acme.com");

    // Export + import round-trips the fields.
    let bundle = store.export();
    let dir2 = TempDir::new().unwrap();
    let store2 = ApplicationStore::open(dir2.path()).unwrap();
    store2.import(&bundle).unwrap();
    let imported = store2.get(&id).unwrap();
    assert_eq!(imported.recipient_name, "Jane Smith");
    assert_eq!(imported.recipient_email, "jane@acme.com");

    // Clearing via empty string leaves the fields empty.
    store
        .update_fields(
            &id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(String::new()),
            Some(String::new()),
        )
        .unwrap();
    let cleared = store.get(&id).unwrap();
    assert_eq!(cleared.recipient_name, "");
    assert_eq!(cleared.recipient_email, "");
}

/// Migration round-trip: seed a DB at user_version=5 (has recipient columns, no
/// salary columns), open the store, verify migration 6 adds them, and confirm
/// a pre-existing row survives with `None` salary (NULL, never 0).
#[test]
fn salary_columns_migrate_from_pre_salary_schema() {
    let dir = TempDir::new().unwrap();
    let legacy_id = "app-legacy-salary-001";
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE applications (
                id              TEXT PRIMARY KEY,
                status          TEXT NOT NULL DEFAULT 'saved',
                applied_at      INTEGER,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL,
                job_url         TEXT NOT NULL DEFAULT '',
                board           TEXT NOT NULL DEFAULT '',
                company         TEXT NOT NULL DEFAULT '',
                title           TEXT NOT NULL DEFAULT '',
                candidate       TEXT NOT NULL DEFAULT '',
                answers         TEXT NOT NULL DEFAULT '[]',
                brief           TEXT NOT NULL DEFAULT '',
                notes           TEXT NOT NULL DEFAULT '',
                next_action_at  INTEGER,
                comp            TEXT NOT NULL DEFAULT '',
                contact_name    TEXT NOT NULL DEFAULT '',
                contact_email   TEXT NOT NULL DEFAULT '',
                job_description TEXT NOT NULL DEFAULT '',
                job_summary     TEXT NOT NULL DEFAULT '',
                recipient_name  TEXT NOT NULL DEFAULT '',
                recipient_email TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_applications_job_url
                ON applications(job_url);
            CREATE TABLE status_events (
                application_id  TEXT NOT NULL,
                from_status     TEXT NOT NULL DEFAULT '',
                to_status       TEXT NOT NULL,
                at              INTEGER NOT NULL,
                note            TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_status_events_app
                ON status_events(application_id);
            PRAGMA user_version = 5;",
        )
        .unwrap();
        // Insert one row — no salary columns yet.
        conn.execute(
            "INSERT INTO applications (id, status, created_at, updated_at)
             VALUES (?1, 'applied', 1000, 1000)",
            rusqlite::params![legacy_id],
        )
        .unwrap();
    }

    // Opening the store runs migration 6 (ADD COLUMN salary_min/max/currency).
    let store = ApplicationStore::open(dir.path()).unwrap();
    let app = store
        .get(legacy_id)
        .expect("legacy row must be readable after migration");
    assert_eq!(
        app.salary_min, None,
        "legacy row must get NULL (None), never 0, for salary_min after migration"
    );
    assert_eq!(app.salary_max, None);
    assert_eq!(app.salary_currency, None);
    assert_eq!(app.id, legacy_id, "row id must be unchanged");
}

/// Salary persists and round-trips through `upsert_for_origin` and
/// export/import, and a second upsert with unknown salary (`None`) never
/// clobbers an already-known value (COALESCE(new, old) merge).
#[test]
fn salary_fields_persist_merge_and_export_import_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();

    let with_salary = ApplicationMeta {
        salary_min: Some(70_000.0),
        salary_max: Some(90_000.0),
        salary_currency: Some("EUR".into()),
        ..meta("Acme", "Engineer")
    };
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/salary/1",
            "aggregator",
            &with_salary,
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.salary_min, Some(70_000.0));
    assert_eq!(app.salary_max, Some(90_000.0));
    assert_eq!(app.salary_currency, Some("EUR".to_string()));

    // A later re-track with unknown salary must NOT clobber the known values.
    let unknown_salary = meta("Acme", "Engineer");
    store
        .upsert_for_origin(
            "https://acme.com/job/salary/1",
            "aggregator",
            &unknown_salary,
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();
    let unchanged = store.get(&id).unwrap();
    assert_eq!(
        unchanged.salary_min,
        Some(70_000.0),
        "an unknown incoming salary must not clobber an already-known value"
    );
    assert_eq!(unchanged.salary_max, Some(90_000.0));
    assert_eq!(unchanged.salary_currency, Some("EUR".to_string()));

    // Export + import round-trips the fields.
    let bundle = store.export();
    let dir2 = TempDir::new().unwrap();
    let store2 = ApplicationStore::open(dir2.path()).unwrap();
    store2.import(&bundle).unwrap();
    let imported = store2.get(&id).unwrap();
    assert_eq!(imported.salary_min, Some(70_000.0));
    assert_eq!(imported.salary_max, Some(90_000.0));
    assert_eq!(imported.salary_currency, Some("EUR".to_string()));
}

// ── merge_answers (extension bridge `answers.save`'s store-write boundary) ────
// APPEND-only dedup merge — deliberately NOT `upsert_internal`'s meta path
// (which REPLACES `answers` wholesale). See `extension_bridge::answers_save`
// for the caller.

#[test]
fn merge_answers_adds_new_answers_and_returns_count() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/merge/1",
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let added = store
        .merge_answers(
            &id,
            vec![
                ApplicationAnswer {
                    id: String::new(),
                    question: "Why this role?".to_string(),
                    answer: "Because I love it.".to_string(),
                },
                ApplicationAnswer {
                    id: String::new(),
                    question: "Salary expectation?".to_string(),
                    answer: "100k".to_string(),
                },
            ],
        )
        .unwrap();
    assert_eq!(added, 2);

    let app = store.get(&id).unwrap();
    assert_eq!(app.answers.len(), 2);
    // Each merged answer gets a fresh, non-empty generated id.
    assert!(app.answers.iter().all(|a| !a.id.is_empty()));
}

#[test]
fn merge_answers_dedups_by_normalized_question_and_never_overwrites() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let mut m = meta("Acme", "Engineer");
    m.answers = vec![ApplicationAnswer {
        id: "seed-1".to_string(),
        question: "Why this role?".to_string(),
        answer: "Original".to_string(),
    }];
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/merge/2",
            "linkedin",
            &m,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    // A re-capture with different whitespace/case for the SAME question, plus
    // one genuinely new question.
    let added = store
        .merge_answers(
            &id,
            vec![
                ApplicationAnswer {
                    id: String::new(),
                    question: "  WHY this   role?".to_string(),
                    answer: "A newer answer that must be dropped".to_string(),
                },
                ApplicationAnswer {
                    id: String::new(),
                    question: "New question?".to_string(),
                    answer: "New answer".to_string(),
                },
            ],
        )
        .unwrap();
    assert_eq!(added, 1, "only the genuinely new question is added");

    let app = store.get(&id).unwrap();
    assert_eq!(app.answers.len(), 2);
    let original = app
        .answers
        .iter()
        .find(|a| a.question == "Why this role?")
        .unwrap();
    assert_eq!(
        original.answer, "Original",
        "existing answer never overwritten"
    );
    assert_eq!(
        original.id, "seed-1",
        "existing answer's id is untouched too"
    );
}

#[test]
fn merge_answers_dedups_within_the_same_incoming_batch() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/merge/3",
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    // Two entries in ONE call that normalize to the same question — only the
    // first should be added.
    let added = store
        .merge_answers(
            &id,
            vec![
                ApplicationAnswer {
                    id: String::new(),
                    question: "Why this role?".to_string(),
                    answer: "First".to_string(),
                },
                ApplicationAnswer {
                    id: String::new(),
                    question: "why THIS role?".to_string(),
                    answer: "Second (dropped)".to_string(),
                },
            ],
        )
        .unwrap();
    assert_eq!(added, 1);
    let app = store.get(&id).unwrap();
    assert_eq!(app.answers.len(), 1);
    assert_eq!(app.answers[0].answer, "First");
}

#[test]
fn merge_answers_leaves_updated_at_unchanged_when_nothing_new_added() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let mut m = meta("Acme", "Engineer");
    m.answers = vec![ApplicationAnswer {
        id: "seed-1".to_string(),
        question: "Why this role?".to_string(),
        answer: "Original".to_string(),
    }];
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/merge/4",
            "linkedin",
            &m,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    let before = store.get(&id).unwrap().updated_at;

    let added = store
        .merge_answers(
            &id,
            vec![ApplicationAnswer {
                id: String::new(),
                question: "Why this role?".to_string(),
                answer: "Ignored".to_string(),
            }],
        )
        .unwrap();
    assert_eq!(added, 0);
    assert_eq!(
        store.get(&id).unwrap().updated_at,
        before,
        "an all-dedup merge (nothing added) must not touch updated_at"
    );
}

#[test]
fn merge_answers_returns_error_for_unknown_id() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let err = store.merge_answers("does-not-exist", vec![]).unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn merge_answers_caps_total_stored_answers_and_drops_the_rest() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let mut m = meta("Acme", "Engineer");
    // Seed to exactly (cap - 2) existing distinct answers.
    m.answers = (0..MAX_TOTAL_ANSWERS - 2)
        .map(|i| ApplicationAnswer {
            id: format!("seed-{i}"),
            question: format!("Existing question {i}?"),
            answer: format!("Existing answer {i}"),
        })
        .collect();
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/merge/cap",
            "linkedin",
            &m,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    // 5 new distinct questions incoming — only 2 fit under the cap; the rest
    // are dropped (the caller derives `skipped` from `incoming_len - added`).
    let incoming: Vec<ApplicationAnswer> = (0..5)
        .map(|i| ApplicationAnswer {
            id: String::new(),
            question: format!("New question {i}?"),
            answer: format!("New answer {i}"),
        })
        .collect();

    let added = store.merge_answers(&id, incoming).unwrap();
    assert_eq!(added, 2, "only enough to reach the cap are added");

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.answers.len(),
        MAX_TOTAL_ANSWERS,
        "total stored answers never exceeds the per-application cap"
    );
    // A seeded answer (by content, not just count) must survive the cap
    // untouched — the cap drops INCOMING overflow, never existing rows.
    let seeded = app
        .answers
        .iter()
        .find(|a| a.id == "seed-0")
        .expect("a seeded answer must survive the cap by id");
    assert_eq!(seeded.question, "Existing question 0?");
    assert_eq!(seeded.answer, "Existing answer 0");
}

/// HIGH regression: when `existing.answers` already sits AT or OVER
/// `MAX_TOTAL_ANSWERS` (a legacy row seeded before the on-creation cap
/// shipped), `merge_answers_by_question` used to remove the matching existing
/// answer from `merged` to make room for the incoming replacement, then the
/// cap check unconditionally blocked that same replacement from ever being
/// pushed back in — the question vanished entirely instead of being
/// rewritten. Seed a row with `MAX_TOTAL_ANSWERS + 1` answers via a raw SQL
/// UPDATE (bypassing the store's own cap enforcement, simulating a legacy
/// row), then upsert one incoming answer matching an existing question: it
/// must survive with the incoming text, and the total must not grow.
#[test]
fn merge_answers_by_question_swaps_a_replacement_even_when_existing_is_already_over_cap() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let url = "https://acme.com/job/merge/over-cap-swap";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    // Seed MAX_TOTAL_ANSWERS + 1 distinct answers directly via raw SQL — a
    // legacy shape the on-creation cap (which runs on every upsert today)
    // would never itself produce, but a pre-existing over-cap row must still
    // be handled safely.
    let seeded: Vec<ApplicationAnswer> = (0..MAX_TOTAL_ANSWERS + 1)
        .map(|i| ApplicationAnswer {
            id: format!("seed-{i}"),
            question: format!("Existing question {i}?"),
            answer: format!("Existing answer {i}"),
        })
        .collect();
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute(
            "UPDATE applications SET answers = ?1 WHERE id = ?2",
            rusqlite::params![serde_json::to_string(&seeded).unwrap(), id],
        )
        .unwrap();
    }

    // Re-upsert with one incoming answer matching an existing question (a
    // rewrite) — no genuinely new question, so this exercises the swap path
    // alone.
    let mut m = meta("Acme", "Engineer");
    m.answers = vec![ApplicationAnswer {
        id: String::new(),
        question: "Existing question 0?".to_string(),
        answer: "Rewritten answer".to_string(),
    }];
    let id2 = store
        .upsert_for_origin(url, "linkedin", &m, ApplicationOrigin::Saved, None)
        .unwrap();
    assert_eq!(id, id2, "same url merges into the same Application");

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.answers.len(),
        MAX_TOTAL_ANSWERS + 1,
        "a same-question swap must not grow (or shrink) an already over-cap row"
    );
    let swapped = app
        .answers
        .iter()
        .find(|a| a.question == "Existing question 0?")
        .expect("the matching question must survive the swap, not vanish");
    assert_eq!(
        swapped.answer, "Rewritten answer",
        "the incoming replacement must win, not silently disappear"
    );
    assert_eq!(
        app.answers
            .iter()
            .filter(|a| a.question != "Existing question 0?")
            .count(),
        MAX_TOTAL_ANSWERS,
        "every other seeded answer must be untouched"
    );
}

/// MEDIUM fix: `upsert_internal`'s NEW-ROW branch used to store `meta.answers`
/// verbatim, bypassing `MAX_TOTAL_ANSWERS` entirely (only the existing-row merge
/// branch enforced it). Creating a brand-new Application (no prior row for the
/// url) with an oversized, duplicate-question `meta.answers` must still come out
/// deduped-by-question and capped at `MAX_TOTAL_ANSWERS`.
#[test]
fn upsert_for_origin_caps_and_dedupes_answers_on_new_row_creation() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();

    // MAX_TOTAL_ANSWERS + 2 distinct questions, plus one duplicate (different
    // case/whitespace of question 0) inserted right after it — still over the
    // cap even after the duplicate is dropped.
    let mut answers: Vec<ApplicationAnswer> = (0..MAX_TOTAL_ANSWERS + 2)
        .map(|i| ApplicationAnswer {
            id: String::new(),
            question: format!("Question {i}?"),
            answer: format!("Answer {i}"),
        })
        .collect();
    answers.insert(
        1,
        ApplicationAnswer {
            id: String::new(),
            question: "  question 0?  ".to_string(),
            answer: "Duplicate (dropped)".to_string(),
        },
    );

    let mut m = meta("Acme", "Engineer");
    m.answers = answers;

    let id = store
        .upsert_for_origin(
            "https://acme.com/job/new-row-cap",
            "linkedin",
            &m,
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.answers.len(),
        MAX_TOTAL_ANSWERS,
        "a brand-new Application's answers are capped on creation, not just on merge"
    );
    // The genuine duplicate must never reach the stored row at all (dropped as a
    // within-batch dupe, not merely truncated by the cap).
    assert_eq!(
        app.answers
            .iter()
            .filter(|a| a.answer == "Duplicate (dropped)")
            .count(),
        0,
        "a duplicate-question answer must be deduped, not stored twice"
    );
    let first = app
        .answers
        .iter()
        .find(|a| a.question == "Question 0?")
        .expect("the first-seen answer for the duplicated question must survive");
    assert_eq!(first.answer, "Answer 0");
}

/// Regression for the HIGH cross-feature data-loss hazard: `upsert_internal`'s
/// meta-merge path used to REPLACE `answers` wholesale, so an
/// `ai_generations_save`-shaped upsert (a non-empty `meta.answers`) silently
/// wiped every answer the extension's `answers.save` had appended in
/// between. Seed extension-captured answers via `merge_answers`, then run an
/// `upsert_for_origin` carrying a DIFFERENT non-empty answer set (one
/// matching an existing question, one genuinely new) and assert: the
/// extension-only answer survives, the matching question's text is updated
/// to the incoming (AI) text, and the new AI answer is added.
#[test]
fn upsert_for_origin_merges_answers_by_question_instead_of_replacing() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let url = "https://acme.com/job/merge/cross-feature";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    // Extension-captured answers land first, via the append-only path.
    store
        .merge_answers(
            &id,
            vec![
                ApplicationAnswer {
                    id: String::new(),
                    question: "Why this role?".to_string(),
                    answer: "Extension-captured original".to_string(),
                },
                ApplicationAnswer {
                    id: String::new(),
                    question: "Are you willing to relocate?".to_string(),
                    answer: "Extension-only, no AI equivalent".to_string(),
                },
            ],
        )
        .unwrap();

    // Simulates `ai_generations_save`: a full generated answer set, one
    // question matching an existing extension answer (an in-app rewrite),
    // one genuinely new.
    let mut ai_meta = meta("Acme", "Engineer");
    ai_meta.answers = vec![
        ApplicationAnswer {
            id: String::new(),
            question: "Why this role?".to_string(),
            answer: "AI-rewritten answer".to_string(),
        },
        ApplicationAnswer {
            id: String::new(),
            question: "What's your expected salary?".to_string(),
            answer: "100k".to_string(),
        },
    ];
    store
        .upsert_for_origin(url, "linkedin", &ai_meta, ApplicationOrigin::Generate, None)
        .unwrap();

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.answers.len(),
        3,
        "all 3 distinct questions must be present"
    );

    let relocate = app
        .answers
        .iter()
        .find(|a| a.question == "Are you willing to relocate?")
        .expect("the extension-only answer must survive the AI upsert");
    assert_eq!(relocate.answer, "Extension-only, no AI equivalent");

    let rewritten = app
        .answers
        .iter()
        .find(|a| a.question == "Why this role?")
        .expect("the matching question must still be present");
    assert_eq!(
        rewritten.answer, "AI-rewritten answer",
        "a matching question must be updated to the incoming text"
    );

    assert!(
        app.answers
            .iter()
            .any(|a| a.question == "What's your expected salary?" && a.answer == "100k"),
        "a genuinely new AI answer must be added"
    );
}

/// Regression proxy for the upsert/`merge_answers` TOCTOU fix:
/// `upsert_internal` used to look up the existing row via the self-locking
/// `find_by_job_url` (its own lock acquired and released BEFORE the write
/// transaction re-acquired the lock), leaving a gap where a concurrent
/// `merge_answers` commit could be silently overwritten by the upsert's
/// stale pre-gap snapshot. The fix folds both into one lock/transaction via
/// `row_by_job_url_conn`.
///
/// A deterministic reproduction of the OLD race isn't feasible here: its
/// window was the interval between two `Mutex` acquisitions inside a single
/// call, on the order of nanoseconds, and hitting it reliably would need a
/// test-only pause hook inside `upsert_internal` — production-code scope
/// creep beyond this fix. As an honest proxy, this test instead hammers the
/// SAME Application from two real threads — one repeatedly appending via
/// `merge_answers`, the other repeatedly upserting via `upsert_for_origin` —
/// each iteration contributing one distinct, individually-traceable
/// question, and asserts every single one survives. With the fix, the
/// lookup+write critical section is atomic under the shared lock, so no
/// interleaving can lose an update; this test would be flaky (and could
/// fail) against the old two-lock structure under real contention.
#[test]
fn upsert_and_merge_answers_race_never_loses_an_update() {
    let dir = TempDir::new().unwrap();
    let store = std::sync::Arc::new(ApplicationStore::open(dir.path()).unwrap());
    let url = "https://acme.com/job/race";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    const ITERS: usize = 40;

    let merge_store = store.clone();
    let merge_id = id.clone();
    let merge_thread = std::thread::spawn(move || {
        for i in 0..ITERS {
            merge_store
                .merge_answers(
                    &merge_id,
                    vec![ApplicationAnswer {
                        id: String::new(),
                        question: format!("merge-question-{i}"),
                        answer: format!("merge-answer-{i}"),
                    }],
                )
                .unwrap();
        }
    });

    let upsert_store = store.clone();
    let upsert_url = url.to_string();
    let upsert_thread = std::thread::spawn(move || {
        for i in 0..ITERS {
            let mut m = meta("Acme", "Engineer");
            m.answers = vec![ApplicationAnswer {
                id: String::new(),
                question: format!("upsert-question-{i}"),
                answer: format!("upsert-answer-{i}"),
            }];
            upsert_store
                .upsert_for_origin(
                    &upsert_url,
                    "linkedin",
                    &m,
                    ApplicationOrigin::Generate,
                    None,
                )
                .unwrap();
        }
    });

    merge_thread.join().unwrap();
    upsert_thread.join().unwrap();

    let app = store.get(&id).unwrap();
    for i in 0..ITERS {
        assert!(
            app.answers
                .iter()
                .any(|a| a.question == format!("merge-question-{i}")),
            "merge_answers entry {i} was lost to a concurrent upsert"
        );
        assert!(
            app.answers
                .iter()
                .any(|a| a.question == format!("upsert-question-{i}")),
            "upsert entry {i} was lost to a concurrent merge_answers"
        );
    }
    assert_eq!(
        app.answers.len(),
        ITERS * 2,
        "no answer from either concurrent writer may be dropped"
    );
}

/// `set_status` read the row through a lock it then RELEASED before opening the
/// write transaction, so a concurrent transition could land in the gap and the
/// `status_events` row it appended recorded a `from_status` the row no longer
/// had — a history chain that never happened.
///
/// Two threads drive the same Application between two statuses. Ordered by
/// `rowid` (insert order under the shared connection, so commit order), the
/// events must form an unbroken chain: each `from_status` is the previous
/// event's `to_status`. `at` is millisecond-resolution and ties freely, which is
/// why this reads `rowid` directly rather than going through `events()`.
#[test]
fn set_status_records_a_consistent_history_chain_under_contention() {
    let dir = TempDir::new().unwrap();
    let store = std::sync::Arc::new(ApplicationStore::open(dir.path()).unwrap());
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/status-race",
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    const ITERS: usize = 60;

    let threads: Vec<_> = [ApplicationStatus::Applied, ApplicationStatus::Saved]
        .into_iter()
        .map(|to| {
            let store = store.clone();
            let id = id.clone();
            std::thread::spawn(move || {
                for _ in 0..ITERS {
                    store.set_status(&id, to, "").unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }

    let conn = store.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT from_status, to_status FROM status_events
             WHERE application_id = ?1 ORDER BY rowid",
        )
        .unwrap();
    let events: Vec<(String, String)> = stmt
        .query_map(params![id], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    // Two threads × ITERS unconditional transitions each append exactly one
    // event (ITERS * 2), plus the single seed event from the Application's
    // creation above.
    assert_eq!(
        events.len(),
        ITERS * 2 + 1,
        "every transition records exactly one event (+1 creation seed)"
    );
    for (i, (from, _to)) in events.iter().enumerate().skip(1) {
        assert_eq!(
            from,
            &events[i - 1].1,
            "event {i} records from_status {from:?}, but the previous event moved the row to {:?} \
             — the read happened outside the write's transaction",
            events[i - 1].1
        );
    }
}

/// `update_fields` carried the SAME two-lock structure `upsert_internal` was
/// fixed for: `get` took and released the mutex, then the write retook it and
/// re-persisted EVERY column from the now-stale snapshot. A concurrent
/// `merge_answers` (extension `answers.save`) commit landing in that gap was
/// silently clobbered.
///
/// Same honest proxy as the test above — hammer one Application from two
/// threads, one appending an individually-traceable answer per iteration, the
/// other patching an unrelated field — and assert every answer survives.
#[test]
fn update_fields_and_merge_answers_race_never_loses_an_answer() {
    let dir = TempDir::new().unwrap();
    let store = std::sync::Arc::new(ApplicationStore::open(dir.path()).unwrap());
    let id = store
        .upsert_for_origin(
            "https://acme.com/job/update-race",
            "linkedin",
            &meta("Acme", "Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    const ITERS: usize = 40;

    let merge_store = store.clone();
    let merge_id = id.clone();
    let merge_thread = std::thread::spawn(move || {
        for i in 0..ITERS {
            merge_store
                .merge_answers(
                    &merge_id,
                    vec![ApplicationAnswer {
                        id: String::new(),
                        question: format!("merge-question-{i}"),
                        answer: format!("merge-answer-{i}"),
                    }],
                )
                .unwrap();
        }
    });

    let update_store = store.clone();
    let update_id = id.clone();
    let update_thread = std::thread::spawn(move || {
        for i in 0..ITERS {
            update_store
                .update_fields(
                    &update_id,
                    Some(format!("note-{i}")),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
        }
    });

    merge_thread.join().unwrap();
    update_thread.join().unwrap();

    let app = store.get(&id).unwrap();
    for i in 0..ITERS {
        assert!(
            app.answers
                .iter()
                .any(|a| a.question == format!("merge-question-{i}")),
            "merge_answers entry {i} was lost to a concurrent update_fields"
        );
    }
    assert_eq!(
        app.answers.len(),
        ITERS,
        "no answer may be dropped by a concurrent update_fields"
    );
    assert!(
        app.notes.starts_with("note-"),
        "the last update_fields patch must survive, got {:?}",
        app.notes
    );
}

// ── Contact unification (migration `unify_application_contact`) ───────────────
//
// `contact_name`/`contact_email` became THE single primary contact per
// application; `recipient_name`/`recipient_email` are deprecated aliases. The
// migration promotes an alias-only value onto the canonical pair and leaves the
// deprecated COLUMNS untouched (additive-only, never destructive).

/// Seed `applications.db` at `user_version = 6` — the pre-unification schema
/// (recipient + salary columns present) — with one row per interesting
/// contact/recipient population combination.
///
/// The four fields are independently empty-or-not (16 states); these rows cover
/// every state the promotion rule can treat differently — both pairs fully
/// populated / fully empty, EITHER pair half-populated (the cases that
/// distinguish a pair-atomic promotion from a per-column one), each flavour of
/// whitespace-only canonical pair, and an identical alias pair. Returns the ids
/// in declaration order.
fn seed_pre_unification_db(dir: &std::path::Path) -> [&'static str; 12] {
    let ids = [
        "app-recipient-only",
        "app-contact-only",
        "app-both",
        "app-neither",
        // Canonical HALF-populated + a full alias pair. The per-column rule
        // fused the two people here; a pair-atomic one must not promote at all.
        "app-contact-name-only",
        "app-contact-email-only",
        // Canonical empty + an alias pair that is itself half-populated: the
        // promotion must move BOTH columns, leaving the empty side empty.
        "app-recipient-name-only",
        "app-recipient-email-only",
        // Whitespace-only canonical pairs — reachable from pre-trim builds. Each
        // flavour is seeded separately because SQLite's BARE `TRIM(x)` strips
        // only U+0020: a TAB or an NBSP (endemic in text copied out of scraped
        // HTML) read as non-empty in SQL while `str::trim` calls them empty, so
        // the same row folded one way in place and the other way through a
        // restored bundle until the migration passed an explicit charset.
        "app-space-contact",
        "app-tab-contact",
        "app-nbsp-contact",
        // Alias pair IDENTICAL to the canonical one — nothing is being dropped,
        // so the `<>` distinctness guard must suppress the preserved note.
        "app-identical-pair",
    ];
    let conn = Connection::open(dir.join("applications.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE applications (
            id              TEXT PRIMARY KEY,
            status          TEXT NOT NULL DEFAULT 'saved',
            applied_at      INTEGER,
            created_at      INTEGER NOT NULL,
            updated_at      INTEGER NOT NULL,
            job_url         TEXT NOT NULL DEFAULT '',
            board           TEXT NOT NULL DEFAULT '',
            company         TEXT NOT NULL DEFAULT '',
            title           TEXT NOT NULL DEFAULT '',
            candidate       TEXT NOT NULL DEFAULT '',
            answers         TEXT NOT NULL DEFAULT '[]',
            brief           TEXT NOT NULL DEFAULT '',
            notes           TEXT NOT NULL DEFAULT '',
            next_action_at  INTEGER,
            comp            TEXT NOT NULL DEFAULT '',
            contact_name    TEXT NOT NULL DEFAULT '',
            contact_email   TEXT NOT NULL DEFAULT '',
            job_description TEXT NOT NULL DEFAULT '',
            job_summary     TEXT NOT NULL DEFAULT '',
            recipient_name  TEXT NOT NULL DEFAULT '',
            recipient_email TEXT NOT NULL DEFAULT '',
            salary_min      REAL,
            salary_max      REAL,
            salary_currency TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_applications_job_url
            ON applications(job_url);
        CREATE TABLE status_events (
            application_id  TEXT NOT NULL,
            from_status     TEXT NOT NULL DEFAULT '',
            to_status       TEXT NOT NULL,
            at              INTEGER NOT NULL,
            note            TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_status_events_app
            ON status_events(application_id);
        PRAGMA user_version = 6;",
    )
    .unwrap();
    let rows: [(&str, &str, &str, &str, &str); 12] = [
        (ids[0], "", "", "Rita Recruiter", "rita@acme.com"),
        (ids[1], "Cora Contact", "cora@acme.com", "", ""),
        (
            ids[2],
            "Cora Contact",
            "cora@acme.com",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        (ids[3], "", "", "", ""),
        (
            ids[4],
            "Cora Contact",
            "",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        (
            ids[5],
            "",
            "cora@acme.com",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        (ids[6], "", "", "Rita Recruiter", ""),
        (ids[7], "", "", "", "rita@acme.com"),
        (ids[8], "   ", "  ", "Rita Recruiter", "rita@acme.com"),
        (ids[9], "\t", "\t\t", "Rita Recruiter", "rita@acme.com"),
        (
            ids[10],
            "\u{A0}",
            "\u{A0} \u{A0}",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        (
            ids[11],
            "Rita Recruiter",
            "rita@acme.com",
            "Rita Recruiter",
            "rita@acme.com",
        ),
    ];
    for (id, contact_name, contact_email, recipient_name, recipient_email) in rows {
        conn.execute(
            "INSERT INTO applications
             (id, status, created_at, updated_at, contact_name, contact_email,
              recipient_name, recipient_email)
             VALUES (?1, 'applied', 1000, 1000, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                id,
                contact_name,
                contact_email,
                recipient_name,
                recipient_email
            ],
        )
        .unwrap();
    }
    ids
}

/// Read a raw column straight from SQLite, bypassing the store's projection —
/// used to assert the DEPRECATED columns were left alone by the migration.
fn raw_column(dir: &std::path::Path, id: &str, column: &str) -> String {
    let conn = Connection::open(dir.join("applications.db")).unwrap();
    conn.query_row(
        &format!("SELECT {column} FROM applications WHERE id = ?1"),
        rusqlite::params![id],
        |r| r.get::<_, String>(0),
    )
    .unwrap()
}

#[test]
fn contact_backfill_promotes_the_pair_atomically() {
    let dir = TempDir::new().unwrap();
    let [recipient_only, contact_only, both, neither, contact_name_only, contact_email_only, recipient_name_only, recipient_email_only, space_contact, tab_contact, nbsp_contact, identical_pair] =
        seed_pre_unification_db(dir.path());

    // Opening runs migration 7 (unify) + 8 (reminder marker).
    let store = ApplicationStore::open(dir.path()).unwrap();
    let contact_of = |id: &str| {
        let a = store.get(id).unwrap();
        (a.contact_name, a.contact_email)
    };

    // 1. recipient-only → the whole pair is promoted.
    assert_eq!(
        contact_of(recipient_only),
        ("Rita Recruiter".into(), "rita@acme.com".into())
    );

    // 2. contact-only → untouched.
    assert_eq!(
        contact_of(contact_only),
        ("Cora Contact".into(), "cora@acme.com".into())
    );

    // 3. both populated → the canonical pair wins; the alias is not merged in.
    assert_eq!(
        contact_of(both),
        ("Cora Contact".into(), "cora@acme.com".into())
    );

    // 4. neither → still empty (no phantom value invented).
    assert_eq!(contact_of(neither), (String::new(), String::new()));

    // 5-6. THE IDENTITY-FUSION CASES. The canonical pair is half-populated, so
    // it already belongs to someone (Cora). A per-column promotion would fill
    // the empty half from the alias row and hand back "Cora Contact
    // <rita@acme.com>" — a mailto: addressed to Rita under Cora's name, or
    // Cora's address under Rita's name. Pair-atomic: promote nothing.
    assert_eq!(
        contact_of(contact_name_only),
        ("Cora Contact".into(), String::new()),
        "a half-populated canonical pair must never absorb the alias EMAIL"
    );
    assert_eq!(
        contact_of(contact_email_only),
        (String::new(), "cora@acme.com".into()),
        "a half-populated canonical pair must never absorb the alias NAME"
    );

    // 7-8. Canonical empty + a half-populated alias → both columns move, and the
    // empty half stays empty (never back-filled from anywhere else).
    assert_eq!(
        contact_of(recipient_name_only),
        ("Rita Recruiter".into(), String::new())
    );
    assert_eq!(
        contact_of(recipient_email_only),
        (String::new(), "rita@acme.com".into())
    );

    // 9-11. A whitespace-only canonical pair counts as EMPTY and is promoted like
    // a truly empty one — for every flavour of whitespace, not just U+0020.
    // SQLite's bare `TRIM(x)` strips only spaces, so the TAB and NBSP rows failed
    // to promote here while `str::trim` (the import path) folded them: the same
    // row ended up with a different contact depending on whether it migrated in
    // place or came back through a restored bundle.
    for (id, flavour) in [
        (space_contact, "spaces"),
        (tab_contact, "tabs"),
        (nbsp_contact, "NBSP"),
    ] {
        assert_eq!(
            contact_of(id),
            ("Rita Recruiter".into(), "rita@acme.com".into()),
            "a canonical pair holding only {flavour} must count as empty and promote"
        );
    }

    // 12. Alias pair IDENTICAL to the canonical one: nothing is being dropped, so
    // the `<>` distinctness guard must suppress the preserved note (asserted
    // below) while the contact itself stays exactly as it was.
    assert_eq!(
        contact_of(identical_pair),
        ("Rita Recruiter".into(), "rita@acme.com".into())
    );

    // Every response mirrors the canonical pair onto the deprecated wire names,
    // so a renderer still reading `recipientName` sees the unified contact.
    for id in [
        recipient_only,
        contact_only,
        both,
        neither,
        contact_name_only,
        contact_email_only,
        recipient_name_only,
        recipient_email_only,
        space_contact,
        tab_contact,
        nbsp_contact,
        identical_pair,
    ] {
        let app = store.get(id).unwrap();
        assert_eq!(
            app.recipient_name, app.contact_name,
            "{id}: recipientName must mirror the canonical contactName"
        );
        assert_eq!(
            app.recipient_email, app.contact_email,
            "{id}: recipientEmail must mirror the canonical contactEmail"
        );
    }

    // The alias pair that was NOT promoted (it belongs to a second, distinct
    // person) is preserved into notes — the store stops reading the deprecated
    // columns and an export mirrors the canonical pair, so this is its only
    // recoverable home.
    for id in [both, contact_name_only, contact_email_only] {
        assert!(
            store
                .get(id)
                .unwrap()
                .notes
                .contains("Apply-by-email: Rita Recruiter <rita@acme.com>"),
            "{id}: a dropped distinct apply-by-email contact must survive in notes"
        );
    }
    // Nothing was dropped for these, so no note may be appended: the promoted
    // rows (their alias pair BECAME the contact), the empty one, and — the case
    // the `<>` distinctness guard exists for — the row whose alias pair was
    // already identical to its canonical pair.
    for id in [
        recipient_only,
        space_contact,
        tab_contact,
        nbsp_contact,
        neither,
        identical_pair,
    ] {
        assert!(
            !store.get(id).unwrap().notes.contains("Apply-by-email:"),
            "{id}: nothing was dropped, so no note may be appended"
        );
    }

    // Non-destructive: the DEPRECATED columns keep their original values on disk
    // (the migration only ever writes the canonical pair).
    assert_eq!(
        raw_column(dir.path(), both, "recipient_name"),
        "Rita Recruiter"
    );
    assert_eq!(
        raw_column(dir.path(), both, "recipient_email"),
        "rita@acme.com"
    );
    assert_eq!(
        raw_column(dir.path(), recipient_only, "recipient_name"),
        "Rita Recruiter"
    );
}

#[test]
fn contact_backfill_appends_the_preserved_note_after_existing_text() {
    let dir = TempDir::new().unwrap();
    seed_pre_unification_db(dir.path());
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute(
            "UPDATE applications SET notes = 'call back Tuesday' WHERE id = 'app-both'",
            [],
        )
        .unwrap();
    }
    let store = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(
        store.get("app-both").unwrap().notes,
        "call back Tuesday\n\nApply-by-email: Rita Recruiter <rita@acme.com>",
        "the user's own note must be kept, with the preserved contact appended"
    );
}

#[test]
fn contact_backfill_sql_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let ids = seed_pre_unification_db(dir.path());
    // Snapshot `notes` too: the preservation statement APPENDS, so a replay that
    // is not guarded would stack duplicate "Apply-by-email:" lines.
    let snapshot = |store: &ApplicationStore| -> Vec<(String, String, String)> {
        ids.iter()
            .map(|id| {
                let a = store.get(id).unwrap();
                (a.contact_name, a.contact_email, a.notes)
            })
            .collect()
    };
    let store = ApplicationStore::open(dir.path()).unwrap();
    let before = snapshot(&store);
    drop(store);

    // Re-run the migration BODY itself (not just `run_migrations`, which
    // short-circuits on user_version) — the SQL must be safe to replay.
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        let unify = &super::migrations::MIGRATIONS[6];
        assert_eq!(
            unify.name, "unify_application_contact",
            "migration order is pinned — entries are append-only"
        );
        (unify.up)(&conn).unwrap();
        (unify.up)(&conn).unwrap();
    }

    let store = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(
        before,
        snapshot(&store),
        "replaying the backfill must change nothing"
    );
}

#[test]
fn both_inbound_contact_names_write_the_same_storage() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .track_manual("", "", &meta("Acme", "Engineer"))
        .unwrap();

    // Alias write (what ApplyByEmailTab sends today) lands on the canonical pair.
    patch(
        &store,
        &id,
        Patch {
            recipient_name: Some("Rita Recruiter".into()),
            recipient_email: Some("rita@acme.com".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.contact_name, "Rita Recruiter");
    assert_eq!(app.contact_email, "rita@acme.com");
    assert_eq!(app.recipient_name, "Rita Recruiter");

    // A canonical write is visible under BOTH names too.
    patch(
        &store,
        &id,
        Patch {
            contact_name: Some("Cora Contact".into()),
            contact_email: Some("cora@acme.com".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.contact_name, "Cora Contact");
    assert_eq!(app.recipient_name, "Cora Contact");
    assert_eq!(app.recipient_email, "cora@acme.com");

    // Both names in ONE patch: the canonical one wins.
    patch(
        &store,
        &id,
        Patch {
            contact_name: Some("Canonical".into()),
            recipient_name: Some("Alias".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(store.get(&id).unwrap().contact_name, "Canonical");

    // Clearing through the alias clears the canonical pair — the stale mirror
    // must never resurrect the old value.
    patch(
        &store,
        &id,
        Patch {
            recipient_name: Some(String::new()),
            recipient_email: Some(String::new()),
            ..Default::default()
        },
    )
    .unwrap();
    let cleared = store.get(&id).unwrap();
    assert_eq!(cleared.contact_name, "");
    assert_eq!(cleared.contact_email, "");
    assert_eq!(cleared.recipient_name, "");
}

/// One bundle entry for a pre-unification export, with the four contact fields
/// under test and everything else at a harmless default.
fn imported_row(
    id: &str,
    contact_name: &str,
    contact_email: &str,
    recipient_name: &str,
    recipient_email: &str,
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "status": "applied",
        "createdAt": 1000,
        "updatedAt": 1000,
        "jobUrl": "",
        "board": "",
        "company": "Acme",
        "title": "Engineer",
        "candidate": "Jane",
        "answers": [],
        "brief": "",
        "notes": "",
        "comp": "",
        "contactName": contact_name,
        "contactEmail": contact_email,
        "recipientName": recipient_name,
        "recipientEmail": recipient_email
    })
}

#[test]
fn importing_a_pre_unification_bundle_folds_exactly_like_the_migration() {
    // `canonicalize_contact` is the import-time copy of the migration's rule;
    // these are the same cases as `contact_backfill_promotes_the_pair_atomically`
    // driven through the bundle path instead of SQL.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let bundle = serde_json::json!([
        imported_row("imp-alias-only", "", "", "Rita Recruiter", "rita@acme.com"),
        imported_row(
            "imp-contact-name-only",
            "Cora Contact",
            "",
            "Rita Recruiter",
            "rita@acme.com"
        ),
        imported_row("imp-alias-name-only", "", "", "Rita Recruiter", ""),
        imported_row("imp-whitespace-contact", "  ", " ", "Rita Recruiter", ""),
        // The lockstep case: a blank canonical pair facing an equally blank
        // ALIAS pair. The migration's WHERE requires a non-blank alias, so SQL
        // leaves this row alone — and after the ruling in `contact.rs` so does
        // the import path. Before it, Rust promoted on `canonical_empty` alone
        // and OVERWROTE the stored whitespace with the empty alias.
        imported_row("imp-blank-both", " ", "", "\t", "  "),
    ]);
    assert_eq!(store.import(&bundle).unwrap(), 5);
    let contact_of = |id: &str| {
        let a = store.get(id).unwrap();
        (a.contact_name, a.contact_email)
    };

    // Alias-only → the whole pair is promoted.
    assert_eq!(
        contact_of("imp-alias-only"),
        ("Rita Recruiter".into(), "rita@acme.com".into())
    );
    // …and mirrored back onto the deprecated wire name.
    assert_eq!(
        store.get("imp-alias-only").unwrap().recipient_name,
        "Rita Recruiter"
    );

    // THE FUSION CASE: a half-populated canonical pair must not absorb the alias
    // email, or the mailto: sink would address Rita under Cora's name.
    assert_eq!(
        contact_of("imp-contact-name-only"),
        ("Cora Contact".into(), String::new())
    );
    // The distinct contact is preserved instead of silently dropped — an export
    // mirrors the canonical pair, so this note is its only recoverable home.
    assert_eq!(
        store.get("imp-contact-name-only").unwrap().notes,
        "Apply-by-email: Rita Recruiter <rita@acme.com>"
    );

    // A half-populated ALIAS pair still moves as a unit; the empty half stays empty.
    assert_eq!(
        contact_of("imp-alias-name-only"),
        ("Rita Recruiter".into(), String::new())
    );
    // Whitespace-only canonical counts as empty, same TRIM rule as the migration.
    assert_eq!(
        contact_of("imp-whitespace-contact"),
        ("Rita Recruiter".into(), String::new())
    );
    // LOCKSTEP: nothing to promote → no write at all, byte-for-byte what the
    // migration's `WHERE … AND (TRIM(recipient_name) <> '' OR …)` leaves on disk.
    assert_eq!(
        contact_of("imp-blank-both"),
        (" ".to_string(), String::new()),
        "an empty alias pair must never overwrite the canonical pair"
    );
    // …and no preservation note is invented for a contact that does not exist.
    assert_eq!(store.get("imp-blank-both").unwrap().notes, "");
}

#[test]
fn re_importing_does_not_stack_duplicate_preserved_contacts() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let bundle = serde_json::json!([imported_row(
        "imp-repeat",
        "Cora Contact",
        "",
        "Rita Recruiter",
        "rita@acme.com"
    )]);
    store.import(&bundle).unwrap();
    let once = store.get("imp-repeat").unwrap().notes;

    // Re-importing the SAME pre-unification bundle must not append a second copy.
    store.import(&bundle).unwrap();
    assert_eq!(store.get("imp-repeat").unwrap().notes, once);

    // Nor does exporting the now-unified row and importing that back.
    let round_tripped = store.export();
    store.import(&round_tripped).unwrap();
    assert_eq!(store.get("imp-repeat").unwrap().notes, once);
}

// ── Follow-up reminder marker (`add_applications_next_action_notified_at`) ────

#[test]
fn follow_up_candidates_only_carry_rows_with_a_reminder() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let with_reminder = store
        .track_manual("", "", &meta("Acme", "Engineer"))
        .unwrap();
    let without = store
        .track_manual("", "", &meta("Globex", "Designer"))
        .unwrap();
    set_reminder(&store, &with_reminder, Some(5_000));

    let candidates = store.follow_up_candidates();
    let ids: Vec<&str> = candidates.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec![with_reminder.as_str()]);
    assert!(!ids.contains(&without.as_str()));
    let c = candidates.first().expect("one candidate");
    assert_eq!(c.next_action_at, Some(5_000));
    assert_eq!(c.notified_at, None, "a fresh reminder starts un-notified");
    assert_eq!(c.company, "Acme");
    assert_eq!(c.title, "Engineer");
    assert_eq!(c.status, ApplicationStatus::Applied);
}

#[test]
fn the_notified_marker_survives_unrelated_edits_and_clears_on_reschedule() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .track_manual("", "", &meta("Acme", "Engineer"))
        .unwrap();
    let marker = || {
        store
            .follow_up_candidates()
            .first()
            .and_then(|c| c.notified_at)
    };

    set_reminder(&store, &id, Some(5_000));
    assert!(
        store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
        "stamping the due date the sweep read must match a row"
    );
    assert_eq!(marker(), Some(9_000), "the sweep's stamp persists");

    // An unrelated patch rewrites every column — the marker must NOT be lost,
    // or a still-overdue reminder would re-notify on the very next sweep.
    patch(
        &store,
        &id,
        Patch {
            notes: Some("called them".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(marker(), Some(9_000), "an unrelated edit must not clear it");

    // Patching next_action_at to the SAME value is not a reschedule.
    set_reminder(&store, &id, Some(5_000));
    assert_eq!(
        marker(),
        Some(9_000),
        "an unchanged due date is not a reschedule"
    );

    // Moving the due date IS — the new date must be announceable once.
    set_reminder(&store, &id, Some(7_000));
    assert_eq!(marker(), None, "rescheduling clears the marker");

    // Clearing the reminder entirely also clears the marker, so re-setting the
    // same date later still notifies.
    store.mark_next_action_notified(&id, 7_000, 9_500).unwrap();
    set_reminder(&store, &id, None);
    assert!(
        store.follow_up_candidates().is_empty(),
        "a cleared reminder leaves no candidate"
    );
    set_reminder(&store, &id, Some(7_000));
    assert_eq!(marker(), None, "clearing then re-setting starts fresh");
}

#[test]
fn stamping_a_stale_due_date_matches_nothing_and_leaves_the_new_one_notifiable() {
    // The sweep's read → stamp window: the user reschedules in between. An
    // unconditional stamp would mark the row notified for a due date the sweep
    // never evaluated, silencing the NEW reminder forever.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .track_manual("", "", &meta("Acme", "Engineer"))
        .unwrap();

    set_reminder(&store, &id, Some(5_000)); // the sweep reads due = 5_000 …
    set_reminder(&store, &id, Some(7_000)); // … the user reschedules before the stamp lands

    assert!(
        !store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
        "a stamp for the OLD due date must match no row"
    );
    let candidate = store.follow_up_candidates();
    assert_eq!(
        candidate.first().and_then(|c| c.notified_at),
        None,
        "the new due date must stay notifiable"
    );

    // A missing id is the same no-op, not an error.
    assert!(!store
        .mark_next_action_notified("does-not-exist", 7_000, 9_000)
        .unwrap());
}

#[test]
fn a_status_change_to_terminal_between_the_read_and_the_stamp_blocks_the_claim() {
    // The sweep reads candidates under one lock and stamps under another. In the
    // window between, the user can close the pursuit — and a Rust-side
    // `is_terminal` re-check would have that exact same window. Only the
    // predicate INSIDE the stamping UPDATE closes it.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .track_manual("", "", &meta("Acme", "Engineer"))
        .unwrap();
    set_reminder(&store, &id, Some(5_000));

    // 1. The sweep reads: due, un-notified, not terminal → it would notify.
    let read = store
        .follow_up_candidates()
        .into_iter()
        .next()
        .expect("one candidate");
    assert!(
        !read.status.is_terminal(),
        "baseline: notifiable at read time"
    );
    assert_eq!(read.notified_at, None);

    // 2. The user rejects the application, inside the window.
    store
        .set_status(&id, ApplicationStatus::Rejected, "")
        .unwrap();

    // 3. The claim must lose.
    assert!(
        !store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
        "a pursuit that went terminal after the read must not be claimed"
    );
    assert_eq!(
        store
            .follow_up_candidates()
            .first()
            .and_then(|c| c.notified_at),
        None,
        "a refused claim leaves the row unstamped, so reviving it still reminds"
    );

    // Reviving it makes the very same claim succeed — the ONLY thing that
    // changed is the status, which is what pins the predicate.
    store
        .set_status(&id, ApplicationStatus::Interviewing, "")
        .unwrap();
    assert!(
        store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
        "a revived pursuit is claimable again"
    );
}

#[test]
fn every_terminal_status_blocks_the_claim_and_ghosted_does_not() {
    // The predicate must track `is_terminal` exactly — `ghosted` is deliberately
    // soft-terminal (a ghosted pursuit can revive), so it still reminds.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    for status in ApplicationStatus::ALL {
        let id = store
            .track_manual("", "", &meta("Acme", "Engineer"))
            .unwrap();
        set_reminder(&store, &id, Some(5_000));
        store.set_status(&id, *status, "").unwrap();
        assert_eq!(
            store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
            !status.is_terminal(),
            "{status:?}: claimability must follow is_terminal()"
        );
    }
}

#[test]
fn a_broken_follow_up_query_degrades_to_an_empty_sweep_instead_of_panicking() {
    // The reminder sweep is this query's only consumer. If a schema change ever
    // renames or drops a column it reads, an `unwrap` would take the scheduler
    // task down and a bare `.ok()` would silently kill EVERY reminder for the
    // rest of the process. It must return empty and log. Forced with the same
    // second-raw-connection trick as
    // `transition_status_if_rolls_back_status_when_event_insert_fails`.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .track_manual("", "", &meta("Acme", "Engineer"))
        .unwrap();
    set_reminder(&store, &id, Some(5_000));
    assert_eq!(
        store.follow_up_candidates().len(),
        1,
        "baseline: the row IS a candidate before the table goes away"
    );

    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute("DROP TABLE applications", []).unwrap();
    }

    // SQLite compiles a statement against the CONNECTION's cached schema, and
    // only reloads it when a statement actually steps. So the same broken schema
    // surfaces through two different arms, in this order — both are exercised
    // here, and both must degrade to an empty sweep rather than a panic.
    //
    // Arm 1 — the store has not noticed yet: `prepare` succeeds against the
    // stale schema and the failure arrives while iterating (`SQLITE_SCHEMA` →
    // re-prepare → "no such table"), i.e. the per-row arm.
    assert!(
        store.follow_up_candidates().is_empty(),
        "a mid-iteration schema failure must yield an empty sweep, never a panic"
    );

    // Arm 2 — that step reloaded the schema, so `prepare` itself now fails. This
    // is the arm a renamed/dropped COLUMN hits on every subsequent sweep, i.e.
    // the one that would silence reminders forever if it were swallowed.
    assert!(
        store.follow_up_candidates().is_empty(),
        "a prepare failure must yield an empty sweep, never a panic"
    );
    // Verified empirically, not assumed: replacing the `match conn.prepare(…)`
    // guard with `.expect(…)` panics on THIS second call with
    // `no such table: applications` (and passes on the first).

    // (The remaining arm — `query_map` itself returning `Err` — is unreachable
    // for this statement: `query_map` only fails while binding, and the
    // statement takes zero parameters. It stays a defensive `match` arm rather
    // than an `unwrap` so a future parameterised rewrite cannot become a panic.)
}

#[test]
fn a_follow_up_row_that_fails_to_decode_is_skipped_and_the_healthy_ones_still_sweep() {
    // The per-row arm: one corrupted row must not cost the user every OTHER
    // reminder. A `next_action_at` holding text SQLite cannot coerce to an
    // integer is what a hand-edited or partially-restored db looks like — it
    // passes the `IS NOT NULL` filter, then fails `row.get::<_, i64>`.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let good = store
        .track_manual("", "", &meta("Acme", "Engineer"))
        .unwrap();
    set_reminder(&store, &good, Some(5_000));

    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute(
            "INSERT INTO applications (id, status, created_at, updated_at, next_action_at)
             VALUES ('bad-row', 'applied', 1000, 1000, 'not-a-timestamp')",
            [],
        )
        .unwrap();
        // Fixture sanity: INTEGER affinity keeps a non-numeric string as TEXT,
        // which is what makes the row undecodable in the first place.
        let kind: String = conn
            .query_row(
                "SELECT typeof(next_action_at) FROM applications WHERE id = 'bad-row'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kind, "text", "fixture must store a non-integer value");
    }

    let ids: Vec<String> = store
        .follow_up_candidates()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(
        ids,
        vec![good],
        "the undecodable row is skipped; the healthy reminder still sweeps"
    );
}

// ── Migration 8 backfill (no banner storm on the first post-upgrade sweep) ────

#[test]
fn migration_8_pre_marks_already_due_reminders_so_the_first_sweep_is_quiet() {
    // Without the backfill every pre-existing overdue reminder is NULL after the
    // upgrade, so the first sweep treats the user's whole backlog as brand new
    // and announces it. MAX_PER_SWEEP paces that; it does not suppress it.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("applications.db");
    let conn = crate::db::open(&path).unwrap();

    // Build the schema up to — but NOT including — the marker migration.
    let all = super::migrations::MIGRATIONS;
    for m in &all[..7] {
        (m.up)(&conn).unwrap();
    }
    let seed = |id: &str, next: Option<i64>| {
        conn.execute(
            "INSERT INTO applications (id, status, created_at, updated_at, next_action_at)
             VALUES (?1, 'applied', 1000, 1000, ?2)",
            rusqlite::params![id, next],
        )
        .unwrap();
    };
    let before = now_ms();
    seed("long-overdue", Some(1_000));
    seed("due-a-minute-ago", Some(ts_to_db(before - 60_000)));
    seed("due-later-today", Some(ts_to_db(before + 3_600_000)));
    seed("no-reminder", None);

    let m8 = &all[7];
    assert_eq!(
        m8.name, "add_applications_next_action_notified_at",
        "migration order is pinned — entries are append-only"
    );
    (m8.up)(&conn).unwrap();
    let after = now_ms();

    let marker = |id: &str| -> Option<i64> {
        conn.query_row(
            "SELECT next_action_notified_at FROM applications WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap()
    };
    for id in ["long-overdue", "due-a-minute-ago"] {
        let stamped = marker(id).unwrap_or_else(|| panic!("{id} must be pre-marked as announced"));
        assert!(
            stamped >= ts_to_db(before) && stamped <= ts_to_db(after),
            "{id} must carry the migration's own timestamp, got {stamped}"
        );
    }
    assert_eq!(
        marker("due-later-today"),
        None,
        "a reminder that has NOT come due yet must still be announceable"
    );
    assert_eq!(
        marker("no-reminder"),
        None,
        "a row with no reminder is untouched"
    );

    // End to end: the sweep's own read now reports the backlog as delivered and
    // the future one as pending — exactly what `should_notify` filters on.
    // (`user_version` is set by hand because the bodies were replayed directly;
    // `ApplicationStore::open` would otherwise try to re-run all eight.)
    conn.execute_batch("PRAGMA user_version = 8").unwrap();
    drop(conn);
    let store = ApplicationStore::open(dir.path()).unwrap();
    let pending: Vec<String> = store
        .follow_up_candidates()
        .into_iter()
        .filter(|c| c.notified_at.is_none())
        .map(|c| c.id)
        .collect();
    assert_eq!(
        pending,
        vec!["due-later-today"],
        "only the not-yet-due reminder may still notify after the upgrade"
    );
}

#[test]
fn a_re_upsert_of_the_same_job_url_carries_the_notified_marker_forward() {
    // `write_row_conn` now WRITES this column, so every `Application` literal has
    // to carry the stored value forward. `update_fields` gets it free via
    // `..existing`; `upsert_internal`'s merge branch enumerates all fields, and
    // that is the one a re-scrape/re-track goes through. Getting it wrong there
    // re-arms an already-announced reminder on the next scrape.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let url = "https://acme.example/job/1";
    let id = store
        .track_manual(url, "b", &meta("Acme", "Engineer"))
        .unwrap();
    set_reminder(&store, &id, Some(5_000));
    assert!(store.mark_next_action_notified(&id, 5_000, 9_000).unwrap());

    // Same URL again — merges into the SAME row through `upsert_internal`.
    let same = store
        .track_manual(url, "b", &meta("Acme Corp", "Senior Engineer"))
        .unwrap();
    assert_eq!(
        same, id,
        "fixture sanity: the re-track must merge, not create"
    );
    assert_eq!(
        store
            .follow_up_candidates()
            .first()
            .and_then(|c| c.notified_at),
        Some(9_000),
        "a re-upsert must not resurrect an already-announced reminder"
    );
}

#[test]
fn the_notified_marker_survives_an_export_import_round_trip() {
    // `export`/`import` is the backup path. Dropping the marker there meant
    // restoring a backup re-fired every reminder the user had already seen.
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .track_manual("", "", &meta("Acme", "Engineer"))
        .unwrap();
    set_reminder(&store, &id, Some(5_000));
    assert!(store.mark_next_action_notified(&id, 5_000, 9_000).unwrap());

    let bundle = store.export();
    assert_eq!(
        bundle[0]["nextActionNotifiedAt"],
        serde_json::json!(9_000),
        "the marker must be on the wire, under the camelCase name the TS type declares"
    );

    let restore_dir = TempDir::new().unwrap();
    let restored = ApplicationStore::open(restore_dir.path()).unwrap();
    restored.import(&bundle).unwrap();
    assert_eq!(
        restored
            .follow_up_candidates()
            .first()
            .and_then(|c| c.notified_at),
        Some(9_000),
        "a restored backup must not re-announce an already-delivered reminder"
    );

    // A bundle exported by a build that predates the marker simply has no such
    // key — it must still import (serde default) and start un-notified.
    let mut legacy = bundle.clone();
    legacy[0]
        .as_object_mut()
        .unwrap()
        .remove("nextActionNotifiedAt")
        .expect("the field was present before removal");
    restored.import(&legacy).unwrap();
    assert_eq!(
        restored
            .follow_up_candidates()
            .first()
            .and_then(|c| c.notified_at),
        None,
        "a pre-marker bundle deserializes and starts un-notified"
    );
}

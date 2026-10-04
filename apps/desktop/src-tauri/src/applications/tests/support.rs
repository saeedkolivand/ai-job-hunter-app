//! Fixtures shared by the `applications` store tests.

use super::*;

/// A fresh store in a temp dir. Hold the guard for as long as the store is used.
pub(super) fn open_store() -> (TempDir, ApplicationStore) {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    (dir, store)
}

/// The sibling `AiGenerationStore` over the same data dir (its own migrations run on open).
pub(super) fn open_gen_store(dir: &Path) -> crate::ai_generations::AiGenerationStore {
    crate::ai_generations::AiGenerationStore::open(&dir.to_path_buf()).unwrap()
}

/// The common creation call: `applied: None`, so the `origin` decides the status.
pub(super) fn upsert(
    store: &ApplicationStore,
    job_url: &str,
    board: &str,
    meta: &ApplicationMeta,
    origin: ApplicationOrigin,
) -> String {
    store
        .upsert_for_origin(job_url, board, meta, origin, None)
        .unwrap()
}

/// A `saved` Application for `url` (LinkedIn, Acme / Engineer) — the base row the answer
/// and race tests build on.
pub(super) fn saved(store: &ApplicationStore, url: &str) -> String {
    upsert(
        store,
        url,
        "linkedin",
        &meta("Acme", "Engineer"),
        ApplicationOrigin::Saved,
    )
}

/// An answer with no id yet (what a capture or a save request carries in).
pub(super) fn ans(question: impl Into<String>, answer: impl Into<String>) -> ApplicationAnswer {
    ans_with_id("", question, answer)
}

/// An answer that already carries an id (a stored/seeded one).
pub(super) fn ans_with_id(
    id: impl Into<String>,
    question: impl Into<String>,
    answer: impl Into<String>,
) -> ApplicationAnswer {
    ApplicationAnswer {
        id: id.into(),
        question: question.into(),
        answer: answer.into(),
    }
}

/// A hand-tracked, url-less Application (always `applied`).
pub(super) fn track(store: &ApplicationStore, company: &str, title: &str) -> String {
    store.track_manual("", "", &meta(company, title)).unwrap()
}

/// Insert a bare generation row directly into `ai_generations.db`, setting
/// `application_id` to the supplied value (or NULL when None).  Used by the
/// delete/detach cross-store tests so they don't depend on the backfill path.
/// `created_at` is the hardcoded legacy `1000` — provably before
/// [`APPLICATIONS_FEATURE_EPOCH_MS`].
pub(super) fn insert_gen(
    gen_conn: &Connection,
    id: &str,
    job_url: &str,
    application_id: Option<&str>,
) {
    insert_gen_at(gen_conn, id, job_url, application_id, 1000);
}

/// Same as [`insert_gen`], but with an explicit `created_at` —
/// for the `APPLICATIONS_FEATURE_EPOCH_MS` vintage-gate tests, which need a
/// row on a specific side of that boundary rather than the hardcoded legacy
/// `1000` every other caller of `insert_gen` relies on.
pub(super) fn insert_gen_at(
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
            params![id, created_at, job_url, application_id],
        )
        .unwrap();
}

/// 2026-07-01T00:00:00Z — safely after
/// [`super::APPLICATIONS_FEATURE_EPOCH_MS`], an arbitrary fixed value rather
/// than a wall-clock read so these tests are deterministic.
pub(super) const MODERN_CREATED_AT: i64 = 1_782_864_000_000;

/// Return the `application_id` column for a generation row (None when NULL).
pub(super) fn gen_application_id(gen_conn: &Connection, gen_id: &str) -> Option<String> {
    gen_conn
        .query_row(
            "SELECT application_id FROM ai_generations WHERE id = ?1",
            params![gen_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .unwrap()
}

/// Whether the one-shot legacy-backfill marker is set in `applications.db`
/// at `dir` — direct SQL against the on-disk file, since `legacy_backfill_
/// done` is private to `applications::migrations`, a sibling module of
/// `applications::tests`, and so is not reachable from here.
pub(super) fn legacy_backfill_marker_set(dir: &Path) -> bool {
    let conn = Connection::open(dir.join("applications.db")).unwrap();
    conn.query_row(
        "SELECT 1 FROM backfill_state WHERE name = 'legacy_generations_backfill'",
        [],
        |_| Ok(()),
    )
    .is_ok()
}

/// Return the number of rows in `ai_generations` matching an `application_id`.
pub(super) fn gen_count_for_app(gen_conn: &Connection, application_id: &str) -> i64 {
    gen_conn
        .query_row(
            "SELECT COUNT(*) FROM ai_generations WHERE application_id = ?1",
            params![application_id],
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
pub(super) fn open_gen_db(dir: &Path) -> Connection {
    // Opening the store runs all migrations (including add_application_id).
    // We then drop it immediately; the DB file stays on disk.
    drop(open_gen_store(dir));
    // Re-open raw for direct SQL reads/writes in the test.
    Connection::open(dir.join("ai_generations.db")).unwrap()
}

/// Hand-build `applications.db` as it stood at schema `user_version` (`0`, or `2..=6`) —
/// the `create_applications` columns plus whatever migrations 3-6 added by then, both
/// indexes, `status_events`, and the `PRAGMA user_version` — so the migrations after it
/// run on a real old file. Hand-written on purpose (not derived from `MIGRATIONS`): a
/// shipped migration edited in place must not silently move the fixture with it.
/// Returns the connection for seeding rows; drop it before opening the store.
pub(super) fn legacy_db(dir: &Path, user_version: u32) -> Connection {
    let extra_columns: String = [
        (3, ", job_description TEXT NOT NULL DEFAULT ''"),
        (4, ", job_summary TEXT NOT NULL DEFAULT ''"),
        (
            5,
            ", recipient_name TEXT NOT NULL DEFAULT '', recipient_email TEXT NOT NULL DEFAULT ''",
        ),
        (
            6,
            ", salary_min REAL, salary_max REAL, salary_currency TEXT",
        ),
    ]
    .iter()
    .filter(|(introduced, _)| *introduced <= user_version)
    .map(|(_, columns)| *columns)
    .collect();
    let conn = Connection::open(dir.join("applications.db")).unwrap();
    conn.execute_batch(&format!(
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
            contact_email   TEXT NOT NULL DEFAULT ''{extra_columns}
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
        PRAGMA user_version = {user_version};"
    ))
    .unwrap();
    conn
}

/// [`legacy_db`] plus one bare `applied` row — the old row every additive migration
/// must keep readable.
pub(super) fn seed_legacy_row(dir: &Path, user_version: u32, id: &str) {
    legacy_db(dir, user_version)
        .execute(
            "INSERT INTO applications (id, status, created_at, updated_at)
             VALUES (?1, 'applied', 1000, 1000)",
            params![id],
        )
        .unwrap();
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
pub(super) struct Patch {
    pub(super) notes: Option<String>,
    /// Outer `None` = leave the reminder alone; `Some(None)` = clear it.
    pub(super) next_action_at: Option<Option<u64>>,
    pub(super) comp: Option<String>,
    pub(super) contact_name: Option<String>,
    pub(super) contact_email: Option<String>,
    pub(super) job_description: Option<String>,
    pub(super) job_summary: Option<String>,
    pub(super) recipient_name: Option<String>,
    pub(super) recipient_email: Option<String>,
}

/// Forward a [`Patch`] to [`ApplicationStore::update_fields`] in the one place
/// the positional order has to be spelled out.
pub(super) fn patch(store: &ApplicationStore, id: &str, p: Patch) -> AppResult<()> {
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

/// Apply `edit` to a default [`Patch`] and send it: `edit(&store, &id, |p| p.notes =
/// Some("x".into()))` patches just that field.
pub(super) fn edit(store: &ApplicationStore, id: &str, apply: impl FnOnce(&mut Patch)) {
    let mut p = Patch::default();
    apply(&mut p);
    patch(store, id, p).unwrap();
}

/// Shorthand for the overwhelmingly common single-field case: set/clear the
/// follow-up reminder.
pub(super) fn set_reminder(store: &ApplicationStore, id: &str, at: Option<u64>) {
    edit(store, id, |p| p.next_action_at = Some(at));
}

/// Metadata for `company`/`title`; `candidate` is the one non-default field, every other
/// field is the empty/`None` default.
pub(super) fn meta(company: &str, title: &str) -> ApplicationMeta {
    ApplicationMeta {
        company: company.into(),
        title: title.into(),
        candidate: "Jane".into(),
        ..Default::default()
    }
}

/// An unconfirmed, email-sourced `from -> to` write (what the email watcher produces).
pub(super) fn email_write(
    store: &ApplicationStore,
    id: &str,
    from: ApplicationStatus,
    to: ApplicationStatus,
    note: Option<&str>,
) -> bool {
    store
        .transition_status_if_sourced(id, from, to, note, EVENT_SOURCE_EMAIL, false)
        .unwrap()
}

/// The dedupe marker the sweep currently reads for the first reminder candidate.
pub(super) fn notified_at(store: &ApplicationStore) -> Option<u64> {
    store
        .follow_up_candidates()
        .first()
        .and_then(|c| c.notified_at)
}

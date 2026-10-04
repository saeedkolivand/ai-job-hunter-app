//! Found jobs persisted in SQLite (#1277) and the per-record cap.

use super::super::*;
use super::support::*;

/// Rows the found-jobs table currently holds for `id`.
fn row_count(store: &AutopilotStore, id: &str) -> usize {
    store.found_jobs_db.as_ref().unwrap().lock().row_count(id)
}

// ── Found jobs persisted in SQLite (#1277) ────────────────────────────────────

fn two_jobs() -> Vec<FoundJob> {
    vec![
        found_job_full("https://jobs.example/one", "Backend Engineer", "Acme", 1),
        found_job_full("https://jobs.example/two", "Data Engineer", "Globex", 2),
    ]
}

fn sorted_urls(ap: &Autopilot) -> Vec<String> {
    let mut urls: Vec<String> = ap.found_jobs.iter().map(|j| j.url.clone()).collect();
    urls.sort();
    urls
}

/// A store whose autopilot has two found jobs, recorded through a real run.
fn store_with_found_jobs(dir: &std::path::PathBuf) -> (AutopilotStore, String) {
    let store = AutopilotStore::new(dir);
    let ap = create_ap(&store, "FJ", "linkedin", 0.0, "manual");
    record(&store, &ap.id, 2, two_jobs());
    (store, ap.id)
}

#[test]
fn found_jobs_survive_a_restart_and_stay_out_of_autopilots_json() {
    let (_temp, dir) = temp_dir();
    let (_store, id) = store_with_found_jobs(&dir);

    let json = std::fs::read_to_string(dir.join("autopilots.json")).unwrap();
    assert!(
        !json.contains("jobs.example"),
        "found jobs are not in autopilots.json"
    );

    let reopened = AutopilotStore::new(&dir);
    assert_eq!(
        sorted_urls(&reopened.get(&id).unwrap()),
        vec!["https://jobs.example/one", "https://jobs.example/two"]
    );
}

/// The point of #1277: a status change used to rewrite every found job.
#[test]
fn a_status_change_writes_no_found_job_rows() {
    let (_temp, dir) = temp_dir();
    let (store, id) = store_with_found_jobs(&dir);
    let written = || store.found_jobs_db.as_ref().unwrap().lock().rows_written;
    let before = written();

    store.set_status(&id, AutopilotStatus::Paused);

    assert_eq!(written(), before, "no found-job row rewritten");
    assert_eq!(store.get(&id).unwrap().status, AutopilotStatus::Paused);
}

/// A legacy `autopilots.json` with found jobs inside, as every existing
/// install has, produced by a store whose database can't open (that fallback
/// keeps found jobs in the JSON, exactly as before this change).
fn legacy_file(scratch: &std::path::Path) -> (Vec<u8>, String) {
    let legacy_dir = scratch.to_path_buf();
    std::fs::create_dir_all(legacy_dir.join("autopilot_found_jobs.db")).unwrap();
    let (store, id) = store_with_found_jobs(&legacy_dir);
    assert!(store.found_jobs_db.is_none(), "database blocked on purpose");
    let bytes = std::fs::read(legacy_dir.join("autopilots.json")).unwrap();
    assert!(
        String::from_utf8_lossy(&bytes).contains("jobs.example"),
        "without a database, found jobs stay in autopilots.json"
    );
    (bytes, id)
}

#[test]
fn a_legacy_file_migrates_its_found_jobs_into_the_database() {
    let temp = tempfile::TempDir::new().unwrap();
    let (legacy, id) = legacy_file(&temp.path().join("legacy"));
    let dir = temp.path().join("app");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("autopilots.json"), &legacy).unwrap();

    let store = AutopilotStore::new(&dir);
    assert_eq!(sorted_urls(&store.get(&id).unwrap()).len(), 2);

    let json = std::fs::read_to_string(dir.join("autopilots.json")).unwrap();
    assert!(
        !json.contains("jobs.example"),
        "stripped from the JSON after migrating"
    );
    assert_eq!(
        std::fs::read(dir.join("autopilots.json.pre-sqlite")).unwrap(),
        legacy,
        "the original file is kept once"
    );
    let reopened = AutopilotStore::new(&dir);
    assert_eq!(sorted_urls(&reopened.get(&id).unwrap()).len(), 2);
}

/// A crash after the rows were committed but before the JSON was rewritten
/// leaves both copies: the next load must not double them.
#[test]
fn an_interrupted_migration_reruns_without_duplicating_jobs() {
    let temp = tempfile::TempDir::new().unwrap();
    let (legacy, id) = legacy_file(&temp.path().join("legacy"));
    let dir = temp.path().join("app");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("autopilots.json"), &legacy).unwrap();
    drop(AutopilotStore::new(&dir).list()); // migrate once
    std::fs::write(dir.join("autopilots.json"), &legacy).unwrap(); // "crash": JSON back

    let store = AutopilotStore::new(&dir);
    assert_eq!(store.get(&id).unwrap().found_jobs.len(), 2);
    assert_eq!(row_count(&store, &id), 2);
}

/// A corrupt `autopilots.json` loads empty; saving that must not wipe the found
/// jobs, which may be the only surviving copy.
#[test]
fn saving_after_a_corrupt_load_keeps_other_autopilots_found_jobs() {
    let (_temp, dir) = temp_dir();
    let (store, id) = store_with_found_jobs(&dir);
    drop(store);
    std::fs::write(dir.join("autopilots.json"), b"\0\0\0\0").unwrap();

    let store = AutopilotStore::new(&dir);
    assert!(store.list().is_empty());
    create_ap(&store, "after the corruption", "linkedin", 0.0, "manual");

    assert_eq!(row_count(&store, &id), 2);
}

#[test]
fn deleting_an_autopilot_deletes_its_found_jobs() {
    let (_temp, dir) = temp_dir();
    let (store, id) = store_with_found_jobs(&dir);
    store.remove(&id);
    assert_eq!(row_count(&store, &id), 0);
}

#[test]
fn a_restore_replaces_the_stored_found_jobs() {
    let (_temp, dir) = temp_dir();
    let (store, id) = store_with_found_jobs(&dir);
    let mut restored = store.get(&id).unwrap();
    restored.found_jobs.truncate(1);
    store.replace_all(vec![restored]);

    assert_eq!(row_count(&store, &id), 1);
    assert_eq!(
        AutopilotStore::new(&dir).get(&id).unwrap().found_jobs.len(),
        1
    );

    // A backup without this autopilot at all: its found jobs must go too.
    store.replace_all(vec![]);
    assert_eq!(row_count(&store, &id), 0);
}

/// The rows could not be read this session: the empty stand-ins must never be
/// synced, or every stored found job would be trimmed away (CodeRabbit, #1281).
#[test]
fn a_failed_read_never_lets_a_save_delete_the_stored_rows() {
    let (_temp, dir) = temp_dir();
    let (_store, id) = store_with_found_jobs(&dir);

    let store = AutopilotStore::new(&dir);
    store.found_jobs_db.as_ref().unwrap().lock().fail_load = true;
    assert!(store.get(&id).unwrap().found_jobs.is_empty(), "read failed");
    store.set_status(&id, AutopilotStatus::Paused);

    assert_eq!(row_count(&store, &id), 2, "rows survive the save");
    let reopened = AutopilotStore::new(&dir);
    assert_eq!(reopened.get(&id).unwrap().found_jobs.len(), 2);
    assert_eq!(reopened.get(&id).unwrap().status, AutopilotStatus::Paused);
}

/// Writing the rows failed: the JSON keeps carrying the found jobs, and the next
/// load moves them into the table again.
#[test]
fn a_failed_row_write_keeps_found_jobs_in_the_json() {
    let (_temp, dir) = temp_dir();
    let store = AutopilotStore::new(&dir);
    let ap = create_ap(&store, "FJ", "linkedin", 0.0, "manual");
    store.found_jobs_db.as_ref().unwrap().lock().fail_sync = true;
    record(&store, &ap.id, 2, two_jobs());

    let json = std::fs::read_to_string(dir.join("autopilots.json")).unwrap();
    assert!(
        json.contains("jobs.example"),
        "the JSON carries them instead"
    );

    let reopened = AutopilotStore::new(&dir);
    assert_eq!(reopened.get(&ap.id).unwrap().found_jobs.len(), 2);
    assert_eq!(
        row_count(&reopened, &ap.id),
        2,
        "migrated into the table on the next load"
    );
}

/// A restore that fails writes nothing: the old rows are all still there
/// (the delete and the inserts share one transaction).
#[test]
fn a_failed_restore_keeps_the_old_rows() {
    let (_temp, dir) = temp_dir();
    let (store, id) = store_with_found_jobs(&dir);

    store.found_jobs_db.as_ref().unwrap().lock().fail_sync = true;
    store.replace_found_jobs(&HashMap::new());

    assert_eq!(row_count(&store, &id), 2);
}

// ── Found-jobs cap (#1277) ────────────────────────────────────────────────────

/// `n` jobs with distinct titles (so clustering never merges them), found at
/// times `first_at`, `first_at + 1`, … in list order.
fn jobs_found_from(first_at: u64, n: usize) -> Vec<FoundJob> {
    (0..n)
        .map(|i| {
            found_job_full(
                &format!("https://jobs.example/{}", first_at + i as u64),
                &format!("Role {}", first_at + i as u64),
                "Acme",
                first_at + i as u64,
            )
        })
        .collect()
}

#[test]
fn the_cap_keeps_the_newest_jobs_in_their_existing_order() {
    // Oldest first in the list, so "newest" and "last in the list" coincide.
    let mut jobs = jobs_found_from(1, cap::MAX_FOUND_JOBS + 7);
    cap::cap_found_jobs(&mut jobs);

    assert_eq!(jobs.len(), cap::MAX_FOUND_JOBS);
    assert_eq!(jobs.first().unwrap().found_at, 8, "the 7 oldest are gone");
    assert!(
        jobs.windows(2).all(|w| w[0].found_at < w[1].found_at),
        "order kept"
    );
}

#[test]
fn the_cap_leaves_a_list_under_the_limit_alone() {
    let mut jobs = jobs_found_from(1, 3);
    let before: Vec<String> = jobs.iter().map(|j| j.url.clone()).collect();
    cap::cap_found_jobs(&mut jobs);
    let after: Vec<String> = jobs.iter().map(|j| j.url.clone()).collect();
    assert_eq!(after, before);
}

/// The owner's decision on #1277, written out by hand: a test that only read the
/// constant would pass whatever it was changed to.
#[test]
fn the_cap_is_the_500_newest() {
    assert_eq!(cap::MAX_FOUND_JOBS, 500);
}

#[test]
fn a_run_trims_found_jobs_to_the_cap_dropping_the_oldest() {
    let (_temp, dir) = temp_dir();
    let store = AutopilotStore::new(&dir);
    let ap = create_ap(&store, "Capped", "linkedin", 0.0, "manual");

    // An earlier run already holds the cap's worth of older jobs...
    let old = jobs_found_from(1_000, cap::MAX_FOUND_JOBS);
    record(&store, &ap.id, 0, old);
    // ...and a new run finds 10 more, all newer.
    let new = jobs_found_from(9_000, 10);
    record(&store, &ap.id, 0, new);

    let reopened = AutopilotStore::new(&dir);
    let jobs = reopened.get(&ap.id).unwrap().found_jobs;
    assert_eq!(jobs.len(), cap::MAX_FOUND_JOBS);
    assert!(jobs.iter().any(|j| j.found_at == 9_009), "newest kept");
    assert!(
        jobs.iter().all(|j| j.found_at >= 1_010),
        "the 10 oldest dropped"
    );
    assert_eq!(
        row_count(&reopened, &ap.id),
        cap::MAX_FOUND_JOBS,
        "the table was trimmed too"
    );
}

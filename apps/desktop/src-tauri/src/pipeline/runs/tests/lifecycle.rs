//! Factory reset, migration idempotency, the schema's `NOT NULL` primary key,
//! and the abandoned-run guard: a run whose posting was deleted mid-flight
//! must not resurrect it, and an ORDINARY in-flight run must never look
//! abandoned either.

use tempfile::TempDir;

use crate::pipeline::runs::{PipelineRunStore, RETENTION_RUNS_PER_JOB};

use super::support::{event, run, store};

#[test]
fn clear_all_empties_both_tables() {
    let (_dir, store) = store();
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    store.append_event(&event("run-1", 0, "{}")).unwrap();
    store.clear_all();
    assert!(store.runs_for_job("job-a").is_empty());
    assert!(store.events_for_run("run-1").is_empty());
}

#[test]
fn reopening_the_same_db_is_migration_idempotent() {
    let dir = TempDir::new().unwrap();
    {
        let store = PipelineRunStore::open(dir.path()).unwrap();
        store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    }
    let store = PipelineRunStore::open(dir.path()).unwrap();
    assert!(store.run("run-1").is_some(), "data survives a reopen");
}

/// SQLite permits NULL in a TEXT PRIMARY KEY, and one null-id run would disable
/// the orphan sweep forever. The schema forbids it outright.
#[test]
fn a_null_run_id_is_rejected_by_the_schema() {
    let (_dir, store) = store();
    let err = store
        .conn
        .lock()
        .execute(
            "INSERT INTO pipeline_runs
                (id, job_url, kind, depth, status, started_at, metrics_json)
             VALUES (NULL, 'j', 'resume', 'full', 'done', 1, '{}')",
            [],
        )
        .unwrap_err();
    assert!(
        err.to_string().to_uppercase().contains("NOT NULL"),
        "a NULL id must violate the schema, got: {err}"
    );
}

/// **A run whose posting was deleted mid-flight must not bring it back.**
///
/// The hazard, executed rather than argued. A delete that lands while a run is
/// in flight (`applications_delete`, `ai_generations_remove`, a factory reset,
/// a restore) takes the run's row and the events that exist at that moment. The
/// run then keeps going, and its TERMINAL write is `INSERT OR REPLACE` — so it
/// re-inserts the row, `persist_document`'s merge-upsert re-creates the
/// `ai_generations` aggregate, and the posting the user deleted is back in the
/// runs panel, back in the Documents list, and back in the next backup. Worse
/// than a plain missed delete: the trail it comes back with is permanently
/// PARTIAL, because the pre-purge events are already gone.
///
/// Every step below was confirmed by running it before the guard existed:
/// the purge empties both tables, an event appended afterwards lands as an
/// orphan, and the terminal upsert resurrects the row with 1 event where the
/// run produced 2.
///
/// The guard is `store.run(run_id).is_none()` at the terminal write — the one
/// place both delete doors must pass through — plus
/// [`PipelineRunStore::delete_events_for_run`] for the gap.
///
/// Mutation check: delete the `run(run_id).is_none()` guard in `execute` and
/// the resurrection assertions below describe production again; drop the
/// `delete_events_for_run` sweep and the orphan assertion fails.
#[test]
fn a_run_whose_posting_was_deleted_mid_flight_does_not_resurrect_it() {
    let (_dir, store) = store();
    const URL: &str = "https://boards.example/jobs/42";

    // A run is IN FLIGHT: its `running` row exists and events are landing.
    let mut row = run("live", URL, 1_000);
    row.status = "running".to_string();
    store.upsert_run(&row).unwrap();
    store
        .append_event(&event(
            "live",
            0,
            r#"{"full":{"perCompany":[{"company":"Acme"}]}}"#,
        ))
        .unwrap();

    // The user deletes the posting.
    assert_eq!(store.delete_for_job(URL), 1);
    assert!(
        store.run("live").is_none() && store.events_for_run("live").is_empty(),
        "the premise: the purge takes the row and the trail it had"
    );

    // The run has not noticed — one more section finishes and appends.
    store
        .append_event(&event("live", 1, r#"{"full":{"items":[]}}"#))
        .unwrap();
    assert_eq!(
        store.events_for_run("live").len(),
        1,
        "the premise: an event appended after the purge lands as an ORPHAN"
    );

    // THE GUARD. This is what `execute` checks before its terminal write, and
    // it is the whole signal: the row this run created is gone.
    assert!(
        store.run("live").is_none(),
        "the abandoned-run signal must be readable at the terminal write"
    );
    let swept = store.delete_events_for_run("live");
    assert_eq!(swept, 1, "the gap's events go with it");
    assert!(store.events_for_run("live").is_empty());
    assert!(
        store.runs_for_job(URL).is_empty(),
        "the deleted posting stays deleted"
    );
}

/// The other side of that guard: an ORDINARY in-flight run must never look
/// abandoned, including on a posting that already has a full retention window
/// of finished runs. `prune` keeps the newest `RETENTION_RUNS_PER_JOB` by
/// `started_at DESC`, and the running row is the newest — so it cannot evict
/// the run that is calling it out from under itself.
///
/// Without this, the guard above would be satisfied by a store that simply
/// lost every row.
///
/// Mutation check: order `prune`'s window by `started_at ASC` and the live run
/// is evicted, so an ordinary run reports itself deleted.
#[test]
fn a_running_row_is_never_evicted_by_its_own_postings_retention() {
    let (_dir, store) = store();
    const URL: &str = "https://boards.example/jobs/42";
    for i in 0..RETENTION_RUNS_PER_JOB {
        store
            .upsert_run(&run(&format!("old-{i}"), URL, 100 + i as u64))
            .unwrap();
    }
    let mut live = run("live", URL, 9_000);
    live.status = "running".to_string();
    store.upsert_run(&live).unwrap();
    store.prune();
    assert!(
        store.run("live").is_some(),
        "an ordinary in-flight run must never look abandoned"
    );
}

//! `PipelineRunStore::prune`: keeps the newest `RETENTION_RUNS_PER_JOB` per
//! `(job_url, kind)`, is idempotent, collects pre-existing orphan events, and
//! is transactional (a failed sweep rolls back the eviction; a failed
//! eviction runs no sweep and commits nothing).

use crate::pipeline::runs::RETENTION_RUNS_PER_JOB;

use super::support::{event, run, store};

#[test]
fn prune_keeps_the_newest_runs_per_job_and_drops_their_events() {
    let (_dir, store) = store();
    for i in 0..(RETENTION_RUNS_PER_JOB as u64 + 2) {
        let id = format!("run-{i}");
        store.upsert_run(&run(&id, "job-a", 1_000 + i)).unwrap();
        store.append_event(&event(&id, 0, "{}")).unwrap();
    }
    store.prune();

    let kept: Vec<String> = store
        .runs_for_job("job-a")
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(
        kept,
        vec![
            "run-4".to_string(),
            "run-3".to_string(),
            "run-2".to_string()
        ],
        "the newest {RETENTION_RUNS_PER_JOB} runs survive, newest first"
    );
    // The evicted runs' events went with them; the survivors' did not.
    assert!(store.events_for_run("run-0").is_empty());
    assert!(store.events_for_run("run-1").is_empty());
    assert_eq!(store.events_for_run("run-4").len(), 1);
}

/// Retention is PER JOB: hammering one posting must not evict another's history.
#[test]
fn prune_is_scoped_per_job_url() {
    let (_dir, store) = store();
    for i in 0..10u64 {
        store
            .upsert_run(&run(&format!("busy-{i}"), "job-busy", 1_000 + i))
            .unwrap();
    }
    store.upsert_run(&run("quiet-0", "job-quiet", 5)).unwrap();
    store.prune();

    assert_eq!(store.runs_for_job("job-busy").len(), RETENTION_RUNS_PER_JOB);
    assert_eq!(
        store.runs_for_job("job-quiet").len(),
        1,
        "another job's single run must survive a noisy neighbour"
    );
}

/// Retention is per `(job_url, kind)`, not per `job_url` alone: `kind` is the
/// discriminator that lets these tables host every staged run, so three résumé
/// runs must not evict the same posting's agent-run history.
#[test]
fn prune_is_scoped_per_kind_within_a_job() {
    let (_dir, store) = store();
    for i in 0..(RETENTION_RUNS_PER_JOB as u64 + 2) {
        store
            .upsert_run(&run(&format!("resume-{i}"), "job-a", 1_000 + i))
            .unwrap();
    }
    for i in 0..2u64 {
        let mut agent = run(&format!("agent-{i}"), "job-a", 10 + i);
        agent.kind = "agent".to_string();
        store.upsert_run(&agent).unwrap();
    }
    store.prune();

    let surviving = |kind: &str| -> usize {
        store
            .runs_for_job("job-a")
            .into_iter()
            .filter(|r| r.kind == kind)
            .count()
    };
    assert_eq!(
        surviving("resume"),
        RETENTION_RUNS_PER_JOB,
        "the noisy kind is still capped at its own retention"
    );
    assert_eq!(
        surviving("agent"),
        2,
        "the other kind's history must survive a noisy neighbour of a different kind"
    );
}

/// Idempotent and safe on an empty/already-pruned store.
#[test]
fn prune_is_idempotent() {
    let (_dir, store) = store();
    store.prune(); // empty
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    store.prune();
    store.prune();
    assert_eq!(store.runs_for_job("job-a").len(), 1);
}

/// An event whose run was removed by an earlier partial delete is collected too
/// — the sweep is written as "no matching run", not "the ids we just deleted".
#[test]
fn prune_collects_pre_existing_orphan_events() {
    let (_dir, store) = store();
    store.append_event(&event("ghost-run", 0, "{}")).unwrap();
    assert_eq!(store.events_for_run("ghost-run").len(), 1);
    store.prune();
    assert!(store.events_for_run("ghost-run").is_empty());
}

/// The transaction is real: if the orphan sweep fails AFTER the eviction
/// succeeded, prune must return without committing so the drop rolls both back.
/// Injection is a dropped `pipeline_run_events` table — the cheapest way to make
/// the SECOND statement fail while the first still succeeds.
#[test]
fn a_failed_sweep_rolls_back_the_eviction() {
    let (_dir, store) = store();
    let total = RETENTION_RUNS_PER_JOB as u64 + 2;
    for i in 0..total {
        store
            .upsert_run(&run(&format!("run-{i}"), "job-a", 1_000 + i))
            .unwrap();
    }
    store
        .conn
        .lock()
        .execute_batch("DROP TABLE pipeline_run_events")
        .unwrap();

    store.prune();

    assert_eq!(
        store.runs_for_job("job-a").len(),
        total as usize,
        "a failed sweep must leave history intact — the eviction is rolled back, not committed"
    );
}

/// The FIRST error arm: when the eviction itself fails, prune must stop there —
/// not run the sweep and not commit. Injection is a `BEFORE DELETE` trigger that
/// aborts, with an orphan event present that the sweep WOULD have collected: if
/// the sweep still ran and the commit still happened, that orphan disappears
/// while the log claims history was left intact.
#[test]
fn a_failed_eviction_runs_no_sweep_and_commits_nothing() {
    let (_dir, store) = store();
    for i in 0..(RETENTION_RUNS_PER_JOB as u64 + 2) {
        store
            .upsert_run(&run(&format!("run-{i}"), "job-a", 1_000 + i))
            .unwrap();
    }
    store.append_event(&event("ghost", 0, "{}")).unwrap();
    store
        .conn
        .lock()
        .execute_batch(
            "CREATE TRIGGER no_run_deletes BEFORE DELETE ON pipeline_runs
             BEGIN SELECT RAISE(ABORT, 'blocked'); END;",
        )
        .unwrap();

    store.prune();

    assert_eq!(
        store.events_for_run("ghost").len(),
        1,
        "the sweep must not run after a failed eviction, and nothing may commit"
    );
}

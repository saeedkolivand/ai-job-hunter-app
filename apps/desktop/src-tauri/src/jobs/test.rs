use super::*;
use serde_json::{json, Value};

#[test]
fn test_start_job() {
    let mut tracker = JobTracker::default();
    tracker.start("job-1", "scrape");
    let job = tracker.get("job-1").unwrap();
    assert_eq!(job.id, "job-1");
    assert_eq!(job.kind, "scrape");
    assert_eq!(job.status, JobStatus::Running);
    assert_eq!(job.progress, 0.0);
    assert!(job.result.is_none());
    assert!(job.error.is_none());
}

#[test]
fn test_update_progress() {
    let mut tracker = JobTracker::default();
    tracker.start("job-1", "scrape");
    tracker.update_progress("job-1", 0.5);
    let job = tracker.get("job-1").unwrap();
    assert_eq!(job.progress, 0.5);
}

#[test]
fn test_complete_job() {
    let mut tracker = JobTracker::default();
    tracker.start("job-1", "scrape");
    let result = json!({ "count": 10 });
    tracker.complete("job-1", result.clone());
    let job = tracker.get("job-1").unwrap();
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(job.progress, 1.0);
    assert_eq!(job.result, Some(result));
}

#[test]
fn test_fail_job() {
    let mut tracker = JobTracker::default();
    tracker.start("job-1", "scrape");
    tracker.fail("job-1", "network error".to_string());
    let job = tracker.get("job-1").unwrap();
    assert_eq!(job.status, JobStatus::Failed);
    assert_eq!(job.error, Some("network error".to_string()));
}

#[test]
fn test_cancel_job() {
    let mut tracker = JobTracker::default();
    tracker.start("job-1", "scrape");
    tracker.cancel("job-1");
    let job = tracker.get("job-1").unwrap();
    assert_eq!(job.status, JobStatus::Cancelled);
}

#[test]
fn test_list_jobs() {
    let mut tracker = JobTracker::default();
    tracker.start("job-1", "scrape");
    tracker.start("job-2", "apply");
    let jobs = tracker.list();
    assert_eq!(jobs.len(), 2);
    // Should be sorted by created_at desc
    assert!(jobs[0].created_at >= jobs[1].created_at);
}

#[test]
fn test_get_job() {
    let mut tracker = JobTracker::default();
    tracker.start("job-1", "scrape");
    assert!(tracker.get("job-1").is_some());
    assert!(tracker.get("nonexistent").is_none());
}

#[test]
fn test_multiple_jobs() {
    let mut tracker = JobTracker::default();
    tracker.start("job-1", "scrape");
    tracker.start("job-2", "apply");
    tracker.start("job-3", "scrape");
    tracker.update_progress("job-1", 0.3);
    tracker.complete("job-2", json!({ "success": true }));
    tracker.fail("job-3", "error".to_string());

    assert_eq!(tracker.get("job-1").unwrap().status, JobStatus::Running);
    assert_eq!(tracker.get("job-2").unwrap().status, JobStatus::Completed);
    assert_eq!(tracker.get("job-3").unwrap().status, JobStatus::Failed);
}

#[test]
fn test_lifecycle_timestamps() {
    let mut tracker = JobTracker::default();
    tracker.start("j", "ai.generate");
    let j = tracker.get("j").unwrap();
    assert!(j.started_at.is_some());
    assert!(j.finished_at.is_none());
    assert_eq!(j.retries, 0);
    assert_eq!(j.payload, Value::Null);
    let at_start = j.updated_at;

    tracker.complete("j", json!({ "ok": true }));
    let j = tracker.get("j").unwrap();
    assert_eq!(j.status, JobStatus::Completed);
    assert!(j.finished_at.is_some());
    assert!(j.updated_at >= at_start);
}

#[test]
fn test_persist_reload_migration_roundtrip() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    {
        let mut tracker = JobTracker::open(dir.path());
        tracker.start("j-1", "scrape.board");
        tracker.complete("j-1", json!({ "count": 3 }));
    }
    // Reopen: the appended-migration columns (payload/retries/finished_at/…) must
    // round-trip, and a completed job stays completed (not flagged interrupted).
    let reopened = JobTracker::open(dir.path());
    let j = reopened.get("j-1").expect("job survives reload");
    assert_eq!(j.kind, "scrape.board");
    assert_eq!(j.status, JobStatus::Completed);
    assert_eq!(j.result, Some(json!({ "count": 3 })));
    assert!(j.finished_at.is_some());
    assert_eq!(j.retries, 0);
}

// ── start_exclusive: the "second call while busy" re-entrancy guard shared
// by the embedding jobs (`ai.reembed`/`ai.indexStale`) and `ai.pull_model`
// ────────────────────────────────────────────────────────────────────────

#[test]
fn test_start_exclusive_first_caller_starts_the_job() {
    let mut tracker = JobTracker::default();
    let existing = tracker.start_exclusive("job-1", "ai.pull_model", &["ai.pull_model"]);
    assert!(existing.is_none());
    assert_eq!(tracker.get("job-1").unwrap().status, JobStatus::Running);
}

/// The re-entrancy guard itself: a second call while the first job of an
/// exclusive kind is still running must NOT start a second job — it must
/// report the first job's id back instead.
#[test]
fn test_start_exclusive_second_caller_joins_the_running_job() {
    let mut tracker = JobTracker::default();
    let first = tracker.start_exclusive("job-1", "ai.pull_model", &["ai.pull_model"]);
    assert!(first.is_none());

    let second = tracker.start_exclusive("job-2", "ai.pull_model", &["ai.pull_model"]);
    assert_eq!(second, Some("job-1".to_string()));
    // The second call must not have registered "job-2" at all.
    assert!(tracker.get("job-2").is_none());
}

/// Queued/streaming count as "busy" too, not just Running — a job parked
/// behind the concurrency limiter is still work in flight.
#[test]
fn test_start_exclusive_blocks_on_queued_and_streaming() {
    for status in [JobStatus::Queued, JobStatus::Streaming] {
        let mut tracker = JobTracker::default();
        tracker.start("job-1", "ai.pull_model");
        tracker.jobs.get_mut("job-1").unwrap().status = status.clone();
        let second = tracker.start_exclusive("job-2", "ai.pull_model", &["ai.pull_model"]);
        assert_eq!(
            second,
            Some("job-1".to_string()),
            "status {status:?} should block"
        );
    }
}

/// The guard releases once the job reaches a terminal state — it must not
/// latch permanently after the run that claimed it finishes.
#[test]
fn test_start_exclusive_releases_after_completion() {
    let mut tracker = JobTracker::default();
    tracker.start_exclusive("job-1", "ai.pull_model", &["ai.pull_model"]);
    tracker.complete("job-1", json!({ "done": true }));

    let second = tracker.start_exclusive("job-2", "ai.pull_model", &["ai.pull_model"]);
    assert!(second.is_none(), "a finished job must not block a new run");
    assert_eq!(tracker.get("job-2").unwrap().status, JobStatus::Running);
}

/// Two independent exclusive groups don't interfere: a busy job in the
/// "ai.pull_model" group must not block a start checked against an unrelated
/// group's kinds (e.g. the embed jobs), and vice versa.
#[test]
fn test_start_exclusive_ignores_unrelated_groups() {
    let mut tracker = JobTracker::default();
    tracker.start_exclusive("job-1", "ai.pull_model", &["ai.pull_model"]);
    let second = tracker.start_exclusive("job-2", "ai.reembed", &["ai.reembed", "ai.indexStale"]);
    assert!(second.is_none());
    assert_eq!(tracker.get("job-2").unwrap().status, JobStatus::Running);
}

// ── start_exclusive_keyed: `ai.pull_model` must dedupe per MODEL, not just
// kind — PR #1036 review finding. `start_exclusive` alone would join a
// request for "qwen2.5" to an already-running "llama3" pull, handing the
// caller progress for a model it never asked for while its own request
// silently never runs.
// ────────────────────────────────────────────────────────────────────────

/// Same model, second request: still joins the running job and returns its
/// id — the ORIGINAL `start_exclusive` fix, which must not regress.
#[test]
fn test_start_exclusive_keyed_same_key_joins_the_running_job() {
    let mut tracker = JobTracker::default();
    let first = tracker.start_exclusive_keyed("job-1", "ai.pull_model", "model", "llama3");
    assert_eq!(first, Ok(None));

    let second = tracker.start_exclusive_keyed("job-2", "ai.pull_model", "model", "llama3");
    assert_eq!(second, Ok(Some("job-1".to_string())));
    // The second call must not have registered "job-2" at all.
    assert!(tracker.get("job-2").is_none());
}

/// Different model, second request: refused rather than silently joined to
/// the wrong job — the defect this test guards against.
#[test]
fn test_start_exclusive_keyed_different_key_is_refused_not_joined() {
    let mut tracker = JobTracker::default();
    let first = tracker.start_exclusive_keyed("job-1", "ai.pull_model", "model", "llama3");
    assert_eq!(first, Ok(None));

    let second = tracker.start_exclusive_keyed("job-2", "ai.pull_model", "model", "qwen2.5");
    assert_eq!(
        second,
        Err("llama3".to_string()),
        "a different model must be refused, never silently joined to job-1"
    );
    assert!(
        tracker.get("job-2").is_none(),
        "a refused request must not register a job either"
    );
}

/// The key survives the round trip: a joined caller (or anything reading the
/// job list) can read back which model a pull is actually for.
#[test]
fn test_start_exclusive_keyed_records_the_key_on_the_payload() {
    let mut tracker = JobTracker::default();
    let _ = tracker.start_exclusive_keyed("job-1", "ai.pull_model", "model", "llama3");
    let job = tracker.get("job-1").unwrap();
    assert_eq!(job.payload, json!({ "model": "llama3" }));
}

#[test]
fn test_interrupted_running_job_marked_failed_on_reload() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    {
        let mut tracker = JobTracker::open(dir.path());
        tracker.start("j-run", "ai.generate"); // left running (simulated crash)
    }
    let reopened = JobTracker::open(dir.path());
    let j = reopened.get("j-run").expect("job survives reload");
    assert_eq!(j.status, JobStatus::Failed);
    assert!(j.finished_at.is_some());
}

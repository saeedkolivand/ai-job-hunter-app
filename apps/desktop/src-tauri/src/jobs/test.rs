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

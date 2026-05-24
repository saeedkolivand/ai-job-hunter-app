/// In-process job tracker for the Tauri shell.
///
/// Records dispatched jobs and their current status so the UI can show
/// progress, cancellation, and history. The actual work happens in
/// `ScraperEngine` (in-process Rust).
///
/// Jobs are kept in memory for the session and are not persisted to disk.
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRecord {
    pub id: String,
    pub kind: String,
    pub status: JobStatus,
    /// 0.0 – 1.0
    pub progress: f64,
    pub created_at: u64,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct JobTracker {
    jobs: HashMap<String, JobRecord>,
}

impl JobTracker {
    /// Register a new job as running. Call before dispatching to `ScraperEngine`.
    pub fn start(&mut self, id: &str, kind: &str) {
        self.jobs.insert(
            id.to_string(),
            JobRecord {
                id: id.to_string(),
                kind: kind.to_string(),
                status: JobStatus::Running,
                progress: 0.0,
                created_at: now_ms(),
                result: None,
                error: None,
            },
        );
    }

    pub fn update_progress(&mut self, id: &str, p: f64) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.progress = p;
        }
    }

    pub fn complete(&mut self, id: &str, result: Value) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Completed;
            job.progress = 1.0;
            job.result = Some(result);
        }
    }

    pub fn fail(&mut self, id: &str, error: String) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Failed;
            job.error = Some(error);
        }
    }

    pub fn cancel(&mut self, id: &str) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Cancelled;
        }
    }

    pub fn list(&self) -> Vec<&JobRecord> {
        let mut jobs: Vec<&JobRecord> = self.jobs.values().collect();
        jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        jobs
    }

    pub fn get(&self, id: &str) -> Option<&JobRecord> {
        self.jobs.get(id)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}


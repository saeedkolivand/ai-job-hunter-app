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
mod test;


/// In-process job tracker for the Tauri shell.
///
/// Mirrors the Electron JobQueue's observable state that the renderer reads via
/// jobs_list / jobs_get. The actual work happens in the scraper-runtime sidecar;
/// this struct just records what was dispatched and its current status so the UI
/// can show progress, cancellation, and history.
///
/// Jobs are kept in memory for the session. They are not persisted to disk —
/// the sidecar is the authoritative source of truth for in-flight work.
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
    /// Register a new job as pending. Call before sending the command to the sidecar.
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


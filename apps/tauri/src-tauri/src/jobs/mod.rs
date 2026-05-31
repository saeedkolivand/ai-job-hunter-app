/// In-process job tracker for the Tauri shell.
///
/// Records dispatched jobs and their status both in memory (for fast lookups)
/// and in SQLite (for crash recovery). On startup, incomplete jobs from the
/// last session are loaded and surfaced as `failed` so the UI can show them.
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;

use crate::db::{run_migrations, Migration};

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

impl JobStatus {
    fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "cancelled" => JobStatus::Cancelled,
            "pending" => JobStatus::Pending,
            _ => JobStatus::Failed, // treat unknown/interrupted as failed
        }
    }
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
    db: Option<Connection>,
}

impl JobTracker {
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
            name: "create_jobs",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS jobs (
                        id         TEXT PRIMARY KEY,
                        kind       TEXT NOT NULL,
                        status     TEXT NOT NULL,
                        progress   REAL NOT NULL DEFAULT 0.0,
                        created_at INTEGER NOT NULL,
                        result     TEXT,
                        error      TEXT
                    );
                    CREATE INDEX IF NOT EXISTS idx_jobs_created ON jobs(created_at DESC);",
                )
            },
        },
    ];

    /// Open a persistent job tracker backed by SQLite in `data_dir`.
    /// Incomplete jobs from the previous session are loaded as `failed`.
    pub fn open(data_dir: &Path) -> Self {
        let db_path = data_dir.join("jobs.db");
        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("[jobs] failed to open jobs.db, running in-memory only: {e}");
                return Self::default();
            }
        };
        if let Err(e) = run_migrations(&conn, Self::MIGRATIONS) {
            log::warn!("[jobs] migration failed, running in-memory only: {e}");
            return Self::default();
        }

        // Load recent jobs (last 24 h) and mark any interrupted running jobs as failed.
        let cutoff = now_ms().saturating_sub(24 * 60 * 60 * 1_000);
        let _ = conn.execute(
            "UPDATE jobs SET status = 'failed', error = 'Interrupted by app restart'
             WHERE status IN ('running', 'pending') AND created_at > ?1",
            params![cutoff as i64],
        );

        let mut jobs = HashMap::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, kind, status, progress, created_at, result, error
             FROM jobs WHERE created_at > ?1 ORDER BY created_at DESC",
        ) {
            let rows = stmt.query_map(params![cutoff as i64], |row| {
                let result_str: Option<String> = row.get(5)?;
                let result = result_str
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok());
                Ok(JobRecord {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    status: JobStatus::from_str(&row.get::<_, String>(2)?),
                    progress: row.get(3)?,
                    created_at: row.get::<_, i64>(4)? as u64,
                    result,
                    error: row.get(6)?,
                })
            });
            if let Ok(rows) = rows {
                for r in rows.flatten() {
                    jobs.insert(r.id.clone(), r);
                }
            }
        }

        log::info!("[jobs] loaded {} recent job(s) from disk", jobs.len());
        Self { jobs, db: Some(conn) }
    }

    /// Register a new job as running.
    pub fn start(&mut self, id: &str, kind: &str) {
        let record = JobRecord {
            id: id.to_string(),
            kind: kind.to_string(),
            status: JobStatus::Running,
            progress: 0.0,
            created_at: now_ms(),
            result: None,
            error: None,
        };
        self.persist_upsert(&record);
        self.jobs.insert(id.to_string(), record);
    }

    pub fn update_progress(&mut self, id: &str, p: f64) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.progress = p;
            if let Some(db) = &self.db {
                let _ = db.execute(
                    "UPDATE jobs SET progress = ?1 WHERE id = ?2",
                    params![p, id],
                );
            }
        }
    }

    pub fn complete(&mut self, id: &str, result: Value) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Completed;
            job.progress = 1.0;
            job.result = Some(result.clone());
            let result_str = serde_json::to_string(&result).ok();
            if let Some(db) = &self.db {
                let _ = db.execute(
                    "UPDATE jobs SET status = 'completed', progress = 1.0, result = ?1 WHERE id = ?2",
                    params![result_str, id],
                );
            }
        }
    }

    pub fn fail(&mut self, id: &str, error: String) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Failed;
            job.error = Some(error.clone());
            if let Some(db) = &self.db {
                let _ = db.execute(
                    "UPDATE jobs SET status = 'failed', error = ?1 WHERE id = ?2",
                    params![error, id],
                );
            }
        }
    }

    pub fn cancel(&mut self, id: &str) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Cancelled;
            if let Some(db) = &self.db {
                let _ = db.execute(
                    "UPDATE jobs SET status = 'cancelled' WHERE id = ?1",
                    params![id],
                );
            }
        }
    }

    pub fn list(&self) -> Vec<&JobRecord> {
        let mut jobs: Vec<&JobRecord> = self.jobs.values().collect();
        jobs.sort_by_key(|j| std::cmp::Reverse(j.created_at));
        jobs
    }

    pub fn get(&self, id: &str) -> Option<&JobRecord> {
        self.jobs.get(id)
    }

    fn persist_upsert(&self, record: &JobRecord) {
        if let Some(db) = &self.db {
            let result_str = record.result.as_ref().and_then(|r| serde_json::to_string(r).ok());
            let _ = db.execute(
                "INSERT OR REPLACE INTO jobs (id, kind, status, progress, created_at, result, error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    record.id,
                    record.kind,
                    record.status.as_str(),
                    record.progress,
                    record.created_at as i64,
                    result_str,
                    record.error,
                ],
            );
        }
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

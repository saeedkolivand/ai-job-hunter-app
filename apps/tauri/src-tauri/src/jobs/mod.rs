/// In-process job tracker for the Tauri shell.
///
/// Records dispatched jobs and their status both in memory (for fast lookups)
/// and in SQLite (for crash recovery). On startup, incomplete jobs from the
/// last session are loaded and surfaced as `failed` so the UI can show them.
///
/// The record shape mirrors the shared `JobRecord` (packages/shared/src/types)
/// 1:1 in camelCase: id, kind, status, progress, payload, result, error,
/// retries, maxRetries, createdAt, updatedAt, startedAt, finishedAt. The tracker
/// is L1 (pure data + disk, AppHandle-free); Tauri-event emission for each
/// transition lives in the L3 wrapper (`commands::jobs`), so this module never
/// reaches the shell.
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
    Queued,
    Running,
    Streaming,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

impl JobStatus {
    fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Streaming => "streaming",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
            JobStatus::Retrying => "retrying",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            // `pending` is a legacy alias from before the status model expanded.
            "queued" | "pending" => JobStatus::Queued,
            "running" => JobStatus::Running,
            "streaming" => JobStatus::Streaming,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "cancelled" => JobStatus::Cancelled,
            "retrying" => JobStatus::Retrying,
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
    /// The original dispatch payload (kept for retry re-dispatch). `Null` when
    /// the dispatcher didn't record one.
    pub payload: Value,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub retries: u32,
    pub max_retries: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
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
        // Appended for the unified 12-field JobRecord. SQLite ADD COLUMN can't
        // re-run, but the user_version runner gates each migration to exactly
        // once. `create_jobs` above is never edited.
        Migration {
            name: "jobs_add_lifecycle_fields",
            up: |conn| {
                conn.execute_batch(
                    "ALTER TABLE jobs ADD COLUMN payload TEXT NOT NULL DEFAULT '{}';
                     ALTER TABLE jobs ADD COLUMN retries INTEGER NOT NULL DEFAULT 0;
                     ALTER TABLE jobs ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 0;
                     ALTER TABLE jobs ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
                     ALTER TABLE jobs ADD COLUMN started_at INTEGER;
                     ALTER TABLE jobs ADD COLUMN finished_at INTEGER;",
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

        // Load recent jobs (last 24 h) and mark any interrupted in-flight jobs as
        // failed (records the failure time too).
        let cutoff = now_ms().saturating_sub(24 * 60 * 60 * 1_000);
        let now = now_ms();
        let _ = conn.execute(
            "UPDATE jobs SET status = 'failed', error = 'Interrupted by app restart',
                 updated_at = ?1, finished_at = ?1
             WHERE status IN ('running', 'pending', 'queued', 'streaming', 'retrying')
               AND created_at > ?2",
            params![now as i64, cutoff as i64],
        );

        let mut jobs = HashMap::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, kind, status, progress, payload, result, error, retries,
                    max_retries, created_at, updated_at, started_at, finished_at
             FROM jobs WHERE created_at > ?1 ORDER BY created_at DESC",
        ) {
            let rows = stmt.query_map(params![cutoff as i64], |row| {
                let payload_str: Option<String> = row.get(4)?;
                let payload = payload_str
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(Value::Null);
                let result_str: Option<String> = row.get(5)?;
                let result = result_str
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok());
                Ok(JobRecord {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    status: JobStatus::from_str(&row.get::<_, String>(2)?),
                    progress: row.get(3)?,
                    payload,
                    result,
                    error: row.get(6)?,
                    retries: row.get::<_, i64>(7)? as u32,
                    max_retries: row.get::<_, i64>(8)? as u32,
                    created_at: row.get::<_, i64>(9)? as u64,
                    updated_at: row.get::<_, i64>(10)? as u64,
                    started_at: row.get::<_, Option<i64>>(11)?.map(|v| v as u64),
                    finished_at: row.get::<_, Option<i64>>(12)?.map(|v| v as u64),
                })
            });
            if let Ok(rows) = rows {
                for r in rows.flatten() {
                    jobs.insert(r.id.clone(), r);
                }
            }
        }

        log::info!("[jobs] loaded {} recent job(s) from disk", jobs.len());
        Self {
            jobs,
            db: Some(conn),
        }
    }

    /// Register a new job as running.
    pub fn start(&mut self, id: &str, kind: &str) {
        let now = now_ms();
        let record = JobRecord {
            id: id.to_string(),
            kind: kind.to_string(),
            status: JobStatus::Running,
            progress: 0.0,
            payload: Value::Null,
            result: None,
            error: None,
            retries: 0,
            max_retries: 0,
            created_at: now,
            updated_at: now,
            started_at: Some(now),
            finished_at: None,
        };
        self.persist_upsert(&record);
        self.jobs.insert(id.to_string(), record);
    }

    /// Wipe the job-execution log — in-memory records and the `jobs` table.
    /// Used by the factory reset.
    pub fn clear(&mut self) {
        self.jobs.clear();
        if let Some(db) = &self.db {
            let _ = db.execute("DELETE FROM jobs", []);
        }
    }

    pub fn update_progress(&mut self, id: &str, p: f64) {
        let record = match self.jobs.get_mut(id) {
            Some(job) => {
                job.progress = p;
                job.updated_at = now_ms();
                job.clone()
            }
            None => return,
        };
        self.persist_upsert(&record);
    }

    pub fn complete(&mut self, id: &str, result: Value) {
        let record = match self.jobs.get_mut(id) {
            Some(job) => {
                let now = now_ms();
                job.status = JobStatus::Completed;
                job.progress = 1.0;
                job.result = Some(result);
                job.updated_at = now;
                job.finished_at = Some(now);
                job.clone()
            }
            None => return,
        };
        self.persist_upsert(&record);
    }

    pub fn fail(&mut self, id: &str, error: String) {
        let record = match self.jobs.get_mut(id) {
            Some(job) => {
                let now = now_ms();
                job.status = JobStatus::Failed;
                job.error = Some(error);
                job.updated_at = now;
                job.finished_at = Some(now);
                job.clone()
            }
            None => return,
        };
        self.persist_upsert(&record);
    }

    pub fn cancel(&mut self, id: &str) {
        let record = match self.jobs.get_mut(id) {
            Some(job) => {
                let now = now_ms();
                job.status = JobStatus::Cancelled;
                job.updated_at = now;
                job.finished_at = Some(now);
                job.clone()
            }
            None => return,
        };
        self.persist_upsert(&record);
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
            let payload_str = serde_json::to_string(&record.payload).ok();
            let result_str = record
                .result
                .as_ref()
                .and_then(|r| serde_json::to_string(r).ok());
            let _ = db.execute(
                "INSERT OR REPLACE INTO jobs
                    (id, kind, status, progress, payload, result, error, retries,
                     max_retries, created_at, updated_at, started_at, finished_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    record.id,
                    record.kind,
                    record.status.as_str(),
                    record.progress,
                    payload_str,
                    result_str,
                    record.error,
                    record.retries as i64,
                    record.max_retries as i64,
                    record.created_at as i64,
                    record.updated_at as i64,
                    record.started_at.map(|v| v as i64),
                    record.finished_at.map(|v| v as i64),
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

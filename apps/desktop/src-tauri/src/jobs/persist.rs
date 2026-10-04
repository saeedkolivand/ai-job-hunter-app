//! SQLite persistence for [`JobTracker`]: the schema, the boot-time load of
//! recent jobs (interrupted in-flight ones come back as `failed`) and the
//! per-transition upsert. Everything here moved verbatim out of `mod.rs`; the
//! tracker's in-memory lifecycle stays there.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::params;
use serde_json::Value;

use super::{JobRecord, JobStatus, JobTracker};
use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::observability::sanitize_reason;

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
        let mut conn = match crate::db::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "[jobs] failed to open jobs.db, running in-memory only: {}",
                    e.code()
                );
                return Self::default();
            }
        };
        if let Err(e) = run_migrations(&mut conn, Self::MIGRATIONS) {
            log::warn!(
                "[jobs] migration failed, running in-memory only: {}",
                sanitize_reason(&e.to_string())
            );
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
            params![ts_to_db(now), ts_to_db(cutoff)],
        );

        let mut jobs = HashMap::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, kind, status, progress, payload, result, error, retries,
                    max_retries, created_at, updated_at, started_at, finished_at
             FROM jobs WHERE created_at > ?1 ORDER BY created_at DESC",
        ) {
            let rows = stmt.query_map(params![ts_to_db(cutoff)], |row| {
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
                    created_at: ts_from_db(row.get::<_, i64>(9)?),
                    updated_at: ts_from_db(row.get::<_, i64>(10)?),
                    started_at: row.get::<_, Option<i64>>(11)?.map(ts_from_db),
                    finished_at: row.get::<_, Option<i64>>(12)?.map(ts_from_db),
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

    pub(super) fn persist_upsert(&self, record: &JobRecord) {
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
                    ts_to_db(record.created_at),
                    ts_to_db(record.updated_at),
                    record.started_at.map(ts_to_db),
                    record.finished_at.map(ts_to_db),
                ],
            );
        }
    }
}

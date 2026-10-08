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

use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

use crate::db::now_ms;

/// Cancellation tokens for in-flight jobs/runs, keyed by id — the tracker's
/// sibling: this module records what a job DID, `cancel` records how to STOP it.
pub mod cancel;
mod persist;

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

/// Outcome of [`JobTracker::start_exclusive_keyed`].
///
/// A plain three-way result, not `Result<Option<String>, String>`: `Busy`
/// carries the OTHER job's key (e.g. "llama3" when the caller asked for
/// "qwen2.5") — that is data the caller routes on, not a diagnostic, so
/// forcing it through `Err` would just make the call site decode a
/// convention instead of reading a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyedExclusiveStart {
    /// Nothing of `kind` was running; this job started.
    Started,
    /// A job of `kind` with the SAME key is already active — join it (its id).
    Joined(String),
    /// A job of `kind` with a DIFFERENT key is active — refused. Carries
    /// that job's key so the caller can say what's already running.
    Busy(String),
}

/// Separates a parent job id from a part's name in a [child stream
/// id](child_stream_id).
const CHILD_SEP: char = '#';

/// The id a part of job `parent` streams under (`<parent>#<part>`), so two
/// concurrent streams of one run do not interleave on one `ai:stream` jobId.
/// Cancelling `parent` cancels it ([`JobTracker::is_cancelled`]). Mirrored by
/// the renderer's `use-resume-pipeline-session`.
pub fn child_stream_id(parent: &str, part: &str) -> String {
    // `is_child_id` is a bare `#` test, so a parent id must never contain one.
    debug_assert!(!parent.contains(CHILD_SEP), "nested child id: {parent}");
    format!("{parent}{CHILD_SEP}{part}")
}

/// Whether `id` is a [child stream id](child_stream_id). Such a record exists
/// only to capture one stream's text and cancel state: it is never persisted,
/// listed, read over IPC, or announced by a `job.*` event.
pub fn is_child_id(id: &str) -> bool {
    id.contains(CHILD_SEP)
}

#[derive(Default)]
pub struct JobTracker {
    jobs: HashMap<String, JobRecord>,
    db: Option<Connection>,
}

impl JobTracker {
    /// Register a new job as running.
    pub fn start(&mut self, id: &str, kind: &str) {
        self.start_with_payload(id, kind, Value::Null);
    }

    /// [`Self::start`], recording `payload` instead of `Value::Null` — the
    /// shared body [`Self::start_exclusive_keyed`] uses to stamp the
    /// distinguishing key (e.g. the model being pulled) onto the record it
    /// creates, so a later caller can read it back off `job.payload`.
    fn start_with_payload(&mut self, id: &str, kind: &str, payload: Value) {
        let now = now_ms();
        let record = JobRecord {
            id: id.to_string(),
            kind: kind.to_string(),
            status: JobStatus::Running,
            progress: 0.0,
            payload,
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

    /// Register `id` as running UNLESS a job of one of `exclusive_kinds` is
    /// already active — returning that job's id instead.
    ///
    /// The scan and the insert happen under the SAME lock on purpose. Doing them
    /// as two calls is check-then-act: two commands can both observe "nothing
    /// running" before either registers, and both proceed. For the embedding
    /// jobs this guards, that means two concurrent runs embedding the same
    /// documents — a cloud provider billed twice for identical work.
    ///
    /// `None` when this job was started; `Some(existing_id)` when one was already
    /// running — that is not an error, it is the job the caller should watch.
    pub fn start_exclusive(
        &mut self,
        id: &str,
        kind: &str,
        exclusive_kinds: &[&str],
    ) -> Option<String> {
        if let Some(existing) = self.jobs.values().find(|j| {
            exclusive_kinds.contains(&j.kind.as_str())
                && matches!(
                    j.status,
                    JobStatus::Running | JobStatus::Queued | JobStatus::Streaming
                )
        }) {
            return Some(existing.id.clone());
        }
        self.start(id, kind);
        None
    }

    /// Like [`Self::start_exclusive`], but the exclusion group is `kind` PLUS
    /// a caller-supplied `key` (e.g. the model being pulled) rather than
    /// `kind` alone.
    ///
    /// `kind`-only exclusivity is wrong here: a returning caller must join a
    /// pull of the SAME model already in flight, but a request for a
    /// DIFFERENT model must not silently adopt that unrelated job — the
    /// caller would watch (and eventually believe it received) a download it
    /// never asked for, while its own request never ran at all. Reusing
    /// `start_exclusive`'s kind-only match here would do exactly that.
    ///
    /// Three-way outcome, not a `Result` — [`KeyedExclusiveStart::Busy`] is an
    /// expected routing outcome (which other key is active), not a
    /// diagnostic, so wrapping it in `Err` would be a stringly-typed
    /// `Result<_, String>` for something that never fails.
    pub fn start_exclusive_keyed(
        &mut self,
        id: &str,
        kind: &str,
        key_field: &str,
        key: &str,
    ) -> KeyedExclusiveStart {
        if let Some(existing) = self.jobs.values().find(|j| {
            j.kind == kind
                && matches!(
                    j.status,
                    JobStatus::Running | JobStatus::Queued | JobStatus::Streaming
                )
        }) {
            let existing_key = existing.payload.get(key_field).and_then(Value::as_str);
            return if existing_key == Some(key) {
                KeyedExclusiveStart::Joined(existing.id.clone())
            } else {
                KeyedExclusiveStart::Busy(existing_key.unwrap_or("unknown").to_string())
            };
        }
        self.start_with_payload(id, kind, serde_json::json!({ key_field: key }));
        KeyedExclusiveStart::Started
    }

    /// Wipe the job-execution log — in-memory records and the `jobs` table.
    /// Used by the factory reset.
    pub fn clear(&mut self) {
        self.jobs.clear();
        if let Some(db) = &self.db {
            let _ = db.execute("DELETE FROM jobs", []);
        }
    }

    /// Move a live job between [`JobStatus::Queued`] and [`JobStatus::Running`].
    ///
    /// Only those two: a job parked behind the concurrency limiter has not
    /// started yet, and the renderer must be able to tell that apart from a
    /// started-but-slow job — otherwise its stream deadline counts down while the
    /// job is still waiting its turn, and a legitimately queued generation fails
    /// having never issued a request. Terminal states go through
    /// [`Self::complete`] / [`Self::fail`] / [`Self::cancel`], which also stamp
    /// `finished_at`; a no-op here for an already-finished job keeps a late
    /// transition from resurrecting one.
    pub fn set_waiting(&mut self, id: &str, queued: bool) {
        let record = match self.jobs.get_mut(id) {
            Some(job) if matches!(job.status, JobStatus::Queued | JobStatus::Running) => {
                job.status = if queued {
                    JobStatus::Queued
                } else {
                    JobStatus::Running
                };
                job.updated_at = now_ms();
                job.clone()
            }
            _ => return,
        };
        self.persist_upsert(&record);
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
        let mut jobs: Vec<&JobRecord> =
            self.jobs.values().filter(|j| !is_child_id(&j.id)).collect();
        jobs.sort_by_key(|j| std::cmp::Reverse(j.created_at));
        jobs
    }

    pub fn get(&self, id: &str) -> Option<&JobRecord> {
        self.jobs.get(id)
    }

    /// Whether `id` — or the job it is a [child stream](child_stream_id) of —
    /// has been cancelled. What the stream loops poll, so cancelling a run's
    /// umbrella job also stops a part streaming under its own id.
    pub fn is_cancelled(&self, id: &str) -> bool {
        std::iter::once(id)
            .chain(id.split_once(CHILD_SEP).map(|(parent, _)| parent))
            .any(|id| {
                self.get(id)
                    .is_some_and(|j| j.status == JobStatus::Cancelled)
            })
    }

    /// Drop a record from memory and from disk. For short-lived records that
    /// exist only to capture one stream's text ([`child_stream_id`]); a normal
    /// job is history and is never removed.
    pub fn forget(&mut self, id: &str) {
        if self.jobs.remove(id).is_some() {
            self.persist_delete(id);
        }
    }
}

#[cfg(test)]
mod tests;

//! Shared fixtures: a fresh temp-dir-backed store, and minimal `RunRow`/
//! `RunEventRow` builders.

use tempfile::TempDir;

use super::super::{PipelineRunStore, RunEventRow, RunRow};

pub(super) fn store() -> (TempDir, PipelineRunStore) {
    let dir = TempDir::new().unwrap();
    let store = PipelineRunStore::open(dir.path()).unwrap();
    (dir, store)
}

pub(super) fn run(id: &str, job_url: &str, started_at: u64) -> RunRow {
    RunRow {
        id: id.to_string(),
        job_url: job_url.to_string(),
        kind: "resume".to_string(),
        depth: "full".to_string(),
        status: "running".to_string(),
        started_at,
        finished_at: None,
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    }
}

pub(super) fn event(run_id: &str, seq: u32, artifact: &str) -> RunEventRow {
    RunEventRow {
        run_id: run_id.to_string(),
        seq,
        ts: 1_700_000_000_000 + u64::from(seq),
        stage: "draft".to_string(),
        phase: "finish".to_string(),
        artifact_json: artifact.to_string(),
    }
}

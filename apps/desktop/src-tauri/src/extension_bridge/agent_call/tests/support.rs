//! Shared fixtures for the `confirm_and_run`/grace-window test topics (`refusal.rs`'s and
//! `dispatch_confirm.rs`'s own tests both build on the SAME ineligible-for-a-grace-window
//! `ProofSource`, so it lives once here rather than duplicated in each).

use serde_json::{json, Value};

use super::super::super::agent_cli::policy::ProofSource;

// ── confirm_and_run — the ceremony's own decision, without an AppHandle ──
// `dispatch_irreversible_confirmed` takes a concrete `&AppHandle` and the
// crate has no Tauri mock, so none of these three outcomes had a test.
// `confirm_and_run` is the same decision with the handle factored out.

/// A distinctive proof value + a distinctive wrong guess: both must stay out
/// of the mismatch refusal's own `detail` (the ADR-038 §4 rule already
/// pinned for the fixed string, re-checked here against the values that
/// actually flowed through the comparison).
pub(super) const PROOF_VALUE: &str = "proof-value-9f2c";
pub(super) const WRONG_GUESS: &str = "wrong-guess-1a3d";
/// A command name unique to this test group, used as an INELIGIBLE (non-`ai_spend_summary`)
/// `ProofSource::Scalar::read_command` -- `confirm_and_run` gets no grace window for it, so these
/// tests exercise the ordinary exact-match/mismatch path, never the snapshot map.
const CMD: &str = "confirm_and_run_test_command";
pub(super) const CMD_SOURCE: ProofSource = ProofSource::Scalar {
    read_command: CMD,
    path: &[],
};

/// A real `JobRecord` as `jobs_get` serializes one, completed with `result`.
pub(super) fn completed_job_record_fixture(result: Value) -> Value {
    use crate::jobs::{JobRecord, JobStatus};

    serde_json::to_value(JobRecord {
        id: "job-1".to_string(),
        kind: "ai.generate".to_string(),
        status: JobStatus::Completed,
        progress: 1.0,
        payload: json!({}),
        result: Some(result),
        error: None,
        retries: 0,
        max_retries: 0,
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
        started_at: Some(1_700_000_000_000),
        finished_at: Some(1_700_000_000_000),
    })
    .unwrap()
}

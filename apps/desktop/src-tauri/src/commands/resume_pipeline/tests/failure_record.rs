//! A failed run is readable (#1496): its row exists, and carries a kind and a sanitized message.

use serde_json::{json, Value};

use super::super::failure::{fallback_job_url, message_of, record};
use super::super::read::summary;
use super::super::resolve::JobSource;
use crate::error::AppError;
use crate::pipeline::runs::{PipelineRunStore, RunRow};

const STARTED: u64 = 1_700_000_000_000;

fn store() -> (tempfile::TempDir, PipelineRunStore) {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = PipelineRunStore::open(dir.path()).expect("store opens");
    (dir, store)
}

fn running_row(id: &str, metrics: Value) -> RunRow {
    RunRow {
        id: id.to_string(),
        job_url: "https://boards.example/jobs/2".to_string(),
        kind: "resume".to_string(),
        depth: "quality".to_string(),
        status: "running".to_string(),
        started_at: STARTED,
        finished_at: None,
        stopped_reason: Some("provider_error".to_string()),
        metrics_json: metrics.to_string(),
    }
}

/// The #1496 case: the run died resolving its inputs, before `execute` ever wrote a row, so
/// `resume_pipeline_get` answered `null`. Mutation check: delete the row creation in `record`
/// and `store.run` stays `None`.
#[test]
fn a_run_that_failed_before_it_had_a_row_is_readable_with_its_kind() {
    let (_dir, store) = store();
    let error = AppError::Provider("ollama-cloud 402: Add usage credits or subscribe".to_string());

    record(
        &store,
        "run-early",
        Some("https://boards.example/jobs/1"),
        STARTED,
        &error,
    );

    let row = store
        .run("run-early")
        .expect("the failed run must be readable, not null");
    assert_eq!(row.status, "failed");
    assert_eq!(row.kind, "resume");
    assert!(row.finished_at.is_some());
    let wire = summary(&row);
    assert_eq!(wire["errorKind"], "billing");
    assert!(wire["errorMessage"]
        .as_str()
        .unwrap()
        .contains("usage credits"));
}

#[test]
fn a_run_left_running_is_closed_and_keeps_its_metrics() {
    let (_dir, store) = store();
    let metrics = json!({ "calls": 3, "research": { "status": "ok" } });
    store.upsert_run(&running_row("run-late", metrics)).unwrap();

    let error = AppError::Config("openai: invalid or unauthorized API key.".into());
    record(&store, "run-late", None, STARTED, &error);

    let wire = summary(&store.run("run-late").unwrap());
    assert_eq!(wire["status"], "failed");
    assert_eq!(
        wire["stoppedReason"], "provider_error",
        "the stop reason is not relabelled"
    );
    assert_eq!(wire["errorKind"], "auth");
    assert_eq!(wire["metrics"]["calls"], 3);
    assert_eq!(wire["research"], json!({ "status": "ok" }));
}

/// A validation failure is not a provider failure: it gets a message but no kind.
#[test]
fn an_unclassified_failure_has_a_message_and_a_null_kind() {
    let (_dir, store) = store();
    let error = AppError::Validation("resume not found: r1".into());
    record(
        &store,
        "run-v",
        Some("https://boards.example/jobs/3"),
        STARTED,
        &error,
    );
    let wire = summary(&store.run("run-v").unwrap());
    assert_eq!(wire["errorKind"], Value::Null);
    assert_eq!(wire["errorMessage"], "resume not found: r1");
}

/// ADR-032 point 7: the message is redacted, not the raw error.
/// Mutation check: return `error.to_string()` from `message_of` and the key and path leak.
#[test]
fn the_recorded_message_carries_no_credential_url_or_path() {
    let raw = r"error sending request to https://api.example.com/v1?api_key=SECRET123: reset reading C:\Users\alice\AppData\x";
    let message = message_of(&AppError::Network(raw.to_string()));
    for leaked in ["SECRET123", "api.example.com", "alice"] {
        assert!(!message.contains(leaked), "{leaked} leaked: {message}");
    }
}

/// A row written before this change has no kind, message or research: it still loads, as null.
#[test]
fn an_old_row_without_the_new_fields_still_reads() {
    let mut row = running_row("run-old", json!({ "calls": 2, "cached": 0 }));
    row.status = "completed".to_string();
    row.finished_at = Some(STARTED + 1);
    row.stopped_reason = Some("done".to_string());
    let wire = summary(&row);
    for key in ["errorKind", "errorMessage", "research"] {
        assert_eq!(wire[key], Value::Null, "{key}");
    }
    assert_eq!(wire["metrics"]["calls"], 2);
}

/// A run whose row `execute` wrote and the user then deleted must stay deleted when a refused
/// save reports its failure. Mutation check: build the fallback row even when no URL is given
/// (use `unwrap_or("")`) and the deleted run comes back.
#[test]
fn a_deleted_run_is_not_resurrected_by_its_failure_report() {
    let (_dir, store) = store();
    let error = AppError::Message("the document was refused".into());
    record(&store, "run-deleted", None, STARTED, &error);
    assert!(store.run("run-deleted").is_none());
}

/// An id-only request (no `jobUrl`) failing before `execute` wrote a row is keyed by the cached
/// posting's own URL; with nothing resolvable there is no key, so no row. Mutation check:
/// return `job_url` unchanged on the `Cache` arm and the first assertion gets `None`.
#[test]
fn an_id_only_failure_is_keyed_by_the_cached_postings_url() {
    let cached = |_: &str| Some("https://boards.example/jobs/9".to_string());
    assert_eq!(
        fallback_job_url("", Some(JobSource::Cache("job-1")), cached).as_deref(),
        Some("https://boards.example/jobs/9")
    );
    assert_eq!(
        fallback_job_url("", Some(JobSource::Cache("gone")), |_| None),
        None,
        "an unresolved id must not write a row under an empty key"
    );
    assert_eq!(
        fallback_job_url(
            "https://boards.example/jobs/4",
            Some(JobSource::Cache("gone")),
            |_| None
        )
        .as_deref(),
        Some("https://boards.example/jobs/4")
    );
    let text_key = fallback_job_url("", Some(JobSource::Text("a pasted ad")), |_| None)
        .expect("a pasted ad has its own partition key");
    assert!(text_key.starts_with("https://unlinked.ajh.invalid/"));
}

//! Run-store pins: round-trip, the artifact byte cap, retention, and the
//! backup-bundle contract.

use tempfile::TempDir;

use crate::data_store::DataStore;

use super::{
    clamp_artifact, PipelineRunStore, RunEventRow, RunRow, ARTIFACT_CAP_BYTES,
    ARTIFACT_TRUNCATION_MARKER, RETENTION_RUNS_PER_JOB,
};

fn store() -> (TempDir, PipelineRunStore) {
    let dir = TempDir::new().unwrap();
    let store = PipelineRunStore::open(dir.path()).unwrap();
    (dir, store)
}

fn run(id: &str, job_url: &str, started_at: u64) -> RunRow {
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

fn event(run_id: &str, seq: u32, artifact: &str) -> RunEventRow {
    RunEventRow {
        run_id: run_id.to_string(),
        seq,
        ts: 1_700_000_000_000 + u64::from(seq),
        stage: "draft".to_string(),
        phase: "finish".to_string(),
        artifact_json: artifact.to_string(),
    }
}

// ── Round-trip through SQLite ────────────────────────────────────────────────

#[test]
fn a_run_round_trips_every_field() {
    let (_dir, store) = store();
    let mut r = run("run-1", "https://example.test/job/1", 1_700_000_000_000);
    r.status = "stopped".to_string();
    r.finished_at = Some(1_700_000_050_000);
    r.stopped_reason = Some("max_repairs".to_string());
    r.metrics_json = r#"{"tokens":1234}"#.to_string();
    store.upsert_run(&r).unwrap();

    assert_eq!(store.run("run-1").as_ref(), Some(&r));
}

/// The insert is REPLACE, so the terminal update is the same call as the insert
/// — one code path, and a crashed run leaves a `running` row rather than none.
#[test]
fn upsert_replaces_rather_than_duplicating() {
    let (_dir, store) = store();
    let r = run("run-1", "job-a", 10);
    store.upsert_run(&r).unwrap();

    let mut done = r.clone();
    done.status = "done".to_string();
    done.finished_at = Some(99);
    store.upsert_run(&done).unwrap();

    let all = store.runs_for_job("job-a");
    assert_eq!(all.len(), 1, "the same id must not create a second row");
    assert_eq!(all[0].status, "done");
    assert_eq!(all[0].finished_at, Some(99));
}

#[test]
fn events_come_back_in_seq_order() {
    let (_dir, store) = store();
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    for seq in [2u32, 0, 1] {
        store.append_event(&event("run-1", seq, "{}")).unwrap();
    }
    let seqs: Vec<u32> = store
        .events_for_run("run-1")
        .into_iter()
        .map(|e| e.seq)
        .collect();
    assert_eq!(seqs, vec![0, 1, 2]);
}

#[test]
fn events_are_scoped_to_their_run() {
    let (_dir, store) = store();
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    store.upsert_run(&run("run-2", "job-a", 20)).unwrap();
    store.append_event(&event("run-1", 0, "{}")).unwrap();
    store.append_event(&event("run-2", 0, "{}")).unwrap();

    assert_eq!(store.events_for_run("run-1").len(), 1);
    assert_eq!(store.events_for_run("run-2").len(), 1);
    assert!(store.events_for_run("run-nope").is_empty());
}

/// `kind` discriminates — the same tables host résumé runs and (from Phase 3)
/// agent runs, so a query must be able to tell them apart.
#[test]
fn kind_discriminates_runs_sharing_the_tables() {
    let (_dir, store) = store();
    let mut agent = run("run-agent", "job-a", 20);
    agent.kind = "agent".to_string();
    store.upsert_run(&run("run-resume", "job-a", 10)).unwrap();
    store.upsert_run(&agent).unwrap();

    let kinds: Vec<String> = store
        .runs_for_job("job-a")
        .into_iter()
        .map(|r| r.kind)
        .collect();
    assert_eq!(kinds, vec!["agent".to_string(), "resume".to_string()]);
}

// ── The artifact byte cap ────────────────────────────────────────────────────

#[test]
fn an_artifact_at_or_below_the_cap_is_untouched() {
    let exact = "x".repeat(ARTIFACT_CAP_BYTES);
    assert_eq!(clamp_artifact(&exact), exact);
    assert_eq!(clamp_artifact("{}"), "{}");
}

#[test]
fn an_oversized_artifact_is_truncated_and_marked() {
    let clamped = clamp_artifact(&"x".repeat(ARTIFACT_CAP_BYTES + 5_000));
    assert!(clamped.ends_with(ARTIFACT_TRUNCATION_MARKER));
    assert_eq!(
        clamped.len(),
        ARTIFACT_CAP_BYTES + ARTIFACT_TRUNCATION_MARKER.len()
    );
}

/// The cap is a BYTE cap cut on a char boundary: a multi-byte artifact must not
/// panic and must not produce a value that is no longer valid UTF-8. `€` is 3
/// bytes, which does not divide the cap evenly — so the cut lands mid-character
/// and the walk-back is exercised.
#[test]
fn the_cap_cuts_on_a_utf8_boundary() {
    let multibyte = "€".repeat(ARTIFACT_CAP_BYTES); // 3× the cap in bytes
    let clamped = clamp_artifact(&multibyte);
    assert!(clamped.ends_with(ARTIFACT_TRUNCATION_MARKER));
    let body = clamped.trim_end_matches(ARTIFACT_TRUNCATION_MARKER);
    assert!(
        body.len() <= ARTIFACT_CAP_BYTES,
        "the body must not exceed the byte cap"
    );
    assert!(
        body.chars().all(|c| c == '€'),
        "no partial character survived the cut"
    );
}

/// The clamp is enforced at the WRITE site, so no caller can bypass it.
#[test]
fn append_event_clamps_at_the_write_site() {
    let (_dir, store) = store();
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    store
        .append_event(&event("run-1", 0, &"y".repeat(ARTIFACT_CAP_BYTES * 2)))
        .unwrap();

    let stored = &store.events_for_run("run-1")[0].artifact_json;
    assert!(stored.ends_with(ARTIFACT_TRUNCATION_MARKER));
    assert_eq!(
        stored.len(),
        ARTIFACT_CAP_BYTES + ARTIFACT_TRUNCATION_MARKER.len()
    );
}

// ── Retention ────────────────────────────────────────────────────────────────

#[test]
fn prune_keeps_the_newest_runs_per_job_and_drops_their_events() {
    let (_dir, store) = store();
    for i in 0..(RETENTION_RUNS_PER_JOB as u64 + 2) {
        let id = format!("run-{i}");
        store.upsert_run(&run(&id, "job-a", 1_000 + i)).unwrap();
        store.append_event(&event(&id, 0, "{}")).unwrap();
    }
    store.prune();

    let kept: Vec<String> = store
        .runs_for_job("job-a")
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(
        kept,
        vec![
            "run-4".to_string(),
            "run-3".to_string(),
            "run-2".to_string()
        ],
        "the newest {RETENTION_RUNS_PER_JOB} runs survive, newest first"
    );
    // The evicted runs' events went with them; the survivors' did not.
    assert!(store.events_for_run("run-0").is_empty());
    assert!(store.events_for_run("run-1").is_empty());
    assert_eq!(store.events_for_run("run-4").len(), 1);
}

/// Retention is PER JOB: hammering one posting must not evict another's history.
#[test]
fn prune_is_scoped_per_job_url() {
    let (_dir, store) = store();
    for i in 0..10u64 {
        store
            .upsert_run(&run(&format!("busy-{i}"), "job-busy", 1_000 + i))
            .unwrap();
    }
    store.upsert_run(&run("quiet-0", "job-quiet", 5)).unwrap();
    store.prune();

    assert_eq!(store.runs_for_job("job-busy").len(), RETENTION_RUNS_PER_JOB);
    assert_eq!(
        store.runs_for_job("job-quiet").len(),
        1,
        "another job's single run must survive a noisy neighbour"
    );
}

/// Idempotent and safe on an empty/already-pruned store.
#[test]
fn prune_is_idempotent() {
    let (_dir, store) = store();
    store.prune(); // empty
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    store.prune();
    store.prune();
    assert_eq!(store.runs_for_job("job-a").len(), 1);
}

/// An event whose run was removed by an earlier partial delete is collected too
/// — the sweep is written as "no matching run", not "the ids we just deleted".
#[test]
fn prune_collects_pre_existing_orphan_events() {
    let (_dir, store) = store();
    store.append_event(&event("ghost-run", 0, "{}")).unwrap();
    assert_eq!(store.events_for_run("ghost-run").len(), 1);
    store.prune();
    assert!(store.events_for_run("ghost-run").is_empty());
}

// ── Backup bundle (DataStore) ────────────────────────────────────────────────

#[test]
fn export_import_round_trips_runs_and_events() {
    let (_dir, source) = store();
    let mut finished = run("run-1", "job-a", 10);
    finished.status = "done".to_string();
    finished.finished_at = Some(42);
    finished.stopped_reason = Some("done".to_string());
    finished.metrics_json = r#"{"steps":3}"#.to_string();
    source.upsert_run(&finished).unwrap();
    source.upsert_run(&run("run-2", "job-b", 20)).unwrap();
    source
        .append_event(&event("run-1", 0, r#"{"issues":0}"#))
        .unwrap();
    source.append_event(&event("run-1", 1, "{}")).unwrap();

    let bundle = source.export();
    assert_eq!(source.key(), "pipelineRuns");

    let (_dir2, restored) = store();
    assert_eq!(restored.import(&bundle).unwrap(), 2);
    assert_eq!(restored.run("run-1"), source.run("run-1"));
    assert_eq!(restored.run("run-2"), source.run("run-2"));
    assert_eq!(
        restored.events_for_run("run-1"),
        source.events_for_run("run-1")
    );
}

/// Import is REPLACE, not merge — a restored bundle is the whole truth.
#[test]
fn import_replaces_existing_rows() {
    let (_dir, store) = store();
    store.upsert_run(&run("old", "job-old", 1)).unwrap();
    store.append_event(&event("old", 0, "{}")).unwrap();

    let bundle = serde_json::json!({
        "runs": [{
            "id": "new", "jobUrl": "job-new", "kind": "resume", "depth": "brief",
            "status": "done", "startedAt": 7, "metricsJson": "{}"
        }],
        "events": []
    });
    store.import(&bundle).unwrap();

    assert!(store.run("old").is_none());
    assert!(store.events_for_run("old").is_empty());
    assert_eq!(store.run("new").unwrap().job_url, "job-new");
}

/// Deserialize-all-before-mutate: a malformed row aborts the whole import with
/// the pre-existing data intact.
#[test]
fn a_malformed_row_fails_the_import_and_preserves_existing_data() {
    let (_dir, store) = store();
    store.upsert_run(&run("keep", "job-a", 1)).unwrap();

    let bundle = serde_json::json!({
        "runs": [
            { "id": "ok", "jobUrl": "j", "kind": "resume", "depth": "full",
              "status": "done", "startedAt": 1, "metricsJson": "{}" },
            // `startedAt` missing → the whole import must abort.
            { "id": "bad", "jobUrl": "j", "kind": "resume", "depth": "full",
              "status": "done", "metricsJson": "{}" }
        ],
        "events": []
    });
    assert!(store.import(&bundle).is_err());
    assert!(
        store.run("keep").is_some(),
        "existing rows survive a failed import"
    );
    assert!(store.run("ok").is_none(), "no partial insert");
}

/// A hand-edited bundle must not be able to write past the cap the live write
/// path enforces.
#[test]
fn import_re_clamps_an_oversized_artifact() {
    let (_dir, store) = store();
    let bundle = serde_json::json!({
        "runs": [],
        "events": [{
            "runId": "r", "seq": 0, "ts": 1, "stage": "s", "phase": "finish",
            "artifactJson": "z".repeat(ARTIFACT_CAP_BYTES * 3)
        }]
    });
    store.import(&bundle).unwrap();
    assert_eq!(
        store.events_for_run("r")[0].artifact_json.len(),
        ARTIFACT_CAP_BYTES + ARTIFACT_TRUNCATION_MARKER.len()
    );
}

/// An older bundle that predates a later-added optional field still restores
/// (`#[serde(default)]` on the optionals + `metricsJson`).
#[test]
fn a_minimal_legacy_bundle_still_imports() {
    let (_dir, store) = store();
    let bundle = serde_json::json!({
        "runs": [{
            "id": "r", "jobUrl": "j", "kind": "resume", "depth": "full",
            "status": "done", "startedAt": 1
        }]
    });
    assert_eq!(store.import(&bundle).unwrap(), 1);
    let restored = store.run("r").unwrap();
    assert_eq!(restored.metrics_json, "{}");
    assert_eq!(restored.finished_at, None);
    assert_eq!(restored.stopped_reason, None);
}

// ── Factory reset + migrations ───────────────────────────────────────────────

#[test]
fn clear_all_empties_both_tables() {
    let (_dir, store) = store();
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    store.append_event(&event("run-1", 0, "{}")).unwrap();
    store.clear_all();
    assert!(store.runs_for_job("job-a").is_empty());
    assert!(store.events_for_run("run-1").is_empty());
}

#[test]
fn reopening_the_same_db_is_migration_idempotent() {
    let dir = TempDir::new().unwrap();
    {
        let store = PipelineRunStore::open(dir.path()).unwrap();
        store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    }
    let store = PipelineRunStore::open(dir.path()).unwrap();
    assert!(store.run("run-1").is_some(), "data survives a reopen");
}

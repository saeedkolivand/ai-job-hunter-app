//! Round-trip through SQLite, and the two free-form JSON columns' byte caps
//! (`artifact_json`/`metrics_json`).

use crate::data_store::DataStore;
use crate::pipeline::runs::{
    clamp_artifact, clamp_metrics, ARTIFACT_CAP_BYTES, METRICS_CAP_BYTES, TRUNCATION_MARKER,
};

use super::support::{event, run, store};

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

/// The cap is INCLUSIVE of the marker: a clamped value must never be longer
/// than the cap it was clamped to, or the cap does not mean what it says.
#[test]
fn an_oversized_artifact_is_truncated_and_marked() {
    let clamped = clamp_artifact(&"x".repeat(ARTIFACT_CAP_BYTES + 5_000));
    assert!(clamped.ends_with(TRUNCATION_MARKER));
    assert_eq!(
        clamped.len(),
        ARTIFACT_CAP_BYTES,
        "the marker is reserved inside the cap, never added on top of it"
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
    assert!(clamped.ends_with(TRUNCATION_MARKER));
    let body = clamped.trim_end_matches(TRUNCATION_MARKER);
    assert!(
        clamped.len() <= ARTIFACT_CAP_BYTES,
        "the clamped value, marker included, must not exceed the byte cap"
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
    assert!(stored.ends_with(TRUNCATION_MARKER));
    assert_eq!(stored.len(), ARTIFACT_CAP_BYTES);
}

// ── The metrics byte cap (the run-level twin of the artifact cap) ────────────

#[test]
fn metrics_at_or_below_the_cap_are_untouched() {
    let exact = "x".repeat(METRICS_CAP_BYTES);
    assert_eq!(clamp_metrics(&exact), exact);
    assert_eq!(clamp_metrics(r#"{"tokens":12}"#), r#"{"tokens":12}"#);
}

/// `metrics_json` is the OTHER free-form JSON column, and it is capped at its
/// own write site — a stage that hands the recorder its whole model output must
/// not be able to write a multi-megabyte run row.
#[test]
fn upsert_run_clamps_metrics_at_the_write_site() {
    let (_dir, store) = store();
    let mut r = run("run-1", "job-a", 10);
    r.metrics_json = "m".repeat(METRICS_CAP_BYTES * 4);
    store.upsert_run(&r).unwrap();

    let stored = store.run("run-1").unwrap().metrics_json;
    assert!(stored.ends_with(TRUNCATION_MARKER));
    assert_eq!(stored.len(), METRICS_CAP_BYTES);
}

/// The import twin of `import_re_clamps_an_oversized_artifact`: a hand-edited
/// backup must not be able to restore a metrics blob past the cap the live path
/// enforces — otherwise the oversized row is permanent.
#[test]
fn import_re_clamps_oversized_metrics() {
    let (_dir, store) = store();
    let bundle = serde_json::json!({
        "runs": [{
            "id": "r", "jobUrl": "j", "kind": "resume", "depth": "full",
            "status": "done", "startedAt": 1,
            "metricsJson": "m".repeat(METRICS_CAP_BYTES * 4)
        }],
        "events": []
    });
    store.import(&bundle).unwrap();

    let stored = store.run("r").unwrap().metrics_json;
    assert!(stored.ends_with(TRUNCATION_MARKER));
    assert_eq!(stored.len(), METRICS_CAP_BYTES);
}

/// The truncation marker is deliberately NOT valid JSON: a truncated value must
/// FAIL a reader, never half-parse into a value that reads as complete.
#[test]
fn a_truncated_value_cannot_be_parsed_as_json() {
    let clamped = clamp_metrics(&format!(
        r#"{{"tokens":{}}}"#,
        "9".repeat(METRICS_CAP_BYTES)
    ));
    assert!(
        serde_json::from_str::<serde_json::Value>(&clamped).is_err(),
        "a truncated metrics blob must not parse"
    );
}

//! The backup bundle (`DataStore::export`/`import`), and import hardening:
//! identity/text columns are REJECTED past their cap (never truncated), and
//! the bundle's own row counts are bounded.

use crate::data_store::DataStore;
use crate::pipeline::runs::import_export::check_bundle_size;
use crate::pipeline::runs::{
    ARTIFACT_CAP_BYTES, IMPORT_ID_CAP_BYTES, IMPORT_JOB_URL_CAP_BYTES, IMPORT_LABEL_CAP_BYTES,
    IMPORT_MAX_EVENTS, IMPORT_MAX_RUNS,
};

use super::support::{event, run, store};

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
        ARTIFACT_CAP_BYTES
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

// ── Import hardening: identity/text caps + row caps ──────────────────────────

/// One valid run + its event, as the JSON a bundle carries. The rejection tests
/// oversize exactly ONE column of this, so each case differs from a passing
/// import by that column alone.
fn a_valid_bundle() -> serde_json::Value {
    serde_json::json!({
        "runs": [{
            "id": "r", "jobUrl": "https://example.test/job/1", "kind": "resume",
            "depth": "full", "status": "done", "startedAt": 1,
            "stoppedReason": "done", "metricsJson": "{}"
        }],
        "events": [{
            "runId": "r", "seq": 0, "ts": 1, "stage": "draft",
            "phase": "finish", "artifactJson": "{}"
        }]
    })
}

/// Identity columns are REJECTED past their cap, never clamped: truncating an
/// `id`/`job_url`/`run_id` does not shorten the value, it changes what the row
/// points at (a different run, a different posting, an orphaned event).
///
/// One case per guarded column, so deleting any single `check_len` call fails
/// here rather than leaving one column quietly unbounded — the shape this store
/// already got wrong once by capping only the two JSON blobs.
#[test]
fn import_rejects_an_oversized_identity_column_and_preserves_existing_data() {
    let cases: &[(&str, &str, usize)] = &[
        ("runs", "id", IMPORT_ID_CAP_BYTES),
        ("runs", "jobUrl", IMPORT_JOB_URL_CAP_BYTES),
        ("runs", "kind", IMPORT_LABEL_CAP_BYTES),
        ("runs", "depth", IMPORT_LABEL_CAP_BYTES),
        ("runs", "status", IMPORT_LABEL_CAP_BYTES),
        ("runs", "stoppedReason", IMPORT_LABEL_CAP_BYTES),
        ("events", "runId", IMPORT_ID_CAP_BYTES),
        ("events", "stage", IMPORT_LABEL_CAP_BYTES),
    ];
    for (section, column, cap) in cases {
        let (_dir, store) = store();
        store.upsert_run(&run("keep", "job-a", 1)).unwrap();

        let oversized = "x".repeat(cap + 1);
        let mut bundle = a_valid_bundle();
        bundle[*section][0][*column] = serde_json::json!(oversized);

        let err = match store.import(&bundle) {
            Ok(n) => panic!("{section}.{column} past its cap must fail the import, imported {n}"),
            Err(e) => e.to_string(),
        };
        assert!(
            err.contains(*column),
            "the error must name the offending column ({column}), got: {err}"
        );
        assert!(
            !err.contains(&"x".repeat(64)),
            "the error must not echo the oversized value — this store is content-free"
        );
        assert!(
            store.run("keep").is_some(),
            "{section}.{column}: existing history must survive a rejected bundle"
        );
        assert!(
            store.run("r").is_none(),
            "{section}.{column}: nothing from the rejected bundle may land"
        );
    }
}

/// The boundary: a value EXACTLY at its cap is legal. A cap that rejects the
/// value it names is an off-by-one, not a cap.
#[test]
fn import_accepts_identity_columns_exactly_at_their_caps() {
    let (_dir, store) = store();
    let id = "i".repeat(IMPORT_ID_CAP_BYTES);
    let mut bundle = a_valid_bundle();
    bundle["runs"][0]["id"] = serde_json::json!(id);
    bundle["runs"][0]["jobUrl"] = serde_json::json!("u".repeat(IMPORT_JOB_URL_CAP_BYTES));
    bundle["runs"][0]["kind"] = serde_json::json!("k".repeat(IMPORT_LABEL_CAP_BYTES));
    bundle["runs"][0]["depth"] = serde_json::json!("d".repeat(IMPORT_LABEL_CAP_BYTES));
    bundle["runs"][0]["status"] = serde_json::json!("s".repeat(IMPORT_LABEL_CAP_BYTES));
    bundle["runs"][0]["stoppedReason"] = serde_json::json!("p".repeat(IMPORT_LABEL_CAP_BYTES));
    bundle["events"][0]["runId"] = serde_json::json!(id);
    bundle["events"][0]["stage"] = serde_json::json!("g".repeat(IMPORT_LABEL_CAP_BYTES));

    assert_eq!(store.import(&bundle).unwrap(), 1);
    assert!(store.run(&id).is_some(), "an at-cap id must restore");
    assert_eq!(store.events_for_run(&id).len(), 1);
}

/// The row caps are a PURE decision, so their boundary is testable without
/// building a 50 000-row fixture: at the cap is legal, one past it is not.
#[test]
fn the_row_caps_admit_the_cap_and_reject_one_more() {
    check_bundle_size(IMPORT_MAX_RUNS, IMPORT_MAX_EVENTS)
        .expect("a bundle exactly at both caps must be accepted");

    let err = check_bundle_size(IMPORT_MAX_RUNS + 1, 0)
        .expect_err("one run past the cap must be rejected")
        .to_string();
    assert!(err.contains("runs"), "got: {err}");

    let err = check_bundle_size(0, IMPORT_MAX_EVENTS + 1)
        .expect_err("one event past the cap must be rejected")
        .to_string();
    assert!(err.contains("events"), "got: {err}");
}

/// …and the row cap is WIRED into `import`, refusing the bundle before any row
/// lands rather than after inserting `IMPORT_MAX_RUNS` of them.
#[test]
fn import_rejects_a_bundle_over_the_row_cap_and_preserves_existing_data() {
    let (_dir, store) = store();
    store.upsert_run(&run("keep", "job-a", 1)).unwrap();

    let runs: Vec<serde_json::Value> = (0..=IMPORT_MAX_RUNS)
        .map(|i| {
            serde_json::json!({
                "id": format!("bulk-{i}"), "jobUrl": "j", "kind": "resume",
                "depth": "full", "status": "done", "startedAt": 1, "metricsJson": "{}"
            })
        })
        .collect();
    let err = store
        .import(&serde_json::json!({ "runs": runs, "events": [] }))
        .expect_err("a bundle past the run cap must be rejected")
        .to_string();

    assert!(err.contains("runs"), "got: {err}");
    assert!(
        store.run("keep").is_some(),
        "existing history must survive a bundle refused by the row cap"
    );
    assert!(store.run("bulk-0").is_none(), "no partial insert");
}

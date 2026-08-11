//! Run-store pins: round-trip, the artifact byte cap, the closed `phase`
//! vocabulary, retention, and the backup-bundle contract.

use std::collections::BTreeSet;

use tempfile::TempDir;

use crate::data_store::DataStore;
use crate::ipc_contracts::events::PIPELINE_STAGE_PHASES;

use super::{
    check_bundle_size, clamp_artifact, clamp_metrics, PipelineRunStore, RunEventRow, RunRow,
    ARTIFACT_CAP_BYTES, CREATE_PIPELINE_RUNS_SQL, IMPORT_ID_CAP_BYTES, IMPORT_JOB_URL_CAP_BYTES,
    IMPORT_LABEL_CAP_BYTES, IMPORT_MAX_EVENTS, IMPORT_MAX_RUNS, METRICS_CAP_BYTES,
    RETENTION_RUNS_PER_JOB, TRUNCATION_MARKER,
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

// ── The closed `phase` vocabulary ────────────────────────────────────────────

/// The literals inside the events table's `CHECK (phase IN (...))`, as a set.
///
/// Parses the migration's STATIC SQL — the same string the migration executes,
/// so the two cannot diverge. A reformat that breaks this parse fails loudly
/// (the `expect` below) rather than passing vacuously.
fn phases_in_the_schema_check() -> BTreeSet<String> {
    const NEEDLE: &str = "CHECK (phase IN (";
    let after = CREATE_PIPELINE_RUNS_SQL
        .find(NEEDLE)
        .map(|i| &CREATE_PIPELINE_RUNS_SQL[i + NEEDLE.len()..])
        .expect("the events table must close `phase` with a `CHECK (phase IN (...))` clause");
    let body = &after[..after.find(')').expect("unterminated CHECK clause")];
    body.split(',')
        .map(|value| value.trim().trim_matches('\'').to_string())
        .collect()
}

/// DRIFT GUARD. `phase` is closed BY DESIGN (unlike `stopped_reason`, which is
/// deliberately loose TEXT so a new variant needs no migration), and its
/// vocabulary is frozen in TS — `PIPELINE_STAGE_PHASES` in
/// `packages/shared/src/events/pipeline.ts`, which `pnpm gen:ipc` emits into
/// `ipc_contracts::events` and CI re-checks. So: widening the TS array widens
/// the generated const, and this assertion then FAILS until someone appends the
/// migration that widens the CHECK on already-migrated installs too. Widening
/// the CHECK alone (a schema that accepts a phase the contract does not know)
/// fails here as well — the comparison is a set equality, not a subset.
#[test]
fn phase_check_matches_the_generated_contract() {
    let schema = phases_in_the_schema_check();
    let contract: BTreeSet<String> = PIPELINE_STAGE_PHASES
        .iter()
        .map(|phase| (*phase).to_string())
        .collect();
    assert!(
        !contract.is_empty(),
        "an empty contract vocabulary would make this comparison vacuous"
    );
    assert_eq!(
        schema, contract,
        "the `phase` CHECK and PIPELINE_STAGE_PHASES have drifted; a phase added \
         to the TS contract needs an APPENDED migration widening the CHECK"
    );
}

/// The CHECK is real at the write site, and it accepts exactly the contract's
/// values — a typo'd CHECK that dropped `start` would still reject the bogus
/// phase below, so the accepted half is pinned too.
#[test]
fn the_schema_accepts_every_contract_phase() {
    let (_dir, store) = store();
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    for (seq, phase) in PIPELINE_STAGE_PHASES.iter().enumerate() {
        let mut e = event("run-1", seq as u32, "{}");
        e.phase = (*phase).to_string();
        store
            .append_event(&e)
            .unwrap_or_else(|err| panic!("the schema must accept contract phase '{phase}': {err}"));
    }
    assert_eq!(
        store.events_for_run("run-1").len(),
        PIPELINE_STAGE_PHASES.len()
    );
}

/// …and rejects anything else, at the SCHEMA rather than in the caller: the
/// write path takes `phase: String`, so the table is the only place that can
/// stop a stage emitter (or a future one) from inventing a fourth phase.
#[test]
fn the_schema_rejects_an_out_of_vocabulary_phase() {
    let (_dir, store) = store();
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    let mut bogus = event("run-1", 0, "{}");
    bogus.phase = "cancelled".to_string();

    let err = store.append_event(&bogus).unwrap_err();
    assert!(
        err.to_string().to_uppercase().contains("CHECK"),
        "an unknown phase must violate the schema CHECK, got: {err}"
    );
    assert!(
        store.events_for_run("run-1").is_empty(),
        "the rejected event must not have landed"
    );
}

/// The import path enforces it too — that is the WHOLE point of putting the
/// vocabulary in the schema rather than in the emitter. A hand-edited bundle
/// with a bogus phase aborts the whole import (the transaction is dropped
/// without committing) and the pre-existing rows survive, exactly like the
/// deserialize-time abort above.
#[test]
fn import_rejects_an_out_of_vocabulary_phase_and_preserves_existing_data() {
    let (_dir, store) = store();
    store.upsert_run(&run("keep", "job-a", 1)).unwrap();

    let bundle = serde_json::json!({
        "runs": [{
            "id": "r", "jobUrl": "j", "kind": "resume", "depth": "full",
            "status": "done", "startedAt": 1, "metricsJson": "{}"
        }],
        "events": [{
            "runId": "r", "seq": 0, "ts": 1, "stage": "draft",
            "phase": "cancelled", "artifactJson": "{}"
        }]
    });
    assert!(store.import(&bundle).is_err());
    assert!(
        store.run("keep").is_some(),
        "existing rows survive an import rejected by the schema"
    );
    assert!(
        store.run("r").is_none(),
        "the run inserted before the bad event must be rolled back, not committed"
    );
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

/// Retention is per `(job_url, kind)`, not per `job_url` alone: `kind` is the
/// discriminator that lets these tables host every staged run, so three résumé
/// runs must not evict the same posting's agent-run history.
#[test]
fn prune_is_scoped_per_kind_within_a_job() {
    let (_dir, store) = store();
    for i in 0..(RETENTION_RUNS_PER_JOB as u64 + 2) {
        store
            .upsert_run(&run(&format!("resume-{i}"), "job-a", 1_000 + i))
            .unwrap();
    }
    for i in 0..2u64 {
        let mut agent = run(&format!("agent-{i}"), "job-a", 10 + i);
        agent.kind = "agent".to_string();
        store.upsert_run(&agent).unwrap();
    }
    store.prune();

    let surviving = |kind: &str| -> usize {
        store
            .runs_for_job("job-a")
            .into_iter()
            .filter(|r| r.kind == kind)
            .count()
    };
    assert_eq!(
        surviving("resume"),
        RETENTION_RUNS_PER_JOB,
        "the noisy kind is still capped at its own retention"
    );
    assert_eq!(
        surviving("agent"),
        2,
        "the other kind's history must survive a noisy neighbour of a different kind"
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

/// The transaction is real: if the orphan sweep fails AFTER the eviction
/// succeeded, prune must return without committing so the drop rolls both back.
/// Injection is a dropped `pipeline_run_events` table — the cheapest way to make
/// the SECOND statement fail while the first still succeeds.
#[test]
fn a_failed_sweep_rolls_back_the_eviction() {
    let (_dir, store) = store();
    let total = RETENTION_RUNS_PER_JOB as u64 + 2;
    for i in 0..total {
        store
            .upsert_run(&run(&format!("run-{i}"), "job-a", 1_000 + i))
            .unwrap();
    }
    store
        .conn
        .lock()
        .execute_batch("DROP TABLE pipeline_run_events")
        .unwrap();

    store.prune();

    assert_eq!(
        store.runs_for_job("job-a").len(),
        total as usize,
        "a failed sweep must leave history intact — the eviction is rolled back, not committed"
    );
}

/// The FIRST error arm: when the eviction itself fails, prune must stop there —
/// not run the sweep and not commit. Injection is a `BEFORE DELETE` trigger that
/// aborts, with an orphan event present that the sweep WOULD have collected: if
/// the sweep still ran and the commit still happened, that orphan disappears
/// while the log claims history was left intact.
#[test]
fn a_failed_eviction_runs_no_sweep_and_commits_nothing() {
    let (_dir, store) = store();
    for i in 0..(RETENTION_RUNS_PER_JOB as u64 + 2) {
        store
            .upsert_run(&run(&format!("run-{i}"), "job-a", 1_000 + i))
            .unwrap();
    }
    store.append_event(&event("ghost", 0, "{}")).unwrap();
    store
        .conn
        .lock()
        .execute_batch(
            "CREATE TRIGGER no_run_deletes BEFORE DELETE ON pipeline_runs
             BEGIN SELECT RAISE(ABORT, 'blocked'); END;",
        )
        .unwrap();

    store.prune();

    assert_eq!(
        store.events_for_run("ghost").len(),
        1,
        "the sweep must not run after a failed eviction, and nothing may commit"
    );
}

/// SQLite permits NULL in a TEXT PRIMARY KEY, and one null-id run would disable
/// the orphan sweep forever. The schema forbids it outright.
#[test]
fn a_null_run_id_is_rejected_by_the_schema() {
    let (_dir, store) = store();
    let err = store
        .conn
        .lock()
        .execute(
            "INSERT INTO pipeline_runs
                (id, job_url, kind, depth, status, started_at, metrics_json)
             VALUES (NULL, 'j', 'resume', 'full', 'done', 1, '{}')",
            [],
        )
        .unwrap_err();
    assert!(
        err.to_string().to_uppercase().contains("NOT NULL"),
        "a NULL id must violate the schema, got: {err}"
    );
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

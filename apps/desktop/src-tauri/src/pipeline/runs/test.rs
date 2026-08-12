//! Run-store pins: round-trip, the artifact byte cap, the closed `phase`
//! vocabulary, retention, and the backup-bundle contract.

use std::collections::BTreeSet;

use tempfile::TempDir;

use crate::data_store::DataStore;
use crate::ipc_contracts::events::PIPELINE_STAGE_PHASES;
use serde_json::json;

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

/// **Deleting an application takes its run trail with it.**
///
/// The trail is not a cache: at max depth a run persists the FULL re-seeded
/// strategy (the whole employment history) and the full evidence map (verbatim
/// quotes out of the résumé) in `artifact_json`, and nothing else ever removes
/// them for a posting the user deleted — `prune` only evicts the fourth run of
/// a posting still being run, and `export` ships every event row into the
/// user's backups.
///
/// The match is on the NORMALIZED url on BOTH sides, because an `Application`
/// is keyed by `normalize_job_url` while a run row carries the postings cache's
/// raw url. A `WHERE job_url = ?` would look right and delete nothing for every
/// posting whose link carries a tracking param.
///
/// Mutation check: compare the raw `job_url` strings instead of the normalized
/// ones and the `utm_source` run survives; drop the events DELETE and the
/// orphan assertion fails.
#[test]
fn deleting_a_job_removes_its_runs_and_their_artifacts() {
    let (_dir, store) = store();
    // Two runs of the SAME posting, written with the raw urls the postings
    // cache hands the pipeline — one of them carrying tracking noise.
    store
        .upsert_run(&run("run-a", "https://boards.example/jobs/42", 1_000))
        .unwrap();
    store
        .upsert_run(&run(
            "run-b",
            "https://www.Boards.example/jobs/42?utm_source=newsletter#apply",
            2_000,
        ))
        .unwrap();
    store
        .append_event(&event("run-a", 0, r#"{"full":{"perCompany":[]}}"#))
        .unwrap();
    store
        .append_event(&event("run-b", 0, r#"{"full":{"items":[]}}"#))
        .unwrap();
    // …and one run of a DIFFERENT posting, which must survive untouched.
    store
        .upsert_run(&run("other", "https://boards.example/jobs/99", 3_000))
        .unwrap();
    store.append_event(&event("other", 0, "{}")).unwrap();

    let removed = store.delete_for_job("https://boards.example/jobs/42");

    assert_eq!(removed, 2, "both spellings of the same posting go");
    assert!(store
        .runs_for_job("https://boards.example/jobs/42")
        .is_empty());
    assert!(
        store.events_for_run("run-a").is_empty() && store.events_for_run("run-b").is_empty(),
        "the artifacts are the point — a deleted run whose events survive keeps the résumé quotes"
    );
    assert_eq!(
        store.runs_for_job("https://boards.example/jobs/99").len(),
        1
    );
    assert_eq!(store.events_for_run("other").len(), 1);

    // An unlinked application has no posting url; matching it against every
    // empty-url run would delete other people's history rather than nothing.
    store.upsert_run(&run("unlinked", "", 4_000)).unwrap();
    assert_eq!(store.delete_for_job(""), 0);
    assert_eq!(store.delete_for_job("   "), 0);
    assert!(store.run("unlinked").is_some(), "…and it survives");
}

/// The same empty-key guard on the READ side.
///
/// `normalized_job_url` maps anything it cannot read as an http(s) url to `""`
/// — a `javascript:` scheme, a control-character paste, whitespace — and `""`
/// is also what every UNLINKED run is stored under. `resume_pipeline_list_for_job`
/// takes its url from the renderer, so without the guard a junk url would list
/// every unlinked run in the store: other postings' history, under a url that
/// names none of them.
///
/// Mutation check: drop the `wanted.is_empty()` early return in `runs_for_job`
/// and every case below returns the unlinked runs.
#[test]
fn a_junk_url_lists_no_runs_rather_than_every_unlinked_one() {
    let (_dir, store) = store();
    store.upsert_run(&run("unlinked-1", "", 1_000)).unwrap();
    store.upsert_run(&run("unlinked-2", "", 2_000)).unwrap();
    store
        .upsert_run(&run("linked", "https://boards.example/jobs/1", 3_000))
        .unwrap();

    for junk in [
        "",
        "   ",
        "javascript:alert(1)",
        "data:text/html,x",
        "\u{1}\u{2}",
    ] {
        assert!(
            store.runs_for_job(junk).is_empty(),
            "runs_for_job({junk:?}) must not answer with someone else's history"
        );
    }
    // …and a real url still resolves, so the guard is about the empty key.
    assert_eq!(store.runs_for_job("https://boards.example/jobs/1").len(), 1);
}

/// **A selection larger than SQLite's host-parameter limit still cascades.**
///
/// `delete_for_jobs` builds one `IN (?, ?, …)` per posting. SQLite refuses to
/// prepare past `SQLITE_MAX_VARIABLE_NUMBER` (32 766 bundled, 999 on older
/// builds), and the failure is silent in the direction that matters: the delete
/// returns 0, the run-trail purge does not happen, and the artifacts it was
/// meant to remove stay on disk. The user's Documents-page selection is
/// unbounded, so the statement has to be chunked.
///
/// **What this test can and cannot reach, measured rather than assumed.** The
/// batch below crosses four chunk boundaries, so it pins that chunking deletes
/// everything and stays atomic — but it does NOT reach SQLite's own limit, and
/// running the unchunked mutation against it PASSES. Seeding 32 766+ postings
/// to make that mutation fail costs minutes of suite time for a bound the
/// assertion below states directly and for free.
///
/// Mutation check: raise `MAX_SQL_PARAMS` above the connection's reported
/// `SQLITE_LIMIT_VARIABLE_NUMBER` and the first assertion fails; drop the
/// chunking and the count/atomicity assertions still hold at this size (which
/// is why the limit assertion is here at all).
#[test]
fn a_selection_past_the_sql_parameter_limit_still_purges_every_trail() {
    let (_dir, store) = store();
    // Comfortably past both the 999 and the 32 766 limits' chunk boundary, and
    // past MAX_SQL_PARAMS several times over.
    let count = crate::db::MAX_SQL_PARAMS * 4 + 7;
    let urls: Vec<String> = (0..count)
        .map(|i| format!("https://boards.example/jobs/{i}"))
        .collect();
    for (i, url) in urls.iter().enumerate() {
        let id = format!("run-{i}");
        store.upsert_run(&run(&id, url, 1_000 + i as u64)).unwrap();
        store
            .append_event(&event(&id, 0, r#"{"full":{"perCompany":[]}}"#))
            .unwrap();
    }
    // …plus one posting nobody selected.
    store
        .upsert_run(&run("keep", "https://boards.example/keep", 9_000))
        .unwrap();
    store.append_event(&event("keep", 0, "{}")).unwrap();

    // The bound the chunking exists for. SQLite refuses to PREPARE a statement
    // with more host parameters than `SQLITE_MAX_VARIABLE_NUMBER` — 32 766 on
    // the bundled build, but 999 on anything older, and rusqlite's runtime
    // accessor for it sits behind a feature this crate does not enable. The
    // conservative floor is the one worth pinning: a chunk size safe there is
    // safe everywhere, and this fails the moment someone raises the constant
    // past it.
    const SQLITE_OLDEST_VARIABLE_LIMIT: usize = 999;
    // A const block: both operands are compile-time constants, and clippy is
    // right that a runtime `assert!` on two of them proves nothing at test time
    // that it would not prove at build time.
    const _: () = assert!(
        crate::db::MAX_SQL_PARAMS < SQLITE_OLDEST_VARIABLE_LIMIT,
        "MAX_SQL_PARAMS must stay under the oldest SQLite host-parameter limit"
    );
    assert!(
        urls.len() > SQLITE_OLDEST_VARIABLE_LIMIT,
        "the premise: this selection would blow that limit as one statement"
    );

    let removed = store.delete_for_jobs(&urls);

    assert_eq!(
        removed, count,
        "every selected posting's runs go in one call"
    );
    assert!(
        (0..count).all(|i| store.events_for_run(&format!("run-{i}")).is_empty()),
        "no event row may survive — they are the artifacts the purge exists for"
    );
    assert_eq!(store.runs_for_job("https://boards.example/keep").len(), 1);
    assert_eq!(store.events_for_run("keep").len(), 1);
}

/// **What the list shows and what the delete removes are the SAME set.**
///
/// They were not. `execute` wrote the postings cache's RAW url while
/// `delete_for_job` normalized before comparing, so a delete correctly took the
/// trail of a posting whose link carried a `utm_*` param — and `runs_for_job`,
/// called with the application's own normalized key, could not find that run to
/// list it. A store whose delete and list disagree about which rows belong to a
/// posting reports one thing and does another.
///
/// The seam is `normalized_job_url`, applied where a url ENTERS (the write) and
/// where one arrives (the by-url readers), so every spelling of a posting
/// resolves to one set of rows.
///
/// Mutation check: store `run.job_url` raw in `upsert_run` and the
/// normalized-lookup assertion fails; drop the normalization in `runs_for_job`
/// and the RAW-lookup one does (that is the arm the review's "keep readers
/// exact-match" prescription would have broken — the renderer passes
/// `posting.url`, not the normalized key).
#[test]
fn every_spelling_of_a_posting_resolves_to_the_same_runs() {
    let (_dir, store) = store();
    const RAW: &str = "https://www.Boards.example/jobs/42?utm_source=newsletter#apply";
    const NORMALIZED: &str = "https://boards.example/jobs/42";

    store.upsert_run(&run("run-1", RAW, 1_000)).unwrap();
    store.append_event(&event("run-1", 0, "{}")).unwrap();

    // The row is STORED in one spelling…
    assert_eq!(
        store.run("run-1").expect("the run").job_url,
        NORMALIZED,
        "the write site is the seam"
    );
    // …and every spelling finds it, including the raw one the renderer holds.
    for spelling in [RAW, NORMALIZED, "https://Boards.example/jobs/42/"] {
        assert_eq!(
            store.runs_for_job(spelling).len(),
            1,
            "runs_for_job({spelling}) must resolve"
        );
    }
    // The list and the delete agree: what one showed, the other removes.
    assert_eq!(store.runs_for_job(NORMALIZED).len(), 1);
    assert_eq!(store.delete_for_job(RAW), 1);
    assert!(store.runs_for_job(NORMALIZED).is_empty());
    assert!(store.events_for_run("run-1").is_empty());
}

/// **A RESTORED bundle is a write, and it goes through the same seam.**
///
/// `import` is the restore path, and it bound `run.job_url` raw while
/// `upsert_run` normalized — so restoring any backup taken before the
/// normalization landed (they all carry the postings cache's raw urls:
/// `utm_*`, fragments, `www.`, uppercase host) wrote rows in a spelling no
/// reader and no delete could reach. `data_import` runs against the LIVE
/// managed store with no re-open, so `normalize_existing_job_urls` does not get
/// a chance to repair them either.
///
/// What that cost, end to end: the runs panel came back empty for those
/// postings, `delete_for_job` matched zero rows and still reported SUCCESS, so
/// both delete cascades silently no-opped — and the next restart's sweep then
/// normalized rows whose owner the user had already deleted, leaving the full
/// strategy and evidence map (employment history, verbatim résumé quotes) on
/// disk permanently and riding into every later backup.
///
/// The two round-trip guards above could not catch it: their fixtures are
/// pre-normalized on both sides, so a raw-binding import looks identical.
///
/// Mutation check: bind `run.job_url` raw in `import` and both assertions fail.
#[test]
fn an_imported_run_is_normalized_like_any_other_write() {
    let (_dir, store) = store();
    const RAW: &str = "https://www.Boards.example/jobs/7?utm_campaign=x";
    const NORMALIZED: &str = "https://boards.example/jobs/7";

    let bundle = json!({
        "runs": [{
            "id": "restored",
            "jobUrl": RAW,
            "kind": "resume",
            "depth": "max",
            "status": "completed",
            "startedAt": 1_700_000_000_000i64,
            "finishedAt": 1_700_000_100_000i64,
            "stoppedReason": "done",
            "metricsJson": "{}",
        }],
        "events": [{
            "runId": "restored",
            "seq": 0,
            "ts": 1_700_000_000_000i64,
            "stage": "strategy",
            "phase": "finish",
            "artifactJson": r#"{"full":{"perCompany":[{"company":"Acme"}]}}"#,
        }],
    });
    assert_eq!(store.import(&bundle).expect("the bundle restores"), 1);

    assert_eq!(
        store.run("restored").expect("the run").job_url,
        NORMALIZED,
        "a restore is a write, and the write site is the seam"
    );
    assert_eq!(
        store.runs_for_job(NORMALIZED).len(),
        1,
        "a restored run must be listable"
    );
    assert_eq!(
        store.delete_for_job(NORMALIZED),
        1,
        "…and deletable — a delete that matches nothing still reports success, \
         so an unreachable row is PII with no owner and no eviction"
    );
    assert!(store.events_for_run("restored").is_empty());
}

/// A row written by a BUILD THAT PREDATES the normalization is repaired once, at
/// open — otherwise it stays invisible to every reader that now normalizes, and
/// its artifacts stay undeleteable.
///
/// Written straight through `rusqlite` rather than through `upsert_run`,
/// because `upsert_run` is exactly the thing that would normalize it and there
/// would be nothing to repair.
///
/// Mutation check: delete the `normalize_existing_job_urls` call from `open`
/// and the legacy row is unreachable after the reopen.
#[test]
fn a_legacy_row_written_before_the_normalization_is_repaired_at_open() {
    let dir = TempDir::new().unwrap();
    const RAW: &str = "https://www.Boards.example/jobs/7?utm_campaign=x";
    const NORMALIZED: &str = "https://boards.example/jobs/7";
    {
        let store = PipelineRunStore::open(dir.path()).unwrap();
        store
            .upsert_run(&run("legacy", "placeholder", 1_000))
            .unwrap();
        store.append_event(&event("legacy", 0, "{}")).unwrap();
        // Put the row back the way an older build wrote it.
        store
            .conn
            .lock()
            .execute(
                "UPDATE pipeline_runs SET job_url = ?1 WHERE id = ?2",
                rusqlite::params![RAW, "legacy"],
            )
            .unwrap();
        assert_eq!(store.run("legacy").expect("the run").job_url, RAW);
    }

    let store = PipelineRunStore::open(dir.path()).unwrap();
    assert_eq!(
        store.run("legacy").expect("the run").job_url,
        NORMALIZED,
        "the sweep rewrites a stale spelling once, at open"
    );
    assert_eq!(store.runs_for_job(NORMALIZED).len(), 1);
    assert_eq!(store.delete_for_job(NORMALIZED), 1);
    assert!(store.events_for_run("legacy").is_empty());
}

/// The BATCH cascade behind `ai_generations_remove_bulk` — the Documents page's
/// multi-select delete.
///
/// One posting's trail going is [`PipelineRunStore::delete_for_job`]'s job;
/// this pins that several go together and that a posting nobody deleted keeps
/// its history, which is the property a loop written at three call sites would
/// eventually get wrong at one of them.
///
/// Mutation check: sum nothing (return 0) and the count assertion fails; delete
/// every run regardless of url and the survivor assertion does.
#[test]
fn deleting_several_jobs_purges_exactly_those_trails() {
    let (_dir, store) = store();
    for (id, url) in [
        ("run-a", "https://boards.example/jobs/1"),
        ("run-b", "https://boards.example/jobs/2"),
        ("run-keep", "https://boards.example/jobs/3"),
    ] {
        store.upsert_run(&run(id, url, 1_000)).unwrap();
        store
            .append_event(&event(id, 0, r#"{"full":{"perCompany":[]}}"#))
            .unwrap();
    }

    let removed = store.delete_for_jobs(&[
        "https://boards.example/jobs/1".to_string(),
        "https://boards.example/jobs/2".to_string(),
    ]);

    assert_eq!(removed, 2);
    assert!(store.events_for_run("run-a").is_empty());
    assert!(store.events_for_run("run-b").is_empty());
    assert_eq!(
        store.runs_for_job("https://boards.example/jobs/3").len(),
        1,
        "a posting nobody deleted keeps its history"
    );
    assert_eq!(store.events_for_run("run-keep").len(), 1);
    // Nothing to delete is not an error, and deletes nothing.
    assert_eq!(store.delete_for_jobs(&[]), 0);
    assert_eq!(store.events_for_run("run-keep").len(), 1);
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

/// **A run whose posting was deleted mid-flight must not bring it back.**
///
/// The hazard, executed rather than argued. A delete that lands while a run is
/// in flight (`applications_delete`, `ai_generations_remove`, a factory reset,
/// a restore) takes the run's row and the events that exist at that moment. The
/// run then keeps going, and its TERMINAL write is `INSERT OR REPLACE` — so it
/// re-inserts the row, `persist_document`'s merge-upsert re-creates the
/// `ai_generations` aggregate, and the posting the user deleted is back in the
/// runs panel, back in the Documents list, and back in the next backup. Worse
/// than a plain missed delete: the trail it comes back with is permanently
/// PARTIAL, because the pre-purge events are already gone.
///
/// Every step below was confirmed by running it before the guard existed:
/// the purge empties both tables, an event appended afterwards lands as an
/// orphan, and the terminal upsert resurrects the row with 1 event where the
/// run produced 2.
///
/// The guard is `store.run(run_id).is_none()` at the terminal write — the one
/// place both delete doors must pass through — plus
/// [`PipelineRunStore::delete_events_for_run`] for the gap.
///
/// Mutation check: delete the `run(run_id).is_none()` guard in `execute` and
/// the resurrection assertions below describe production again; drop the
/// `delete_events_for_run` sweep and the orphan assertion fails.
#[test]
fn a_run_whose_posting_was_deleted_mid_flight_does_not_resurrect_it() {
    let (_dir, store) = store();
    const URL: &str = "https://boards.example/jobs/42";

    // A run is IN FLIGHT: its `running` row exists and events are landing.
    let mut row = run("live", URL, 1_000);
    row.status = "running".to_string();
    store.upsert_run(&row).unwrap();
    store
        .append_event(&event(
            "live",
            0,
            r#"{"full":{"perCompany":[{"company":"Acme"}]}}"#,
        ))
        .unwrap();

    // The user deletes the posting.
    assert_eq!(store.delete_for_job(URL), 1);
    assert!(
        store.run("live").is_none() && store.events_for_run("live").is_empty(),
        "the premise: the purge takes the row and the trail it had"
    );

    // The run has not noticed — one more section finishes and appends.
    store
        .append_event(&event("live", 1, r#"{"full":{"items":[]}}"#))
        .unwrap();
    assert_eq!(
        store.events_for_run("live").len(),
        1,
        "the premise: an event appended after the purge lands as an ORPHAN"
    );

    // THE GUARD. This is what `execute` checks before its terminal write, and
    // it is the whole signal: the row this run created is gone.
    assert!(
        store.run("live").is_none(),
        "the abandoned-run signal must be readable at the terminal write"
    );
    let swept = store.delete_events_for_run("live");
    assert_eq!(swept, 1, "the gap's events go with it");
    assert!(store.events_for_run("live").is_empty());
    assert!(
        store.runs_for_job(URL).is_empty(),
        "the deleted posting stays deleted"
    );
}

/// The other side of that guard: an ORDINARY in-flight run must never look
/// abandoned, including on a posting that already has a full retention window
/// of finished runs. `prune` keeps the newest `RETENTION_RUNS_PER_JOB` by
/// `started_at DESC`, and the running row is the newest — so it cannot evict
/// the run that is calling it out from under itself.
///
/// Without this, the guard above would be satisfied by a store that simply
/// lost every row.
///
/// Mutation check: order `prune`'s window by `started_at ASC` and the live run
/// is evicted, so an ordinary run reports itself deleted.
#[test]
fn a_running_row_is_never_evicted_by_its_own_postings_retention() {
    let (_dir, store) = store();
    const URL: &str = "https://boards.example/jobs/42";
    for i in 0..RETENTION_RUNS_PER_JOB {
        store
            .upsert_run(&run(&format!("old-{i}"), URL, 100 + i as u64))
            .unwrap();
    }
    let mut live = run("live", URL, 9_000);
    live.status = "running".to_string();
    store.upsert_run(&live).unwrap();
    store.prune();
    assert!(
        store.run("live").is_some(),
        "an ordinary in-flight run must never look abandoned"
    );
}

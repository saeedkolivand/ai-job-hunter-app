use crate::pipeline::resume::{RunLedger, QUALITY_STAGES};
use crate::pipeline::runs::{PipelineRunStore, RunEventRow, RunRow};
use serde_json::json;

/// A whole run through the store, written the way `execute` + `RunHooks` write
/// one: the `running` row, six stages' `start`/`finish` pairs carrying real
/// ledger artifacts, then the terminal row.
///
/// Worth a test rather than trusting the store's own suite because THIS is
/// where the two sides meet: the store's `phase` CHECK is a schema constraint,
/// and an emitter that wrote `"finished"` would fail every insert at runtime
/// with nothing but a `warn!` to show for it. It also pins that the emitter's
/// `kind` is what `listForJob` filters on.
///
/// The artifact goes straight through `ledger.artifact(stage)`, exactly as
/// `RunHooks::after` reads it — so a stage's counts reach the row unchanged
/// (`validate`'s `criticals`).
///
/// Mutation check: emit a phase outside the CHECK's vocabulary and the event
/// count drops to zero; change `RUN_KIND` on one side only and the filtered
/// list is empty.
#[test]
fn a_run_round_trips_through_the_store_with_real_stage_events() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = PipelineRunStore::open(dir.path()).expect("store opens");

    let ledger = RunLedger::new();
    ledger.record("analyze_job", json!({ "cached": false, "mustHave": 4 }));
    ledger.record("strategy", json!({ "cached": false, "companies": 3 }));
    ledger.record("validate", json!({ "issues": 3, "criticals": 1 }));
    ledger.count_call(false);
    ledger.count_call(true);
    ledger.note_repair(1, false);

    let mut row = RunRow {
        id: "run-1".to_string(),
        job_url: "https://boards.example/jobs/42".to_string(),
        kind: super::super::RUN_KIND.to_string(),
        depth: "quality".to_string(),
        status: "running".to_string(),
        started_at: 1_700_000_000_000,
        finished_at: None,
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    };
    store
        .upsert_run(&row)
        .expect("the running row is written first");

    let mut seq = 0u32;
    for (index, stage) in QUALITY_STAGES.iter().enumerate() {
        for phase in ["start", "finish"] {
            store
                .append_event(&RunEventRow {
                    run_id: row.id.clone(),
                    seq,
                    ts: 1_700_000_000_000 + u64::from(seq),
                    stage: (*stage).to_string(),
                    phase: phase.to_string(),
                    // Exactly as `RunHooks::after` builds it.
                    artifact_json: ledger
                        .artifact(stage)
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "{}".to_string()),
                })
                .unwrap_or_else(|e| panic!("stage {index} phase {phase} must persist: {e}"));
            seq += 1;
        }
    }

    row.status = "needsReview".to_string();
    row.finished_at = Some(1_700_000_100_000);
    row.stopped_reason = Some("max_repairs".to_string());
    row.metrics_json = ledger.metrics().to_string();
    store
        .upsert_run(&row)
        .expect("the terminal row replaces it");

    let read = store.run("run-1").expect("the run is readable");
    assert_eq!(read.status, "needsReview");
    assert_eq!(read.stopped_reason.as_deref(), Some("max_repairs"));
    let metrics: serde_json::Value = serde_json::from_str(&read.metrics_json).expect("metrics");
    assert_eq!(metrics["calls"], json!(1));
    assert_eq!(metrics["cached"], json!(1));
    assert_eq!(metrics["repairRounds"], json!(1));

    let events = store.events_for_run("run-1");
    assert_eq!(
        events.len(),
        QUALITY_STAGES.len() * 2,
        "every stage's start/finish pair must survive the schema's phase CHECK"
    );
    assert!(
        events.windows(2).all(|pair| pair[0].seq < pair[1].seq),
        "seq order"
    );
    let validate = events
        .iter()
        .find(|event| event.stage == "validate" && event.phase == "finish")
        .expect("the validate stage's finish event");
    let artifact: serde_json::Value =
        serde_json::from_str(&validate.artifact_json).expect("the artifact is JSON");
    assert_eq!(artifact["criticals"], json!(1));

    let strategy = events
        .iter()
        .find(|event| event.stage == "strategy" && event.phase == "finish")
        .expect("the strategy stage's finish event");
    let artifact: serde_json::Value =
        serde_json::from_str(&strategy.artifact_json).expect("the artifact is JSON");
    assert_eq!(artifact["companies"], json!(3));

    // `listForJob` filters on this flow's kind; a run of another kind against
    // the same posting must not appear in the résumé runs list.
    store
        .upsert_run(&RunRow {
            id: "run-agent".to_string(),
            kind: "agent".to_string(),
            ..row.clone()
        })
        .expect("an agent run shares the tables");
    let resume_runs: Vec<_> = store
        .runs_for_job(&row.job_url)
        .into_iter()
        .filter(|candidate| candidate.kind == super::super::RUN_KIND)
        .collect();
    assert_eq!(resume_runs.len(), 1);
    assert_eq!(resume_runs[0].id, "run-1");
}

/// **A write command refuses an OLDER run of the same posting.**
///
/// Every run of a posting merges into ONE `ai_generations` aggregate, so
/// `find_for_job` can only ever return the newest run's document while
/// `listForJob` legitimately advertises three runs. Without this guard,
/// `regenerateSection(oldRunId)` rewrote the NEWEST document and returned a
/// detail whose `resumeText` was never that run's — a silent cross-run edit,
/// which is worse than a refusal.
///
/// Mutation check: return `Ok(())` unconditionally from `ensure_latest_run` and
/// the older-run assertion fails; drop the empty-`job_url` exemption and the
/// unlinked case fails.
#[test]
fn a_write_against_an_older_run_of_the_same_posting_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = PipelineRunStore::open(dir.path()).expect("store opens");

    let base = RunRow {
        id: String::new(),
        job_url: "https://boards.example/jobs/42".to_string(),
        kind: super::super::RUN_KIND.to_string(),
        depth: "quality".to_string(),
        status: "completed".to_string(),
        started_at: 0,
        finished_at: None,
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    };
    let older = RunRow {
        id: "run-older".to_string(),
        started_at: 1_700_000_000_000,
        ..base.clone()
    };
    let newer = RunRow {
        id: "run-newer".to_string(),
        started_at: 1_700_000_500_000,
        ..base.clone()
    };
    store.upsert_run(&older).expect("older run persists");
    store.upsert_run(&newer).expect("newer run persists");

    assert!(
        super::super::ensure_latest_run(&store, &newer).is_ok(),
        "the newest run owns the document"
    );
    let refused = super::super::ensure_latest_run(&store, &older);
    assert!(
        matches!(refused, Err(crate::error::AppError::Validation(_))),
        "an older run must be refused rather than silently editing the newest document"
    );

    // A run of ANOTHER kind against the same posting is not competition — the
    // aggregate is partitioned by `(job_url, kind)`.
    store
        .upsert_run(&RunRow {
            id: "run-agent".to_string(),
            kind: "agent".to_string(),
            started_at: 1_700_000_900_000,
            ..base.clone()
        })
        .expect("an agent run shares the tables");
    assert!(
        super::super::ensure_latest_run(&store, &newer).is_ok(),
        "a newer run of a different kind must not lock the résumé flow out"
    );

    // An UNLINKED run has no aggregate at all, so this guard must stand aside
    // and let the "no saved résumé" error be the one the user sees.
    let unlinked = RunRow {
        id: "run-unlinked".to_string(),
        job_url: String::new(),
        started_at: 1_700_000_100_000,
        ..base
    };
    store.upsert_run(&unlinked).expect("unlinked run persists");
    assert!(super::super::ensure_latest_run(&store, &unlinked).is_ok());
}

/// **A GAP, not a guarantee: a run killed mid-flight locks the last good run's
/// report out forever.**
///
/// `execute` writes its `running` row before stage one and replaces it with a
/// terminal one at the end. A SIGKILL/power loss between the two leaves the
/// `running` row on disk — and nothing at startup reconciles it:
/// `PipelineRunStore::open` runs migrations and a url normalisation only, with
/// no equivalent of `JobTracker::open`'s interrupted-job sweep or
/// `AutopilotStore::mark_interrupted_runs`.
///
/// So the orphan is permanently the newest run for its posting, and
/// [`super::super::ensure_latest_run`] — correctly, by its own rule — refuses every
/// write against the older run that actually produced the saved document. The
/// user is told to *"wait for it to finish"* for a run whose process no longer
/// exists. This test asserts that refusal because it is what ships today; it is
/// NOT a statement that the behaviour is right.
///
/// The kill itself is exercised for real (a child process is spawned and
/// killed) in `tests/pipeline_kill_recovery.rs`; this arm is the same state
/// reaching the real, private decision function.
///
/// Mutation check: this fails the moment interrupted-run reconciliation is
/// added at startup and applied before the refusal — which is the fix, not a
/// regression. Read a failure here as "the gap was closed; update this test".
///
/// **PR #1020** is the record behind that instruction: it documents the
/// lockout, why the two stores cannot simply be joined (nothing on disk links
/// a `pipeline_runs` row to its `jobs.db` row — the ids are generated
/// independently and `job_start` records a null payload), and the finding that
/// [`super::super::ensure_latest_run`] keys on RECENCY, not status — so a sweep that
/// only rewrites `status` fixes the run badge and leaves THIS refusal exactly
/// as it is. Read it before deciding what a red assertion here means.
#[test]
fn a_crashed_running_run_locks_out_the_last_good_runs_report() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = PipelineRunStore::open(dir.path()).expect("store opens");

    let job_url = "https://boards.example/jobs/killed".to_string();
    let finished = RunRow {
        id: "run-finished".to_string(),
        job_url: job_url.clone(),
        kind: super::super::RUN_KIND.to_string(),
        depth: "quality".to_string(),
        status: super::super::STATUS_NEEDS_REVIEW.to_string(),
        started_at: 1_700_000_000_000,
        finished_at: Some(1_700_000_240_000),
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    };
    // Byte-for-byte what `execute` writes before its first stage.
    let killed = RunRow {
        id: "run-killed".to_string(),
        status: "running".to_string(),
        started_at: 1_700_000_600_000,
        finished_at: None,
        ..finished.clone()
    };
    store.upsert_run(&finished).expect("finished run persists");
    store.upsert_run(&killed).expect("running row persists");

    // Pinned to the literal, not to `killed.status`: the whole claim is that
    // this value is never rewritten by anything, so it must be compared with
    // an absolute rather than with the value the test just wrote.
    assert_eq!(
        store.run("run-killed").map(|row| row.status),
        Some("running".to_string()),
        "no startup path reconciles an interrupted pipeline run"
    );

    let refused = super::super::ensure_latest_run(&store, &finished);
    assert!(
        matches!(refused, Err(crate::error::AppError::Validation(_))),
        "GAP: the orphaned `running` row outranks the only run with a saved \
         document, so `regenerateSection`/`resolveFabrication` are refused"
    );
    let Err(crate::error::AppError::Validation(message)) = refused else {
        unreachable!("asserted above")
    };
    assert!(
        message.contains("wait for it to finish"),
        "GAP: the refusal tells the user to wait for a run whose process is \
         gone and which nothing will ever finish: {message}"
    );

    // The orphan blocks itself out of nothing — it is the newest run, so a
    // write against IT is allowed, against a document it never wrote.
    assert!(super::super::ensure_latest_run(&store, &killed).is_ok());
}

//! A résumé pipeline killed mid-run — what survives on disk, and what nothing
//! ever cleans up.
//!
//! # This test DOCUMENTS A GAP. It does not assert correct behaviour.
//!
//! The résumé pipeline is the longest, most expensive, most stateful operation
//! in the app: it streams for minutes across eight stages
//! (`pipeline::resume::QUALITY_STAGES`) and spends real provider budget. It
//! writes its `pipeline_runs` row `status = "running"` BEFORE the first stage
//! deliberately, so "a crash mid-run leaves a `running` row a user can see,
//! rather than no evidence the run happened" (`commands::resume_pipeline::execute`).
//!
//! What is missing is the other half. `jobs::JobTracker::open` sweeps its own
//! table on every startup — interrupted rows become `failed` with a stated
//! reason — and `autopilot::AutopilotStore::mark_interrupted_runs` does the
//! same for autopilot. `pipeline::runs::PipelineRunStore::open` runs migrations
//! and a `job_url` normalisation and NOTHING ELSE: no status reconciliation
//! exists for the pipeline, in that store or anywhere else at startup.
//!
//! So the assertions below pin TODAY'S BEHAVIOUR, and today's behaviour is the
//! defect: after a kill the row reads `running` forever.
//! `a_killed_run_is_left_running_while_its_job_row_is_swept` is written as a
//! **contrast**: one kill, two stores, one of them reconciled. Nothing here
//! should be read as "the pipeline recovers correctly" — it does not recover at
//! all. See `the_orphaned_running_row_outranks_the_last_good_run` for the
//! user-visible consequence.
//!
//! # What is REAL here and what is simulated
//!
//! **Real: the kill.** This test binary re-executes ITSELF as a child process
//! (`crashed_run_writer_child`, an `#[ignore]`d arm gated on an env var), the
//! child opens the same stores the app opens and writes the state a run in its
//! `draft` stage would have written, and the parent then `Child::kill()`s it —
//! `TerminateProcess`/`SIGKILL`, no unwinding, no destructors, no SQLite
//! close. The parent, which has never opened those files, then opens them the
//! way startup does. That is a genuine process death with committed WAL state
//! read back by a fresh process.
//!
//! **Simulated: the run itself.** The child writes the run/event/job rows
//! directly rather than executing eight AI stages, because a real run needs a
//! provider, an `AppHandle` and minutes of wall clock. So this proves what
//! happens to *that on-disk state* across a kill; it does not prove the
//! pipeline writes exactly that state (the shape is taken from `execute` and
//! `commands::resume_pipeline::hooks`, and the stage vocabulary is checked
//! against the real `QUALITY_STAGES` below so a rename fails loudly here).
//!
//! **No sleeps, no polling, no wall-clock race.** The parent blocks on a line
//! read from the child's stdout, which arrives only after the child's SQLite
//! writes have committed; the child then blocks forever on a stdin read the
//! parent never writes to. There is one duration in this file
//! (`READY_WATCHDOG`) and it is a watchdog, not a synchronisation — it turns a
//! wedged child into a failure instead of a hung suite, and no assertion
//! depends on its value. The one clock read in the code under test is
//! `JobTracker::open`'s 24-HOUR retention window, which parent and child cross
//! milliseconds apart.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use ajh_tauri::jobs::{JobStatus, JobTracker};
use ajh_tauri::pipeline::resume::QUALITY_STAGES;
use ajh_tauri::pipeline::runs::{PipelineRunStore, RunEventRow, RunRow};

/// Set by the parent on the child arm; its value is the shared data dir.
const CHILD_DIR_ENV: &str = "AJH_PIPELINE_KILL_DATA_DIR";
/// The child's name in this binary's test list.
const CHILD_TEST: &str = "crashed_run_writer_child";
/// Printed by the child once — and only once — every row is committed.
const READY_LINE: &str = "AJH-PIPELINE-KILL-STATE-COMMITTED";
/// The child's exit code if its stdin ever closes, so a self-exit can never be
/// mistaken for the parent's kill.
const CHILD_SELF_EXIT: i32 = 97;

// ── The fixture: one posting, a completed run, then a run killed in `draft` ──

const JOB_URL: &str = "https://boards.example/jobs/pipeline-kill";
/// Mirrors `commands::resume_pipeline::RUN_KIND` / `RUN_DEPTH`, which are
/// private. `a_crashed_running_run_locks_out_the_last_good_runs_report` in
/// `src/commands/resume_pipeline/test.rs` uses the real consts, so drift is
/// caught there rather than silently making this fixture unrealistic.
const RUN_KIND: &str = "resume";
const RUN_DEPTH: &str = "quality";

const COMPLETED_RUN_ID: &str = "run-before-the-kill";
const KILLED_RUN_ID: &str = "run-killed-mid-draft";
/// Fixed epoch-ms stamps — the store orders by `started_at DESC`, and a run
/// ordering that depends on when the suite happens to run is a race.
const COMPLETED_STARTED_AT: u64 = 1_700_000_000_000;
const COMPLETED_FINISHED_AT: u64 = 1_700_000_240_000;
const KILLED_STARTED_AT: u64 = 1_700_000_600_000;

const JOB_ID: &str = "job-killed-mid-draft";
/// The literal `resume_pipeline_run` passes to `jobs::job_start`.
const JOB_KIND: &str = "resumePipeline.run";
/// The progress the child stamps: the `draft` stage boundary, three of eight
/// stages done. Asserted unchanged after the sweep.
const JOB_PROGRESS: f64 = 0.375;

/// The stage trail a run cut down during `draft` leaves behind: three stages
/// closed, the fourth opened and never finished. HAND-WRITTEN rather than
/// derived from `QUALITY_STAGES` so the expected trail is an absolute the
/// parent can assert against; `stage_fixture_matches_the_real_pipeline` checks
/// the names still exist in the real pipeline.
const KILLED_TRAIL: &[(u32, &str, &str)] = &[
    (0, "analyze_job", "start"),
    (1, "analyze_job", "finish"),
    (2, "match_evidence", "start"),
    (3, "match_evidence", "finish"),
    (4, "strategy", "start"),
    (5, "strategy", "finish"),
    (6, "draft", "start"),
];

fn completed_run() -> RunRow {
    RunRow {
        id: COMPLETED_RUN_ID.to_string(),
        job_url: JOB_URL.to_string(),
        kind: RUN_KIND.to_string(),
        depth: RUN_DEPTH.to_string(),
        status: "needsReview".to_string(),
        started_at: COMPLETED_STARTED_AT,
        finished_at: Some(COMPLETED_FINISHED_AT),
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    }
}

/// Exactly what `execute` writes before stage one: no `finished_at`, no
/// `stopped_reason`, empty metrics.
fn killed_run() -> RunRow {
    RunRow {
        id: KILLED_RUN_ID.to_string(),
        job_url: JOB_URL.to_string(),
        kind: RUN_KIND.to_string(),
        depth: RUN_DEPTH.to_string(),
        status: "running".to_string(),
        started_at: KILLED_STARTED_AT,
        finished_at: None,
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    }
}

/// The state an app process has on disk the instant before it is killed
/// mid-`draft`.
fn write_pre_kill_state(dir: &Path) {
    let store = PipelineRunStore::open(dir).expect("the run store opens");
    store
        .upsert_run(&completed_run())
        .expect("the earlier, finished run persists");
    store
        .upsert_run(&killed_run())
        .expect("the running row is written before stage one");
    for &(seq, stage, phase) in KILLED_TRAIL {
        store
            .append_event(&RunEventRow {
                run_id: KILLED_RUN_ID.to_string(),
                seq,
                ts: KILLED_STARTED_AT + u64::from(seq) * 1_000,
                stage: stage.to_string(),
                phase: phase.to_string(),
                artifact_json: "{}".to_string(),
            })
            .expect("a stage event persists");
    }

    // The OTHER half of what a live run has on disk: `resume_pipeline_run`
    // registers a job before it ever reaches `execute`. Separate database,
    // separate id, and — see the parent test — separate fate.
    let mut jobs = JobTracker::open(dir);
    jobs.start(JOB_ID, JOB_KIND);
    jobs.update_progress(JOB_ID, JOB_PROGRESS);
}

// ── The child arm ────────────────────────────────────────────────────────────

/// Not a test: the crashing app process, re-entered through this same binary.
///
/// `#[ignore]` keeps it out of a normal run, and the env-var guard keeps it a
/// no-op even under `--include-ignored`, so it can never park a plain suite.
#[test]
#[ignore = "child process arm of a_killed_run_is_left_running_while_its_job_row_is_swept"]
fn crashed_run_writer_child() {
    let Ok(dir) = std::env::var(CHILD_DIR_ENV) else {
        return;
    };
    write_pre_kill_state(Path::new(&dir));

    // Announce only after every write has committed, so the parent's kill can
    // never land on a half-written store. `--nocapture` puts this on the pipe.
    println!("{READY_LINE}");
    std::io::stdout().flush().expect("the ready line flushes");

    // Park until killed: a blocking read on a stdin the parent holds open and
    // never writes to. No sleep, no timeout, no busy loop.
    let mut sink = String::new();
    let _ = std::io::stdin().read_line(&mut sink);
    std::process::exit(CHILD_SELF_EXIT);
}

/// Purely a WATCHDOG, never a synchronisation: the happy path is unblocked the
/// instant the child's ready line arrives (single-digit milliseconds), and this
/// only exists so a child that wedges before committing fails the suite instead
/// of hanging it. Nothing about the assertions depends on its value.
const READY_WATCHDOG: Duration = Duration::from_secs(60);

/// Spawn the child, block until it says its state is committed, then kill it.
/// Leaves `dir` holding the on-disk state of a process that died mid-run.
fn kill_a_run_in_flight(dir: &Path) {
    let mut child: Child = Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", "--ignored", "--nocapture", CHILD_TEST])
        .env(CHILD_DIR_ENV, dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("the child app process starts");

    // The scan runs on its own thread so the kill below can always happen: a
    // child that printed nothing and never exits would otherwise block this
    // thread forever on a pipe that never reaches EOF, leaving a parked process
    // behind. The kill closes the pipe, which ends the thread either way.
    let stdout = child.stdout.take().expect("the child's stdout is piped");
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line.trim() == READY_LINE {
                let _ = tx.send(());
                return;
            }
        }
    });
    let committed = rx.recv_timeout(READY_WATCHDOG).is_ok();

    child.kill().expect("the app process is killed outright");
    let status = child.wait().expect("the killed process is reaped");
    reader.join().expect("the stdout scan ends with the pipe");

    assert!(
        committed,
        "the child never committed its pre-kill state — there is no crash to test"
    );
    assert!(
        !status.success(),
        "the child must die by the kill, not exit on its own"
    );
    assert_ne!(
        status.code(),
        Some(CHILD_SELF_EXIT),
        "the child's stdin closed and it exited itself — that is not a kill"
    );
}

// ── The gap ──────────────────────────────────────────────────────────────────

/// **A kill leaves the pipeline run `running` forever; the same kill's job row
/// is reconciled.** One crash, two stores, one startup sweep between them.
///
/// The `jobs.db` half is the POSITIVE CONTROL: it pins recovery that really
/// exists (`JobTracker::open`'s `UPDATE … SET status = 'failed'`), against the
/// absolute string that sweep writes — a value the child never wrote, so this
/// arm cannot pass by round-tripping. Delete that `UPDATE` and this test goes
/// red.
///
/// The `pipeline_runs` half is the GAP: `running`, `finished_at: None`, no
/// stopped reason, unchanged by a startup that just reconciled its sibling.
/// There is nothing to delete to make that half fail — which is the finding.
/// It fails only if reconciliation is ADDED, which is the mutation this test
/// was checked with.
#[test]
fn a_killed_run_is_left_running_while_its_job_row_is_swept() {
    let dir = tempfile::tempdir().expect("temp data dir");
    kill_a_run_in_flight(dir.path());

    // A process that has never touched these files opens them the way startup
    // does. This IS the app's whole pipeline-side restart path.
    let store = PipelineRunStore::open(dir.path()).expect("startup reopens the run store");
    let row = store
        .run(KILLED_RUN_ID)
        .expect("the pre-stage `running` row survived the kill");

    assert_eq!(
        row.status, "running",
        "GAP: nothing at startup reconciles an interrupted pipeline run, so it \
         reads as still in flight — the renderer paints the live blue chip \
         (PipelineRunsList STATUS_TONE.running) for a run whose process is gone"
    );
    assert_eq!(row.finished_at, None, "GAP: the run never ends");
    assert_eq!(
        row.stopped_reason, None,
        "GAP: no reason is recorded, so nothing can tell the user what happened"
    );
    assert_eq!(row.metrics_json, "{}", "no metrics were ever written");
    assert_eq!(row.started_at, KILLED_STARTED_AT);
    assert_eq!(row.depth, RUN_DEPTH);

    // The partial trail survives intact — the evidence the `running` row was
    // written for. Compared against the hand-written absolute, so a trail that
    // silently loses its last (unfinished) event fails here.
    let trail: Vec<(u32, String, String)> = store
        .events_for_run(KILLED_RUN_ID)
        .into_iter()
        .map(|event| (event.seq, event.stage, event.phase))
        .collect();
    let expected: Vec<(u32, String, String)> = KILLED_TRAIL
        .iter()
        .map(|&(seq, stage, phase)| (seq, stage.to_string(), phase.to_string()))
        .collect();
    assert_eq!(trail, expected);
    assert_eq!(
        trail
            .last()
            .map(|(_, stage, phase)| (stage.as_str(), phase.as_str())),
        Some(("draft", "start")),
        "the trail ends on an OPENED stage with no `finish` — the on-disk \
         signature of an interrupted run, which nothing reads"
    );

    // The contrast. Same kill, same startup, sibling store: reconciled.
    let jobs = JobTracker::open(dir.path());
    let job = jobs
        .get(JOB_ID)
        .expect("the job row survived the kill and was loaded");
    assert_eq!(
        job.status,
        JobStatus::Failed,
        "the jobs.db sweep flips an interrupted job — this is the recovery the \
         pipeline run store has no equivalent of"
    );
    assert_eq!(
        job.error.as_deref(),
        Some("Interrupted by app restart"),
        "the sweep states a reason; the pipeline run has none"
    );
    assert!(
        job.finished_at.is_some(),
        "the sweep stamps a finish time; the pipeline run has none"
    );
    assert_eq!(job.kind, JOB_KIND);
    // Written by the child and NOT touched by the sweep: progress is history,
    // status is the thing reconciled.
    assert_eq!(job.progress, JOB_PROGRESS);
}

/// **The consequence: the orphaned row outranks the last run that produced a
/// document.**
///
/// `commands::resume_pipeline::ensure_latest_run` reads exactly this — the
/// first `runs_for_job` row of matching `kind` — and refuses any write against
/// an older run with *"There is a newer run for this posting … If that run is
/// still going, wait for it to finish."* The killed run is never going to
/// finish, so `resume_pipeline_regenerate_section` and
/// `resume_pipeline_resolve_fabrication` are locked out of the last document
/// that actually exists, permanently, behind a message that is false.
///
/// This asserts the STORE-LEVEL input (`ensure_latest_run` is private to the
/// command module and unreachable from an integration test); the refusal
/// itself is asserted against the real function by
/// `a_crashed_running_run_locks_out_the_last_good_runs_report` in
/// `src/commands/resume_pipeline/test.rs`.
#[test]
fn the_orphaned_running_row_outranks_the_last_good_run() {
    let dir = tempfile::tempdir().expect("temp data dir");
    kill_a_run_in_flight(dir.path());

    let store = PipelineRunStore::open(dir.path()).expect("startup reopens the run store");
    let newest = store
        .runs_for_job(JOB_URL)
        .into_iter()
        .find(|candidate| candidate.kind == RUN_KIND)
        .expect("the posting has résumé runs");

    assert_eq!(
        newest.id, KILLED_RUN_ID,
        "GAP: the run that produced nothing owns the posting; the run that \
         produced the saved document is now 'older' and refused"
    );
    assert_eq!(newest.status, "running");
    // And the run holding the document the user can still see is intact — this
    // is not data loss, it is a lockout.
    let survivor = store
        .run(COMPLETED_RUN_ID)
        .expect("the earlier run is still there");
    assert_eq!(survivor.status, "needsReview");
    assert_eq!(survivor.finished_at, Some(COMPLETED_FINISHED_AT));
}

/// The fixture's stage names are the real pipeline's. A hand-written trail is
/// only worth asserting against while it still describes runs this app can
/// produce — and `QUALITY_STAGES` is `pub` precisely so it can be pinned.
#[test]
fn stage_fixture_matches_the_real_pipeline() {
    for &(_, stage, phase) in KILLED_TRAIL {
        assert!(
            QUALITY_STAGES.contains(&stage),
            "{stage} is no longer a stage of the résumé pipeline — re-derive KILLED_TRAIL"
        );
        assert!(
            phase == "start" || phase == "finish",
            "{phase} is not a stage phase"
        );
    }
    // The trail must stop partway: a fixture covering every stage would be a
    // finished run, not an interrupted one.
    assert!(KILLED_TRAIL.len() < QUALITY_STAGES.len() * 2);
}

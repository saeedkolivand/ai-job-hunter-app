//! The run outcome a record carries: derived from the per-board summaries, reconciled after a
//! crash, and cleared when a run never reaches `record_run`.

use super::super::*;
use super::support::*;

#[test]
fn mark_interrupted_runs_flips_only_in_progress() {
    let (_temp, store) = temp_store();

    let make = |name: &str| create_ap(&store, name, "linkedin", 50.0, "manual");
    let running = make("running");
    let done = make("done");

    store.set_run_status(&running.id, RunStatus::InProgress);
    store.set_run_status(&done.id, RunStatus::Completed);

    let reconciled = store.mark_interrupted_runs();
    assert_eq!(
        reconciled.len(),
        1,
        "only the in-progress run is reconciled"
    );
    assert_eq!(
        reconciled[0], running.id,
        "the reconciled id is the interrupted run's, so the scheduler retries the right one"
    );

    let status = |id: &str| store.get(id).unwrap().run_status;
    assert_eq!(status(&running.id), Some(RunStatus::Interrupted));
    assert_eq!(status(&done.id), Some(RunStatus::Completed));

    // Idempotent: a second startup sweep finds nothing to reconcile.
    assert!(store.mark_interrupted_runs().is_empty());
}

#[test]
fn record_run_marks_the_run_completed() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "ap", "linkedin", 50.0, "manual");
    store.set_run_status(&ap.id, RunStatus::InProgress);

    // A run with no per-board problems (empty summaries) records a clean
    // `Completed` — preserving the pre-summaries behavior.
    record(&store, &ap.id, 3, Vec::new());
    assert_eq!(
        store.get(&ap.id).unwrap().run_status,
        Some(RunStatus::Completed)
    );
}

#[test]
fn derive_run_status_maps_summaries_to_honest_outcome() {
    // No boards reported anything → nothing failed, so a clean completion.
    assert_eq!(derive_run_status(&[]), RunStatus::Completed);

    // Every board returned results, no errors/truncation → Completed.
    assert_eq!(
        derive_run_status(&[
            board_summary("greenhouse", 4, None, None, None),
            board_summary("lever", 2, None, None, None),
        ]),
        RunStatus::Completed
    );

    // A board that RAN clean and genuinely found zero (no error, no skip, no
    // truncation) is a real "no jobs today", not a failure — Completed, not
    // Failed. This is the core case the audit called out: "no jobs matched"
    // must stay distinguishable from "everything failed".
    assert_eq!(
        derive_run_status(&[board_summary("greenhouse", 0, None, None, None)]),
        RunStatus::Completed
    );

    // At least one succeeded + at least one errored → CompletedWithErrors.
    assert_eq!(
        derive_run_status(&[
            board_summary("greenhouse", 4, None, None, None),
            board_summary("aggregator", 0, Some("429 Too Many Requests"), None, None),
        ]),
        RunStatus::CompletedWithErrors
    );

    // A partial (truncated) harvest is a partial success → CompletedWithErrors,
    // even when it is the ONLY board.
    assert_eq!(
        derive_run_status(&[board_summary(
            "themuse",
            10,
            None,
            None,
            Some("page 2 of 5 failed: HTTP 429"),
        )]),
        RunStatus::CompletedWithErrors
    );

    // Zero boards succeeded (all errored) → Failed.
    assert_eq!(
        derive_run_status(&[
            board_summary(
                "aggregator",
                0,
                Some("credential store unavailable"),
                None,
                None
            ),
            board_summary("linkedin", 0, Some("blocked"), None, None),
        ]),
        RunStatus::Failed
    );

    // Zero boards ran because all were skipped → Failed (nothing actually ran,
    // so the run produced nothing — an honest failure, not a clean zero).
    assert_eq!(
        derive_run_status(&[board_summary(
            "aggregator",
            0,
            None,
            Some("needs-keys"),
            None
        )]),
        RunStatus::Failed
    );

    // A skip alongside a real success does NOT downgrade — a skipped board is an
    // expected no-op, not a failure of a board that ran.
    assert_eq!(
        derive_run_status(&[
            board_summary("greenhouse", 3, None, None, None),
            board_summary("linkedin", 0, None, Some("needs-login"), None),
        ]),
        RunStatus::Completed
    );
}

#[test]
fn record_run_persists_summaries_and_derives_status() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "ap", "aggregator", 0.0, "manual");

    // One board delivered, one errored → the record must say CompletedWithErrors
    // and keep BOTH summaries so the UI can explain the shortfall later.
    let summaries = vec![
        board_summary("greenhouse", 2, None, None, None),
        board_summary("aggregator", 0, Some("429 Too Many Requests"), None, None),
    ];
    store.record_run(&ap.id, 2, 0, Vec::new(), summaries, &no_tombstones(), &[]);

    let reloaded = store.get(&ap.id).unwrap();
    assert_eq!(reloaded.run_status, Some(RunStatus::CompletedWithErrors));
    assert_eq!(reloaded.last_run_summaries.len(), 2);
    assert_eq!(
        reloaded.last_run_summaries[1].error.as_deref(),
        Some("429 Too Many Requests")
    );
}

#[test]
fn fail_run_without_summaries_marks_failed_and_clears_stale_summaries() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "ap", "aggregator", 0.0, "manual");

    // Seed a prior SUCCESSFUL run — non-empty summaries + a Completed status —
    // the exact stale state `fail_run_without_summaries` must clear so a future
    // chip strip doesn't render the PRIOR run's per-board data as if it
    // belonged to the run that's about to fail outright.
    let summaries = vec![board_summary("greenhouse", 2, None, None, None)];
    store.record_run(&ap.id, 2, 0, Vec::new(), summaries, &no_tombstones(), &[]);
    let seeded = store.get(&ap.id).unwrap();
    assert_eq!(seeded.run_status, Some(RunStatus::Completed));
    assert_eq!(seeded.last_run_summaries.len(), 1);

    // An outright scrape error never reaches `record_run` (no fresh summaries
    // to report) — `fail_run_without_summaries` is the path taken instead.
    store.fail_run_without_summaries(&ap.id);

    let reloaded = store.get(&ap.id).unwrap();
    assert_eq!(
        reloaded.run_status,
        Some(RunStatus::Failed),
        "an outright scrape error must mark the run Failed"
    );
    assert!(
        reloaded.last_run_summaries.is_empty(),
        "the prior run's summaries must be cleared, not left stale on a Failed record"
    );
}

#[test]
fn fail_run_without_summaries_unknown_id_is_a_no_op() {
    let (_temp, store) = temp_store();

    // Unknown id — must not panic (mirrors the "missing" no-op convention used
    // by `record_run`/`set_run_status` elsewhere in this file).
    store.fail_run_without_summaries("missing");
    assert!(
        store.list().is_empty(),
        "an unknown id must not create or otherwise mutate any record"
    );
}

#[test]
fn set_run_status_clearing_summaries_completed_clears_stale_summaries() {
    // Mirrors `fail_run_without_summaries_marks_failed_and_clears_stale_summaries`
    // for the OTHER caller of `set_run_status_clearing_summaries`: a user-cancelled
    // run, which sets `Completed` (not `Failed`) but likewise never reaches
    // `record_run`, so it must clear the PRIOR run's summaries too.

    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "ap", "aggregator", 0.0, "manual");

    // Seed a prior successful run with real summaries — the stale state a
    // cancelled run must not inherit.
    let summaries = vec![board_summary("greenhouse", 2, None, None, None)];
    store.record_run(&ap.id, 2, 0, Vec::new(), summaries, &no_tombstones(), &[]);
    let seeded = store.get(&ap.id).unwrap();
    assert_eq!(seeded.run_status, Some(RunStatus::Completed));
    assert_eq!(seeded.last_run_summaries.len(), 1);

    // A cancelled run (never reaches `record_run`) sets Completed via the
    // clearing variant, same as the autopilot_run command's cancel branch.
    store.set_run_status_clearing_summaries(&ap.id, RunStatus::Completed);

    let reloaded = store.get(&ap.id).unwrap();
    assert_eq!(reloaded.run_status, Some(RunStatus::Completed));
    assert!(
        reloaded.last_run_summaries.is_empty(),
        "a cancelled run must clear the PRIOR run's summaries, not leave them stale"
    );
}

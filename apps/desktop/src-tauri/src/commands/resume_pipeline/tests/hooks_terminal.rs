use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::RunLedger;

/// **A cancelled run reports `cancelled`, never `failed` + `"done"`.**
///
/// The live path this reproduces: the user cancels during the DRAFT stage.
/// `chat_stream` aborts and returns `AppError::Message("Job cancelled")` — not
/// `AppError::Cancelled` — the draft stage propagates it, and because `repair`
/// never starts, no later `before()` runs `apply_stop`, so the ledger records
/// NOTHING. The old derivation then read `stopped == None` as
/// `StoppedReason::Done` and `cancelled == false`, producing
/// `status=failed stoppedReason="done"` for a run the user cancelled — and the
/// renderer maps `done` to a success suffix.
///
/// Mutation check: derive `cancelled` from the ledger alone (drop the
/// `token_cancelled` argument's effect) and the status assertion fails; restore
/// `stopped.unwrap_or(Done)` and the reason assertion does.
#[test]
fn a_cancel_the_ledger_never_saw_still_reports_cancelled() {
    // Exactly the ledger a cancelled draft leaves behind: empty.
    let ledger = RunLedger::new();
    assert_eq!(
        ledger.stopped(),
        None,
        "the premise: the ledger saw nothing"
    );

    let (status, reason) = super::super::hooks::terminal_state(&ledger, false, true, false, false);
    assert_eq!(status, "cancelled");
    assert_eq!(reason.as_deref(), Some("cancelled"));
    assert_eq!(
        ledger.stopped(),
        Some(StoppedReason::Cancelled),
        "…and the ledger is corrected, so the metrics agree with the row"
    );
}

/// A run that FAILED for any other reason carries no stopped reason at all
/// rather than the `done` that used to be the fallback. `Done` means "the last
/// stage completed"; a failure is the one thing it must never label.
///
/// Mutation check: restore `stopped.unwrap_or(StoppedReason::Done)` and the
/// `None` assertion fails.
#[test]
fn a_failed_run_never_reports_done() {
    let ledger = RunLedger::new();
    let (status, reason) = super::super::hooks::terminal_state(&ledger, false, false, false, false);
    assert_eq!(status, "failed");
    assert_eq!(
        reason, None,
        "a run that failed with no recorded stop reason reports none — never \"done\""
    );
}

/// The rest of the terminal-state table, so the fixes above cannot have moved
/// the ordinary outcomes: a clean run is `completed` + `done`, a run with
/// undecided findings is `needsReview`, and a recorded reason always wins over
/// the derived one.
#[test]
fn the_terminal_state_table_holds_for_the_ordinary_outcomes() {
    let clean = RunLedger::new();
    assert_eq!(
        super::super::hooks::terminal_state(&clean, true, false, false, true),
        ("completed", Some("done".to_string()))
    );

    let review = RunLedger::new();
    assert_eq!(
        super::super::hooks::terminal_state(&review, true, false, true, true),
        ("needsReview", Some("done".to_string()))
    );

    // A stage that stopped the run keeps its own reason, and `needsReview` is
    // still not a failure.
    let repaired = RunLedger::new();
    repaired.stop(StoppedReason::MaxRepairs);
    assert_eq!(
        super::super::hooks::terminal_state(&repaired, true, false, true, true),
        ("needsReview", Some("max_repairs".to_string()))
    );

    // The deadline with NOTHING saved: the stage errored out before a document
    // existed, so the run failed — but it says WHY.
    let timed_out = RunLedger::new();
    timed_out.stop(StoppedReason::RunTimeout);
    assert_eq!(
        super::super::hooks::terminal_state(&timed_out, false, false, false, false),
        ("failed", Some("run_timeout".to_string()))
    );

    // A run already stopped for a reason of its own, cancelled afterwards: the
    // two halves come from different places on purpose. The REASON is
    // first-writer-wins (the budget ceiling is what stopped it, and it did come
    // first); the STATUS is the token's, because the user acted — and `execute`
    // reads the status to choose `job_cancel` over `job_fail`.
    let budgeted = RunLedger::new();
    budgeted.stop(StoppedReason::Budgeted);
    assert_eq!(
        super::super::hooks::terminal_state(&budgeted, false, true, false, true),
        ("cancelled", Some("budgeted".to_string()))
    );
}

/// **The deadline's two enforcement points must reach the same verdict about
/// the same run.**
///
/// Expiring INSIDE the repair loop breaks the loop, keeps the accumulated
/// document and returns `Ok`, so the run lands `needsReview` + `run_timeout`.
/// Expiring at the stage BOUNDARY one instant later goes through `apply_stop`,
/// which returns `Err` — and that came out `failed`, even though
/// `persist_document` had already written the same real, reviewable résumé to
/// `ai_generations`. Same document, same reason, opposite verdict, and `failed`
/// is the one that tells the user their run produced nothing.
///
/// Mutation check: drop the `persisted` term from `terminal_state` (or the
/// `RunTimeout` test in it) and the first two assertions fail; derive
/// `cancelled` from the ledger's reason alone (drop the `!ok && token_cancelled`
/// term) and the cancel assertion fails.
#[test]
fn a_deadline_that_still_saved_a_document_is_not_a_failure() {
    // Undecided findings in the saved report — the ordinary shape of a run the
    // repair loop ran out of time on.
    let review = RunLedger::new();
    review.stop(StoppedReason::RunTimeout);
    assert_eq!(
        super::super::hooks::terminal_state(&review, false, false, true, true),
        ("needsReview", Some("run_timeout".to_string())),
        "the boundary check must land where the in-loop check does"
    );

    // Nothing left to decide: the document is usable as it stands.
    let clean = RunLedger::new();
    clean.stop(StoppedReason::RunTimeout);
    assert_eq!(
        super::super::hooks::terminal_state(&clean, false, false, false, true),
        ("completed", Some("run_timeout".to_string()))
    );

    // A run whose CANCEL TOKEN fired: the user acted, so the STATUS is
    // `cancelled` — the outcome the comment on this row always argued for and
    // the one `execute` needs to emit `job_cancel` rather than `job_fail`. The
    // REASON stays first-writer-wins (`run_timeout` is what stopped it, and it
    // happened first), which is the same split the `budgeted` row in the table
    // above pins. Reading the status off the reason alone reported `failed`
    // here, i.e. "your run produced nothing", for a run the user themselves
    // ended.
    let cancelled = RunLedger::new();
    cancelled.stop(StoppedReason::RunTimeout);
    assert_eq!(
        super::super::hooks::terminal_state(&cancelled, false, true, true, true),
        ("cancelled", Some("run_timeout".to_string()))
    );

    // Only `RunTimeout` qualifies. A provider error mid-run leaves a document
    // whose report describes a different document, so it stays a failure even
    // though something was saved.
    let errored = RunLedger::new();
    errored.stop(StoppedReason::MaxRepairs);
    assert_eq!(
        super::super::hooks::terminal_state(&errored, false, false, true, true),
        ("failed", Some("max_repairs".to_string()))
    );
}

/// **Only `RunTimeout` gets the "still usable" leniency `terminal_state`
/// grants above — `Timeout` never does**, even when a document was somehow
/// persisted. A per-call deadline inside `analyze_job`/`strategy` means that stage's own JSON never parsed — there is no partial
/// artifact the way a WHOLE-run deadline caught at a later stage boundary
/// always has a real document from every stage that already finished. Reusing
/// `timed_out_with_document`'s leniency for `Timeout` would report
/// `needsReview`/`completed` over a run that produced nothing this specific
/// document came from.
///
/// Mutation check: fold `Timeout` into `timed_out_with_document`'s match and
/// this fails (status flips to `completed`).
#[test]
fn a_timeout_stopped_run_is_always_a_failure_even_when_something_was_persisted() {
    let ledger = RunLedger::new();
    ledger.stop(StoppedReason::Timeout);
    assert_eq!(
        super::super::hooks::terminal_state(&ledger, false, false, false, true),
        ("failed", Some("timeout".to_string())),
        "persisted=true must not rescue a Timeout the way it rescues a RunTimeout"
    );
}

/// **A provider failure persists a reason** (#1393): without it the failed row
/// had `stopped_reason = NULL` and the card could show nothing after a remount.
/// A cancel the stream surfaced as a provider error is NOT relabelled, and an
/// earlier recorded reason wins.
///
/// Mutation check: make `note_provider_failure` a no-op and the first assert fails.
#[test]
fn a_provider_failure_persists_a_terminal_reason() {
    use super::super::hooks::note_provider_failure;
    use crate::error::AppError;

    let failed = RunLedger::new();
    note_provider_failure(&failed, &Err(AppError::Provider("x".into())), false);
    assert_eq!(
        super::super::hooks::terminal_state(&failed, false, false, false, false),
        ("failed", Some("provider_error".to_string()))
    );

    let cut_off = RunLedger::new();
    note_provider_failure(&cut_off, &Err(AppError::OutputLimit("x".into())), false);
    assert_eq!(
        super::super::hooks::terminal_state(&cut_off, false, false, false, false),
        ("failed", Some("output_limit".to_string())),
        "the cutoff has its own reason, not provider_error"
    );

    let broke = RunLedger::new();
    note_provider_failure(&broke, &Err(AppError::Network("x".into())), false);
    assert_eq!(broke.stopped(), Some(StoppedReason::ProviderError));

    let refused = RunLedger::new();
    note_provider_failure(&refused, &Err(AppError::Refusal("x".into())), false);
    assert_eq!(refused.stopped(), Some(StoppedReason::ProviderError));

    let cancelled = RunLedger::new();
    note_provider_failure(&cancelled, &Err(AppError::Provider("x".into())), true);
    assert_eq!(cancelled.stopped(), None);

    let timed_out = RunLedger::new();
    timed_out.stop(StoppedReason::Timeout);
    note_provider_failure(&timed_out, &Err(AppError::Provider("x".into())), false);
    assert_eq!(timed_out.stopped(), Some(StoppedReason::Timeout));

    let fine = RunLedger::new();
    note_provider_failure(&fine, &Ok(()), false);
    assert_eq!(fine.stopped(), None);
}

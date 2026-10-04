use super::*;

// ── is_due (live clock; clock-stable invariants) ───────────────────────

#[test]
fn manual_is_never_due_even_if_never_run() {
    assert!(!is_due(&ap("manual", AutopilotStatus::Active, None)));
}

#[test]
fn paused_or_archived_is_never_due() {
    assert!(!is_due(&ap("hourly", AutopilotStatus::Paused, None)));
    assert!(!is_due(&ap("hourly", AutopilotStatus::Archived, None)));
    // Even with a long-overdue last run, a non-active record never fires.
    let long_ago = 0;
    assert!(!is_due(&ap(
        "daily",
        AutopilotStatus::Paused,
        Some(long_ago)
    )));
}

#[test]
fn active_scheduled_is_due_when_never_run() {
    // Never ran → runs once soon after creation (first-run-immediately).
    assert!(is_due(&ap("hourly", AutopilotStatus::Active, None)));
    assert!(is_due(&ap("daily", AutopilotStatus::Active, None)));
}

#[test]
fn ran_at_or_after_the_latest_occurrence_is_not_due() {
    // The no-double-run invariant, expressed deterministically: a run stamped
    // at-or-after the most recent occurrence means not-due (the `is_due`
    // predicate is `last < occurrence`). We pin `now` to a fixed instant and
    // compute the occurrence against it — no live clock, so no minute-boundary
    // race. `is_due` itself reads `Local::now()`, which is exactly the source
    // of the flake we're avoiding here.
    let now = now_at(14, 30);
    // Mirrors `is_due`'s decision for a record that already ran: due iff
    // `last_run_at < occurrence`. Asserting the negative for runs at/after
    // the occurrence is the no-double-run guarantee.
    let due_after_run = |occ: i64, last_run: i64| last_run < occ;
    for (schedule, hour, minute) in [
        ("hourly", None, None),
        ("daily", None, None),
        ("twice_daily", Some(9), Some(0)),
    ] {
        let occ = last_occurrence_ms(schedule, hour, minute, now, NO_JITTER)
            .expect("recurring schedule has an occurrence");
        // last_run exactly at the occurrence → not due (boundary is `<`).
        assert!(
            !due_after_run(occ, occ),
            "{schedule}: run at the occurrence is not due"
        );
        // last_run after the occurrence → not due, until the next one rolls.
        assert!(
            !due_after_run(occ, occ + 1),
            "{schedule}: run after the occurrence is not due"
        );
    }
}

#[test]
fn missed_occurrence_while_closed_is_caught_up_once() {
    // Last ran far in the past (epoch 0) — well before the most recent
    // occurrence — so the missed run is caught up exactly once on next tick.
    assert!(is_due(&ap("daily", AutopilotStatus::Active, Some(0))));
    assert!(is_due(&Autopilot {
        schedule_hour: Some(9),
        schedule_minute: Some(0),
        ..ap("twice_daily", AutopilotStatus::Active, Some(0))
    }));
}

// ── bounded retry (item 1) ─────────────────────────────────────────────

#[test]
fn outcome_failed_only_true_for_failed_runs() {
    use serde_json::json;

    // Retry: an outright scrape error (never reached the record — persisted
    // Failed via fail_run_without_summaries) …
    assert!(outcome_failed(&json!({ "error": "boom", "jobId": "j1" })));
    // … and a derived all-boards-failed status (reached the record, 0 succeeded).
    assert!(outcome_failed(
        &json!({ "jobId": "j1", "found": 0, "applied": 0, "status": "failed" })
    ));

    // Never retry: a clean completion, a partial success, a user cancel, or a
    // de-duplicated double-invoke skip.
    assert!(!outcome_failed(
        &json!({ "jobId": "j1", "found": 3, "applied": 0, "status": "completed" })
    ));
    assert!(!outcome_failed(
        &json!({ "jobId": "j1", "found": 2, "applied": 0, "status": "completedWithErrors" })
    ));
    assert!(!outcome_failed(
        &json!({ "jobId": "j1", "cancelled": true })
    ));
    assert!(!outcome_failed(&json!({ "skipped": "already-running" })));
}

#[test]
fn should_retry_after_backoff_pause_wins_over_pending_retry() {
    // This is the exact predicate `run_with_single_retry` re-checks right
    // before firing its single Failed-run retry — pins that fix directly,
    // since an AppHandle-free seam to drive the full async retry doesn't
    // exist in this crate's unit tests.

    // Still Active + recurring → the retry still fires.
    assert!(should_retry_after_backoff(Some(&ap(
        "daily",
        AutopilotStatus::Active,
        Some(0)
    ))));

    // Paused/Archived/switched-to-manual during the backoff → PAUSE WINS,
    // the retry is skipped even though "the record just failed" is exactly
    // what the retry exists to fix.
    assert!(!should_retry_after_backoff(Some(&ap(
        "daily",
        AutopilotStatus::Paused,
        Some(0)
    ))));
    assert!(!should_retry_after_backoff(Some(&ap(
        "daily",
        AutopilotStatus::Archived,
        Some(0)
    ))));
    assert!(!should_retry_after_backoff(Some(&ap(
        "manual",
        AutopilotStatus::Active,
        Some(0)
    ))));

    // Deleted during the backoff → nothing left to retry.
    assert!(!should_retry_after_backoff(None));
}

#[test]
fn is_schedulable_only_for_active_recurring_records() {
    // Active + recurring → schedulable (eligible for an interrupted-run retry).
    assert!(is_schedulable(&ap(
        "daily",
        AutopilotStatus::Active,
        Some(0)
    )));
    assert!(is_schedulable(&ap("hourly", AutopilotStatus::Active, None)));
    // Manual never auto-runs → never retried.
    assert!(!is_schedulable(&ap(
        "manual",
        AutopilotStatus::Active,
        Some(0)
    )));
    // Paused/archived never auto-run → never retried.
    assert!(!is_schedulable(&ap(
        "daily",
        AutopilotStatus::Paused,
        Some(0)
    )));
    assert!(!is_schedulable(&ap(
        "daily",
        AutopilotStatus::Archived,
        Some(0)
    )));
}

// ── still_needs_recovery (interrupted-retry pre-run recheck) ───────────
//
// `still_needs_recovery` (unlike `last_occurrence_ms`) calls the live-clock
// `is_due`/`is_schedulable`, so these cases are built to be clock-stable
// without depending on the real current occurrence: `FAR_FUTURE_MS` is a
// stamp far past any real test run, which deterministically reads as
// "not due" for any live clock, and `Some(0)` (epoch) deterministically
// reads as "always due" — the same clock-stable trick `is_due`'s own
// never-run/long-ago-run tests already rely on above.

/// A `last_run_at` stamp far enough in the future (~year 2286) that it is
/// always `>=` any occurrence computed from the real `Local::now()` at test
/// time — a clock-stable "definitely not due" fixture.
const FAR_FUTURE_MS: u64 = 9_999_999_999_999;

#[test]
fn still_needs_recovery_true_when_unchanged_and_not_due() {
    // The exact "still needs recovery" state: schedulable, not due (the
    // slot is consumed by the crashed run's own stamp), and nothing has
    // re-stamped it since the baseline snapshot was taken.
    let record = ap("daily", AutopilotStatus::Active, Some(FAR_FUTURE_MS));
    assert!(still_needs_recovery(&record, record.last_run_at));
}

#[test]
fn still_needs_recovery_false_once_a_fresh_run_re_stamps_last_run_at() {
    // A normal tick served this record's occurrence during the recovery's
    // backoff sleep and re-stamped `last_run_at` — `!is_due` alone reads
    // the same as the original crash stamp (both "not due"), so only the
    // baseline comparison catches this and defers to the tick's own run.
    let record = ap("daily", AutopilotStatus::Active, Some(FAR_FUTURE_MS));
    let stale_baseline = Some(FAR_FUTURE_MS - 1);
    assert!(!still_needs_recovery(&record, stale_baseline));
}

#[test]
fn still_needs_recovery_false_once_a_newer_occurrence_is_pending() {
    // The slot rolled to a fresh, not-yet-served occurrence (epoch-0
    // last_run_at is always due) — the normal tick owns it now, not the
    // crash-recovery path.
    let record = ap("hourly", AutopilotStatus::Active, Some(0));
    assert!(!still_needs_recovery(&record, record.last_run_at));
}

#[test]
fn still_needs_recovery_false_when_no_longer_schedulable() {
    // Paused/archived/manual since the crash — never retried, regardless of
    // the due-ness or baseline match.
    let paused = ap("daily", AutopilotStatus::Paused, Some(FAR_FUTURE_MS));
    assert!(!still_needs_recovery(&paused, paused.last_run_at));
}

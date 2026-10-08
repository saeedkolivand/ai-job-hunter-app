use super::super::hooks::{apply_stop, apply_timeout, timeout_failure_data, timeout_message};
use super::support::stage_info;
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::resume::RunLedger;
use crate::pipeline::StageOutcome;
use std::time::Duration;

/// `StageHooks::before` is the cancellation seam: a cancelled run stops at the
/// NEXT stage boundary, before paying for another provider call, and records
/// `Cancelled` so the command marks the job cancelled rather than failed.
///
/// Asserted on [`apply_stop`], which IS that decision — the emit half of
/// `before` needs an `AppHandle` this crate has no harness for, and a decision
/// only provable by reading the code is not a guard (the same seam shape as
/// `Completer::from_config`).
///
/// Mutation check: drop the `cancelled` branch and both assertions fail; drop
/// the `ledger.stop(...)` and the reason assertion does.
#[test]
fn a_cancelled_run_stops_at_the_next_stage_boundary() {
    let ledger = RunLedger::new();
    let outcome = apply_stop(
        &ledger,
        true,
        Duration::from_secs(1),
        Duration::from_secs(600),
        &stage_info(true),
    );
    assert!(outcome.is_err(), "a cancelled run must not enter the stage");
    assert_eq!(ledger.stopped(), Some(StoppedReason::Cancelled));

    // A cancel stops a FREE stage too: the deadline's exemption is about not
    // throwing away paid work, and a user who pressed Cancel asked for the run
    // to stop rendering as well as to stop spending.
    let ledger = RunLedger::new();
    assert!(apply_stop(
        &ledger,
        true,
        Duration::from_secs(1),
        Duration::from_secs(600),
        &stage_info(false)
    )
    .is_err());
    assert_eq!(ledger.stopped(), Some(StoppedReason::Cancelled));
}

/// The same seam is where the RUN DEADLINE becomes real — the only place
/// `StoppedReason::RunTimeout` is reachable. Mutation check: remove the
/// deadline branch and both assertions fail.
#[test]
fn a_run_past_its_deadline_stops_with_run_timeout() {
    let ledger = RunLedger::new();
    let outcome = apply_stop(
        &ledger,
        false,
        Duration::from_secs(2_701),
        Duration::from_secs(2_700),
        &stage_info(true),
    );
    assert!(outcome.is_err());
    assert_eq!(ledger.stopped(), Some(StoppedReason::RunTimeout));
    // …and a run still inside its deadline proceeds — otherwise the test above
    // would pass against a hook that stops every run.
    let ok = RunLedger::new();
    assert!(apply_stop(
        &ok,
        false,
        Duration::from_secs(1),
        Duration::from_secs(2_700),
        &stage_info(true)
    )
    .is_ok());
    assert_eq!(ok.stopped(), None);
}

/// **An expired deadline stops the next PAID stage, not the next stage.**
///
/// The boundary check's whole justification is that a stage boundary is where
/// stopping is free — nothing is in flight, and the next provider call has not
/// been paid for. Applied to a stage that makes NO call, that reasoning inverts
/// into its opposite: a max run whose clock ran out mid-fan-out had `assemble`
/// (pure) and `validate` (deterministic) refused, so eleven paid section
/// answers became an empty draft, no report, nothing persisted and
/// `status=failed`.
///
/// The run is still STOPPED — the reason is recorded on this path too, which is
/// what makes `terminal_state` resolve it to `needsReview`/`completed` +
/// `run_timeout` rather than to a clean finish.
///
/// Mutation check: return the error unconditionally (drop the `costs_a_call`
/// guard) and the free-stage assertion fails; skip `ledger.stop` on the free
/// path and the reason assertion does.
#[test]
fn a_zero_call_stage_still_runs_after_the_deadline_and_the_run_still_says_so() {
    let ledger = RunLedger::new();
    let outcome = apply_stop(
        &ledger,
        false,
        Duration::from_secs(2_701),
        Duration::from_secs(2_700),
        &stage_info(false),
    );
    assert!(
        outcome.is_ok(),
        "a stage that costs nothing must be allowed to turn paid work into a document"
    );
    assert_eq!(
        ledger.stopped(),
        Some(StoppedReason::RunTimeout),
        "the run is still deadline-stopped — running the free stages does not hide that"
    );
}

/// Cancellation wins over the deadline when both hold: the user asked for a
/// cancel, and "it timed out" is a worse answer to the same event (and maps to
/// a different terminal job state).
#[test]
fn cancellation_outranks_the_deadline() {
    let ledger = RunLedger::new();
    let _ = apply_stop(
        &ledger,
        true,
        Duration::from_secs(9_999),
        Duration::ZERO,
        &stage_info(true),
    );
    assert_eq!(ledger.stopped(), Some(StoppedReason::Cancelled));
}

/// The THIRD stop seam: a per-call HTTP deadline expiring INSIDE a stage's own
/// body — as opposed to `apply_stop`'s stage-BOUNDARY checks above, which never
/// let the stage run at all. `apply_timeout` is `RunHooks::after`'s pure half,
/// same shape as `apply_stop` is `before`'s.
///
/// Mutation check: drop the `if outcome.timed_out` guard in `apply_timeout` and
/// the second assertion (an ordinary failure) fails; drop `note_timeout` and
/// the detail assertion does.
#[test]
fn apply_timeout_stops_the_run_only_when_the_outcome_says_so() {
    let ledger = RunLedger::new();
    apply_timeout(
        &ledger,
        &stage_info(true),
        StageOutcome {
            ok: false,
            ms: 300_021,
            timed_out: true,
        },
    );
    assert_eq!(ledger.stopped(), Some(StoppedReason::Timeout));
    assert_eq!(ledger.timeout_detail(), Some(("repair", 300_021)));

    let untouched = RunLedger::new();
    apply_timeout(
        &untouched,
        &stage_info(true),
        StageOutcome {
            ok: false,
            ms: 50,
            timed_out: false,
        },
    );
    assert_eq!(
        untouched.stopped(),
        None,
        "a non-timeout failure must not stop the run through this seam"
    );
}

/// The actionable text `execute` hands `job_fail` once `apply_timeout` has
/// named the stage and the duration — content-free per ADR-027 (a stage name
/// and a rounded duration, never prompt or document text).
///
/// Rounds UP (301s, not 300, for 300_021ms): a `/ 1000` truncation would
/// silently drop the trailing 21ms, and for a genuinely sub-second timeout
/// that same truncation floors all the way to "0s" (see
/// [`a_sub_second_timeout_never_renders_as_0s`]).
#[test]
fn timeout_message_names_the_stage_and_rounds_the_duration_to_seconds() {
    assert_eq!(
        timeout_message("strategy", 300_021, false),
        "The \"strategy\" step didn't get a response within 301s. Try a faster model or a \
         lower effort level."
    );
}

/// **Mutation-testing hook for the fix itself**: a sub-second timeout must
/// never render as "0s" or ship `seconds: 0` — the exact defect a `/ 1000`
/// truncation produced. Neither of the two pinned tests above alone could
/// catch a regression back to truncation because 300_021ms rounds to a
/// non-zero value either way; this drives a value where truncation and
/// ceiling diverge to zero vs one.
///
/// Mutation check: swap `round_up_seconds`'s `div_ceil(1000).max(1)` back to
/// plain `/ 1000` and both assertions below fail (`0s`/`0`, not `1s`/`1`) —
/// applied and reverted.
#[test]
fn a_sub_second_timeout_never_renders_as_0s() {
    assert!(
        timeout_message("strategy", 250, false).contains("within 1s"),
        "{}",
        timeout_message("strategy", 250, false)
    );
    assert_eq!(
        timeout_failure_data("strategy", 250, false),
        serde_json::json!({ "kind": "timeout", "stage": "strategy", "seconds": 1 })
    );
}

/// **`job.failed`'s STRUCTURED payload for the same failure** — what the
/// renderer actually renders through `pipeline.timeout` now, instead of
/// [`timeout_message`]'s English sentence (that string still lands on the
/// job's own tracked `error`, never on `job.failed`'s event `data` — see
/// `fail`'s doc comment). `"kind": "timeout"` is a discriminator so a
/// consumer never has to guess a bare `{ stage, seconds }` shape apart from
/// some other job's payload.
///
/// Mutation check: drop the rounding (emit raw milliseconds instead) and the
/// `seconds` assertion below fails.
#[test]
fn timeout_failure_data_carries_the_stage_and_the_rounded_duration() {
    assert_eq!(
        timeout_failure_data("strategy", 300_021, false),
        serde_json::json!({ "kind": "timeout", "stage": "strategy", "seconds": 301 })
    );
}

/// #1393: a stage that already ran at the cheapest effort must not be told to
/// lower it. Mutation check: ignore `lowest_effort` and the assertions fail.
#[test]
fn the_timeout_hint_drops_lower_effort_when_effort_is_already_lowest() {
    let message = timeout_message("strategy", 300_000, true);
    assert!(!message.contains("lower effort"), "{message}");
    assert!(message.contains("smaller or faster model"), "{message}");
    assert_eq!(
        timeout_failure_data("strategy", 300_000, true),
        serde_json::json!({
            "kind": "timeout", "stage": "strategy", "seconds": 300, "lowestEffort": true
        })
    );
}

use super::super::stages::criticals_by_section;
use super::super::stages::sections;
use super::super::types::{JobAnalysis, SectionKey};
use super::super::RunLedger;
use super::support::{
    expired_deadline, live_deadline, repair_report, repair_revalidate, REPAIR_DRAFT,
};
use crate::pipeline::budget::StoppedReason;
use crate::validate::content::{validate_content, ContentInput, DocKind};

/// **The repair loop is not the only stage that calls a provider twice.**
///
/// `Completer::complete_json` is allowed exactly one re-ask, and it decides on
/// that second call by itself — between two `ollama_completion_deadline`-bounded
/// round trips, with no stage boundary in between. `analyze_job` and `strategy`
/// each go through it, so before this guard a run whose deadline
/// expired during the first call paid for a second one nothing would look at
/// (two stages × 300 s of it, worst case) and only THEN hit the boundary check.
///
/// Driven through the real [`complete_json_with`] seam with the real
/// `guard_deadline`, and with a deadline that is LIVE at the first charge and
/// spent by the second — the case a single expired-from-the-start deadline
/// cannot distinguish (it would refuse the first call too, and pass against a
/// guard that ran only once).
///
/// Mutation check: pass `|| Ok(())` as the guard (i.e. the pre-fix
/// `complete_json`) and `calls` becomes 2 with no recorded stop reason.
#[tokio::test]
async fn a_json_stage_does_not_pay_for_a_re_ask_after_the_deadline() {
    use crate::commands::ai_provider::Usage;

    let ledger = RunLedger::new();
    let deadline = super::super::RunDeadline::starting_now(std::time::Duration::from_millis(500));
    let mut calls = 0u32;

    let parsed: crate::error::AppResult<JobAnalysis> = crate::pipeline::complete_json_with(
        || super::super::guard_deadline(&ledger, deadline),
        |_reask| {
            calls += 1;
            async move {
                // The first call outlives the run's remaining time — the
                // ordinary slow-local-model case, not a hang.
                tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                Ok(("this is not JSON".to_string(), Usage::default()))
            }
        },
        |_usage| {},
    )
    .await;

    assert_eq!(
        calls, 1,
        "the re-ask must be refused once the run is out of time"
    );
    assert_eq!(
        ledger.stopped(),
        Some(StoppedReason::RunTimeout),
        "…and the run must say WHY it stopped, not blame the model's JSON"
    );
    let message = parsed
        .expect_err("the stage cannot produce an artifact")
        .to_string();
    assert!(
        message.contains("ran past its"),
        "the error is the run's deadline, not the parse failure: {message}"
    );
}

/// **The deadline is enforced INSIDE the loop.** `StageHooks::before` cannot
/// reach here: `repair` is the last stage, so there is no boundary after it,
/// and one round can spend four provider calls at up to
/// `OLLAMA_COMPLETION_BASELINE` each. A run past its deadline makes NO call
/// and stops with `RunTimeout`,
/// keeping whatever it already had.
///
/// Mutation check: delete the `deadline.passed()` check at the top of the loop
/// and `calls` becomes 1 while `timed_out` becomes false — which is exactly the
/// ~2400 s overrun the run deadline was silently allowing.
#[tokio::test]
async fn the_repair_loop_stops_at_the_run_deadline_without_paying_for_a_call() {
    let mut calls = 0u32;
    let (document, _report, _letter, stats) = super::super::stages::repair_loop(
        REPAIR_DRAFT.to_string(),
        repair_report(REPAIR_DRAFT),
        None,
        2,
        expired_deadline(),
        |_key, _document, _issues| {
            calls += 1;
            async move { Ok(super::super::stages::SectionOutcome::Replaced(String::new())) }
        },
        |_: &str| None,
        repair_revalidate,
    )
    .await
    .expect("re-validation ran");

    assert_eq!(calls, 0, "a run out of time must not pay for another call");
    assert!(stats.timed_out, "…and must say WHY it stopped");
    assert_eq!(stats.rounds, 0);
    assert_eq!(
        document, REPAIR_DRAFT,
        "the document produced so far is kept, never discarded"
    );
}

/// **The deadline is checked between the round's own CALLS, not only between
/// rounds.** One round can spend `MAX_SECTIONS_PER_ROUND` (4) calls at up to
/// `OLLAMA_COMPLETION_BASELINE` (300 s) each: a round-granular check lets a run overrun
/// its deadline by ~20 minutes, which is most of the gap the whole AH2 finding
/// is about.
///
/// The deadline here is LIVE when the round starts and expires inside the first
/// call, so only the per-section check can catch it.
///
/// Mutation check: this is the one the between-ROUNDS check cannot cover —
/// delete the per-section `deadline.passed()` and `calls` becomes 2 while
/// `timed_out` stays false. (Verified: with only the round-level check present,
/// this test fails and `..._without_paying_for_a_call` still passes.)
#[tokio::test]
async fn the_repair_loop_stops_between_section_calls_not_only_between_rounds() {
    // Two failing sections, so there IS a second call for the check to refuse.
    let source = "Jane Doe\n\nPROFESSIONAL SUMMARY\nA payments engineer.\n\nSKILLS\nGo, Rust\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";
    let draft = "PROFESSIONAL SUMMARY\nA payments engineer who cut costs by 47% and grew revenue by 220%.\n\nSKILLS\nGo, Rust, Kubernetes across 370 clusters\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";
    let report = validate_content(&ContentInput {
        generated: draft,
        source_resume: source,
        job_ad: "We need a payments engineer.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    assert!(
        criticals_by_section(draft, &report).len() >= 2,
        "the premise: the round has more than one section to work through"
    );

    let mut calls = 0u32;
    let (document, _report, _letter, stats) = super::super::stages::repair_loop(
        draft.to_string(),
        report,
        None,
        2,
        // Live now, spent by the time the first call returns. Half a second,
        // not the 40 ms this first had: the deadline has to survive the
        // grouping + validation that run before the first call, and a loaded CI
        // box can spend longer than 40 ms there — which would assert
        // `calls == 1` against a loop that made ZERO. Both margins are
        // one-sided: a slower machine only makes the first call finish further
        // PAST the deadline, never before it.
        super::super::RunDeadline::starting_now(std::time::Duration::from_millis(500)),
        |key, document, _issues| {
            calls += 1;
            let clean = match key {
                SectionKey::Summary => "PROFESSIONAL SUMMARY\nA payments engineer.",
                _ => "SKILLS\nGo, Rust",
            };
            let split = sections::split(&document);
            let section = sections::find(&split, key).expect("the section exists");
            let spliced = sections::splice(&document, section, clean);
            async move {
                // The overrun a round-granular check cannot see.
                tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                Ok(super::super::stages::SectionOutcome::Replaced(spliced))
            }
        },
        |_: &str| None,
        |candidate| {
            let candidate = candidate.clone();
            async move {
                Ok((
                    validate_content(&ContentInput {
                        generated: &candidate,
                        source_resume: source,
                        job_ad: "We need a payments engineer.",
                        top_requirements: &[],
                        target_language: "en",
                        doc_kind: DocKind::Resume,
                    }),
                    None,
                ))
            }
        },
    )
    .await
    .expect("re-validation ran");

    assert_eq!(
        calls, 1,
        "the second section must not be paid for once the run is out of time"
    );
    assert!(stats.timed_out);
    assert_eq!(stats.rounds, 1);
    assert!(
        !document.contains("47%"),
        "the work the round DID finish is kept — the deadline ends the loop, it does not \
         discard progress"
    );
}

/// **A per-section provider error is a failed attempt, not a failed run**, and
/// **a daily-cap refusal is `Budgeted`** — the two halves of "the terminal state
/// must not lie". A `?` here used to throw away a document the run had already
/// produced, which is the opposite of what every `StoppedReason` promises.
///
/// Mutation check: restore the `?` on `regenerate_one_section` and the first
/// case returns `Err` (no document at all); treat `RateLimited` as an ordinary
/// error and `budgeted` stays false.
#[tokio::test]
async fn the_repair_loop_survives_a_section_error_and_stops_on_the_daily_cap() {
    let (document, _report, _letter, stats) = super::super::stages::repair_loop(
        REPAIR_DRAFT.to_string(),
        repair_report(REPAIR_DRAFT),
        None,
        2,
        live_deadline(),
        |_key, _document, _issues| async move {
            Err(crate::error::AppError::Provider(
                "the model fell over".to_string(),
            ))
        },
        |_: &str| None,
        repair_revalidate,
    )
    .await
    .expect("a provider error must not fail the stage");
    assert_eq!(stats.failed, 1, "counted as a failed attempt");
    assert!(!stats.budgeted);
    assert_eq!(
        document, REPAIR_DRAFT,
        "the run keeps the document it already had"
    );

    let (document, _report, _letter, stats) = super::super::stages::repair_loop(
        REPAIR_DRAFT.to_string(),
        repair_report(REPAIR_DRAFT),
        None,
        2,
        live_deadline(),
        |_key, _document, _issues| async move {
            Err(crate::error::AppError::RateLimited(
                "daily provider ceiling reached".to_string(),
            ))
        },
        |_: &str| None,
        repair_revalidate,
    )
    .await
    .expect("a budget refusal must not fail the stage either");
    assert!(
        stats.budgeted,
        "the day's cap has its own StoppedReason precisely so this is not reported as a failure"
    );
    assert_eq!(stats.failed, 0, "a refusal is not a failed attempt");
    assert_eq!(document, REPAIR_DRAFT);
}

/// **A section the document does not have costs NO provider round-trip, and is
/// not counted as one.**
///
/// `regenerate_one_section` used to fold "no such section" and "the model
/// answered unusably" into one `Ok(None)`, and the loop counted a call for
/// both — so every run whose validator named a section the split could not
/// resolve over-reported its own provider spend in the metrics the user (and
/// the cost accounting) reads. Three outcomes, not two.
///
/// Mutation check: count a call for `Missing` and `stats.calls` becomes 1.
#[tokio::test]
async fn a_missing_section_is_not_counted_as_a_provider_call() {
    let (document, _report, _letter, stats) = super::super::stages::repair_loop(
        REPAIR_DRAFT.to_string(),
        repair_report(REPAIR_DRAFT),
        None,
        2,
        live_deadline(),
        |_key, _document, _issues| async move { Ok(super::super::stages::SectionOutcome::Missing) },
        |_: &str| None,
        repair_revalidate,
    )
    .await
    .expect("re-validation ran");

    assert_eq!(stats.calls, 0, "no provider was asked anything");
    assert_eq!(
        stats.truncated, 0,
        "…and it is not a truncated answer either"
    );
    assert_eq!(stats.failed, 0, "…nor an error");
    assert_eq!(
        stats.rounds, 1,
        "the round happened; it just achieved nothing"
    );
    assert_eq!(document, REPAIR_DRAFT);
}

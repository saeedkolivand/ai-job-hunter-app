use super::super::stages::validate_documents;
use crate::validate::content::{validate_content, ContentInput, DocKind};
use crate::validate::Severity;

/// **A résumé that was never written is never graded as one.**
///
/// `Validate::run` hands `validate_documents` `ctx.draft`, which a
/// cover-letter-only run leaves EMPTY on purpose (`Draft::run` no-ops on
/// `include_resume: false`). Under `DocKind::Resume` an empty document against
/// a real source résumé is not "a clean report with a couple of findings" — it
/// is a `factual.dropped_role` Critical per employer. Keeping that report would
/// park the run at `needsReview`, and (before `save_verdict` was scoped) refuse
/// the save of the letter the run actually produced.
///
/// The first half is the ABSOLUTE this hangs on: it proves, rather than
/// assumes, that this exact source/document pair really does produce a blocking
/// résumé report. Without it the second half would pass just as happily against
/// a validator that had stopped finding anything at all.
///
/// Mutation check: drop the `ctx.input.include_resume.then_some(report)` in
/// `Validate::run` and `report_for_a_run_with_no_resume` becomes `Some` with
/// those Criticals in it.
#[tokio::test]
async fn a_resume_that_was_never_written_is_never_graded_as_one() {
    const SOURCE: &str = "Jane Doe\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";
    const JOB_AD: &str = "We need a payments engineer.";
    const LETTER: &str = "Dear hiring team,\n\nI have run settlement systems for four years.\n";

    // The control: an EMPTY document graded under the résumé ruleset really is
    // blocking, so "we skipped it" is a decision with consequences, not a no-op.
    let graded_anyway = validate_content(&ContentInput {
        generated: "",
        source_resume: SOURCE,
        job_ad: JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    assert!(
        graded_anyway
            .issues
            .iter()
            .any(|issue| issue.severity == Severity::Critical),
        "an absent résumé graded as a résumé is Critical — that is exactly what \
         must not reach a cover-letter-only run's report"
    );

    // What the stage actually keeps for the two shapes of run. `validate_documents`
    // is what `Validate::run` calls; the `include_resume` decision is applied to
    // its FIRST return value, so both halves are asserted here on real reports.
    let (resume_report, letter_report) = validate_documents(
        String::new(),
        SOURCE.to_string(),
        JOB_AD.to_string(),
        vec![],
        "en".to_string(),
        LETTER.to_string(),
    )
    .await
    .expect("validation runs");

    // The résumé half really does come back carrying those Criticals — so
    // dropping it is a decision the stage has to make, not something
    // `validate_documents` does on its own.
    assert!(
        resume_report
            .issues
            .iter()
            .any(|issue| issue.severity == Severity::Critical),
        "validate_documents grades whatever it is given — the stage is what          decides a cover-letter-only run keeps none of it"
    );
    // …and the letter it DID write is graded. Not a rewording: gating the wrong
    // half would leave the letter ungraded and every downstream letter check
    // silently vacuous.
    assert!(
        letter_report.is_some(),
        "the letter this run generated is still validated under its own ruleset"
    );

    // And THAT decision, pinned where it is actually made. A source pin for the
    // reason `humanize_does_not_key_the_whole_stage_on_a_resume_report` states:
    // `Validate::run` needs a `QualityCtx`, which needs a `Completer` and an
    // `AppHandle`. Re-deriving the gate here with a literal `false.then_some(…)`
    // instead would assert nothing about the stage — it would pass unchanged
    // against a build that had dropped the gate entirely.
    const SRC: &str = include_str!("../stages/validate.rs");
    let start = SRC
        .find("for Validate {")
        .expect("the Validate stage impl must still exist");
    assert!(
        SRC[start..].contains("ctx.report = ctx.input.include_resume.then_some(report);"),
        "the résumé report must be dropped for a run that wrote no résumé — an          unconditional `Some(report)` hands a cover-letter-only run one Critical          per source employer, about a document that does not exist"
    );
}

/// **HIGH — the humanize stage must not key itself on the RÉSUMÉ's report.**
///
/// `Humanize::run` grades TWO documents, and it is the only stage that rewrites
/// the letter's `voice.*` lines. Its head used to be
/// `let Some(resume_report) = ctx.report.clone() else { … return }` — a résumé
/// report as the precondition for the WHOLE stage, with `letter_flagged`
/// computed below it. A cover-letter-only run has no résumé report by design,
/// so that shape returned before the letter arm was ever reached: the polish
/// pass silently skipped, and the ledger recording an all-zero artifact that
/// reads as "nothing was flagged".
///
/// Nothing else in the suite catches it —
/// `should_humanize_letter_refuses_whenever_the_run_never_requested_a_letter`
/// tests the letter PREDICATE, never whether the caller reaches it.
///
/// A source pin: driving the stage body needs a `Completer`, which needs an
/// `AppHandle`, which this crate's tests cannot build. Both assertions are
/// about the shape that made the failure silent.
///
/// Mutation check: restore the `let Some(resume_report) = ctx.report.clone()
/// else {` head and the first assertion fails.
#[test]
fn humanize_does_not_key_the_whole_stage_on_a_resume_report() {
    const SRC: &str = include_str!("../stages/humanize.rs");
    let start = SRC
        .find("for Humanize {")
        .expect("the Humanize stage impl must still exist");
    let body = &SRC[start..];

    assert!(
        !body.contains("let Some(resume_report) = ctx.report.clone() else {"),
        "keying the stage on the résumé's report skips the LETTER arm entirely \
         for a cover-letter-only run — and records a zero-flag artifact for a \
         letter nothing polished"
    );
    assert!(
        body.contains("if resume_report.is_none() && ctx.letter_report.is_none() {"),
        "the early return must ask about BOTH documents — that, and only that, \
         is the 'validate did not run' case the original guard existed for"
    );
    // The résumé arm keeps its own gate; the letter arm keeps its own predicate.
    assert!(
        body.contains("should_humanize_letter(letter_flagged"),
        "the letter arm must still route through its own predicate"
    );
}

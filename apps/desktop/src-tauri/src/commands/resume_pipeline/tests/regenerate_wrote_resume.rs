use crate::pipeline::runs::RunRow;

/// **A cover-letter-only run has no section to regenerate — and the check that
/// used to answer that question was a PROXY.**
///
/// `resume_pipeline_regenerate_section` asked two things: is this the posting's
/// newest run (`ensure_latest_run`), and does the aggregate hold a non-empty
/// `resume_text`. Both are satisfied by a cover-letter-only run — it IS the
/// newest, and the aggregate still carries whatever an EARLIER run saved — so
/// the command would splice, re-validate and persist a section of a résumé this
/// run never wrote, spending a provider call and overwriting the posting's
/// report on the way. Raised in review on #1078; introduced by that PR, because
/// before it every run wrote its own `ctx.draft` and the proxy was sound.
///
/// **Absent must read as `true`.** No migration touches the run rows that
/// predate `includeResume`, and every one of them wrote a résumé; reading a
/// missing key as `false` would refuse a section regenerate on the entire
/// existing store. The `"{}"` case below is that whole population.
///
/// Mutation check: change `run_wrote_a_resume`'s `unwrap_or(true)` to
/// `unwrap_or(false)` and the legacy/`"{}"` assertions fail; drop the
/// `resumeInRun` read entirely and the cover-only assertion fails.
#[test]
fn a_run_that_wrote_no_resume_is_not_a_run_you_can_regenerate_a_section_of() {
    let row = |metrics: &str| RunRow {
        id: "run-1".to_string(),
        job_url: "https://boards.example/jobs/42".to_string(),
        kind: super::super::RUN_KIND.to_string(),
        depth: "quality".to_string(),
        status: "completed".to_string(),
        started_at: 0,
        finished_at: None,
        stopped_reason: None,
        metrics_json: metrics.to_string(),
    };

    // The run this PR introduced: it said so itself, in its own metrics.
    assert!(!super::super::run_wrote_a_resume(&row(
        r#"{"calls":4,"resumeInRun":false}"#
    )));

    // A résumé-bearing run of the same shape.
    assert!(super::super::run_wrote_a_resume(&row(
        r#"{"calls":5,"resumeInRun":true}"#
    )));

    // Every run that predates the flag — the key is simply not there. This is
    // the assertion that stops the guard from locking out the existing store.
    assert!(super::super::run_wrote_a_resume(&row("{}")));
    assert!(super::super::run_wrote_a_resume(&row(
        r#"{"calls":5,"repairRounds":1}"#
    )));

    // Unparseable metrics are not evidence of absence either — same direction.
    assert!(super::super::run_wrote_a_resume(&row("not json")));
    // …and a non-bool under the key is ignored rather than coerced.
    assert!(super::super::run_wrote_a_resume(&row(
        r#"{"resumeInRun":"false"}"#
    )));
}

/// The flag the guard above reads is actually WRITTEN, and written from the
/// run's own input rather than hardcoded.
///
/// Without this, `run_wrote_a_resume` would be a correct function nothing ever
/// feeds — every run would read as résumé-bearing and the guard would be dead.
/// A source pin for the usual reason (`execute` needs an `AppHandle`), narrow
/// to the one expression that matters.
///
/// Mutation check: change the inserted value to a literal `json!(true)` and the
/// second assertion fails.
#[test]
fn execute_records_whether_the_run_wrote_a_resume() {
    const SRC: &str = include_str!("../run.rs");
    let body = SRC.split_whitespace().collect::<Vec<_>>().join(" ");

    // No trailing commas in either pattern: rustfmt owns whether this call is
    // wrapped or collapsed, and it re-wrapped it once already. The property is
    // the key and the expression, not the layout.
    assert!(
        body.contains(r#""resumeInRun".to_string()"#),
        "execute must persist whether the run wrote a résumé — the guard in \
         regenerate_section has nothing to read otherwise"
    );
    assert!(
        body.contains("json!(ctx.input.include_resume)"),
        "the recorded value must come from the run's own input, not a literal"
    );
}

/// **…and `regenerate_section` actually CALLS it, before it touches the
/// aggregate.**
///
/// Caught by mutating the two tests above: deleting the guard's CALL SITE left
/// both of them green. They pin `run_wrote_a_resume` and the metrics write —
/// neither says the command consults either one, so the whole guard could be
/// removed without a single failure. A function nobody calls is not a guard.
///
/// Ordering is part of the property, not decoration: the refusal has to happen
/// before `find_for_job`, or a cover-letter-only run is rejected by the
/// EMPTINESS filter's message ("this run has no saved résumé to regenerate a
/// section of") whenever the posting happens to have no earlier résumé, and
/// accepted outright whenever it does — the exact confusion the guard exists to
/// remove.
///
/// A source pin for the same reason as the sibling pins in this file: the
/// command needs an `AppHandle`.
///
/// Mutation check: delete the `if !run_wrote_a_resume(&row)` block and the
/// first assertion fails; move it below the `find_for_job` lookup and the
/// ordering assertion fails.
#[test]
fn regenerate_section_refuses_a_run_that_wrote_no_resume_before_reading_the_aggregate() {
    const SRC: &str = include_str!("../regenerate.rs");
    let start = SRC
        .find("pub async fn resume_pipeline_regenerate_section(")
        .expect("the command must still exist under this signature");
    let body = &SRC[start..];

    let guard = body
        .find("if !run_wrote_a_resume(&row) {")
        .expect("regenerate_section must refuse a run that wrote no résumé");
    let aggregate = body
        .find("find_for_job(&row.job_url)")
        .expect("regenerate_section must still look the aggregate up");

    assert!(
        guard < aggregate,
        "the refusal must come BEFORE the aggregate lookup — otherwise which          message a cover-letter-only run gets depends on whether the posting          happens to carry an older résumé"
    );
}

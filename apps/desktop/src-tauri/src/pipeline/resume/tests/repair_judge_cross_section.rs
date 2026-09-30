use super::super::stages::round_is_worse;
use super::support::{criticals, ANY_TEXT};
use crate::validate::content::{
    ContentIssue, ContentMetrics, ContentReport, CONSISTENCY_SKILL_NOT_DEMONSTRATED,
    DUPLICATE_BULLET,
};
use crate::validate::Severity;

/// `n` Warnings of `code` — the cross-section term's COUNT half. Warnings
/// never flip `ok` (only a Critical does), so this is `ok: true` unlike
/// [`criticals`] above.
fn cross_section_warnings(code: &'static str, count: usize) -> ContentReport {
    ContentReport {
        ok: true,
        issues: (0..count)
            .map(|n| ContentIssue {
                severity: Severity::Warning,
                code,
                section: Some("Skills".to_string()),
                message: "a cross-section warning".to_string(),
                evidence: Some(format!("token-{n}")),
            })
            .collect(),
        metrics: ContentMetrics::default(),
    }
}

/// **A round that doubles the document is worse — the audit's own measured
/// regression.** A whole-document duplication produced 5 `duplicate.bullet`
/// warnings at `duplicateRatio = 1.00` on a document that carried none
/// before, and shipped because `repair` only ever read Criticals.
///
/// Mutation check: comment out the `code_grew(before, after, DUPLICATE_BULLET)`
/// term in `round_is_worse` and this goes red (confirmed); restored, it is
/// green (confirmed) — see the module-level report for the exact output.
#[test]
fn a_round_that_doubles_the_document_is_worse() {
    let before = cross_section_warnings(DUPLICATE_BULLET, 0);
    let after = cross_section_warnings(DUPLICATE_BULLET, 5);
    assert!(
        round_is_worse(&before, ANY_TEXT, &after, ANY_TEXT),
        "5 new duplicate.bullet warnings on a document that had none is worse"
    );
}

/// **Confirmation-review finding 5.** The test above only exercises
/// `duplicate.bullet` growth with ZERO Criticals on both sides, so it passed
/// even while `duplicate.bullet` was (wrongly) gated the same way as
/// `consistency.skill_not_demonstrated` — `criticals_after >= criticals_before`
/// reads `0 >= 0` as open regardless. This is the shape that gate would have
/// hidden: a round that FIXES Criticals while ALSO doubling the document.
/// `duplicate.bullet` must still revert it — it is the one signal that would
/// otherwise see nothing wrong with a round that halved the Critical count by
/// duplicating half the résumé underneath the fix.
///
/// Mutation check: gate the `DUPLICATE_BULLET` term behind
/// `criticals_after >= criticals_before` (the same gate
/// `CONSISTENCY_SKILL_NOT_DEMONSTRATED` uses) and this goes red.
#[test]
fn a_doubling_round_is_worse_even_when_it_also_fixed_a_critical() {
    let mut before = criticals(3);
    before
        .issues
        .extend(cross_section_warnings(DUPLICATE_BULLET, 0).issues);
    let mut after = criticals(0);
    after
        .issues
        .extend(cross_section_warnings(DUPLICATE_BULLET, 5).issues);
    assert!(
        round_is_worse(&before, ANY_TEXT, &after, ANY_TEXT),
        "duplicate.bullet growth must revert a round even though it also \
         fixed every Critical"
    );
}

/// **The baseline false positive must NOT revert.** Rewording two Experience
/// bullets to drop one exact shared token — an ordinary section rewrite — was
/// measured to raise `consistency.skill_not_demonstrated` from zero to four
/// on an otherwise truthful document. A document that already carries four
/// before the round and still carries four after must be repairable, or
/// every one of that document's future rounds would be blocked by noise it
/// never introduced.
#[test]
fn a_baseline_cross_section_warning_that_is_merely_carried_does_not_revert() {
    let before = cross_section_warnings(CONSISTENCY_SKILL_NOT_DEMONSTRATED, 4);
    let after = cross_section_warnings(CONSISTENCY_SKILL_NOT_DEMONSTRATED, 4);
    assert!(
        !round_is_worse(&before, ANY_TEXT, &after, ANY_TEXT),
        "carrying the SAME count of a pre-existing warning is not a regression"
    );
}

/// **A round that genuinely GROWS cross-section incoherence is worse.** Four
/// before, nine after — the shape the task names explicitly, and the mirror
/// of the test above: same code, only the direction of the delta differs.
#[test]
fn a_round_that_grows_a_cross_section_warning_is_worse() {
    let before = cross_section_warnings(CONSISTENCY_SKILL_NOT_DEMONSTRATED, 4);
    let after = cross_section_warnings(CONSISTENCY_SKILL_NOT_DEMONSTRATED, 9);
    assert!(
        round_is_worse(&before, ANY_TEXT, &after, ANY_TEXT),
        "growing four warnings to nine is a regression this round introduced"
    );
}

/// **Per-code, not summed — a fix in one code must not hide a regression in
/// the other.** Two `duplicate.bullet` warnings fixed, two new
/// `consistency.skill_not_demonstrated` ones introduced elsewhere: a combined
/// total nets to zero (4 before, 4 after) and would read as "not worse" —
/// exactly the shape `round_is_worse`'s own module doc already warns a bare
/// count can hide a loss behind an unrelated improvement.
///
/// Mutation check: replace the two separate `code_grew` calls in
/// `round_is_worse` with a single summed-total comparison and this goes red
/// (confirmed); restored to per-code, it is green (confirmed).
#[test]
fn a_cross_section_regression_in_one_code_reverts_even_when_the_other_code_improves() {
    let mut before = cross_section_warnings(DUPLICATE_BULLET, 2);
    before
        .issues
        .extend(cross_section_warnings(CONSISTENCY_SKILL_NOT_DEMONSTRATED, 2).issues);
    let mut after = cross_section_warnings(DUPLICATE_BULLET, 0);
    after
        .issues
        .extend(cross_section_warnings(CONSISTENCY_SKILL_NOT_DEMONSTRATED, 4).issues);
    assert!(
        round_is_worse(&before, ANY_TEXT, &after, ANY_TEXT),
        "a per-code regression must not be hidden behind an unrelated fix in the other code"
    );
}

/// **A `consistency.skill_not_demonstrated` Warning must not veto a round
/// that fixed every Critical.** The bug this closes: a round taking
/// Criticals from five to zero while an ordinary Experience bullet reword
/// also nudged `consistency.skill_not_demonstrated` up by one — exactly the
/// "ordinary rewrite noise" the module's own baseline-false-positive doc
/// already names — used to revert regardless, discarding a genuine fix and
/// burning the loop's second budgeted round for nothing.
///
/// Mutation check: drop the `criticals_after >= criticals_before` gate from
/// the `CONSISTENCY_SKILL_NOT_DEMONSTRATED` term in `round_is_worse` (running
/// `code_grew` for it unconditionally) and this goes red.
#[test]
fn a_cross_section_warning_does_not_veto_a_round_that_fixed_every_critical() {
    let before = criticals(5);
    let mut after = criticals(0);
    after.issues.push(ContentIssue {
        severity: Severity::Warning,
        code: CONSISTENCY_SKILL_NOT_DEMONSTRATED,
        section: Some("Skills".to_string()),
        message: "a cross-section warning".to_string(),
        evidence: Some("token-0".to_string()),
    });
    assert!(
        !round_is_worse(&before, ANY_TEXT, &after, ANY_TEXT),
        "fixing every Critical must not be discarded for one new cross-section Warning"
    );
}

/// **An absence-shaped Critical with NO evidence is skipped, deliberately.**
///
/// `absences` keys on the `(code, evidence)` PAIR, so an issue without evidence
/// has no pair and is dropped. That is what keeps a pre-existing absence
/// carryable instead of a permanent block — but nothing exercised the branch,
/// so turning the `?` into a default pair (making every evidence-less
/// `factual.dropped_role` block repair forever) would have kept this file
/// green. Both real emitters always carry evidence, which
/// `a_repair_rewrite_that_drops_a_seeded_employer_raises_a_dropped_role_critical`
/// pins against the actual validator; this pins what happens if one ever stops.
///
/// Mutation check: default the missing evidence to `""` instead of skipping and
/// the improving round below is refused.
#[test]
fn an_absence_with_no_evidence_cannot_block_a_repair_round() {
    let evidenceless = |severity| crate::validate::content::ContentIssue {
        severity,
        code: crate::validate::content::FACTUAL_DROPPED_ROLE,
        section: None,
        message: "an employer went missing".to_string(),
        evidence: None,
    };

    let mut before = criticals(3);
    before
        .issues
        .push(evidenceless(crate::validate::Severity::Critical));
    let mut after = criticals(1);
    after
        .issues
        .push(evidenceless(crate::validate::Severity::Critical));

    assert!(
        !round_is_worse(&before, ANY_TEXT, &after, ANY_TEXT),
        "an issue with no evidence has no pair to compare, so it cannot be a NEW absence"
    );
    // …and it does not become one by appearing for the first time either.
    let mut appeared = criticals(1);
    appeared
        .issues
        .push(evidenceless(crate::validate::Severity::Critical));
    assert!(!round_is_worse(
        &criticals(3),
        ANY_TEXT,
        &appeared,
        ANY_TEXT
    ));
    // The count term still governs it: three criticals becoming five is worse.
    assert!(round_is_worse(
        &criticals(3),
        ANY_TEXT,
        &criticals(5),
        ANY_TEXT
    ));
}

/// `factual.altered_project_link` is emitted from TWO arms and only one of them
/// is an absence — the same split `commands::resume_pipeline::report` makes to
/// decide whether the finding is reviewable at all.
///
/// A link the model INVENTED is IN the generated text: a fabrication, caught by
/// the count like any other, and a round that produces one while removing two
/// others is a legitimate improvement. A SOURCE link that the output no longer
/// carries is a LOSS, and the discriminator is whether the evidence is present
/// in the document.
///
/// Mutation check: treat every `altered_project_link` as an absence (drop the
/// `!text.contains` test) and the invented-link case starts reverting; treat
/// none of them as one and the lost-link case stops.
#[test]
fn only_the_absence_arm_of_an_altered_project_link_makes_a_round_worse() {
    let link_issue = |url: &str| crate::validate::content::ContentIssue {
        severity: crate::validate::Severity::Critical,
        code: crate::validate::content::FACTUAL_ALTERED_PROJECT_LINK,
        section: None,
        message: "a project link does not match the source".to_string(),
        evidence: Some(url.to_string()),
    };
    let with = |count: usize, url: &str| {
        let mut report = criticals(count);
        report.issues.push(link_issue(url));
        report.ok = false;
        report
    };

    const SOURCE_LINK: &str = "https://github.com/janedoe/ledger";
    const INVENTED: &str = "https://github.com/acme/ledger";
    let document_with = |url: &str| format!("Projects\n\n**Ledger CLI** · {url}\n");

    // The source link is GONE from the candidate: its evidence is not in the
    // text, so this is a loss — reverted even though criticals went 2 → 1.
    assert!(
        round_is_worse(
            &criticals(2),
            &document_with(SOURCE_LINK),
            &with(0, SOURCE_LINK),
            "Projects\n\n**Ledger CLI**\n"
        ),
        "a source project link the output no longer carries is an absence"
    );

    // The model INVENTED a link: the evidence is right there in the candidate,
    // so it is an ordinary fabrication and the count decides — 2 → 1 stands.
    assert!(
        !round_is_worse(
            &criticals(2),
            &document_with(SOURCE_LINK),
            &with(0, INVENTED),
            &document_with(INVENTED)
        ),
        "an invented link is IN the document, so it is a fabrication, not a loss"
    );
}

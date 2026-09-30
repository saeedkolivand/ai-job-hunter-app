use super::super::stages::{criticals_by_section, round_is_worse};
use super::super::types::SectionKey;
use super::support::{criticals, ANY_TEXT};
use crate::validate::content::{
    validate_content, ContentInput, ContentIssue, ContentMetrics, ContentReport, DocKind,
    CONTENT_LANGUAGE_MISMATCH,
};
use crate::validate::Severity;

/// The repair loop's input, end to end: a real `validate_content` report over a
/// draft that fabricates a metric, grouped into the section that has to be
/// regenerated.
///
/// An integration test rather than a hand-built report on purpose — the mapping
/// from a validator's `section` LABEL back to a `SectionKey` is where the two
/// halves can silently disagree, and a synthetic report would pin my own
/// assumption about the label instead of what the validator actually emits.
///
/// Mutation check: make `criticals_by_section` include Warnings and the
/// "criticals only" assertion fails; drop the `key_for_label` mapping and the
/// group disappears.
#[test]
fn repair_groups_only_criticals_and_only_ones_it_can_regenerate() {
    let source = "Jane Doe\n\nPROFESSIONAL SUMMARY\nA payments engineer.\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";
    // The summary invents a figure the source never states — a deterministic
    // `factual.unsourced_metric` Critical, attributed to the summary section.
    let generated = "PROFESSIONAL SUMMARY\nA payments engineer who cut costs by 47% across 12 teams.\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";

    let report = validate_content(&ContentInput {
        generated,
        source_resume: source,
        job_ad: "We need a payments engineer with ledger experience.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.code == crate::validate::content::FACTUAL_UNSOURCED_METRIC),
        "fixture must produce the fabricated-metric Critical; got {:?}",
        report.issues.iter().map(|i| i.code).collect::<Vec<_>>()
    );

    // The metric check reports `section: None` by design (it compares number
    // SETS, not sections), so the grouping has to locate the section from the
    // offending span. Mutation check: delete the `sections::containing`
    // fallback in `criticals_by_section` and this is empty.
    assert!(
        report.issues.iter().any(
            |i| i.code == crate::validate::content::FACTUAL_UNSOURCED_METRIC && i.section.is_none()
        ),
        "the fallback's premise: this Critical carries no section label"
    );

    let grouped = criticals_by_section(generated, &report);
    assert!(
        grouped
            .iter()
            .any(|(key, _)| *key == SectionKey::Summary.to_wire()),
        "the fabricated metric's section must be regenerable; got {:?}",
        grouped.iter().map(|(key, _)| key).collect::<Vec<_>>()
    );
    // Only criticals: a report full of warnings must not schedule a rewrite.
    let warning_only = validate_content(&ContentInput {
        generated: source,
        source_resume: source,
        job_ad: "We need Kubernetes, Terraform and Kafka.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    assert!(
        !warning_only.issues.is_empty(),
        "fixture must produce warnings, or the assertion below is vacuous"
    );
    assert!(
        criticals_by_section(source, &warning_only).is_empty(),
        "warnings must not schedule a repair round"
    );
}

/// A document-wide `content.language_mismatch` Critical (`section: None`) must
/// never schedule a repair round. Its evidence used to be the bare
/// target-language code (`d32f755c` now sets it to `None` as belt), but this
/// test constructs the issue with `evidence: Some("de")` ANYWAY — proving the
/// ROUTING rule, not the emission accident, so a later change that restores
/// evidence on this Critical cannot silently re-open the mis-route.
///
/// The fixture's own summary line contains the word "developer" — a plain
/// substring match on "de" — so this also pins that the routing skip fires
/// BEFORE `sections::containing`'s substring fallback ever gets a chance to
/// (mis-)match it.
///
/// Mutation check: remove the `CONTENT_LANGUAGE_MISMATCH` arm in
/// `criticals_by_section` — RAN, went red (the Critical routed to `summary`
/// via the substring fallback), reverted.
#[test]
fn a_document_wide_language_critical_routes_to_no_section() {
    let document = "PROFESSIONAL SUMMARY\nA senior developer leading platform teams.\n\n\
                     WORK EXPERIENCE\n\nAcme Corp | Staff Engineer | 2021 - Present\n\
                     - Built the ledger service\n";
    assert!(
        document.contains("de"),
        "fixture premise: the document must contain the substring the old routing bug matched on"
    );

    let report = ContentReport {
        ok: false,
        issues: vec![ContentIssue {
            severity: Severity::Critical,
            code: CONTENT_LANGUAGE_MISMATCH,
            section: None,
            message: "the document is not in the requested language".to_string(),
            evidence: Some("de".to_string()),
        }],
        metrics: ContentMetrics::default(),
    };

    assert!(
        criticals_by_section(document, &report).is_empty(),
        "a document-wide language Critical must not schedule any section for repair"
    );
}

/// Sibling of the test above: a PER-SECTION `content.language_mismatch`
/// Critical (it carries a `section` label) must still route normally — the
/// fix must not be over-broad and swallow every language finding.
///
/// Mutation check: make the skip in `criticals_by_section` unconditional on
/// the code alone (drop the `issue.section.is_none()` half) — RAN, went red
/// (this Critical stopped scheduling `summary`), reverted.
#[test]
fn a_per_section_language_critical_still_routes() {
    let document = "PROFESSIONAL SUMMARY\nA senior developer leading platform teams.\n\n\
                     WORK EXPERIENCE\n\nAcme Corp | Staff Engineer | 2021 - Present\n\
                     - Built the ledger service\n";
    let report = ContentReport {
        ok: false,
        issues: vec![ContentIssue {
            severity: Severity::Critical,
            code: CONTENT_LANGUAGE_MISMATCH,
            section: Some("PROFESSIONAL SUMMARY".to_string()),
            message: "this section drifted into a different language".to_string(),
            evidence: Some("developer".to_string()),
        }],
        metrics: ContentMetrics::default(),
    };

    let grouped = criticals_by_section(document, &report);
    assert!(
        grouped
            .iter()
            .any(|(key, _)| *key == SectionKey::Summary.to_wire()),
        "a per-section language Critical must still schedule its section for repair; got {:?}",
        grouped.iter().map(|(key, _)| key).collect::<Vec<_>>()
    );
}

/// The same, plus one ABSENCE-shaped Critical naming `company`.
fn criticals_missing(count: usize, company: &str) -> crate::validate::content::ContentReport {
    let mut report = criticals(count);
    report.issues.push(crate::validate::content::ContentIssue {
        severity: crate::validate::Severity::Critical,
        code: crate::validate::content::FACTUAL_DROPPED_ROLE,
        section: None,
        message: "an employer the source has and the output does not".to_string(),
        evidence: Some(company.to_string()),
    });
    report.ok = false;
    report
}

/// **Revert on strictly-worse, and only on strictly-worse.**
///
/// A round that trades one Critical for another has not lost ground, and
/// abandoning it there throws away the second round the budget allows. A round
/// that ADDS a Critical has, and shipping it would leave the user with a
/// document measurably worse than the one the repair replaced.
///
/// This is the COUNT half of the rule; the absence half is
/// `a_repair_round_that_introduces_an_absence_is_worse_whatever_the_count_says`.
///
/// Mutation check: change the comparison to `>=` and the "equal is not worse"
/// case fails; change it to `after > before + 1` and the "one more is worse"
/// case does. The loop AROUND this decision is exercised end to end by
/// `the_repair_loop_*` below, through the injected-provider seam.
#[test]
fn a_repair_round_is_reverted_only_when_it_is_strictly_worse() {
    let worse = |before: usize, after: usize| {
        round_is_worse(&criticals(before), ANY_TEXT, &criticals(after), ANY_TEXT)
    };
    assert!(worse(3, 4), "one more Critical is worse");
    assert!(worse(0, 1), "a clean draft made dirty is worse");
    assert!(
        !worse(3, 3),
        "equal is NOT worse — the swap keeps its budget"
    );
    assert!(!worse(3, 2), "fewer is better");
    assert!(!worse(3, 0), "clean is better");
}

/// **A round that INTRODUCES an absence is worse whatever the count says.**
///
/// The hole this closes, executed: a document with two fabricated metrics,
/// "repaired" by a rewrite that removed them and dropped an employer, came back
/// with ONE Critical against TWO — an improvement by the only measure the loop
/// had, so it was kept and the employer was gone from the saved résumé. An
/// absence has no span, so the review panel deliberately does not list it: the
/// user was told the run needed review and shown nothing to act on.
///
/// The comparison is by `(code, evidence)` PAIR rather than by code, which is
/// what keeps the rule from freezing an already-degraded document — see the
/// third and fourth cases.
///
/// Mutation check: drop the absence term (pure count) and the first case fails;
/// compare by CODE only and the swapped-employer case fails (the
/// already-missing case passes either way — code-only still sees the pair as
/// carried — which is why the swap case is here).
#[test]
fn a_repair_round_that_introduces_an_absence_is_worse_whatever_the_count_says() {
    // Two fabrications traded for one lost employer: fewer criticals, WORSE
    // document.
    assert!(
        round_is_worse(
            &criticals(2),
            ANY_TEXT,
            &criticals_missing(0, "Globex Logistics"),
            ANY_TEXT
        ),
        "losing an employer is not paid for by removing two invented figures"
    );

    // …and the count term still stands on its own for a round that adds one.
    assert!(round_is_worse(
        &criticals(1),
        ANY_TEXT,
        &criticals(2),
        ANY_TEXT
    ));

    // A document that ALREADY lost that employer stays repairable: the pair is
    // carried, not introduced, so an otherwise-improving round is accepted.
    assert!(
        !round_is_worse(
            &criticals_missing(3, "Globex Logistics"),
            ANY_TEXT,
            &criticals_missing(1, "Globex Logistics"),
            ANY_TEXT
        ),
        "a pre-existing absence must not permanently block repair"
    );

    // But SWAPPING which employer is missing is a fresh loss, even though the
    // code and the totals are unchanged.
    assert!(
        round_is_worse(
            &criticals_missing(1, "Globex Logistics"),
            ANY_TEXT,
            &criticals_missing(1, "Initech"),
            ANY_TEXT
        ),
        "a different employer going missing is a NEW absence"
    );
}

/// Audit finding #5 (MEDIUM-HIGH) — `round_is_worse` used to see only
/// Criticals and newly-introduced absences. A round that fixed every Critical
/// while silently dropping an employment entry (`roles_output` fell) or
/// bleeding keyword coverage past `MIN_COVERAGE_DROP_POINTS` was accepted —
/// `repair`'s own up-to-8 blind per-section rewrites had a WEAKER revert rule
/// than `humanize`'s single whole-document one. Both are now first-class
/// terms, the same numbers the audit measured (roles 2→1, coverage 63→56).
///
/// Mutation check: comment out the `roles_output` term in `round_is_worse`
/// and the first assertion fails; comment out the `coverage_dropped` call and
/// the second fails — both verified red, then restored and re-verified green.
#[test]
fn a_repair_round_that_drops_role_count_or_coverage_is_worse_even_with_fewer_criticals() {
    let with = |criticals: usize, roles_output: u32, coverage: Option<f64>| ContentReport {
        ok: criticals == 0,
        issues: (0..criticals)
            .map(|n| ContentIssue {
                severity: Severity::Critical,
                code: crate::validate::content::FACTUAL_UNSOURCED_METRIC,
                section: None,
                message: "an invented figure".to_string(),
                evidence: Some(format!("{n}0%")),
            })
            .collect(),
        metrics: ContentMetrics {
            roles_output,
            keyword_coverage: coverage,
            ..ContentMetrics::default()
        },
    };

    // Two Criticals fixed down to zero — an improvement by the count alone —
    // but a role silently vanished (the exact "roles 2→1" the audit measured).
    assert!(
        round_is_worse(&with(2, 2, None), ANY_TEXT, &with(0, 1, None), ANY_TEXT),
        "a role count that fell is worse even with every Critical fixed"
    );

    // The Critical count improved; coverage fell from 63 to 56 — the exact
    // drop the audit measured, past `MIN_COVERAGE_DROP_POINTS` (5.0).
    assert!(
        round_is_worse(
            &with(1, 2, Some(63.0)),
            ANY_TEXT,
            &with(0, 2, Some(56.0)),
            ANY_TEXT
        ),
        "a coverage drop past the threshold is worse even with fewer criticals"
    );

    // A small coverage move, under the threshold, with the role count
    // unchanged: neither new term fires, and the round is kept.
    assert!(
        !round_is_worse(
            &with(1, 2, Some(63.0)),
            ANY_TEXT,
            &with(0, 2, Some(60.0)),
            ANY_TEXT
        ),
        "a coverage move under the threshold with an unchanged role count must not revert"
    );
}

//! The whole `validate_content` dispatcher over the realistic fixture triples: the clean
//! and paraphrased documents must raise nothing, and each defect fixture must raise exactly
//! its own code.

use super::{support::*, *};

/// The German fixtures against the one German top requirement they share.
fn de_resume(generated: &str) -> ContentReport {
    validate_content(&ContentInput {
        top_requirements: &["Docker und Kubernetes im Produktivbetrieb".to_string()],
        target_language: "de",
        ..content_input(generated, DE_SOURCE, DE_JOB_AD)
    })
}

/// A realistically tailored résumé that fabricates nothing must produce a
/// COMPLETELY empty report. Not "no criticals" — nothing at all.
///
/// This is the test that matters most. Every check in this module is a claim
/// made to a user about their own document; one wrong warning on a correct
/// résumé and they stop reading the panel.
///
/// The fixture is a REWORDING of `en_source_resume.txt`, not a copy of it: every
/// sentence is rewritten while the employers, dates, links and figures stay
/// exactly the candidate's own. That is what generator output looks like, and
/// it is the only version of this assertion worth making — against a byte-copy
/// it would pass no matter how literal the validators were.
#[test]
fn clean_resume_produces_no_issues_at_all() {
    let report = en_resume(EN_CLEAN, &en_requirements());
    assert!(
        report.issues.is_empty(),
        "a clean résumé must produce no findings; got {:#?}",
        report.issues
    );
    assert!(report.ok);
}

/// The same guard in German, against a German posting — the stemmer, the
/// heading classifier and the lexicon all switch language here.
#[test]
fn clean_german_resume_produces_no_issues_at_all() {
    let report = de_resume(DE_CLEAN);
    assert!(
        report.issues.is_empty(),
        "a clean German résumé must produce no findings; got {:#?}",
        report.issues
    );
}

/// The clean fixtures above are rewordings, but they keep every employer, span
/// and link exactly as the source wrote it. This one goes further, the way real
/// output does: links written in a different but equivalent form, company names
/// shortened ("Acme Payments" → "Acme"), an open-ended span resolved to a
/// concrete year. Every one of those is a legitimate tailoring decision, and
/// every one of them produced a false Critical before this pass.
///
/// Criticals are the bar here (not "no issues at all"): a rewording may
/// legitimately move a Warning, but nothing about restating a true fact may ever
/// say the candidate fabricated something.
#[test]
fn paraphrased_but_truthful_resume_raises_no_criticals() {
    let report = en_resume(EN_PARAPHRASED, &en_requirements());
    let criticals = criticals_of(&report);
    assert!(
        criticals.is_empty(),
        "a truthful paraphrase must never be accused of fabrication; got {criticals:#?}"
    );
    assert!(report.ok);
}

/// The same guard in German. The stemmer, the heading classifier and the
/// function-word filter all switch language here, and the shortened company
/// names are the ones a German résumé actually carries.
#[test]
fn paraphrased_but_truthful_german_resume_raises_no_criticals() {
    let report = de_resume(DE_PARAPHRASED);
    let criticals = criticals_of(&report);
    assert!(
        criticals.is_empty(),
        "a truthful German paraphrase must never be accused of fabrication; got {criticals:#?}"
    );
}

/// A grounded, specific cover letter must also come back clean — the prose
/// checks are the easiest place to over-fire.
#[test]
fn grounded_letter_produces_no_issues_at_all() {
    let report = en_letter(EN_LETTER_GROUNDED);
    assert!(
        report.issues.is_empty(),
        "a grounded letter must produce no findings; got {:#?}",
        report.issues
    );
}

/// Live-defect regression: a model-emitted template placeholder ("Your
/// Name") surviving into an otherwise-grounded letter must fire
/// `letter.template_placeholder` as a Critical. See ADR-034 Consequence #2.
#[test]
fn template_placeholder_signature_line_is_critical() {
    let generated = format!("{EN_LETTER_GROUNDED}Your Name\n");
    let report = en_letter(&generated);
    let hits = fired(&report, LETTER_TEMPLATE_PLACEHOLDER);
    assert_eq!(hits[0].severity, Severity::Critical);
    assert_eq!(hits[0].evidence.as_deref(), Some("Your Name"));
    assert!(!report.ok, "a Critical must clear `ok`");
}

#[test]
fn fabricated_metric_is_critical_and_names_the_number() {
    let report = en_resume(EN_FABRICATED_METRIC, &en_requirements());
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].severity, Severity::Critical);
    assert_eq!(hits[0].evidence.as_deref(), Some("72%"));
    assert!(!report.ok, "a Critical must clear `ok`");
    assert_eq!(
        hits.len(),
        1,
        "exactly one fabricated figure in this fixture; got {:?}",
        hits
    );
}

#[test]
fn dropped_role_is_critical_and_names_the_employer() {
    let report = en_resume(EN_DROPPED_ROLE, &en_requirements());
    let hits = fired(&report, FACTUAL_DROPPED_ROLE);
    assert_eq!(hits[0].severity, Severity::Critical);
    assert!(
        hits[0]
            .evidence
            .as_deref()
            .is_some_and(|e| e.contains("Globex")),
        "the evidence must name the missing employer; got {:?}",
        hits[0].evidence
    );
    assert_eq!(report.metrics.roles_source, 2);
    assert_eq!(report.metrics.roles_output, 1);
}

#[test]
fn near_duplicate_bullets_warn_once_on_the_later_bullet() {
    let report = en_resume(EN_DUPLICATES, &en_requirements());
    let hits = fired(&report, DUPLICATE_BULLET);
    assert_eq!(hits.len(), 1, "one pair → one finding; got {hits:#?}");
    assert_eq!(hits[0].severity, Severity::Warning);
    assert!(
        hits[0]
            .evidence
            .as_deref()
            .is_some_and(|e| e.ends_with("answering 12000 requests every second")),
        "the LATER bullet is the one to cut; got {:?}",
        hits[0].evidence
    );
    assert!(
        report.metrics.duplicate_ratio > 0.0,
        "duplicateRatio must reflect the pair"
    );
}

#[test]
fn wrong_language_output_is_critical() {
    let report = en_resume(EN_WRONG_LANGUAGE, &en_requirements());
    let hits = fired(&report, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
    assert!(!report.ok);
    // Posting comparisons are suppressed once the language is wrong — coverage
    // across two languages is noise, and a cascade would bury this finding.
    assert!(
        report.metrics.keyword_coverage.is_none(),
        "coverage must be withheld on a language mismatch"
    );
    assert!(
        !codes(&report).contains(&ALIGNMENT_LOW_COVERAGE),
        "no cascade of derived alignment warnings; got {:?}",
        codes(&report)
    );
}

#[test]
fn ai_tell_laden_letter_fires_voice_warnings_but_no_critical() {
    let report = en_letter(EN_LETTER_AI_TELLS);
    let phrases = fired_evidence(&report, VOICE_AI_TELL_LEXICAL);
    for expected in [
        "leverage",
        "robust",
        "seamless",
        "passionate",
        "studies show",
    ] {
        assert!(
            phrases.contains(&expected),
            "{expected:?} is on the prompt's own ban list and must fire; got {phrases:?}"
        );
    }
    fired(&report, VOICE_TEMPLATE_OPENER);
    assert!(
        report.ok,
        "voice findings are advice — a model may never produce a Critical; got {:?}",
        criticals_of(&report)
    );
}

/// A letter carries no résumé structure, so no résumé-structure check may fire
/// on it — that would be twenty warnings the user can do nothing about.
#[test]
fn letters_skip_every_resume_structure_check() {
    let report = en_letter(EN_LETTER_AI_TELLS);
    for code in [
        ATS_MISSING_SECTION,
        ATS_BULLET_COUNT,
        ATS_LONG_BULLET,
        CONSISTENCY_PROJECT_STRUCTURE,
        CONSISTENCY_SKILL_NOT_DEMONSTRATED,
        FACTUAL_DROPPED_ROLE,
        ALIGNMENT_LOW_COVERAGE,
    ] {
        assert!(
            !codes(&report).contains(&code),
            "{code} must not run on a cover letter; got {:?}",
            codes(&report)
        );
    }
}

/// Empty inputs must not panic and must not accuse anyone of anything.
#[test]
fn empty_inputs_are_inert() {
    let report = report_for("", "", "", &[]);
    assert!(report.ok, "an empty document has fabricated nothing");
    assert!(
        !codes(&report).contains(&FACTUAL_UNSOURCED_METRIC),
        "no factual accusation from an empty document; got {:?}",
        codes(&report)
    );
    assert_eq!(report.metrics.roles_source, 0);
    assert_eq!(report.metrics.duplicate_ratio, 0.0);
}

/// F1 — every other doc-kind-specific metric is zeroed for a cover letter
/// (`topRequirementHits`, `duplicateRatio`), but the role counts were computed
/// unconditionally. A letter has no employment entries, so the quality panel
/// rendered a "2 → 0" roles DROP — a résumé-shaped number, and an alarming one,
/// on a perfectly good letter.
#[test]
fn cover_letter_metrics_carry_no_role_counts() {
    let report = en_letter(EN_LETTER_GROUNDED);
    assert_eq!(
        (report.metrics.roles_source, report.metrics.roles_output),
        (0, 0),
        "a letter has no employment entries to count on either side"
    );
    // The same source résumé really does carry two roles, so this is the letter
    // arm zeroing them rather than an empty fixture proving nothing.
    assert_eq!(
        en_resume(EN_CLEAN, &en_requirements()).metrics.roles_source,
        2
    );
}

//! Content-validation tests.
//!
//! Two layers, on purpose:
//!
//! * **Fixture integration tests** run the whole `validate_content` dispatcher
//!   over realistic en/de résumé + job-ad + generated triples. Each defect
//!   fixture differs from `en_generated_clean.txt` by exactly ONE edit, so a
//!   test that fires the wrong code says so loudly.
//! * **Threshold tests** pin every named `const`, because a silently-loosened
//!   threshold is how a validator stops validating.
//!
//! The most important test in the file is
//! `clean_resume_produces_no_issues_at_all`: this module's failure mode is not
//! missing a defect, it is inventing one.

use super::*;

// ── Fixtures ────────────────────────────────────────────────────────────────

const EN_SOURCE: &str = include_str!("fixtures/en_source_resume.txt");
const EN_JOB_AD: &str = include_str!("fixtures/en_job_ad.txt");
const EN_CLEAN: &str = include_str!("fixtures/en_generated_clean.txt");
const EN_FABRICATED_METRIC: &str = include_str!("fixtures/en_generated_fabricated_metric.txt");
const EN_DROPPED_ROLE: &str = include_str!("fixtures/en_generated_dropped_role.txt");
const EN_ALTERED_LINK: &str = include_str!("fixtures/en_generated_altered_project_link.txt");
const EN_DUPLICATES: &str = include_str!("fixtures/en_generated_duplicate_bullets.txt");
const EN_WRONG_LANGUAGE: &str = include_str!("fixtures/en_generated_wrong_language.txt");
const EN_PROJECTS_TIER2: &str = include_str!("fixtures/en_generated_projects_tier2.txt");
const EN_PROJECTS_TIER3: &str = include_str!("fixtures/en_generated_projects_tier3.txt");
const EN_PROJECTS_BROKEN: &str = include_str!("fixtures/en_generated_projects_broken.txt");
const EN_LETTER_AI_TELLS: &str = include_str!("fixtures/en_letter_ai_tells.txt");
const EN_LETTER_GROUNDED: &str = include_str!("fixtures/en_letter_grounded.txt");
const DE_SOURCE: &str = include_str!("fixtures/de_source_resume.txt");
const DE_JOB_AD: &str = include_str!("fixtures/de_job_ad.txt");
const DE_CLEAN: &str = include_str!("fixtures/de_generated_clean.txt");

fn en_requirements() -> Vec<String> {
    [
        "Strong Rust and Python",
        "Production Docker and Kubernetes",
        "PostgreSQL and Redis at scale",
        "Terraform and AWS",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn en_resume(generated: &str, requirements: &[String]) -> ContentReport {
    validate_content(&ContentInput {
        generated,
        source_resume: EN_SOURCE,
        job_ad: EN_JOB_AD,
        top_requirements: requirements,
        target_language: "en",
        doc_kind: DocKind::Resume,
    })
}

fn en_letter(generated: &str) -> ContentReport {
    validate_content(&ContentInput {
        generated,
        source_resume: EN_SOURCE,
        job_ad: EN_JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::CoverLetter,
    })
}

fn report_for(generated: &str, source: &str, job_ad: &str, reqs: &[String]) -> ContentReport {
    validate_content(&ContentInput {
        generated,
        source_resume: source,
        job_ad,
        top_requirements: reqs,
        target_language: "en",
        doc_kind: DocKind::Resume,
    })
}

fn letter_report_for(generated: &str, source: &str, job_ad: &str) -> ContentReport {
    validate_content(&ContentInput {
        generated,
        source_resume: source,
        job_ad,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::CoverLetter,
    })
}

fn codes(report: &ContentReport) -> Vec<&str> {
    report.issues.iter().map(|i| i.code).collect()
}

/// Assert `code` did NOT fire — the half of a boundary test that proves the
/// threshold is doing work rather than always firing.
#[track_caller]
fn silent(report: &ContentReport, code: &str) {
    assert!(
        !codes(report).contains(&code),
        "expected {code} NOT to fire; report carried {:?}",
        codes(report)
    );
}

/// Assert `code` fired, and return its issues.
#[track_caller]
fn fired<'a>(report: &'a ContentReport, code: &str) -> Vec<&'a ContentIssue> {
    let hits: Vec<&ContentIssue> = report.issues.iter().filter(|i| i.code == code).collect();
    assert!(
        !hits.is_empty(),
        "expected {code} to fire; report carried {:?}",
        codes(report)
    );
    hits
}

// ── The false-positive guard ────────────────────────────────────────────────

/// A realistically tailored résumé that fabricates nothing must produce a
/// COMPLETELY empty report. Not "no criticals" — nothing at all.
///
/// This is the test that matters most. Every check in this module is a claim
/// made to a user about their own document; one wrong warning on a correct
/// résumé and they stop reading the panel.
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
    let report = validate_content(&ContentInput {
        generated: DE_CLEAN,
        source_resume: DE_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &["Docker und Kubernetes im Produktivbetrieb".to_string()],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    assert!(
        report.issues.is_empty(),
        "a clean German résumé must produce no findings; got {:#?}",
        report.issues
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

// ── Per-defect fixtures ─────────────────────────────────────────────────────

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

/// A changed link surfaces as BOTH halves: the candidate's own URL is gone and
/// an unknown one appeared. Both are Critical — a reviewer following the wrong
/// link is the failure this prevents.
#[test]
fn altered_project_link_fires_for_the_drop_and_the_invention() {
    let report = en_resume(EN_ALTERED_LINK, &en_requirements());
    let hits = fired(&report, FACTUAL_ALTERED_PROJECT_LINK);
    assert_eq!(hits.len(), 2, "one drop + one invention; got {hits:#?}");
    assert!(hits.iter().all(|i| i.severity == Severity::Critical));
    let evidence: Vec<&str> = hits.iter().filter_map(|i| i.evidence.as_deref()).collect();
    assert!(
        evidence.contains(&"https://github.com/janedoe/ledger"),
        "the dropped source link must be named; got {evidence:?}"
    );
    assert!(
        evidence.contains(&"https://github.com/jane-doe/ledger-cli"),
        "the invented link must be named; got {evidence:?}"
    );
    // The untouched link must NOT be reported.
    assert!(
        !evidence.contains(&"https://ledger.example.dev"),
        "an unchanged link must never fire; got {evidence:?}"
    );
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
            .is_some_and(|e| e.starts_with("Shipped Docker containers to")),
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

/// The two DEGRADED project tiers are legal — the source simply had less data.
/// Neither may warn.
#[test]
fn degraded_project_tiers_are_accepted() {
    for (name, fixture) in [
        ("name+links+stack", EN_PROJECTS_TIER2),
        ("compact", EN_PROJECTS_TIER3),
    ] {
        let report = en_resume(fixture, &en_requirements());
        assert!(
            !codes(&report).contains(&CONSISTENCY_PROJECT_STRUCTURE),
            "the {name} tier is an accepted degradation; got {:?}",
            codes(&report)
        );
    }
}

#[test]
fn project_outside_the_three_tiers_warns() {
    let report = en_resume(EN_PROJECTS_BROKEN, &en_requirements());
    let hits = fired(&report, CONSISTENCY_PROJECT_STRUCTURE);
    assert_eq!(hits[0].severity, Severity::Warning);
    assert_eq!(hits[0].section.as_deref(), Some("Projects"));
}

#[test]
fn ai_tell_laden_letter_fires_voice_warnings_but_no_critical() {
    let report = en_letter(EN_LETTER_AI_TELLS);
    let tells = fired(&report, VOICE_AI_TELL_LEXICAL);
    let phrases: Vec<&str> = tells.iter().filter_map(|i| i.evidence.as_deref()).collect();
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
        report
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Critical)
            .collect::<Vec<_>>()
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

// ── Edge paths ──────────────────────────────────────────────────────────────

/// Zero keyword overlap between résumé and posting is a real situation (a
/// career change), not a defect. Nothing may fire from it, and coverage must
/// report the honest 0 rather than going silent.
#[test]
fn zero_keyword_overlap_reports_zero_coverage_without_inventing_issues() {
    let report = validate_content(&ContentInput {
        generated: "EXPERIENCE\n\nBaker | Corner Bakery | 2019 - 2021\n- Shaped sourdough loaves\n",
        source_resume:
            "EXPERIENCE\n\nBaker | Corner Bakery | 2019 - 2021\n- Shaped sourdough loaves\n",
        job_ad: "Hiring a welder for structural steel fabrication.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    assert_eq!(report.metrics.keyword_coverage, Some(0.0));
    assert!(
        !codes(&report).contains(&ALIGNMENT_LOW_COVERAGE),
        "matching the source exactly can never be a coverage REGRESSION; got {:?}",
        codes(&report)
    );
    assert!(report.ok, "no criticals from a career change");
}

/// An empty or garbled posting must silence every posting comparison rather
/// than reporting 0% and a pile of derived warnings.
#[test]
fn empty_or_garbled_job_ad_silences_posting_comparisons() {
    for job_ad in ["", "   ", "!!! ??? ...", "\u{fffd}\u{fffd}\u{fffd}"] {
        let report = validate_content(&ContentInput {
            generated: EN_CLEAN,
            source_resume: EN_SOURCE,
            job_ad,
            top_requirements: &en_requirements(),
            target_language: "en",
            doc_kind: DocKind::Resume,
        });
        assert_eq!(
            report.metrics.keyword_coverage, None,
            "no extractable posting keywords must yield None, not 0% (job_ad={job_ad:?})"
        );
        assert_eq!(report.metrics.top_requirement_hits, 0);
        for code in [ALIGNMENT_LOW_COVERAGE, ALIGNMENT_MISSING_TOP_REQUIREMENT] {
            assert!(
                !codes(&report).contains(&code),
                "{code} must not fire against an unusable posting (job_ad={job_ad:?})"
            );
        }
    }
}

/// Empty inputs must not panic and must not accuse anyone of anything.
#[test]
fn empty_inputs_are_inert() {
    let report = validate_content(&ContentInput {
        generated: "",
        source_resume: "",
        job_ad: "",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    assert!(report.ok, "an empty document has fabricated nothing");
    assert!(
        !codes(&report).contains(&FACTUAL_UNSOURCED_METRIC),
        "no factual accusation from an empty document; got {:?}",
        codes(&report)
    );
    assert_eq!(report.metrics.roles_source, 0);
    assert_eq!(report.metrics.duplicate_ratio, 0.0);
}

/// Short text is where `whatlang` guesses, so the Critical language check must
/// go quiet below [`MIN_CHARS_FOR_LANGUAGE_CHECK`] — a two-line draft is not
/// evidence of the wrong language.
#[test]
fn short_documents_never_raise_a_language_critical() {
    let short = "Guten Tag.";
    assert!(
        short.chars().filter(|c| !c.is_whitespace()).count() < MIN_CHARS_FOR_LANGUAGE_CHECK,
        "fixture must be under the floor, or this test proves nothing"
    );
    assert!(!is_language_mismatch(short, "en"));
    // And just past the floor, the same language really is flagged.
    let long_german = "Sehr geehrte Damen und Herren, hiermit bewerbe ich mich auf die \
                       ausgeschriebene Stelle als Backend-Entwicklerin in Ihrem Unternehmen \
                       und freue mich sehr über eine Rückmeldung von Ihnen.";
    assert!(
        long_german.chars().filter(|c| !c.is_whitespace()).count() >= MIN_CHARS_FOR_LANGUAGE_CHECK
    );
    assert!(is_language_mismatch(long_german, "en"));
}

/// Years are never metrics. A résumé is full of them and a fabricated-metric
/// Critical on "2021" would be unusable.
#[test]
fn years_are_never_treated_as_metrics() {
    let metrics = factual::metrics_in("EXPERIENCE\n\nAcme | 2021 - 2024\n- Shipped in 1999\n");
    assert!(
        metrics.is_empty(),
        "1900–2099 must be excluded from metric extraction; got {metrics:?}"
    );
    // Just outside the window, a four-digit run IS a quantity.
    let quantity = factual::metrics_in("EXPERIENCE\n\nAcme\n- Processed 4500 orders a day\n");
    assert_eq!(quantity.len(), 1, "got {quantity:?}");
    assert_eq!(quantity[0].number, "4500");
}

/// Phone numbers and postal codes live in the contact band and are not claims
/// of impact.
#[test]
fn contact_band_digits_are_not_metrics() {
    let text = "Jane Doe\njane@example.com | +49 30 1234567 | 10115 Berlin\n\n\
                EXPERIENCE\n\nAcme\n- Led the migration\n";
    assert!(
        factual::metrics_in(text).is_empty(),
        "the header band must be skipped entirely; got {:?}",
        factual::metrics_in(text)
    );
}

/// An open-ended source span resolved to a concrete end year is the SAME fact,
/// not a fabricated date — the carve-out that keeps this check usable.
#[test]
fn resolving_an_open_ended_span_is_not_an_unsupported_date() {
    let source = "EXPERIENCE\n\nAcme Payments | 2021 - Present\n- Shipped the ledger\n";
    let generated = "EXPERIENCE\n\nAcme Payments | 2021 - 2026\n- Shipped the ledger\n";
    let report = validate_content(&ContentInput {
        generated,
        source_resume: source,
        job_ad: EN_JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    assert!(
        !codes(&report).contains(&FACTUAL_UNSUPPORTED_DATE),
        "a later end year against an open-ended source span is legitimate; got {:?}",
        codes(&report)
    );

    // An INVENTED EARLIER year has no such explanation.
    let backdated = "EXPERIENCE\n\nAcme Payments | 2015 - Present\n- Shipped the ledger\n";
    let report = validate_content(&ContentInput {
        generated: backdated,
        source_resume: source,
        job_ad: EN_JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    let hits = fired(&report, FACTUAL_UNSUPPORTED_DATE);
    assert_eq!(hits[0].evidence.as_deref(), Some("2015"));
    assert_eq!(hits[0].severity, Severity::Critical);
}

/// A second contact block in the body is Critical; a body line that merely
/// mentions an address is not. The second half is the false positive that
/// would make the check unusable.
#[test]
fn header_in_body_needs_a_contact_cluster_not_just_an_email() {
    let with_cluster = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                        Acme | 2021 - Present\n- Led the migration\n\n\
                        REFERENCES\n\nJohn Smith\njohn.smith@acme.example.com\n";
    let report = validate_content(&ContentInput {
        generated: with_cluster,
        source_resume: with_cluster,
        job_ad: EN_JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    let hits = fired(&report, ATS_HEADER_IN_BODY);
    assert_eq!(hits[0].severity, Severity::Critical);

    let mentions_an_email = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                             Acme | 2021 - Present\n\
                             - Ran the support alias support@acme.example.com for the whole team\n";
    let report = validate_content(&ContentInput {
        generated: mentions_an_email,
        source_resume: mentions_an_email,
        job_ad: EN_JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    assert!(
        !codes(&report).contains(&ATS_HEADER_IN_BODY),
        "a bullet mentioning an address is not a contact block; got {:?}",
        codes(&report)
    );
}

// ── One test per remaining validator, each with its boundary ────────────────

/// Coverage is a REGRESSION check: dropping the posting's vocabulary during
/// tailoring fires; keeping it does not, however low the absolute number is.
#[test]
fn low_coverage_fires_on_a_regression_not_on_a_low_absolute_score() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                  - Shipped Docker containers onto a Kubernetes cluster\n\
                  - Wrote the Terraform modules for the AWS estate\n";
    let job = "Docker Kubernetes Terraform AWS platform engineer";
    // Tailoring dropped both technical bullets.
    let stripped = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Organised the team offsite for forty people\n";
    let hits = {
        let report = report_for(stripped, source, job, &[]);
        fired(&report, ALIGNMENT_LOW_COVERAGE).len()
    };
    assert_eq!(hits, 1);
    // The untouched résumé covers the same posting equally — no regression.
    silent(
        &report_for(source, source, job, &[]),
        ALIGNMENT_LOW_COVERAGE,
    );
}

/// A top requirement the candidate cannot meet is not a document defect. Only
/// one the SOURCE evidenced and the output dropped is reported.
#[test]
fn missing_top_requirement_fires_only_when_the_source_had_the_evidence() {
    let source =
        "EXPERIENCE\n\nAcme | 2021 - Present\n- Wrote Terraform modules for the AWS estate\n";
    let job = "Terraform AWS Kubernetes platform engineer";
    let dropped = "EXPERIENCE\n\nAcme | 2021 - Present\n- Kept the AWS estate running\n";
    let evidenced = vec!["Terraform modules".to_string()];
    let hits = fired(
        &report_for(dropped, source, job, &evidenced),
        ALIGNMENT_MISSING_TOP_REQUIREMENT,
    )
    .len();
    assert_eq!(hits, 1);

    // Never evidenced anywhere → the candidate's gap, not the document's.
    let never_had = vec!["Kubernetes operators".to_string()];
    silent(
        &report_for(dropped, source, job, &never_had),
        ALIGNMENT_MISSING_TOP_REQUIREMENT,
    );
    // Still present in the output → counted as a hit, not an issue.
    let report = report_for(source, source, job, &evidenced);
    silent(&report, ALIGNMENT_MISSING_TOP_REQUIREMENT);
    assert_eq!(report.metrics.top_requirement_hits, 1);
}

#[test]
fn date_order_warns_when_a_span_runs_backwards() {
    let source = "EXPERIENCE\n\nAcme | 2018 - 2021\n- Shipped the ledger service\n";
    let backwards = "EXPERIENCE\n\nAcme | 2021 - 2018\n- Shipped the ledger service\n";
    let report = report_for(backwards, source, EN_JOB_AD, &[]);
    let hits = fired(&report, CONSISTENCY_DATE_ORDER);
    assert_eq!(hits[0].severity, Severity::Warning);
    silent(
        &report_for(source, source, EN_JOB_AD, &[]),
        CONSISTENCY_DATE_ORDER,
    );
}

/// A different job title at the same employer. A promotion that SHARES a word
/// ("Senior Engineer" → "Staff Engineer") is not drift — only a wholly
/// different role is.
#[test]
fn title_drift_warns_on_an_unrelated_title_at_the_same_employer() {
    let source = "EXPERIENCE\n\nSenior Engineer | Acme Payments | 2021 - Present\n\
                  - Shipped the ledger service\n";
    let drifted = "EXPERIENCE\n\nProduct Manager | Acme Payments | 2021 - Present\n\
                   - Shipped the ledger service\n";
    let report = report_for(drifted, source, EN_JOB_AD, &[]);
    let hits = fired(&report, CONSISTENCY_TITLE_DRIFT);
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("Senior Engineer → Product Manager")
    );

    let promoted = "EXPERIENCE\n\nStaff Engineer | Acme Payments | 2021 - Present\n\
                    - Shipped the ledger service\n";
    silent(
        &report_for(promoted, source, EN_JOB_AD, &[]),
        CONSISTENCY_TITLE_DRIFT,
    );
}

#[test]
fn skill_not_demonstrated_warns_on_a_claim_nothing_backs() {
    let doc = "EXPERIENCE\n\nAcme | 2021 - Present\n- Shipped Docker containers to production\n\n\
               SKILLS\n\nDocker · Kubernetes\n";
    let report = report_for(doc, doc, EN_JOB_AD, &[]);
    let hits = fired(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
    assert_eq!(hits.len(), 1, "only the unbacked skill; got {hits:#?}");
    assert!(hits[0]
        .evidence
        .as_deref()
        .is_some_and(|e| e.starts_with("kubernet")));
}

/// The occurrence ceiling: exactly [`ats::MAX_KEYWORD_OCCURRENCES`] is fine,
/// one more is stuffing.
#[test]
fn keyword_density_boundary_is_the_occurrence_ceiling() {
    let doc = |repeats: usize| {
        format!(
            "EXPERIENCE\n\nAcme | 2021 - Present\n- Shipped {}\n",
            "kubernetes ".repeat(repeats).trim()
        )
    };
    let at_limit = doc(ats::MAX_KEYWORD_OCCURRENCES);
    silent(
        &report_for(&at_limit, &at_limit, EN_JOB_AD, &[]),
        ATS_KEYWORD_DENSITY,
    );
    let over = doc(ats::MAX_KEYWORD_OCCURRENCES + 1);
    let report = report_for(&over, &over, EN_JOB_AD, &[]);
    let hits = fired(&report, ATS_KEYWORD_DENSITY);
    assert!(hits[0]
        .evidence
        .as_deref()
        .is_some_and(|e| e.starts_with("kubernetes ×")));
}

#[test]
fn missing_section_warns_once_per_absent_standard_section() {
    let only_experience = "EXPERIENCE\n\nAcme | 2021 - Present\n- Shipped the ledger service\n";
    let report = report_for(only_experience, only_experience, EN_JOB_AD, &[]);
    let hits = fired(&report, ATS_MISSING_SECTION);
    let named: Vec<&str> = hits.iter().filter_map(|i| i.evidence.as_deref()).collect();
    assert_eq!(named, vec!["Education", "Skills"]);
}

/// Exactly [`ats::MAX_BULLET_CHARS`] is fine; one character more is not.
#[test]
fn long_bullet_boundary_is_the_char_budget() {
    let doc = |chars: usize| {
        format!(
            "EXPERIENCE\n\nAcme | 2021 - Present\n- {}\n",
            "a".repeat(chars)
        )
    };
    let at_limit = doc(ats::MAX_BULLET_CHARS);
    silent(
        &report_for(&at_limit, &at_limit, EN_JOB_AD, &[]),
        ATS_LONG_BULLET,
    );
    let over = doc(ats::MAX_BULLET_CHARS + 1);
    fired(&report_for(&over, &over, EN_JOB_AD, &[]), ATS_LONG_BULLET);
}

/// Both ends of the 1..=6 band, plus the band itself.
#[test]
fn bullet_count_boundaries_are_the_role_band() {
    let doc = |bullets: usize| {
        let mut out = String::from("EXPERIENCE\n\nAcme | 2021 - Present\n");
        for i in 0..bullets {
            out.push_str(&format!("- Shipped release number {i} to production\n"));
        }
        out
    };
    let at_max = doc(ats::MAX_BULLETS_PER_ROLE);
    silent(
        &report_for(&at_max, &at_max, EN_JOB_AD, &[]),
        ATS_BULLET_COUNT,
    );

    let too_many = doc(ats::MAX_BULLETS_PER_ROLE + 1);
    fired(
        &report_for(&too_many, &too_many, EN_JOB_AD, &[]),
        ATS_BULLET_COUNT,
    );

    let empty_role = doc(0);
    let report = report_for(&empty_role, &empty_role, EN_JOB_AD, &[]);
    let hits = fired(&report, ATS_BULLET_COUNT);
    assert!(
        hits[0].message.contains("0 bullets"),
        "a role with no results must be reported too; got {:?}",
        hits[0].message
    );
}

/// A technology in neither the source résumé nor the ad. Only vocabulary the
/// keyword kernel itself recognises is policed, so ordinary rephrasing is safe.
#[test]
fn unsourced_term_warns_on_an_ungrounded_technology() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n- Built the billing service in Python\n";
    let job = "Backend engineer, Python and PostgreSQL.";
    let invented = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Built the billing service in Python and TensorFlow\n";
    let report = report_for(invented, source, job, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_TERM);
    assert_eq!(hits[0].evidence.as_deref(), Some("tensorflow"));
    assert_eq!(hits[0].severity, Severity::Warning);
    // Rewording ordinary prose is not a factual claim.
    let reworded = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Owned the invoicing service, written in Python\n";
    silent(
        &report_for(reworded, source, job, &[]),
        FACTUAL_UNSOURCED_TERM,
    );
}

/// Burstiness needs [`voice::MIN_SENTENCES_FOR_BURSTINESS`] sentences before it
/// says anything — below that, uniformity is a coincidence.
#[test]
fn low_burstiness_needs_enough_sentences_and_real_uniformity() {
    let uniform = |count: usize| {
        (0..count)
            .map(|_| "The team shipped the release to production without any real trouble.")
            .collect::<Vec<_>>()
            .join(" ")
    };
    let short = uniform(voice::MIN_SENTENCES_FOR_BURSTINESS - 1);
    silent(
        &letter_report_for(&short, EN_SOURCE, EN_JOB_AD),
        VOICE_LOW_BURSTINESS,
    );

    let long = uniform(voice::MIN_SENTENCES_FOR_BURSTINESS);
    fired(
        &letter_report_for(&long, EN_SOURCE, EN_JOB_AD),
        VOICE_LOW_BURSTINESS,
    );

    // Varied prose of the same length must stay silent.
    let varied = "I shipped it. The retry scheduler was rewritten in Rust over two long \
                  weeks after the settlement backlog grew past anything the old design could \
                  absorb. Latency dropped. Then we moved on to the ledger itself, which took \
                  most of the following quarter and taught me more about idempotency than any \
                  book had. It worked. Barely. The second attempt held up under the Black \
                  Friday peak and has not needed a rollback since, which is the only metric \
                  I trust. Good enough.";
    silent(
        &letter_report_for(varied, EN_SOURCE, EN_JOB_AD),
        VOICE_LOW_BURSTINESS,
    );
}

/// Two triplets per ten sentences is the ceiling, not the trigger.
#[test]
fn rule_of_three_density_boundary() {
    let triplet = "We shipped the ledger, the scheduler, and the cache.";
    let plain = "The rollout held.";
    // 2 triplets in 10 sentences == the ceiling → silent.
    let at_limit = format!("{triplet} {triplet} {}", plain.repeat(8));
    silent(
        &letter_report_for(&at_limit, EN_SOURCE, EN_JOB_AD),
        VOICE_RULE_OF_THREE_DENSITY,
    );
    // 3 in 10 → over.
    let over = format!("{triplet} {triplet} {triplet} {}", plain.repeat(7));
    fired(
        &letter_report_for(&over, EN_SOURCE, EN_JOB_AD),
        VOICE_RULE_OF_THREE_DENSITY,
    );
}

/// One clause dash per [`voice::EM_DASH_WORDS_PER_ALLOWED`] words is natural;
/// a numeric range never counts.
#[test]
fn em_dash_overuse_ignores_numeric_ranges() {
    let ranges = "I worked at Acme from 2018–2021 and at Globex from 2021–2024 on the ledger.";
    silent(
        &letter_report_for(ranges, EN_SOURCE, EN_JOB_AD),
        VOICE_EM_DASH_OVERUSE,
    );
    let overused = "The ledger — which we rewrote — is fast now — faster than before.";
    let report = letter_report_for(overused, EN_SOURCE, EN_JOB_AD);
    let hits = fired(&report, VOICE_EM_DASH_OVERUSE);
    assert_eq!(hits[0].severity, Severity::Warning);
}

#[test]
fn generic_letter_warns_when_nothing_is_posting_specific() {
    let generic = "Dear Hiring Manager, I would be a great addition to your organisation and \
                   look forward to hearing from you soon. Best regards, Jane";
    let report = letter_report_for(generic, EN_SOURCE, EN_JOB_AD);
    let hits = fired(&report, VOICE_GENERIC_LETTER);
    assert_eq!(hits[0].severity, Severity::Warning);
    // The grounded fixture names the stack and must not fire.
    silent(&en_letter(EN_LETTER_GROUNDED), VOICE_GENERIC_LETTER);
}

// ── Contract + vocabulary ───────────────────────────────────────────────────

/// `ok` is exactly "no Criticals" — nothing else may clear or set it.
#[test]
fn ok_tracks_criticals_only() {
    let report = en_resume(EN_DUPLICATES, &en_requirements());
    assert!(
        report
            .issues
            .iter()
            .all(|i| i.severity == Severity::Warning),
        "this fixture must carry warnings only; got {:?}",
        report.issues
    );
    assert!(report.ok, "warnings alone must not clear `ok`");

    let report = en_resume(EN_FABRICATED_METRIC, &en_requirements());
    assert!(!report.ok);
}

/// Every code any fixture can produce must be registered with the severity the
/// table declares — the constructor reads the table, so this proves no check
/// reaches the "unregistered → Warning" fallback.
#[test]
fn every_emitted_code_is_registered_with_its_declared_severity() {
    let reports = [
        en_resume(EN_CLEAN, &en_requirements()),
        en_resume(EN_FABRICATED_METRIC, &en_requirements()),
        en_resume(EN_DROPPED_ROLE, &en_requirements()),
        en_resume(EN_ALTERED_LINK, &en_requirements()),
        en_resume(EN_DUPLICATES, &en_requirements()),
        en_resume(EN_WRONG_LANGUAGE, &en_requirements()),
        en_resume(EN_PROJECTS_BROKEN, &en_requirements()),
        en_letter(EN_LETTER_AI_TELLS),
    ];
    for report in &reports {
        for issue in &report.issues {
            let registered = CONTENT_ISSUE_CODES
                .iter()
                .find(|(c, _)| *c == issue.code)
                .unwrap_or_else(|| panic!("unregistered code emitted: {}", issue.code));
            assert_eq!(
                issue.severity, registered.1,
                "{} emitted with the wrong severity",
                issue.code
            );
        }
    }
}

/// The code vocabulary is a wire contract: the renderer keys i18n off it and a
/// stored report carries codes forever. Adding one is fine; renaming or
/// dropping one is a breaking change that must be deliberate.
#[test]
fn code_table_is_complete_and_unique() {
    let mut seen = std::collections::HashSet::new();
    for (code, _) in CONTENT_ISSUE_CODES {
        assert!(seen.insert(*code), "duplicate code in the table: {code}");
        assert!(
            code.contains('.'),
            "codes are dotted `family.check`; got {code}"
        );
    }
    assert_eq!(
        CONTENT_ISSUE_CODES.len(),
        24,
        "the code vocabulary changed — update the renderer's i18n keys too"
    );
    let criticals = CONTENT_ISSUE_CODES
        .iter()
        .filter(|(_, s)| *s == Severity::Critical)
        .count();
    assert_eq!(
        criticals, 6,
        "Criticals are deterministic factual/language/structure defects only"
    );
}

/// Every issue must carry evidence a user can check for themselves, or a
/// document-wide finding with a message that stands alone.
#[test]
fn every_issue_is_evidence_backed_and_advisory() {
    let report = en_resume(EN_FABRICATED_METRIC, &en_requirements());
    for issue in &report.issues {
        assert!(
            issue.evidence.is_some(),
            "{} must name what it found",
            issue.code
        );
        assert!(
            !issue.message.trim().is_empty(),
            "{} must explain itself",
            issue.code
        );
    }
}

/// Same input, same report — a validator whose output shifts between runs
/// cannot be snapshotted, cached, or trusted.
#[test]
fn validation_is_deterministic() {
    let requirements = en_requirements();
    let first = en_resume(EN_DUPLICATES, &requirements);
    for _ in 0..5 {
        assert_eq!(
            en_resume(EN_DUPLICATES, &requirements),
            first,
            "repeated runs must produce an identical report"
        );
    }
}

// ── Threshold pins ──────────────────────────────────────────────────────────
//
// Each const gets a test. Loosening a threshold has to be a deliberate edit to
// a named expectation, not a one-character change nobody reviews.

#[test]
fn duplicate_threshold_is_pinned() {
    assert_eq!(duplicates::DUPLICATE_JACCARD_THRESHOLD, 0.8);
    assert_eq!(duplicates::MIN_TOKENS_FOR_DUPLICATE, 4);
}

#[test]
fn ats_thresholds_are_pinned() {
    assert_eq!(ats::MAX_KEYWORD_DENSITY_RATIO, 0.04);
    assert_eq!(ats::MAX_KEYWORD_OCCURRENCES, 6);
    assert_eq!(ats::MIN_TOKENS_FOR_DENSITY, 50);
    assert_eq!(ats::MAX_BULLET_CHARS, 200);
    assert_eq!(ats::MIN_BULLETS_PER_ROLE, 1);
    assert_eq!(ats::MAX_BULLETS_PER_ROLE, 6);
}

#[test]
fn voice_thresholds_are_pinned() {
    assert_eq!(voice::MIN_SENTENCES_FOR_BURSTINESS, 8);
    assert_eq!(voice::MIN_SENTENCE_LENGTH_STDDEV, 4.0);
    assert_eq!(voice::MAX_TRIPLETS_PER_TEN_SENTENCES, 2.0);
    assert_eq!(voice::EM_DASH_WORDS_PER_ALLOWED, 150);
    assert_eq!(voice::TEMPLATE_OPENER_SCAN_CHARS, 200);
    assert_eq!(voice::MIN_JOB_SPECIFIC_TOKENS_IN_LETTER, 2);
}

#[test]
fn factual_and_alignment_thresholds_are_pinned() {
    assert_eq!(factual::MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS, 4);
    assert_eq!(factual::MIN_WORDS_IN_LETTER_BODY_LINE, 8);
    assert_eq!(alignment::TOP_REQUIREMENT_MATCH_RATIO, 0.5);
    assert_eq!(consistency::MAX_PROJECT_DESCRIPTION_LINES, 3);
    assert_eq!(MIN_CHARS_FOR_LANGUAGE_CHECK, 120);
}

/// The duplicate threshold must actually BITE at its boundary: a pair just
/// under 0.8 is not a duplicate, a pair at or over it is. Pinning the number
/// without pinning its effect is how a threshold silently stops working.
#[test]
fn duplicate_threshold_boundary_behaves() {
    let a: std::collections::HashSet<String> = ["one", "two", "three", "four", "five"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    // 4 shared of 6 union = 0.666 — under the threshold.
    let under: std::collections::HashSet<String> = ["one", "two", "three", "four", "six"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert!(jaccard(&a, &under) < duplicates::DUPLICATE_JACCARD_THRESHOLD);
    // Identical sets = 1.0 — over it.
    assert!(jaccard(&a, &a) >= duplicates::DUPLICATE_JACCARD_THRESHOLD);
    // Empty vs empty must be 0.0, never "identical".
    assert_eq!(
        jaccard(
            &std::collections::HashSet::new(),
            &std::collections::HashSet::new()
        ),
        0.0
    );
}

// ── Unit checks on the shared helpers ───────────────────────────────────────

#[test]
fn contains_phrase_respects_word_boundaries() {
    assert!(contains_phrase("this is vital work", "vital"));
    assert!(
        !contains_phrase("we revitalized the pipeline", "vital"),
        "a substring inside a longer word must not fire"
    );
    assert!(contains_phrase("not just faster, but cheaper", "not just"));
    assert!(!contains_phrase("", "vital"));
    assert!(!contains_phrase("anything", ""));
}

#[test]
fn number_normalization_is_locale_neutral() {
    // Thousands grouping, three conventions, one answer.
    assert_eq!(factual::normalize_number("1,200"), "1200");
    assert_eq!(factual::normalize_number("1.200"), "1200");
    assert_eq!(factual::normalize_number("1\u{202F}200"), "1200");
    // Decimals, both conventions, one answer.
    assert_eq!(factual::normalize_number("3.5"), "3.5");
    assert_eq!(factual::normalize_number("3,5"), "3.5");
    assert_eq!(factual::normalize_number("42"), "42");
}

#[test]
fn language_normalization_matches_the_prompt_side() {
    assert_eq!(normalize_language("de-DE"), "de");
    assert_eq!(normalize_language("EN"), "en");
    assert_eq!(normalize_language(""), "en");
    assert_eq!(normalize_language("   "), "en");
}

// ── lexicon (generated by `pnpm gen:prompts`) ───────────────────────────────
//
// `lexicon.rs` is @generated and carries no hand-authored tests of its own;
// these exercise only its public interface (`lexicon::ai_tell_lexical` /
// `ai_tell_prose` / `template_openers`), never its private constants, so they
// stay valid across regeneration.

/// An unknown language falls back to English, exactly like
/// `normalizeLanguageCode` on the prompt side — never to an empty list, which
/// would silently disable the whole check.
#[test]
fn lexicon_unknown_language_falls_back_to_english() {
    let en_lexical = lexicon::ai_tell_lexical("en");
    let en_prose = lexicon::ai_tell_prose("en");
    let en_openers = lexicon::template_openers("en");
    for lang in ["fr", "zz", "", "EN"] {
        assert_eq!(
            lexicon::ai_tell_lexical(lang),
            en_lexical,
            "{lang} must fall back to the English lexicon"
        );
        assert_eq!(lexicon::ai_tell_prose(lang), en_prose);
        assert_eq!(lexicon::template_openers(lang), en_openers);
    }
    // German is a genuinely different list, not just a fallback alias.
    assert_ne!(lexicon::ai_tell_lexical("de"), en_lexical);
    assert_ne!(lexicon::template_openers("de"), en_openers);
}

/// Every entry across every language/tier must be lowercase and non-empty:
/// matching lowercases the haystack, so an upper-case entry would be dead,
/// and an empty entry would match everything.
#[test]
fn lexicon_every_entry_is_lowercase_and_non_empty() {
    let all: [&[&str]; 6] = [
        lexicon::ai_tell_lexical("en"),
        lexicon::ai_tell_lexical("de"),
        lexicon::ai_tell_prose("en"),
        lexicon::ai_tell_prose("de"),
        lexicon::template_openers("en"),
        lexicon::template_openers("de"),
    ];
    for list in all {
        assert!(!list.is_empty(), "no list may be empty");
        for entry in list {
            assert!(!entry.trim().is_empty(), "empty entry in lexicon");
            assert_eq!(
                *entry,
                entry.to_lowercase(),
                "lexicon entries must be lowercase; got {entry:?}"
            );
        }
    }
}

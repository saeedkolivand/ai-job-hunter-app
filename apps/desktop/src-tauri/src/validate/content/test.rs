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
const EN_PARAPHRASED: &str = include_str!("fixtures/en_generated_paraphrased.txt");
const DE_PARAPHRASED: &str = include_str!("fixtures/de_generated_paraphrased.txt");
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

/// The byte-copy fixtures above only prove the validators do not fire on text
/// they have already seen. Real output is a PARAPHRASE: the same facts, reworded
/// bullets, links written in a different but equivalent form, company names
/// shortened, an open-ended span resolved to a concrete year. Every one of those
/// is a legitimate tailoring decision, and every one of them produced a false
/// Critical before this pass.
///
/// Criticals are the bar here (not "no issues at all"): a rewording may
/// legitimately move a Warning, but nothing about restating a true fact may ever
/// say the candidate fabricated something.
#[test]
fn paraphrased_but_truthful_resume_raises_no_criticals() {
    let report = en_resume(EN_PARAPHRASED, &en_requirements());
    let criticals: Vec<&ContentIssue> = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .collect();
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
    let report = validate_content(&ContentInput {
        generated: DE_PARAPHRASED,
        source_resume: DE_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &["Docker und Kubernetes im Produktivbetrieb".to_string()],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    let criticals: Vec<&ContentIssue> = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .collect();
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

// ── False-positive regressions (one per reproduced Critical) ────────────────
//
// Every test below is a document that states nothing but the truth and was
// nonetheless issued a Critical. Each names the mechanism that produced it.

/// H1a — a `·`-separated STACK line is a technology list, not a link list.
/// `Socket.IO` is a bare `.io` host, `Bun.sh` a bare `.sh` host, and trimming
/// one technology out of the stack while tailoring reported the other as a
/// project link that had been "missing or altered".
#[test]
fn stack_line_library_names_are_never_read_as_project_links() {
    assert!(
        factual::urls_in("Node.js · Socket.IO · Bun.sh · Deno.dev").is_empty(),
        "library names are not links; got {:?}",
        factual::urls_in("Node.js · Socket.IO · Bun.sh · Deno.dev")
    );
    // …while a real link in any of the accepted forms still is one.
    for real in [
        "https://ledger.example.dev",
        "www.ledger.example.dev",
        "github.com/janedoe/ledger",
        "ledger.example.dev/docs",
    ] {
        assert_eq!(
            factual::urls_in(real).len(),
            1,
            "{real} must still be recognised as a link"
        );
    }

    let source = "PROJECTS\n\n\
                  **Chat Relay** · https://relay.example.dev\n\
                  Node.js · Socket.IO · Bun.sh\n\
                  A tiny websocket relay.\n";
    // Tailoring dropped one technology from the stack line. Nothing else moved.
    let trimmed = "PROJECTS\n\n\
                   **Chat Relay** · https://relay.example.dev\n\
                   Node.js · Socket.IO\n\
                   A tiny websocket relay.\n";
    silent(
        &report_for(trimmed, source, EN_JOB_AD, &[]),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
}

/// H1b — the same link written a different way is the same link. Compared on a
/// canonical key (scheme dropped, host lowercased, trailing `/` removed,
/// markdown href unwrapped); still REPORTED verbatim when it genuinely differs.
#[test]
fn normalized_link_forms_are_not_altered_links() {
    assert_eq!(
        factual::canonical_link("HTTPS://GitHub.com/janedoe/ledger/"),
        "github.com/janedoe/ledger"
    );
    assert_eq!(
        factual::canonical_link("github.com/janedoe/ledger"),
        factual::canonical_link("https://github.com/janedoe/ledger")
    );

    let source = "PROJECTS\n\n\
                  **Ledger CLI** · https://ledger.example.dev · https://github.com/janedoe/ledger\n\
                  Rust · SQLite\n";
    for (name, generated) in [
        (
            "scheme dropped",
            "PROJECTS\n\n\
             **Ledger CLI** · ledger.example.dev/ · github.com/janedoe/ledger\n\
             Rust · SQLite\n",
        ),
        (
            "trailing slash + host case",
            "PROJECTS\n\n\
             **Ledger CLI** · https://ledger.example.dev/ · HTTPS://GitHub.com/janedoe/ledger\n\
             Rust · SQLite\n",
        ),
        (
            "markdown links",
            "PROJECTS\n\n\
             **Ledger CLI** · [Website](https://ledger.example.dev) · \
             [GitHub](https://github.com/janedoe/ledger)\n\
             Rust · SQLite\n",
        ),
    ] {
        let report = report_for(generated, source, EN_JOB_AD, &[]);
        assert!(
            !codes(&report).contains(&FACTUAL_ALTERED_PROJECT_LINK),
            "{name} is the same link written differently; got {:#?}",
            report
                .issues
                .iter()
                .filter(|i| i.code == FACTUAL_ALTERED_PROJECT_LINK)
                .collect::<Vec<_>>()
        );
    }

    // A genuinely different path is still Critical, and the evidence quotes the
    // span verbatim rather than the canonical key.
    let altered = "PROJECTS\n\n\
                   **Ledger CLI** · https://ledger.example.dev · \
                   https://github.com/someone-else/ledger\n\
                   Rust · SQLite\n";
    let report = report_for(altered, source, EN_JOB_AD, &[]);
    let evidence: Vec<&str> = fired(&report, FACTUAL_ALTERED_PROJECT_LINK)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert!(
        evidence.contains(&"https://github.com/someone-else/ledger"),
        "evidence must be the verbatim span, not the comparison key; got {evidence:?}"
    );
}

/// H1c — a link with a non-ASCII character ABORTED the app. `canonical_link`
/// stripped the scheme with `&s[..8]`, a fixed BYTE offset, so any URL whose
/// 8th byte fell inside a multibyte char panicked ("byte index 8 is not a char
/// boundary"). Release builds are `panic = "abort"`: the process died mid-run,
/// before the generated document was saved. A German or French project domain is
/// all it took.
///
/// Every boundary the function cuts at gets a straddling char here — byte 8
/// (`https://`) and byte 7 (`http://`) — plus the two-, three- and four-byte
/// widths, so a future "just slice off the scheme" rewrite fails loudly.
#[test]
fn accented_project_links_are_keyed_without_panicking() {
    // Boundary 8: `é` occupies bytes 7-8 in all three of these, so the very
    // first loop iteration (`https://`, len 8) cut inside it.
    assert_eq!(
        factual::canonical_link("www.café-berlin.de"),
        "www.café-berlin.de"
    );
    assert_eq!(factual::canonical_link("ab.com/éx"), "ab.com/éx");
    // …and stripping the scheme that DOES match still leaves the tail intact.
    assert_eq!(factual::canonical_link("http://éxample.com"), "éxample.com");
    assert_eq!(
        factual::canonical_link("http://éxample.com"),
        factual::canonical_link("https://Éxample.com/"),
        "the same host written three ways is one key"
    );

    // Boundary 7: `é` occupies bytes 6-7, so `&s[..8]` is legal but the SECOND
    // iteration (`http://`, len 7) is the one that cut inside the char.
    assert_eq!(factual::canonical_link("abcdefé.com/x"), "abcdefé.com/x");

    // Three- and four-byte chars across the same offsets.
    assert_eq!(factual::canonical_link("abc.de/日本語"), "abc.de/日本語");
    assert_eq!(factual::canonical_link("abc.de/🚀x"), "abc.de/🚀x");
    // Shorter than either scheme — the length guard, not the boundary check.
    assert_eq!(factual::canonical_link("é.de"), "é.de");
    // Host lowercased, accents preserved; path case and accents untouched.
    assert_eq!(
        factual::canonical_link("HTTPS://Café.Example.DE/Ünicode/"),
        "café.example.de/Ünicode"
    );

    // The three verifier-reproduced URLs, through the FULL validate_content
    // path (the way the panic actually reached a user: a Projects entry).
    let doc = "PROJECTS\n\n\
               **Café Ledger** · www.café-berlin.de · http://éxample.com · ab.com/éx\n\
               Rust · SQLite\n\
               A double-entry bookkeeping tool for freelancers.\n";
    let report = report_for(doc, doc, EN_JOB_AD, &[]);
    silent(&report, FACTUAL_ALTERED_PROJECT_LINK);
    let criticals: Vec<&ContentIssue> = report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .collect();
    assert!(
        criticals.is_empty(),
        "an accented but truthful project link must produce a clean report; got {criticals:#?}"
    );
}

/// H2 — a shortened company name is normal tailoring, not a dropped role.
/// "IBM Deutschland GmbH" has exactly one 4+ character token that is not a legal
/// form — "deutschland" — so writing the employer as "IBM" made the entry look
/// like it had vanished.
#[test]
fn shortened_company_names_are_not_dropped_roles() {
    let source = "EXPERIENCE\n\n\
                  Software Engineer | IBM Deutschland GmbH | 2018 - 2021\n\
                  - Built the billing service in Java\n\n\
                  Senior Developer | SAP Deutschland SE | 2015 - 2018\n\
                  - Ran the integration platform\n";
    let shortened = "EXPERIENCE\n\n\
                     Software Engineer | IBM | 2018 - 2021\n\
                     - Built the billing service in Java\n\n\
                     Senior Developer | SAP | 2015 - 2018\n\
                     - Ran the integration platform\n";
    silent(
        &report_for(shortened, source, EN_JOB_AD, &[]),
        FACTUAL_DROPPED_ROLE,
    );

    // A genuinely dropped employer still fires — and geography alone does NOT
    // count as evidence that it survived.
    let dropped = "EXPERIENCE\n\n\
                   Software Engineer | IBM | 2018 - 2021\n\
                   - Built the billing service in Java\n\
                   - Worked with colleagues across Deutschland\n";
    let report = report_for(dropped, source, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_DROPPED_ROLE);
    assert_eq!(hits.len(), 1, "only the SAP entry is gone; got {hits:#?}");
    assert!(hits[0]
        .evidence
        .as_deref()
        .is_some_and(|e| e.contains("SAP")));

    // A two-character token still needs a word boundary: "SAP" is not evidenced
    // by "sapphire".
    let lookalike = "EXPERIENCE\n\n\
                     Software Engineer | IBM | 2018 - 2021\n\
                     - Built the sapphire billing service in Java\n";
    fired(
        &report_for(lookalike, source, EN_JOB_AD, &[]),
        FACTUAL_DROPPED_ROLE,
    );
}

/// H3 — when the detector reads the SOURCE the same "wrong" way it reads the
/// output, the detector is the unreliable party, not the document. Firing there
/// produced a Critical the user cannot act on AND blanked `keywordCoverage`,
/// taking every alignment finding down with it.
#[test]
fn language_critical_is_withheld_when_the_source_reads_the_same_way() {
    // Both documents are German; the target language says English.
    let report = validate_content(&ContentInput {
        generated: DE_CLEAN,
        source_resume: DE_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    assert!(
        !codes(&report).contains(&CONTENT_LANGUAGE_MISMATCH),
        "source and output detect identically — that is a detector disagreement, \
         not a generation defect; got {:?}",
        codes(&report)
    );
    assert!(
        report.metrics.keyword_coverage.is_some(),
        "the metrics must survive a withheld language Critical"
    );

    // The real defect — an English source, a German output — still fires.
    let real = en_resume(EN_WRONG_LANGUAGE, &en_requirements());
    let hits = fired(&real, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
}

/// H4 — "still there" is spelled in more than one way. A source that writes
/// `seit 2021`, `since 2021` or a bare `2021 –` carries no `Present` marker, and
/// the old `|| !source_open_ended` arm turned every one of those into a Critical
/// the moment the output resolved the span to a concrete year.
#[test]
fn open_ended_spans_without_a_present_marker_resolve_cleanly() {
    use crate::documents::evidence::is_open_ended;
    for span in ["2021 - Present", "seit 2021", "since 2021", "2021 –"] {
        assert!(is_open_ended(span), "{span:?} is an open-ended span");
    }
    for closed in ["2018 - 2021", "Jan 2018 to Mar 2021", "presented in 2019"] {
        assert!(!is_open_ended(closed), "{closed:?} has an end");
    }

    for source_span in ["seit 2021", "since 2021", "2021 –"] {
        let source = format!(
            "EXPERIENCE\n\nSenior Engineer | Acme Payments | {source_span}\n\
             - Shipped the ledger service\n"
        );
        let resolved = "EXPERIENCE\n\nSenior Engineer | Acme Payments | 2021 - 2026\n\
                        - Shipped the ledger service\n";
        silent(
            &report_for(resolved, &source, EN_JOB_AD, &[]),
            FACTUAL_UNSUPPORTED_DATE,
        );
    }

    // An invented EARLIER year still has no explanation.
    let source = "EXPERIENCE\n\nSenior Engineer | Acme Payments | seit 2021\n\
                  - Shipped the ledger service\n";
    let backdated = "EXPERIENCE\n\nSenior Engineer | Acme Payments | 2015 - 2026\n\
                     - Shipped the ledger service\n";
    let report = report_for(backdated, source, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_UNSUPPORTED_DATE);
    assert_eq!(hits[0].evidence.as_deref(), Some("2015"));
}

/// H5 — every present-tense marker hides inside an ordinary word. Matched as
/// substrings, "presented", "knowledge", "currently" and "actually" all turned
/// an ordinary bullet into a date context, and any year in it into a Critical.
#[test]
fn present_markers_only_match_whole_words() {
    use crate::documents::evidence::looks_like_date_span;
    for prose in [
        "presented the roadmap to the board",
        "knowledge sharing across the team",
        "currently owned by the platform group",
        "actually shipped ahead of schedule",
    ] {
        assert!(
            !looks_like_date_span(prose),
            "{prose:?} is prose, not a date span"
        );
    }
    assert!(looks_like_date_span("2021 - Present"));
    assert!(looks_like_date_span("Heute"));

    // End to end: a truthful bullet with a year the source does not carry, in a
    // line whose only "date marker" is the word "Presented".
    let source = "EXPERIENCE\n\nAcme Payments | 2020 - 2021\n\
                  - Shipped the ledger service\n";
    let generated = "EXPERIENCE\n\nAcme Payments | 2020 - 2021\n\
                     - Presented the 2019 roadmap review to the board\n";
    silent(
        &report_for(generated, source, EN_JOB_AD, &[]),
        FACTUAL_UNSUPPORTED_DATE,
    );
}

/// H6 — the contact-cluster Critical needs a REAL address, and the name-like
/// line has to be the section's first. A stray `@` in a bullet and a short line
/// anywhere next to it were enough to claim the document had two headers.
#[test]
fn header_in_body_needs_a_real_address_directly_under_the_heading() {
    // An `@` that is not an address.
    let handle = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                  Acme | 2021 - Present\n\
                  On call\nOwned the @payments rotation for two years\n";
    silent(
        &report_for(handle, handle, EN_JOB_AD, &[]),
        ATS_HEADER_IN_BODY,
    );

    // A real address deeper inside a section, under a body line rather than at
    // the top of it.
    let body_address = "Jane Doe\njane@example.com\n\nPUBLICATIONS\n\n\
                        Scaling ledgers under load\n\
                        Rust Conf\n\
                        Recordings are available from talks@rustconf.example.com\n";
    silent(
        &report_for(body_address, body_address, EN_JOB_AD, &[]),
        ATS_HEADER_IN_BODY,
    );

    // The real thing — a name on the section's first line, an address under it.
    let cluster = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                   Acme | 2021 - Present\n- Led the migration\n\n\
                   REFERENCES\n\nJohn Smith\njohn.smith@acme.example.com\n";
    let report = report_for(cluster, cluster, EN_JOB_AD, &[]);
    let hits = fired(&report, ATS_HEADER_IN_BODY);
    assert_eq!(hits[0].severity, Severity::Critical);
}

/// Two unrelated employers whose spans merely touch the same year are not the
/// same employer. `titled_entries` keeps the date span inside its company
/// string, so "Globex … | 2018 - 2021" and "Initech … | 2015 - 2018" shared the
/// token "2018" and the second entry's title was reported as drift from the
/// first's.
#[test]
fn title_drift_does_not_match_employers_on_a_shared_year() {
    let doc = "EXPERIENCE\n\n\
               Backend Developer | Globex Logistics | 2018 - 2021\n\
               - Built the billing API in Python\n\n\
               IT Consultant | Initech Systems | 2015 - 2018\n\
               - Ran the reporting service\n";
    silent(
        &report_for(doc, doc, EN_JOB_AD, &[]),
        CONSISTENCY_TITLE_DRIFT,
    );
}

/// M1 — a restated number is not a fabricated one. `10k` and `10,000` are the
/// same figure; so are "doubled" and "2x".
#[test]
fn restated_numbers_are_not_fabricated_metrics() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                  - Handled 10k requests per second at peak\n\
                  - Doubled throughput on the ledger service\n";
    let restated = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Handled 10,000 requests per second at peak\n\
                    - Lifted throughput 2x on the ledger service\n";
    silent(
        &report_for(restated, source, EN_JOB_AD, &[]),
        FACTUAL_UNSOURCED_METRIC,
    );

    // Millisecond figures must not read as millions: `480ms` is 480, not
    // 480 000 000, and inventing that expansion would silence a real check.
    let latency = "EXPERIENCE\n\nAcme | 2021 - Present\n- Cut latency from 480ms to 90ms\n";
    let invented = "EXPERIENCE\n\nAcme | 2021 - Present\n- Cut latency from 480ms to 250ms\n";
    let report = report_for(invented, latency, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("250"));
}

/// M2 — the metric check skipped anything `is_contact_shaped` accepted, which
/// includes any line with two `·` separators or the word "website". A body
/// bullet was therefore exempt from the fabricated-metric Critical entirely.
#[test]
fn a_separator_laden_body_bullet_stays_metric_checkable() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n- Rebuilt the marketing website\n";
    let evasive = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                   - Rebuilt the marketing website · cut page weight · shipped 12000 pages\n";
    let report = report_for(evasive, source, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("12000"));

    // The header band is still skipped: a phone number is not a claim of impact.
    let header = "Jane Doe\njane@example.com | +49 30 1234567 | 10115 Berlin\n\n\
                  EXPERIENCE\n\nAcme | 2021 - Present\n- Led the migration\n";
    silent(
        &report_for(header, header, EN_JOB_AD, &[]),
        FACTUAL_UNSOURCED_METRIC,
    );
}

/// M5 — a Snowball stem must never reach the user. "kubernet is listed under
/// skills but never appears in your experience" is a finding nobody can act on.
#[test]
fn user_facing_messages_carry_readable_words_not_stems() {
    let doc = "EXPERIENCE\n\nAcme | 2021 - Present\n- Shipped Docker containers to production\n\n\
               SKILLS\n\nDocker · Kubernetes\n";
    let report = report_for(doc, doc, EN_JOB_AD, &[]);
    let hits = fired(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("kubernetes"),
        "the evidence must be the readable word, not the stem"
    );
    assert!(
        hits[0].message.contains("\"kubernetes\""),
        "the message must quote the readable word too; got {:?}",
        hits[0].message
    );
}

/// M6 — coverage moves in whole keyword steps, so any drop at all fired on every
/// legitimate edit. Only a drop of at least
/// [`alignment::MIN_COVERAGE_DROP_POINTS`] percentage points is reported.
#[test]
fn low_coverage_tolerates_a_drop_smaller_than_the_threshold() {
    // 25 posting keywords → each one is worth exactly 4 percentage points.
    let words: Vec<String> = (0..25).map(|i| format!("skillword{i}")).collect();
    let job = words.join(" ");
    let resume = |kept: usize| {
        format!(
            "EXPERIENCE\n\nAcme | 2021 - Present\n- Delivered {}\n",
            words[..kept].join(" ")
        )
    };
    let source = resume(25);

    // One keyword lost = 4 points, under the threshold.
    silent(
        &report_for(&resume(24), &source, &job, &[]),
        ALIGNMENT_LOW_COVERAGE,
    );
    // Two = 8 points, over it.
    fired(
        &report_for(&resume(23), &source, &job, &[]),
        ALIGNMENT_LOW_COVERAGE,
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
        25,
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

/// The serialized shape of a report IS a wire contract: `ContentReportPayload`
/// in `packages/shared/src/ipc/contracts/resume.ts` is a hand-written mirror of
/// this struct, and nothing in the build compares the two. This test pins the
/// exact key set and the exact serialization of the three fields that are
/// `Option` on the Rust side.
///
/// `section`, `evidence` and `keywordCoverage` serialize as `null`, NOT omitted
/// — there is no `skip_serializing_if` on them, and the renderer's types say
/// `string | null`. If a future edit adds one, TypeScript will keep compiling
/// and every `report.metrics.keywordCoverage === null` branch will silently stop
/// matching. That is what this test is here to catch.
#[test]
fn serialized_report_matches_the_typescript_wire_mirror() {
    let report = ContentReport {
        ok: false,
        issues: vec![
            ContentIssue {
                severity: Severity::Critical,
                code: FACTUAL_UNSOURCED_METRIC,
                section: None,
                message: "m".to_string(),
                evidence: None,
            },
            ContentIssue {
                severity: Severity::Warning,
                code: ALIGNMENT_LOW_COVERAGE,
                section: Some("Experience".to_string()),
                message: "m".to_string(),
                evidence: Some("40% vs 60%".to_string()),
            },
        ],
        metrics: ContentMetrics {
            keyword_coverage: None,
            top_requirement_hits: 3,
            duplicate_ratio: 0.25,
            roles_source: 2,
            roles_output: 1,
        },
    };
    let value = serde_json::to_value(&report).expect("a report must serialize");

    let keys = |v: &serde_json::Value| -> Vec<String> {
        let mut k: Vec<String> = v
            .as_object()
            .expect("object")
            .keys()
            .map(String::from)
            .collect();
        k.sort();
        k
    };
    assert_eq!(keys(&value), ["issues", "metrics", "ok"]);
    assert_eq!(
        keys(&value["issues"][0]),
        ["code", "evidence", "message", "section", "severity"],
        "an issue's key set is the renderer's contract"
    );
    assert_eq!(
        keys(&value["metrics"]),
        [
            "duplicateRatio",
            "keywordCoverage",
            "rolesOutput",
            "rolesSource",
            "topRequirementHits"
        ],
        "metrics keys are camelCase on the wire"
    );

    // Severity is lowercase, matching `'critical' | 'warning'` in TS.
    assert_eq!(value["issues"][0]["severity"], "critical");
    assert_eq!(value["issues"][1]["severity"], "warning");

    // The three nullable fields are PRESENT and null, never absent.
    assert!(value["issues"][0]["section"].is_null());
    assert!(value["issues"][0]["evidence"].is_null());
    assert!(value["metrics"]["keywordCoverage"].is_null());
    assert_eq!(value["issues"][1]["section"], "Experience");
    assert_eq!(value["issues"][1]["evidence"], "40% vs 60%");
    assert_eq!(value["issues"][1]["code"], ALIGNMENT_LOW_COVERAGE);
}

/// M-3: an uncapped `issues` list can grow the serialized report past the
/// save path's `QUALITY_REPORT_MAX_BYTES` (256 KiB) clamp, which then
/// truncates it mid-JSON, fails to parse, and silently discards a fresh
/// report. `MAX_CONTENT_ISSUES` bounds the list at the source so that clamp
/// is never reached — proved here by pinning both the cap AND that a
/// pathological duplicate-bullet flood past it still: stays `ok` exactly
/// when it should, truncates to the cap plus one trailing marker, and
/// serializes comfortably under the byte clamp.
#[test]
fn oversized_issue_list_is_capped_with_a_visible_truncation_marker() {
    assert_eq!(MAX_CONTENT_ISSUES, 200);

    // Start from the CLEAN fixture (so every factual/structure check that
    // compares against the source résumé stays satisfied — zero Criticals),
    // then append one clique of 250 byte-identical bullets.
    // `duplicates::validate` marks every bullet after the first as "involved"
    // once it matches an earlier one, so a clique of k identical bullets
    // fires k-1 `duplicate.bullet` Warnings — 249 here, comfortably past the
    // cap, from only 250 extra bullets (well under
    // `duplicates::MAX_DUP_BULLETS` = 400, so this test stays orthogonal to
    // the M-2 fix).
    let mut generated = EN_CLEAN.to_string();
    for _ in 0..250 {
        generated
            .push_str("- Delivered feature rollout across the payments platform for the team\n");
    }

    let report = en_resume(&generated, &en_requirements());

    assert_eq!(
        report.issues.len(),
        MAX_CONTENT_ISSUES + 1,
        "must truncate to the cap plus exactly one trailing marker"
    );
    let marker = report.issues.last().expect("at least one issue");
    assert_eq!(marker.code, REPORT_TRUNCATED);
    assert_eq!(marker.severity, Severity::Warning);
    assert!(
        !marker.message.trim().is_empty() && marker.evidence.is_some(),
        "the marker itself must still be evidence-backed and advisory"
    );
    assert!(
        report.ok,
        "no Critical was ever produced here, so truncating warnings must not flip ok"
    );

    let bytes = serde_json::to_vec(&report).expect("report must serialize");
    assert!(
        bytes.len() < 256 * 1024,
        "capped report must stay comfortably under the save path's byte clamp, got {} bytes",
        bytes.len()
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
    assert_eq!(duplicates::MAX_DUP_BULLETS, 400);
}

/// M-2: the near-duplicate scan is O(bullets²) — before [`duplicates::MAX_DUP_BULLETS`],
/// a malformed/hostile document with thousands of bullet-shaped lines held a
/// tokio worker for seconds on every save. A pathological bullet count must
/// both (a) return fast and (b) still flag a genuine duplicate placed well
/// inside the cap — the cap must never silently disable the check entirely.
#[test]
fn pathological_bullet_count_returns_quickly_and_still_flags_a_duplicate_within_the_cap() {
    let mut generated =
        String::from("Jane Doe\n\nEXPERIENCE\n\nEngineer | Acme | 2020 - Present\n");
    // A genuine duplicate pair, placed at the very start — well inside
    // MAX_DUP_BULLETS regardless of how many filler bullets follow.
    generated.push_str(
        "- Cut checkout latency from 480ms to 90ms with a Redis cache in front of the ledger\n",
    );
    generated.push_str(
        "- Cut checkout latency from 480ms to 90ms with a Redis cache in front of the ledger service\n",
    );
    // Far more bullets than the cap — a real résumé never has anywhere close
    // to this many.
    for i in 0..6_000 {
        generated.push_str(&format!(
            "- Delivered feature rollout {i} across the payments platform for the {i} team\n"
        ));
    }

    let input = ContentInput {
        generated: &generated,
        source_resume: EN_SOURCE,
        job_ad: EN_JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    };
    let ctx = Analysis::new(&input);

    let start = std::time::Instant::now();
    let (issues, ratio) = duplicates::validate(&ctx);
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "the pairwise duplicate scan must stay bounded past MAX_DUP_BULLETS bullets, \
         took {elapsed:?}"
    );
    assert!(
        issues.iter().any(|i| i.code == DUPLICATE_BULLET),
        "a genuine duplicate well inside the cap must still be flagged"
    );
    assert!(ratio > 0.0, "duplicateRatio must reflect the flagged pair");
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
    assert_eq!(alignment::MIN_COVERAGE_DROP_POINTS, 5.0);
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

/// L-3: a control character (a raw newline in particular) must never survive
/// into the 2-char result — it would otherwise reach `ctx.lang`, and from
/// there this module's own `validate:content` span text and a
/// `content.language_mismatch` issue's `evidence`, a log-injection
/// primitive. `.trim()` alone does NOT catch this: it only strips
/// leading/trailing whitespace, and a newline in the MIDDLE of the string
/// survives trim untouched.
#[test]
fn language_normalization_strips_control_characters() {
    assert_eq!(
        normalize_language("a\nb"),
        "ab",
        "an internal control character must never reach the normalized code"
    );
    assert!(!normalize_language("a\nb").contains('\n'));
    assert_eq!(normalize_language("\t\u{7}de"), "de");
    // A wholly-control-character input degrades to the same "en" default an
    // empty string gets — never an empty-but-not-quite string.
    assert_eq!(normalize_language("\n\n"), "en");
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

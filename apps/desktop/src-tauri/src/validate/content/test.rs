//! Content-validation tests.
//!
//! Two layers, on purpose:
//!
//! * **Fixture integration tests** run the whole `validate_content` dispatcher
//!   over realistic en/de résumé + job-ad + generated triples. Each defect
//!   fixture differs from `en_generated_clean.txt` by exactly ONE edit, so a
//!   test that fires the wrong code says so loudly.
//!
//!   The `*_generated_clean.txt` pair are genuine REWORDINGS of their sources —
//!   same employers, dates, links and figures, different sentences. They used
//!   to be near-byte-copies, which made the suite's strongest assertion
//!   (`clean_resume_produces_no_issues_at_all`, an empty report) prove only that
//!   the validators do not fire on text they have already seen. Real generator
//!   output is never a copy, and every false positive this file records was
//!   found on rephrased text.
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

/// The first `code` issue's evidence, or `None` when `code` never fired.
///
/// The alternative, `fired(report, code)[0].evidence`, asserts inside the
/// SHARED helper, so the failure can only ever name the rule — and a test that
/// runs one claim over several fixtures (the apostrophe-fold pair below renders
/// the same letter twice) then cannot say WHICH fixture missed. Returning an
/// `Option` moves the whole claim into the caller's own `assert_eq!`, whose
/// message names the rule, the fixture, and what the report carried instead.
///
/// `None` covers both "the rule did not fire" and "it fired with no evidence";
/// callers print [`codes`] alongside, which tells the two apart.
fn first_evidence<'a>(report: &'a ContentReport, code: &str) -> Option<&'a str> {
    report
        .issues
        .iter()
        .find(|i| i.code == code)
        .and_then(|i| i.evidence.as_deref())
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
        assert_eq!(
            report.metrics.top_requirement_hits, None,
            "an uncomparable posting measures nothing — None, never a confident 0"
        );
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
    let metrics = factual::metrics_in(
        "EXPERIENCE\n\nAcme | 2021 - 2024\n- Shipped in 1999\n",
        DocKind::Resume,
    );
    assert!(
        metrics.is_empty(),
        "1900–2099 must be excluded from metric extraction; got {metrics:?}"
    );
    // Just outside the window, a four-digit run IS a quantity.
    let quantity = factual::metrics_in(
        "EXPERIENCE\n\nAcme\n- Processed 4500 orders a day\n",
        DocKind::Resume,
    );
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
        factual::metrics_in(text, DocKind::Resume).is_empty(),
        "the header band must be skipped entirely; got {:?}",
        factual::metrics_in(text, DocKind::Resume)
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
///
/// ⚠️ The positive fixture was `REFERENCES / John Smith / john.smith@…` until
/// R13-W1: that is a REFERENCES LIST, not a second header, and this test was
/// pinning the false positive as the expected behaviour. The block is now the
/// candidate's own header repeated, which is what the Critical claims. The
/// referee shape has its own test —
/// `a_reference_block_is_not_a_second_contact_block`.
#[test]
fn header_in_body_needs_a_contact_cluster_not_just_an_email() {
    let with_cluster = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                        Acme | 2021 - Present\n- Led the migration\n\n\
                        CONTACT\n\nJane Doe\njane@example.com\n";
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
    // first loop iteration (`https://`, len 8) cut inside it. (The `www.` here
    // is stripped as of R14-F3 — `a_www_prefix_is_the_same_link` owns that
    // rule and its own boundary twin; what this row still pins is that the
    // accented tail survives whatever the prefix loop did.)
    assert_eq!(
        factual::canonical_link("www.café-berlin.de"),
        "café-berlin.de"
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

    // The real thing — the CANDIDATE's name on the section's first line, their
    // own address under it. (Was a referee's until R13-W1; see
    // `a_reference_block_is_not_a_second_contact_block` for why that shape is
    // not a second header.)
    let cluster = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                   Acme | 2021 - Present\n- Led the migration\n\n\
                   CONTACT\n\nJane Doe\njane@example.com\n";
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
    assert_eq!(report.metrics.top_requirement_hits, Some(1));
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

// ── Round-3 regressions (one per reproduced review finding) ─────────────────

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

/// F2 — `voice.generic_letter` measures the letter against the POSTING, so it
/// is a posting comparison and owes the module's own invariant: once the output
/// is not in the target language, every posting comparison is suppressed
/// (`alignment::validate` returns early on exactly this). Across two languages
/// the letter shares no vocabulary with the ad, so the check reported "0 things
/// specific to the posting" underneath the one finding that matters.
#[test]
fn generic_letter_is_suppressed_on_a_language_mismatch() {
    let german_letter = "Sehr geehrte Damen und Herren, hiermit bewerbe ich mich auf die \
                         ausgeschriebene Stelle in Ihrem Haus. Über eine Rückmeldung von Ihnen \
                         würde ich mich sehr freuen und stehe für ein Gespräch gerne zur \
                         Verfügung. Mit freundlichen Grüßen, Jane Doe";
    let report = en_letter(german_letter);
    fired(&report, CONTENT_LANGUAGE_MISMATCH);
    silent(&report, VOICE_GENERIC_LETTER);

    // …and a same-language letter that really is generic still fires, so the
    // guard suppresses the cascade rather than the check.
    let generic = "Dear Hiring Manager, I would be a great addition to your organisation and \
                   look forward to hearing from you soon. Best regards, Jane";
    fired(&en_letter(generic), VOICE_GENERIC_LETTER);
}

/// F3 — the prompt's own ban is scoped: "Applies to any words YOU introduce,
/// never to exact job-ad keywords already grounded in the résumé"
/// (`natural-voice.ts`, the single source `lexicon.rs` is generated from). A
/// phrase the candidate's own résumé already uses is not a word the model
/// introduced, so flagging it made the validator stricter than the prompt it
/// exists to check — and told a finance candidate their own "leverage
/// calculator" was an AI tell.
///
/// The exemption is per-PHRASE: a source containing "leverage" exempts
/// "leverage" and nothing else.
#[test]
fn ai_tell_phrases_already_in_the_source_resume_are_exempt() {
    let source = "EXPERIENCE\n\nQuant Developer | Acme Capital | 2021 - Present\n\
                  - Built the leverage calculator the trading desk prices margin with\n";
    let generated = "EXPERIENCE\n\nQuant Developer | Acme Capital | 2021 - Present\n\
                     - Rebuilt the leverage calculator behind a seamless margin workflow\n";
    let report = report_for(generated, source, EN_JOB_AD, &[]);
    let phrases: Vec<&str> = report
        .issues
        .iter()
        .filter(|i| i.code == VOICE_AI_TELL_LEXICAL)
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert!(
        !phrases.contains(&"leverage"),
        "the source résumé already says \"leverage\" — the model did not introduce it; \
         got {phrases:?}"
    );
    assert!(
        phrases.contains(&"seamless"),
        "\"seamless\" appears nowhere in the source, so the ban still applies to it; \
         got {phrases:?}"
    );
}

/// F4 — `PERCENT_RE` and `INTEGER_RE` both capture a fabricated figure of 100
/// or more written with a percent sign ("150%" and "150"), and the dedupe key
/// was the RAW matched text, so one invented number produced two Criticals on
/// the same span. Sourcing is decided on the normalized number, so the dedupe
/// has to be too.
#[test]
fn one_fabricated_percentage_produces_exactly_one_critical() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n- Cut checkout latency from 480ms to 90ms\n";
    let three_digit =
        "EXPERIENCE\n\nAcme | 2021 - Present\n- Grew signups 150% after the rewrite\n";
    let report = report_for(three_digit, source, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(
        hits.len(),
        1,
        "one invented figure is one finding, not one per regex that saw it; got {hits:#?}"
    );
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("150%"),
        "the evidence must still quote the span exactly as written"
    );

    // A two-digit percentage never had the collision (`INTEGER_RE` needs three
    // significant digits) and must keep firing exactly once — the dedupe change
    // must not swallow a second, genuinely DIFFERENT number.
    let two_numbers = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                       - Grew signups 72% and cut refunds 150% after the rewrite\n";
    let report = report_for(two_numbers, source, EN_JOB_AD, &[]);
    let evidence: Vec<&str> = fired(&report, FACTUAL_UNSOURCED_METRIC)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert_eq!(
        evidence,
        vec!["72%", "150%"],
        "two different invented figures are two findings"
    );
}

/// F5 — `dropped_role_issues` searches the whole generated document once per
/// SOURCE entry, and `title_drift_issues` compares every generated entry
/// against every source entry: both are O(entries × document) over inputs that
/// admit 200KB each. The sibling near-duplicate scan was explicitly capped for
/// the same risk class ([`duplicates::MAX_DUP_BULLETS`]); this pins the
/// analogous entry cap and proves it BITES without disabling either check.
#[test]
fn entry_scans_are_capped_like_the_duplicate_scan() {
    assert_eq!(factual::MAX_SCANNED_ENTRIES, 200);
    let over = factual::MAX_SCANNED_ENTRIES + 50;
    let doc = |title: &str| {
        let mut out = String::from("EXPERIENCE\n\n");
        for i in 0..over {
            out.push_str(&format!(
                "{title} | Zetacorp{i} Systems | 2010 - 2011\n- Shipped release {i}\n\n"
            ));
        }
        out
    };
    let source = doc("Backend Engineer");
    let count = |issues: Vec<ContentIssue>, code: &str| {
        issues.into_iter().filter(|i| i.code == code).count()
    };

    // `factual.dropped_role`: the generated document mentions none of them.
    let none_survive = "EXPERIENCE\n\nBackend Engineer | Different Employer | 2020 - 2021\n\
                        - Shipped one thing\n";
    let input = ContentInput {
        generated: none_survive,
        source_resume: &source,
        job_ad: EN_JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    };
    assert_eq!(
        count(
            factual::validate(&Analysis::new(&input)),
            FACTUAL_DROPPED_ROLE
        ),
        factual::MAX_SCANNED_ENTRIES,
        "the dropped-role scan must stop AT the cap — and not before it"
    );

    // `consistency.title_drift`: the same employers, a wholly different title.
    let drifted = doc("Pastry Chef");
    let input = ContentInput {
        generated: &drifted,
        source_resume: &source,
        job_ad: EN_JOB_AD,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    };
    assert_eq!(
        count(
            consistency::validate(&Analysis::new(&input)),
            CONSISTENCY_TITLE_DRIFT
        ),
        factual::MAX_SCANNED_ENTRIES,
        "the title-drift scan must stop AT the cap — and not before it"
    );
}

// ── Round-4 regressions (one per reproduced review finding) ─────────────────

/// R4-F1 — an employer whose name reduces to NOTHING but geography and a legal
/// form ("Deutschland GmbH", "Global Group", a bare "Berlin") is still
/// CHECKABLE (`distinctive_tokens` keeps "deutschland"/"global"/"berlin") but
/// has an EMPTY survival-token list, so `company_survives` answered false
/// whatever the document said. The result was an unavoidable Critical claiming
/// the role had vanished while the employer sat VERBATIM in the output.
///
/// Same family as `shortened_company_names_are_not_dropped_roles`: no possible
/// evidence means no accusation.
#[test]
fn companies_made_only_of_geography_and_legal_form_are_never_dropped_roles() {
    for company in ["Deutschland GmbH", "Global Group", "Berlin"] {
        let doc = format!(
            "EXPERIENCE\n\n\
             Software Engineer | {company} | 2018 - 2021\n\
             - Built the billing service in Java\n"
        );
        // Source and output are byte-identical: there is nothing to accuse.
        silent(
            &report_for(&doc, &doc, EN_JOB_AD, &[]),
            FACTUAL_DROPPED_ROLE,
        );
    }

    // The check still works where evidence is POSSIBLE: one identity token
    // ("acme"), and the output never says it.
    let source = "EXPERIENCE\n\n\
                  Software Engineer | Acme Deutschland GmbH | 2018 - 2021\n\
                  - Built the billing service in Java\n";
    let dropped = "EXPERIENCE\n\n\
                   Software Engineer | Globex | 2018 - 2021\n\
                   - Built the billing service in Java\n";
    fired(
        &report_for(dropped, source, EN_JOB_AD, &[]),
        FACTUAL_DROPPED_ROLE,
    );
}

/// R4-F3 — `titled_entries` stripped digits from its identity tokens but not
/// legal forms or geography, so two UNRELATED employers that merely share
/// "GmbH" (or a city) matched as the same employer and their titles were
/// compared. Reproduced on a document that is byte-identical to its source:
/// the second entry's title was reported as drift from the FIRST entry's.
#[test]
fn title_drift_does_not_match_employers_on_a_shared_legal_form_or_city() {
    for shared in ["GmbH", ", Berlin"] {
        let doc = format!(
            "EXPERIENCE\n\n\
             Senior Engineer | Northwind Systems {shared} | 2018 - 2021\n\
             - Ran the integration platform\n\n\
             Product Manager | Vitesse Logistics {shared} | 2015 - 2018\n\
             - Owned the roadmap for the carrier portal\n"
        );
        silent(
            &report_for(&doc, &doc, EN_JOB_AD, &[]),
            CONSISTENCY_TITLE_DRIFT,
        );
    }
}

/// R4-F4 — `ats.keyword_density` counted tokens filtered by an ENGLISH-only
/// stopword list and a BYTE-length test, so ordinary German function words
/// ("werden", "wurde", "durch", "für") counted toward the stuffing thresholds
/// and a truthful German résumé was accused of keyword stuffing.
///
/// The fixture is a realistic German résumé, not a synthetic repeat: passive
/// voice ("wurde … umgestellt") and "verantwortlich für" are how German
/// résumés are written.
#[test]
fn ordinary_german_prose_is_not_keyword_stuffing() {
    let resume = "\
Max Mustermann
max.mustermann@example.de | +49 30 1234567

BERUFSERFAHRUNG

Senior Backend Engineer | Nordwind Systeme | 2021 - Heute
- Verantwortlich für die Zahlungsplattform, die von vier Teams genutzt wird
- Die Abrechnung wurde auf Rust umgestellt, wodurch die Wartezeit sank
- Der Betrieb wurde auf Kubernetes umgezogen und durch Terraform beschrieben
- Verantwortlich für die Bereitschaft und für das Monitoring der Dienste

Backend Developer | Globex Logistik | 2018 - 2021
- Die Schnittstelle wurde in Python neu gebaut und durch Tests abgesichert
- Verantwortlich für die Migration zu AWS, die in drei Etappen erfolgte
- Die Datenbank wurde durch PostgreSQL ersetzt und durch Redis entlastet

KENNTNISSE

Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis

AUSBILDUNG

BSc Informatik, TU Berlin, 2014 - 2018
";
    let report = validate_content(&ContentInput {
        generated: resume,
        source_resume: resume,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    let stuffing: Vec<&str> = report
        .issues
        .iter()
        .filter(|i| i.code == ATS_KEYWORD_DENSITY)
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert!(
        stuffing.is_empty(),
        "German function words must not read as stuffed keywords; got {stuffing:?}"
    );

    // The check still catches real stuffing in German: a genuine skill token
    // repeated past the occurrence ceiling is not a function word.
    let stuffed = resume.replace(
        "- Verantwortlich für die Bereitschaft und für das Monitoring der Dienste",
        "- Kubernetes Kubernetes Kubernetes Kubernetes Kubernetes Kubernetes Kubernetes",
    );
    fired(
        &validate_content(&ContentInput {
            generated: &stuffed,
            source_resume: &stuffed,
            job_ad: DE_JOB_AD,
            top_requirements: &[],
            target_language: "de",
            doc_kind: DocKind::Resume,
        }),
        ATS_KEYWORD_DENSITY,
    );
}

/// R4-F5 — the generated DE lexicon stores UNINFLECTED stems ("nahtlos",
/// "robust", "maßgeschneidert") and `contains_phrase` requires a word boundary
/// at BOTH ends, so the forms German actually writes — "nahtlose Integration",
/// "robuste Systeme", "maßgeschneiderte Lösungen" — never matched. The whole
/// German half of the AI-tell check was dead on real output.
#[test]
fn german_ai_tells_fire_on_inflected_forms() {
    let letter = "Sehr geehrte Damen und Herren,\n\n\
                  Ihre Plattform braucht eine nahtlose Integration der Zahlungsdienste. \
                  Ich habe robuste Systeme für den Zahlungsverkehr gebaut und \
                  maßgeschneiderte Lösungen für zwei Werke betreut.\n\n\
                  Mit freundlichen Grüßen\nJana Mustermann\n";
    let report = validate_content(&ContentInput {
        generated: letter,
        source_resume: DE_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::CoverLetter,
    });
    let evidence: Vec<&str> = fired(&report, VOICE_AI_TELL_LEXICAL)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    for stem in ["nahtlos", "robust", "maßgeschneidert"] {
        assert!(
            evidence.contains(&stem),
            "the inflected form of {stem:?} must fire; got {evidence:?}"
        );
    }
}

/// The other half of R4-F5: the inflection tolerance is DE-only. English
/// matching keeps the exact both-ends word boundary it has always had, so
/// "harnesses" still does not fire the banned "harness".
#[test]
fn english_ai_tell_matching_is_unchanged_by_the_german_inflection_rule() {
    let letter = "Dear Hiring Manager,\n\n\
                  Your team harnesses a lot of data. I spent eight years on payment \
                  systems and shipped a ledger that settles twelve thousand orders a \
                  day. I read the posting twice before writing this.\n\n\
                  Best regards,\nJane Doe\n";
    silent(&en_letter(letter), VOICE_AI_TELL_LEXICAL);
}

// ── no-ai-slop catalog: the two tiers ───────────────────────────────────────
//
// The `no-ai-slop` pattern catalog was curated into a CHECKED tier (fixed
// phrases carrying no factual content) and a PROMPT-ONLY tier (everything a
// real candidate might truthfully write, plus every construction rule) — see
// `AI_TELL_LEXICAL_WORDS_EN`'s doc in `natural-voice.ts` for the four-part
// test an entry has to clear. The split IS the decision, so both halves are
// pinned: the checked entries must fire, and the prompt-only vocabulary must
// never reach a user's document.

/// Every phrase the no-ai-slop pass added to the CHECKED tier fires.
///
/// Deliberately one fixture carrying all of them: an entry that silently
/// stopped matching (the shape of the German inflection bug above) would still
/// pass a test that only asserted "some AI tell fired".
#[test]
fn no_ai_slop_checked_tier_fires_on_a_slop_letter() {
    let letter = "Dear Hiring Manager,\n\n\
                  Your platform is widely regarded as the standard in payments. My \
                  meticulous approach to release engineering carried a multifaceted \
                  team through an ever-evolving market, and that was a genuine \
                  paradigm shift. It is worth noting that in today's world the work \
                  continues.\n\n\
                  Best regards,\nJane Doe\n";
    let report = en_letter(letter);
    let evidence: Vec<&str> = fired(&report, VOICE_AI_TELL_LEXICAL)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    for entry in [
        "widely regarded as",
        "meticulous",
        "multifaceted",
        "ever-evolving",
        "paradigm shift",
        "it is worth noting",
        "in today's world",
    ] {
        assert!(
            evidence.contains(&entry),
            "{entry:?} is on the prompt's own ban list and must fire; got {evidence:?}"
        );
    }
    assert!(
        report.ok,
        "voice findings stay advice — a model may never produce a Critical"
    );
}

/// The contraction spellings a model actually writes, in BOTH apostrophe
/// shapes.
///
/// [`flattened_lower`] normalizes case and whitespace but NOT punctuation, and
/// a model emits the typographic apostrophe (U+2019) about as often as the
/// ASCII one. That is why the first pass refused apostrophe entries outright:
/// either spelling would have been half-dead. `ai_tell_issues` now folds
/// U+2019 onto U+0027 before matching, so ONE ASCII entry covers both — this
/// test is what makes that fold load-bearing (drop it and the typographic half
/// goes silent).
#[test]
fn contraction_ai_tells_fire_with_either_apostrophe() {
    let typographic = "Dear Hiring Manager,\n\n\
                       It\u{2019}s worth noting that I built the settlement ledger you \
                       advertise for. It\u{2019}s important to note that it still runs \
                       every night. In today\u{2019}s world that is rarer than it \
                       sounds.\n\n\
                       Best regards,\nJane Doe\n";
    let ascii = typographic.replace('\u{2019}', "'");
    for (shape, letter) in [
        ("typographic U+2019", typographic.to_string()),
        ("ASCII U+0027", ascii),
    ] {
        let report = en_letter(&letter);
        let evidence: Vec<&str> = fired(&report, VOICE_AI_TELL_LEXICAL)
            .iter()
            .filter_map(|i| i.evidence.as_deref())
            .collect();
        for entry in [
            "it's worth noting",
            "it's important to note",
            "in today's world",
        ] {
            assert!(
                evidence.contains(&entry),
                "{entry:?} must fire on the {shape} spelling; got {evidence:?}"
            );
        }
    }
}

/// The opener check owes the apostrophe fold too.
///
/// [`super::lexicon::TEMPLATE_OPENERS_EN`]'s shape rule is the same
/// one-directional rule the AI-tell arrays carry (U+0027 allowed, U+2019
/// banned), and the prompt-side catalog-shape test asserts it over ALL SIX
/// arrays — but `template_opener_issues` matched UNFOLDED text, so an
/// apostrophe-bearing opener entry was half-dead while the shape rule said it
/// was whole. Folding at this call site too (never inside the shared
/// `flattened_lower`, same reasoning as `ai_tell_issues`) makes the rule true
/// everywhere.
///
/// Its control lives in
/// [`german_template_openers_are_unaffected_by_the_apostrophe_fold`], a
/// SEPARATE test on purpose: inside one test the German half would sit behind
/// the English panic and prove nothing about the mutation.
#[test]
fn template_openers_fire_with_either_apostrophe() {
    let typographic = "Dear Hiring Manager,\n\n\
                       I\u{2019}m excited to apply for the payments role. At Acme I \
                       built the settlement ledger that clears twelve thousand orders \
                       a night, and I read your posting twice before writing.\n\n\
                       Best regards,\nJane Doe\n";
    for (shape, letter) in [
        ("typographic U+2019", typographic.to_string()),
        ("ASCII U+0027", typographic.replace('\u{2019}', "'")),
    ] {
        let report = en_letter(&letter);
        assert_eq!(
            first_evidence(&report, VOICE_TEMPLATE_OPENER),
            Some("i'm excited to apply"),
            "{VOICE_TEMPLATE_OPENER} must report the contraction opener on the {shape} \
             spelling; the report carried {:?}",
            codes(&report)
        );
    }
}

/// German opener entries carry no apostrophe, so the fold above must be a
/// no-op for them. Separate from the English test so that removing the fold
/// leaves this one GREEN — which is what makes it a control rather than a
/// second copy of the same assertion.
#[test]
fn german_template_openers_are_unaffected_by_the_apostrophe_fold() {
    let german = "Sehr geehrte Damen und Herren,\n\n\
                  hiermit bewerbe ich mich auf die ausgeschriebene Stelle als \
                  Backend-Entwicklerin in Ihrem Unternehmen und freue mich sehr über \
                  eine Rückmeldung von Ihnen.\n\n\
                  Mit freundlichen Grüßen\nJana Mustermann\n";
    let report = validate_content(&ContentInput {
        generated: german,
        source_resume: DE_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::CoverLetter,
    });
    assert_eq!(
        first_evidence(&report, VOICE_TEMPLATE_OPENER),
        Some("hiermit bewerbe ich mich"),
        "{VOICE_TEMPLATE_OPENER} must report the German opener on the apostrophe-free German \
         fixture; the report carried {:?}",
        codes(&report)
    );
}

/// The negative control the tiering exists for: a TRUTHFUL letter written
/// entirely out of the catalog's prompt-only vocabulary must stay silent.
///
/// Every phrase here either names something a real candidate really did
/// ("utilize", "facilitate", "embark", "supercharge"), carries a real domain
/// meaning ("beacon", as in a BLE/iBeacon fleet; "transformative justice", a
/// named social-work practice; "Paramount Global", a real employer), or is
/// ordinary human filler ("at the end of the day", "in conclusion", "as you can
/// see"). The prompt tells the model to avoid all of them; a Warning reading
/// "this is an AI tell" on one is a false accusation against the user, which
/// this module prices higher than a missed tell.
///
/// "Paramount" is the third-pass addition and the sharpest case of the class:
/// the per-phrase exemption in [`super::voice`] reads the SOURCE RÉSUMÉ only,
/// never the job ad, so a letter addressed to Paramount Global had the
/// employer's own name reported back as machine-written.
///
/// ## Why ONE `silent` covers BOTH lexicon arrays
///
/// There is no separate `voice.ai_tell_prose` CODE. On the letter path
/// [`super::voice::validate_letter`] calls `ai_tell_issues(ctx, true)`, which
/// merges [`super::lexicon::ai_tell_lexical`] and
/// [`super::lexicon::ai_tell_prose`] into one hit list and reports every hit
/// under [`VOICE_AI_TELL_LEXICAL`] — pinned from the positive side by
/// [`no_ai_slop_checked_tier_fires_on_a_slop_letter`], where the PROSE entries
/// "it is worth noting" and "in today's world" fire under exactly that code. So
/// promoting any phrase in this fixture into EITHER array fails the line below;
/// splitting the prose tier into its own code later would fail the positive
/// test first, which is where that decision has to be made.
///
/// [`VOICE_TEMPLATE_OPENER`] is a genuinely separate code and is pinned
/// separately. It is NOT pinned on the résumé control below: `voice::validate`
/// never calls `template_opener_issues` for a résumé, so the assertion could
/// not fail there for any edit — a guard that cannot fail reads as coverage and
/// is not.
#[test]
fn no_ai_slop_prompt_only_vocabulary_never_reaches_the_validator() {
    let letter = "Dear Hiring Manager,\n\n\
                  I chose to utilize Terraform to facilitate the AWS move at Acme, \
                  which for a payments team is table stakes. At its core the settlement \
                  system is a queue. When it comes to on-call, going forward I want \
                  fewer pages, not more. As you can see in terms of scale, the BLE \
                  beacon fleet I ran was the real game changer, and at the end of the \
                  day I would embark on this again. It did supercharge our deploys. \
                  Before that I coordinated the county's transformative justice pilot \
                  and taught a transformative learning module on Mezirow. The billing \
                  work I did for Paramount Global is the closest match to this role.\n\n\
                  In conclusion, with regard to the timeline, I can start in March.\n\n\
                  Best regards,\nJane Doe\n";
    let report = en_letter(letter);
    silent(&report, VOICE_AI_TELL_LEXICAL);
    silent(&report, VOICE_TEMPLATE_OPENER);
}

/// The same control on the RÉSUMÉ surface, where the lexical tier also runs.
/// Present-tense bullets are an ordinary convention for a current role, and
/// "Utilize" / "Facilitate" / "Spearhead" are exactly what a real résumé writes
/// — the class of word this pass deliberately kept out of the checked tier.
#[test]
fn no_ai_slop_prompt_only_verbs_are_silent_on_an_ordinary_resume() {
    let source = "EXPERIENCE\n\nPlatform Engineer | Acme Payments | 2021 - Present\n\
                  - Provision the AWS fleet with Terraform\n\
                  - Run the weekly release meeting\n\
                  - Own the warehouse beacon rollout\n";
    let generated = "EXPERIENCE\n\nPlatform Engineer | Acme Payments | 2021 - Present\n\
                     - Utilize Terraform to provision the AWS fleet across three regions\n\
                     - Facilitate the weekly release meeting for twelve engineers\n\
                     - Spearhead the BLE beacon rollout across forty warehouses\n";
    silent(
        &report_for(generated, source, EN_JOB_AD, &[]),
        VOICE_AI_TELL_LEXICAL,
    );
}

/// R4-F7 — [`issue`] clamped `message` and `evidence` but copied `section`
/// VERBATIM, and an ATX heading (`# …`) has no length rule of its own. 200
/// per-line issues each carrying a 1 KB heading blew the byte budget the cap
/// exists to keep, so the arithmetic in [`ISSUE_MESSAGE_MAX_BYTES`]' doc was
/// simply wrong.
#[test]
fn a_pathological_section_heading_is_clamped_like_the_message_and_evidence() {
    let heading = "Sonstige Tätigkeiten ".repeat(200); // ~4 KB, multibyte.
    let generated = format!(
        "# {heading}\n\n- {}\n",
        "x".repeat(ats::MAX_BULLET_CHARS + 50)
    );
    let report = report_for(&generated, EN_SOURCE, EN_JOB_AD, &[]);
    let section = fired(&report, ATS_LONG_BULLET)[0]
        .section
        .as_deref()
        .expect("a long bullet under a heading reports its section");
    assert!(
        section.len() <= ISSUE_SECTION_MAX_BYTES,
        "section must be clamped like every other issue field; got {} bytes",
        section.len()
    );
    assert!(
        section.ends_with('…'),
        "a clamped section must read as visibly cut; got {section:?}"
    );
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
        29,
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
            top_requirement_hits: Some(3),
            top_requirements_measured: Some(4),
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
            "topRequirementHits",
            "topRequirementsMeasured"
        ],
        "metrics keys are camelCase on the wire"
    );

    // Severity is lowercase, matching `'critical' | 'warning'` in TS.
    assert_eq!(value["issues"][0]["severity"], "critical");
    assert_eq!(value["issues"][1]["severity"], "warning");

    // The four nullable fields are PRESENT and null, never absent.
    assert!(value["issues"][0]["section"].is_null());
    assert!(value["issues"][0]["evidence"].is_null());
    assert!(value["metrics"]["keywordCoverage"].is_null());
    assert_eq!(value["metrics"]["topRequirementHits"], 3);
    assert_eq!(value["metrics"]["topRequirementsMeasured"], 4);
    let unmeasured = serde_json::to_value(ContentMetrics {
        top_requirement_hits: None,
        top_requirements_measured: None,
        ..Default::default()
    })
    .expect("metrics must serialize");
    for field in ["topRequirementHits", "topRequirementsMeasured"] {
        assert!(
            unmeasured[field].is_null(),
            "{field}: an unmeasured value is present-and-null, matching `number | null` in TS"
        );
    }
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

/// M-3 bounds issue *count* ([`MAX_CONTENT_ISSUES`]); this pins that `issue()`
/// also bounds issue *size* — `message`/`evidence` clamp to
/// [`ISSUE_MESSAGE_MAX_BYTES`]/[`ISSUE_EVIDENCE_MAX_BYTES`], UTF-8
/// char-boundary safe even when the byte cut lands mid-character, and the
/// clamped result carries a visible `…` marker instead of silently losing its
/// tail. `é` is a 2-byte character and the cap (400) minus the marker's 3
/// bytes (397) is odd, so the cut provably lands mid-character — proving the
/// clamp walks back to a real boundary instead of splitting one.
#[test]
fn issue_text_fields_are_clamped_to_their_byte_caps() {
    let long_message = "é".repeat(300); // 600 bytes, well past the 400-byte cap
    let long_evidence = "é".repeat(300);
    let long_section = "é".repeat(300);
    assert!(long_message.len() > ISSUE_MESSAGE_MAX_BYTES);
    assert!(long_evidence.len() > ISSUE_EVIDENCE_MAX_BYTES);
    assert!(long_section.len() > ISSUE_SECTION_MAX_BYTES);

    let built = issue(
        ATS_LONG_BULLET,
        Some(&long_section),
        long_message.clone(),
        Some(long_evidence.clone()),
    );

    // `section` is a heading copied out of the generated document, so it is
    // exactly as untrusted as the other two — and an ATX heading has no length
    // rule of its own anywhere in the parser.
    let section = built
        .section
        .clone()
        .expect("section must survive clamping");
    assert!(
        section.len() <= ISSUE_SECTION_MAX_BYTES,
        "section must be clamped to the cap, got {} bytes",
        section.len()
    );
    assert!(
        section.ends_with('…'),
        "a clamped section must carry the truncation marker"
    );
    assert!(
        long_section.starts_with(section.trim_end_matches('…')),
        "the clamped section must be an exact, unbroken prefix of the original"
    );

    assert!(
        built.message.len() <= ISSUE_MESSAGE_MAX_BYTES,
        "message must be clamped to the cap, got {} bytes",
        built.message.len()
    );
    assert!(
        built.message.ends_with('…'),
        "a clamped message must carry the truncation marker"
    );
    let message_prefix = built.message.trim_end_matches('…');
    assert!(
        long_message.starts_with(message_prefix),
        "the clamped message must be an exact, unbroken prefix of the original"
    );

    let evidence = built.evidence.expect("evidence must survive clamping");
    assert!(
        evidence.len() <= ISSUE_EVIDENCE_MAX_BYTES,
        "evidence must be clamped to the cap, got {} bytes",
        evidence.len()
    );
    assert!(
        evidence.ends_with('…'),
        "a clamped evidence span must carry the truncation marker"
    );
    let evidence_prefix = evidence.trim_end_matches('…');
    assert!(
        long_evidence.starts_with(evidence_prefix),
        "the clamped evidence must be an exact, unbroken prefix of the original"
    );
}

/// The reviewer's exact failure shape: `ats.long_bullet` fires *because* a
/// bullet is long, and used to quote it verbatim as `evidence` — so a single
/// pathological (but count-legal, well under `MAX_CONTENT_ISSUES`) bullet
/// could still blow the byte clamp on its own. Pins that the per-issue clamp
/// in `issue()` catches what the count cap (M-3) cannot.
#[test]
fn long_bullet_evidence_is_clamped_even_though_the_bullet_is_long() {
    let long_bullet = format!("- {}\n", "word ".repeat(400)); // ~2KB bullet
    assert!(long_bullet.len() > 2000);

    let mut generated = EN_CLEAN.to_string();
    generated.push_str(&long_bullet);

    let report = en_resume(&generated, &en_requirements());
    let hits = fired(&report, ATS_LONG_BULLET);

    for issue in hits {
        let evidence = issue
            .evidence
            .as_ref()
            .expect("ats.long_bullet must carry the offending bullet as evidence");
        assert!(
            evidence.len() <= ISSUE_EVIDENCE_MAX_BYTES,
            "evidence for a {}-byte bullet must still clamp to the cap, got {} bytes",
            long_bullet.len(),
            evidence.len()
        );
    }
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

    // The bound discriminates capped from uncapped, not fast from slow: capped,
    // the scan is linear tokenization + a bounded pairwise pass (~2.2s worst
    // observed on a slow shared CI runner in a debug build); uncapped, 6,000
    // bullets is ~18M pairwise comparisons — tens of seconds anywhere. 500ms
    // held locally but failed every CI retry, blocking merges on runner speed.
    assert!(
        elapsed < std::time::Duration::from_secs(10),
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
    // The asymmetry is load-bearing: raising this to the distinctive bar is
    // what turned "IBM Deutschland GmbH" written as "IBM" into a Critical.
    assert_eq!(factual::MIN_SURVIVAL_COMPANY_TOKEN_CHARS, 2);
    assert_eq!(factual::MIN_WORDS_IN_LETTER_BODY_LINE, 8);
    // Shared with the marker-less arm of `HEADER_PHONE_RE` on purpose — the two
    // "is this a phone?" answers may differ in SHAPE, never in length.
    assert_eq!(factual::MIN_BARE_PHONE_LINE_DIGITS, 7);
    assert_eq!(alignment::TOP_REQUIREMENT_MATCH_RATIO, 0.5);
    assert_eq!(alignment::MIN_COVERAGE_DROP_POINTS, 5.0);
    assert_eq!(consistency::MAX_PROJECT_DESCRIPTION_LINES, 3);
    assert_eq!(consistency::MAX_SKILLS_LABEL_WORDS, 3);
    assert_eq!(MIN_CHARS_FOR_LANGUAGE_CHECK, 120);
}

/// The skills-label strip must BITE at its boundary and stop there: a short
/// `Category:` head is dropped, a long one is prose whose content still counts
/// as a claim. Pinning the number without pinning its effect is how a threshold
/// silently starts eating real skills.
#[test]
fn skills_label_strip_boundary_behaves() {
    // A three-word label is stripped, so nothing in front of the colon is a
    // claim …
    let labelled = EN_CLEAN.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
        "Everyday programming languages: Rust, Python\n\
         Docker, Kubernetes, PostgreSQL, AWS, Terraform, Redis",
    );
    silent(
        &report_for(&labelled, EN_SOURCE, EN_JOB_AD, &[]),
        CONSISTENCY_SKILL_NOT_DEMONSTRATED,
    );

    // … while a head too long to be a label keeps counting: this one really
    // does claim a skill nothing demonstrates, and must still be reported.
    let prose = EN_CLEAN.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
        "Shipped four platform services on Elasticsearch: Rust, Python, Docker, \
         Kubernetes, PostgreSQL, AWS, Terraform, Redis",
    );
    let report = report_for(&prose, EN_SOURCE, EN_JOB_AD, &[]);
    let hits = fired(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
    assert!(
        hits.iter()
            .any(|i| i.evidence.as_deref() == Some("elasticsearch")),
        "a claim in front of a long head is still a claim; got {:?}",
        hits.iter().map(|i| &i.evidence).collect::<Vec<_>>()
    );
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

/// An UNCURATED language gets an empty list, never the English one.
///
/// The generated module's own contract (see its header): `natural-voice.ts`
/// hands a language with no curated list a generic, WORDLESS directive
/// (`genericAntiAiTellLexical`/`genericAntiAiTellProse`), so checking French
/// output against English words would flag phrasing the prompt never asked it
/// to avoid — accusing without evidence, the same rule
/// `documents::evidence::has_curated_function_words` enforces for the ATS
/// density ceiling. This test asserted the OLD English-fallback behaviour and
/// was left stale by the regeneration that changed it.
#[test]
fn lexicon_uncurated_language_gets_no_list_rather_than_english() {
    let en_lexical = lexicon::ai_tell_lexical("en");
    assert!(!en_lexical.is_empty(), "English is curated");
    assert!(!lexicon::ai_tell_lexical("de").is_empty(), "German is too");
    for lang in ["fr", "es", "it", "nl", "pt", "zz", ""] {
        assert!(
            lexicon::ai_tell_lexical(lang).is_empty(),
            "{lang} has no curated lexicon — it must not borrow English's"
        );
        assert!(lexicon::ai_tell_prose(lang).is_empty());
        assert!(lexicon::template_openers(lang).is_empty());
    }
    // German is a genuinely different list, not just a fallback alias.
    assert_ne!(lexicon::ai_tell_lexical("de"), en_lexical);
    assert_ne!(
        lexicon::template_openers("de"),
        lexicon::template_openers("en")
    );
}

/// Every entry across every language/tier must be lowercase and non-empty:
/// matching lowercases the haystack, so an upper-case entry would be dead,
/// and an empty entry would match everything. The DE prose tier is exempt
/// from the non-empty rule: every German prose tell turned out to be
/// construction-dependent (see `AI_TELL_PROSE_WORDS_DE`'s doc — the prompt
/// bans them as sentence-opener constructions a substring cannot judge), so
/// an empty list is the honest state, not a codegen accident.
#[test]
fn lexicon_every_entry_is_lowercase_and_non_empty() {
    let curated: [&[&str]; 5] = [
        lexicon::ai_tell_lexical("en"),
        lexicon::ai_tell_lexical("de"),
        lexicon::ai_tell_prose("en"),
        lexicon::template_openers("en"),
        lexicon::template_openers("de"),
    ];
    for list in curated {
        assert!(!list.is_empty(), "no curated list may be empty");
        for entry in list {
            assert!(!entry.trim().is_empty(), "empty entry in lexicon");
            assert_eq!(
                *entry,
                entry.to_lowercase(),
                "lexicon entries must be lowercase; got {entry:?}"
            );
        }
    }
    // The DE prose tier may be empty, but any entry it ever gains must obey
    // the same per-entry rules as the curated lists.
    for entry in lexicon::ai_tell_prose("de") {
        assert!(!entry.trim().is_empty(), "empty entry in lexicon");
        assert_eq!(
            *entry,
            entry.to_lowercase(),
            "lexicon entries must be lowercase; got {entry:?}"
        );
    }
}

// ── PR #963 round-5 findings ────────────────────────────────────────────────

/// `EN_CLEAN` with its whole `PROJECTS` block cut out — the shape a length trim
/// produces. Built here rather than as a fixture file so it stays exactly one
/// edit from the clean fixture no matter how that fixture evolves.
fn en_clean_without_projects() -> String {
    let (head, rest) = EN_CLEAN
        .split_once("PROJECTS")
        .expect("the clean fixture has a PROJECTS section");
    let (_, tail) = rest
        .split_once("SKILLS")
        .expect("SKILLS follows PROJECTS in the clean fixture");
    format!("{head}SKILLS{tail}")
}

/// R5-F1 — dropping the Projects section outright is a legitimate tailoring
/// decision (the commonest one: a length trim), not an altered link. Comparing
/// an ABSENT section against the source's links raised one
/// `factual.altered_project_link` **Critical per source link** on a document
/// that changed nothing else — three Criticals for one editorial choice.
#[test]
fn a_dropped_projects_section_is_not_an_altered_link() {
    let generated = en_clean_without_projects();
    assert!(
        !generated.contains("ledger.example.dev"),
        "the section really is gone"
    );
    let report = report_for(&generated, EN_SOURCE, EN_JOB_AD, &en_requirements());
    silent(&report, FACTUAL_ALTERED_PROJECT_LINK);

    // …while a document that KEEPS the section and rewrites a link still fires.
    fired(
        &en_resume(EN_ALTERED_LINK, &en_requirements()),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
}

/// R5-F2 — the false-positive guard only suppressed when the source's
/// misdetected tag EQUALLED the generated one, so it failed OPEN for a source
/// too short to detect (`whatlang` guesses, `is_language_mismatch` goes quiet,
/// and the guard read that silence as "the source is fine") and for a source
/// misdetected as some THIRD language. A Critical accusation needs a reliable
/// premise on BOTH sides.
#[test]
fn a_language_critical_needs_a_reliable_source_control() {
    // Too short for the detector to read at all — cannot decide.
    let short_source = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\nEngineer | Acme | 2021\n";
    silent(
        &report_for(EN_WRONG_LANGUAGE, short_source, EN_JOB_AD, &[]),
        CONTENT_LANGUAGE_MISMATCH,
    );

    // A source that reads as a THIRD language: the detector disagrees with the
    // target on both documents, which is a detector problem, not something the
    // user can fix by re-generating.
    silent(
        &report_for(EN_WRONG_LANGUAGE, FR_RESUME, EN_JOB_AD, &[]),
        CONTENT_LANGUAGE_MISMATCH,
    );

    // The real defect — a long English source, a German output — still fires.
    let real = en_resume(EN_WRONG_LANGUAGE, &en_requirements());
    let hits = fired(&real, CONTENT_LANGUAGE_MISMATCH);
    assert_eq!(hits[0].severity, Severity::Critical);
}

/// R5-F3 — a labelled skills line ("Languages: Rust, Python") is the commonest
/// skills layout there is, and every one of its CATEGORY LABELS was read as a
/// claimed skill: "languages", "frameworks", "databases" and "tooling" all
/// reported as skills the résumé never demonstrates.
#[test]
fn skills_category_labels_are_not_claimed_skills() {
    let generated = EN_CLEAN.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
        "Languages: Rust, Python\nFrameworks: Docker, Kubernetes\n\
         Databases: PostgreSQL, Redis\nTooling: AWS, Terraform",
    );
    let report = report_for(&generated, EN_SOURCE, EN_JOB_AD, &en_requirements());
    let claimed: Vec<String> = report
        .issues
        .iter()
        .filter(|i| i.code == CONSISTENCY_SKILL_NOT_DEMONSTRATED)
        .filter_map(|i| i.evidence.clone())
        .collect();
    assert!(
        claimed.is_empty(),
        "a category label names no skill; got {claimed:?}"
    );
}

/// The German half of the same shape — the label is a German noun, so an
/// English-only stopword list cannot be what saves it.
#[test]
fn german_skills_category_labels_are_not_claimed_skills() {
    let generated = DE_CLEAN.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
        "Programmiersprachen: Rust, Python\n\
         Werkzeuge: Docker, Kubernetes, AWS, Terraform\n\
         Datenbanken: PostgreSQL, Redis",
    );
    let report = validate_content(&ContentInput {
        generated: &generated,
        source_resume: DE_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    let claimed: Vec<String> = report
        .issues
        .iter()
        .filter(|i| i.code == CONSISTENCY_SKILL_NOT_DEMONSTRATED)
        .filter_map(|i| i.evidence.clone())
        .collect();
    assert!(
        claimed.is_empty(),
        "a German category label names no skill either; got {claimed:?}"
    );
}

/// R5-F4 — `topRequirementHits` rendered a confident "0" for a posting nobody
/// could compare against and for a run that was given no requirements at all.
/// Same class as the letter-metrics fix: an UNMEASURED value must not be
/// presented as a measurement.
#[test]
fn top_requirement_hits_is_unmeasured_rather_than_zero() {
    let wire = |report: ContentReport| {
        serde_json::to_value(&report).expect("a report must serialize")["metrics"]
            ["topRequirementHits"]
            .clone()
    };

    // No requirements were supplied — nothing was measured.
    assert_eq!(en_resume(EN_CLEAN, &[]).metrics.top_requirement_hits, None);
    assert_eq!(
        report_for(EN_CLEAN, EN_SOURCE, "   ", &en_requirements())
            .metrics
            .top_requirement_hits,
        None
    );
    assert_eq!(
        en_letter(EN_LETTER_GROUNDED).metrics.top_requirement_hits,
        None
    );
    assert!(
        wire(en_resume(EN_CLEAN, &[])).is_null(),
        "no requirements means no measurement"
    );
    // …and the OTHER alignment finding is untouched by the metric's absence:
    // `low_coverage` compares two coverages and never reads the requirements
    // list, so gating it on that list would silence a real regression.
    assert!(
        en_resume(EN_CLEAN, &[]).metrics.keyword_coverage.is_some(),
        "coverage is still measured without a requirements list"
    );
    // A posting with no extractable keywords is not comparable.
    assert!(
        wire(report_for(EN_CLEAN, EN_SOURCE, "   ", &en_requirements())).is_null(),
        "an uncomparable posting means no measurement"
    );
    // A cover letter never runs the alignment pass at all.
    assert!(
        wire(en_letter(EN_LETTER_GROUNDED)).is_null(),
        "a letter never measures top-requirement hits"
    );
    // Genuinely measured → a real count on the wire.
    let measured = wire(en_resume(EN_CLEAN, &en_requirements()));
    assert!(
        measured.as_u64().is_some_and(|n| n > 0),
        "the clean fixture evidences at least one top requirement; got {measured}"
    );
}

/// R5-F5 — the keyword-density ceiling counts every token the ENGLISH stopword
/// list does not drop. Only `en` and `de` have a curated function-word list, so
/// ordinary French prose (`pour`, `avec`, `dans`, `responsable`) was counted as
/// repeated keywords and a truthful French résumé was accused of stuffing.
#[test]
fn ordinary_french_prose_is_not_keyword_stuffing() {
    let report = validate_content(&ContentInput {
        generated: FR_RESUME,
        source_resume: FR_RESUME,
        job_ad: FR_JOB_AD,
        top_requirements: &[],
        target_language: "fr",
        doc_kind: DocKind::Resume,
    });
    let density: Vec<String> = report
        .issues
        .iter()
        .filter(|i| i.code == ATS_KEYWORD_DENSITY)
        .filter_map(|i| i.evidence.clone())
        .collect();
    assert!(
        density.is_empty(),
        "French function words are not stuffed keywords; got {density:?}"
    );
}

const FR_RESUME: &str = "\
Jeanne Dupont
jeanne.dupont@example.com | +33 1 23 45 67 89

PROFIL

Ingenieure backend avec huit annees d'experience pour les plateformes de paiement
et pour la construction de systemes de conteneurs dans un contexte europeen.

EXPERIENCE

Ingenieure backend senior | Acme Payments | 2021 - Present
- Responsable pour la reduction du temps de reponse de la caisse avec un cache Redis
- Responsable pour la mise en production des conteneurs Docker avec un cluster Kubernetes
- Responsable pour la diminution des reglements echoues avec un planificateur ecrit en Rust

Developpeuse backend | Globex Logistics | 2018 - 2021
- Responsable pour la construction de l'interface de facturation avec Python et PostgreSQL
- Responsable pour la migration de la flotte avec Terraform dans le nuage

COMPETENCES

Rust · Python · Docker · Kubernetes · PostgreSQL · Terraform · Redis

FORMATION

Licence en informatique, Universite de Lyon, 2014 - 2018
";

const FR_JOB_AD: &str = "\
Nous recherchons une ingenieure backend pour notre plateforme de paiement.
Vous travaillerez avec Rust, Python, Docker et Kubernetes dans une equipe
distribuee. Une experience avec PostgreSQL, Redis et Terraform est demandee
pour ce poste base a Lyon.
";

// ── PR #963 round-6 findings ────────────────────────────────────────────────

/// A one-role résumé around `bullet`, for the metric and date checks.
fn resume_with_bullet(bullet: &str) -> String {
    format!("EXPERIENCE\n\nAcme Payments | 2021 - Present\n- {bullet}\n")
}

/// R6-F1 — `MULTIPLIER_RE` ended in `(?:\b|$)`, and `×` is not a word
/// character: a boundary after it needs a WORD character on the other side, so
/// only `3×5` or a line ENDING in `3×` ever matched. Every mid-sentence
/// typographic multiplier was invisible to the metric pass — never extracted,
/// never cross-checked against the source.
#[test]
fn a_typographic_multiplier_is_extracted_mid_sentence() {
    let doc = resume_with_bullet("Grew throughput 3× while holding the error budget flat");
    let multipliers: Vec<String> = factual::metrics_in(&doc, DocKind::Resume)
        .into_iter()
        .filter(|m| m.kind == factual::MetricKind::Multiplier)
        .map(|m| m.number)
        .collect();
    assert_eq!(
        multipliers,
        vec!["3".to_string()],
        "a mid-sentence `3×` is a multiplier claim"
    );

    // The ASCII spelling is unchanged, mid-sentence and at end of line …
    for line in [
        "Grew throughput 3x while holding the error budget flat",
        "Grew throughput 3x",
        "Grew throughput 3×",
    ] {
        let doc = resume_with_bullet(line);
        assert!(
            factual::metrics_in(&doc, DocKind::Resume)
                .iter()
                .any(|m| m.kind == factual::MetricKind::Multiplier && m.number == "3"),
            "{line:?} states a multiplier"
        );
    }
    // … and `x` inside a word is still not a multiplier.
    let doc = resume_with_bullet("Ran the 3xtra pipeline for the payments team every night");
    assert!(
        !factual::metrics_in(&doc, DocKind::Resume)
            .iter()
            .any(|m| m.kind == factual::MetricKind::Multiplier),
        "`x` inside a word is not a multiplier"
    );

    // End to end: an invented multiplier is a Critical, like every other
    // fabricated figure.
    let source = resume_with_bullet("Improved throughput after rewriting the retry scheduler");
    let invented = resume_with_bullet("Improved throughput 5× after rewriting the retry scheduler");
    let report = report_for(&invented, &source, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("5×"));
}

/// R6-F2 — `INTEGER_RE` accepted `.`, `,`, NBSP and NNBSP as digit grouping but
/// not the ASCII space (nor the Swiss apostrophe), while `normalize_number`'s
/// doc and body both promise "1 200" normalizes to `1200`. So a space-grouped
/// figure split into two numbers, and a document truthfully restating its own
/// source's "1 200" as "1,200" was accused of fabricating it.
#[test]
fn a_space_grouped_figure_is_one_number_not_two() {
    for (written, expected) in [
        ("1 200", "1200"),
        ("1\u{202F}200", "1200"),
        ("1\u{00A0}200", "1200"),
        ("1'200", "1200"),
        ("1\u{2019}200", "1200"),
        ("1,200", "1200"),
        ("1.200", "1200"),
        ("40 000", "40000"),
    ] {
        // Normalization always promised this …
        assert_eq!(factual::normalize_number(written), expected);
        // … and extraction must hand it the whole figure.
        let doc = resume_with_bullet(&format!("Processed {written} refunds a day"));
        let numbers: Vec<String> = factual::metrics_in(&doc, DocKind::Resume)
            .into_iter()
            .filter(|m| m.kind == factual::MetricKind::LargeInteger)
            .map(|m| m.number)
            .collect();
        assert!(
            numbers.contains(&expected.to_string()),
            "{written:?} is one figure, not two; got {numbers:?}"
        );
    }

    // Two separate small numbers stay separate — the space is grouping only
    // when exactly three digits follow it.
    let doc = resume_with_bullet("Ran 5 12 hour shifts across the payments on-call rota");
    assert!(
        !factual::metrics_in(&doc, DocKind::Resume)
            .iter()
            .any(|m| m.number == "512"),
        "a space with two digits after it is not grouping; got {:?}",
        factual::metrics_in(&doc, DocKind::Resume)
    );

    // End to end: restating a space-grouped figure in another convention is
    // not a fabrication.
    for source_form in ["1 200", "1'200"] {
        let source = resume_with_bullet(&format!("Processed {source_form} refunds a day"));
        let restated = resume_with_bullet("Processed 1,200 refunds a day");
        silent(
            &report_for(&restated, &source, EN_JOB_AD, &[]),
            FACTUAL_UNSOURCED_METRIC,
        );
    }
}

/// R6-F3 — `date_order_issues` read EVERY line, took any two years on it as a
/// span, and reported "the end date is before the start" whenever the second
/// was smaller. An ordinary bullet comparing this year against an older
/// baseline was therefore accused of carrying a swapped date span.
#[test]
fn two_years_compared_in_prose_are_not_a_swapped_date_span() {
    for prose in [
        "Cut the incident count to 2024 levels from the 2019 baseline",
        "Migrated every service written between 2024 and 2019 onto one platform",
    ] {
        let doc = resume_with_bullet(prose);
        silent(
            &report_for(&doc, &doc, EN_JOB_AD, &[]),
            CONSISTENCY_DATE_ORDER,
        );
    }

    // A genuinely swapped span still fires, in both entry shapes …
    let backwards = "EXPERIENCE\n\nAcme | 2021 - 2018\n- Shipped the ledger service\n";
    let report = report_for(backwards, backwards, EN_JOB_AD, &[]);
    let hits = fired(&report, CONSISTENCY_DATE_ORDER);
    assert_eq!(hits[0].evidence.as_deref(), Some("2021 - 2018"));
    let two_space = "EXPERIENCE\n\nAcme Payments    Jan 2021 - Mar 2018\n\
                     - Shipped the ledger service\n";
    fired(
        &report_for(two_space, two_space, EN_JOB_AD, &[]),
        CONSISTENCY_DATE_ORDER,
    );
    // … and an ordinary forward span stays quiet.
    let forward = "EXPERIENCE\n\nAcme | 2018 - 2021\n- Shipped the ledger service\n";
    silent(
        &report_for(forward, forward, EN_JOB_AD, &[]),
        CONSISTENCY_DATE_ORDER,
    );
}

/// R6-F4 — the `Category:` strip fired on any head of three words or fewer, so
/// a PROFICIENCY line ("Python: Advanced", "Deutsch: Muttersprache") lost the
/// SKILL and kept the level word: "advanced" was reported as a skill nothing
/// demonstrates while Python itself was never checked at all.
#[test]
fn a_proficiency_line_claims_the_skill_not_the_level() {
    let generated = EN_CLEAN.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
        "Python: Advanced\nDocker: Expert\nElasticsearch: Advanced",
    );
    let report = report_for(&generated, EN_SOURCE, EN_JOB_AD, &[]);
    let claimed: Vec<String> = fired(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED)
        .iter()
        .filter_map(|i| i.evidence.clone())
        .collect();
    assert_eq!(
        claimed,
        vec!["elasticsearch".to_string()],
        "the level words are not claims, and the unbacked SKILL is"
    );
}

/// The German half of the same shape — the level word is a German noun, so an
/// English-only stopword list cannot be what saves it.
#[test]
fn a_german_proficiency_line_claims_the_skill_not_the_level() {
    let generated = DE_CLEAN.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
        "Python: Fortgeschritten\nDocker: Experte\nDeutsch: Muttersprache\n\
         Elasticsearch: Grundkenntnisse",
    );
    let report = validate_content(&ContentInput {
        generated: &generated,
        source_resume: DE_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    let claimed: Vec<String> = report
        .issues
        .iter()
        .filter(|i| i.code == CONSISTENCY_SKILL_NOT_DEMONSTRATED)
        .filter_map(|i| i.evidence.clone())
        .collect();
    for level in [
        "fortgeschritten",
        "experte",
        "muttersprache",
        "grundkenntnisse",
    ] {
        assert!(
            !claimed.iter().any(|c| c == level),
            "{level:?} is a proficiency grade, not a claimed skill; got {claimed:?}"
        );
    }
    // The skills themselves are now checked: two are demonstrated, one is not.
    // ("deutsch" may legitimately appear — a language listed and never shown is
    // exactly what this check reports.)
    for demonstrated in ["python", "docker"] {
        assert!(
            !claimed.iter().any(|c| c == demonstrated),
            "{demonstrated:?} is demonstrated in the experience section; got {claimed:?}"
        );
    }
    assert!(
        claimed.iter().any(|c| c == "elasticsearch"),
        "the unbacked skill in front of the colon must be reported; got {claimed:?}"
    );
}

/// The label strip's discriminator, pinned in both directions — including the
/// boundary the fix knowingly gives up (see [`consistency::strip_skills_label`]).
#[test]
fn skills_label_discriminator_reads_lists_and_grades_apart() {
    let skills = |lines: &str| {
        let generated = EN_CLEAN.replace(
            "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
            lines,
        );
        let report = report_for(&generated, EN_SOURCE, EN_JOB_AD, &[]);
        let mut claimed: Vec<String> = report
            .issues
            .iter()
            .filter(|i| i.code == CONSISTENCY_SKILL_NOT_DEMONSTRATED)
            .filter_map(|i| i.evidence.clone())
            .collect();
        claimed.sort();
        claimed
    };

    // A list tail is a CATEGORY label: the head is dropped, the items are the
    // claims (R5-F3's fix, unchanged).
    assert_eq!(skills("Languages: Rust, Python"), Vec::<String>::new());
    assert_eq!(
        skills("Languages: Rust, Elasticsearch"),
        vec!["elasticsearch".to_string()]
    );
    // (R6's "a multi-word tail is still a list" row moved to R7-F2's
    // `a_space_separated_category_now_reads_as_a_grade` — word count is no
    // longer a list signal, because multi-word GRADES are commoner.)
    //
    // A tail with no separator is a GRADE: the head is the claim.
    assert_eq!(skills("Python: Advanced"), Vec::<String>::new());
    // …which is exactly why a single-item CATEGORY reads as a grade too. The
    // accepted cost of the rule, pinned so it cannot change unnoticed: the item
    // is not policed. The LABEL is not reported in its place either — that was
    // R8-F7, and `a_category_label_is_never_the_claim` owns the rows for it.
    assert_eq!(skills("Frameworks: Elasticsearch"), Vec::<String>::new());
}

// ── PR #963 round-7 findings ────────────────────────────────────────────────

/// The EN half of the R7-F2 rows, and the concession that comes with the fix.
/// Shares `skills`' shape with the R6-F4 test on purpose: same fixture, same
/// extraction, so the two discriminator rules are compared on one surface.
fn en_skills_claims(lines: &str) -> Vec<String> {
    let generated = EN_CLEAN.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
        lines,
    );
    let mut claimed: Vec<String> = report_for(&generated, EN_SOURCE, EN_JOB_AD, &[])
        .issues
        .iter()
        .filter(|i| i.code == CONSISTENCY_SKILL_NOT_DEMONSTRATED)
        .filter_map(|i| i.evidence.clone())
        .collect();
    claimed.sort();
    claimed
}

fn de_skills_claims(lines: &str) -> Vec<String> {
    let generated = DE_CLEAN.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
        lines,
    );
    let mut claimed: Vec<String> = validate_content(&ContentInput {
        generated: &generated,
        source_resume: DE_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    })
    .issues
    .iter()
    .filter(|i| i.code == CONSISTENCY_SKILL_NOT_DEMONSTRATED)
    .filter_map(|i| i.evidence.clone())
    .collect();
    claimed.sort();
    claimed
}

/// R7-F2 — R6-F4's discriminator read "a tail of more than one word" as a LIST,
/// but a proficiency GRADE runs to as many words as the language needs:
/// "verhandlungssicher in Wort und Schrift" is the standard German Sprachen
/// phrasing, and "5 years in production" is its English equivalent. The skill in
/// front of the colon was dropped and the grade words became the claims — the
/// same false-positive family R6-F4 set out to close, louder.
#[test]
fn a_multi_word_grade_is_not_a_skills_list() {
    // The German Sprachen line: the language is the claim, the grade is not.
    let de = de_skills_claims("Englisch: verhandlungssicher in Wort und Schrift");
    for grade in ["verhandlungssicher", "wort", "schrift"] {
        assert!(
            !de.iter().any(|c| c == grade),
            "{grade:?} is part of a proficiency grade, not a claimed skill; got {de:?}"
        );
    }
    assert!(
        de.iter().any(|c| c == "englisch"),
        "the skill in front of the colon is what gets checked; got {de:?}"
    );

    // The English equivalent — a grade written as a measurement. ("years" is
    // also a kernel stopword, so "production" is the load-bearing half here.)
    let en = en_skills_claims("Python: 5 years in production");
    for grade in ["years", "production"] {
        assert!(
            !en.iter().any(|c| c == grade),
            "{grade:?} is part of a proficiency grade, not a claimed skill; got {en:?}"
        );
    }
    assert_eq!(
        en,
        Vec::<String>::new(),
        "and Python itself IS demonstrated in the experience section; got {en:?}"
    );

    // A slash is a GRADE's punctuation ("C1/C2"), not a list separator.
    let levels = de_skills_claims("Deutsch: C1/C2");
    assert_eq!(
        levels,
        vec!["deutsch".to_string()],
        "the language is the claim; the CEFR levels are the grade"
    );
}

/// The boundary this fix knowingly gives up, pinned so it cannot change
/// unnoticed: with the word-count clause gone, a category whose items are
/// separated by SPACES alone reads grade-shaped — the label becomes the claim
/// and the items are not policed. Deliberate: a category list is written with
/// commas or middots, and the alternative is a false finding on every
/// multi-word grade, which is far commoner than an unpunctuated category.
#[test]
fn a_space_separated_category_now_reads_as_a_grade() {
    // The cost, stated as the MISS it is: "elasticsearch" is claimed here and
    // demonstrated nowhere, and an unpunctuated category means it is not
    // policed. (The label "Tools" is itself demonstrated by the projects line,
    // so nothing at all is reported.)
    assert_eq!(
        en_skills_claims("Tools: Docker Kubernetes Elasticsearch"),
        Vec::<String>::new(),
        "the items of a space-separated category are not checked"
    );
    // A separator puts it straight back: the items are the claims again.
    assert_eq!(
        en_skills_claims("Tools: Docker, Kubernetes, Elasticsearch"),
        vec!["elasticsearch".to_string()],
        "punctuation is the whole discriminator"
    );
    // R6-F4's own multi-word row moves to this side of the line for the same
    // reason — it was only ever a list because of the word-count clause. The
    // items stay unpoliced; the LABEL is not reported in their place (R8-F7).
    assert_eq!(
        en_skills_claims("Frameworks: React Native"),
        Vec::<String>::new()
    );
}

/// R8 follow-up — a heading-less document (a cover letter) has no section band,
/// so `metric_lines` skips its contact lines by SHAPE. That shape test was
/// `export::parser`'s loose phone rule (any 7+ run of digits, spaces and
/// hyphens), so a letter paragraph quoting a rate range exempted itself from the
/// fabricated-metric check — the letter-path twin of R8-F3, left open because
/// the helper lived outside that round's file set.
#[test]
fn a_letter_paragraph_quoting_a_range_is_still_metric_checked() {
    let letter = "Jane Doe\njane.doe@example.com\n\n\
                  Dear Hiring Manager,\n\n\
                  Last year I renegotiated our agency rates from 150 - 200 EUR an hour \
                  across twelve suppliers.\n\n\
                  Best regards,\nJane Doe\n";
    let report = en_letter(letter);
    let evidence: Vec<&str> = fired(&report, FACTUAL_UNSOURCED_METRIC)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert!(
        evidence.contains(&"150") && evidence.contains(&"200"),
        "a letter's body prose is not its letterhead; got {evidence:?}"
    );

    // The guard that keeps the shape test on this path at all: a letter carries
    // contact details in its SIGN-OFF as well as its letterhead, and neither is
    // a claim of impact. Both must stay skipped.
    let signed = "Jane Doe\njane.doe@example.com\n\n\
                  Dear Hiring Manager,\n\n\
                  I put a Redis cache in front of the ledger service and checkout latency \
                  went from 480ms to 90ms.\n\n\
                  Best regards,\nJane Doe\n+49 30 1234567\n";
    silent(&en_letter(signed), FACTUAL_UNSOURCED_METRIC);
}

/// The two surfaces that ask "is this line a phone number?" must agree, because
/// they disagreed for a whole round: `ats::header_in_body` took the strict shape
/// and the metric band skip kept the parser's loose one.
#[test]
fn the_header_phone_shape_is_one_rule_for_both_surfaces() {
    for phone in [
        "+49 30 1234567",
        "+49 (0)30 1234567",
        "(030) 12345678",
        "0176 12345678",
        "+1 (555) 123-4567",
    ] {
        assert!(
            looks_like_header_phone(phone),
            "{phone:?} is a phone number"
        );
        assert!(has_real_contact_match(phone), "{phone:?} is a contact line");
    }
    for prose in [
        "Renegotiated agency rates from 150 - 200 EUR per hour",
        "Das Jahresbudget von 90 000 - 110 000 EUR gesteuert",
        "Acme Payments | 2018 - 2021",
        "Processed 4500 orders a day",
    ] {
        assert!(
            !looks_like_header_phone(prose),
            "{prose:?} is prose, not a phone number"
        );
        assert!(
            !has_real_contact_match(prose),
            "{prose:?} must not exempt itself from the metric pass"
        );
    }
    // An address still counts, and still only when it is a real one.
    assert!(has_real_contact_match("jane.doe@example.com"));
    assert!(!has_real_contact_match("owned the @payments rotation"));
}

// ── PR #963 round-8 findings ────────────────────────────────────────────────

/// R8-F2 — `ats.header_in_body` is a CRITICAL, and its phone test was
/// `export::parser`'s `PHONE_RE`, which accepts ANY run of seven or more
/// digits, spaces and hyphens. A salary range in ordinary prose ("150 - 200")
/// satisfies it, so a body line under any short line was reported as a second
/// contact block.
#[test]
fn a_salary_range_in_the_body_is_not_a_second_contact_block() {
    let doc = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
               Acme | 2021 - Present\n- Led the migration\n\n\
               VENDOR MANAGEMENT\n\n\
               Rate negotiation\n\
               Renegotiated agency rates from 150 - 200 EUR per hour across twelve suppliers\n";
    silent(&report_for(doc, doc, EN_JOB_AD, &[]), ATS_HEADER_IN_BODY);

    // A grouped-figure budget line is the same shape in German.
    let de = "Jana Mustermann\njana@example.com\n\nBERUFSERFAHRUNG\n\n\
              Acme | 2021 - Heute\n- Die Abrechnung betreut\n\n\
              BUDGETVERANTWORTUNG\n\n\
              Jahresbudget\n\
              Das Jahresbudget von 90 000 - 110 000 EUR eigenverantwortlich gesteuert\n";
    silent(&report_for(de, de, EN_JOB_AD, &[]), ATS_HEADER_IN_BODY);

    // …and a real phone number under the CANDIDATE's own name still IS a second
    // contact block, in every form a résumé header writes one. (The name was a
    // referee's until R13-W1 — see
    // `a_reference_block_is_not_a_second_contact_block`.)
    for phone in [
        "+49 30 1234567",
        "+49 (0)30 1234567",
        "(030) 12345678",
        "0176 12345678",
        "+1 (555) 123-4567",
    ] {
        let cluster = format!(
            "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
             Acme | 2021 - Present\n- Led the migration\n\n\
             CONTACT\n\nJane Doe\n{phone}\n"
        );
        let report = report_for(&cluster, &cluster, EN_JOB_AD, &[]);
        let hits = fired(&report, ATS_HEADER_IN_BODY);
        assert_eq!(hits[0].severity, Severity::Critical, "{phone}");
    }
}

/// R8-F3 — the metric pass exempted ANY line the parser marked `Contact`, and a
/// wrapped body paragraph carrying a European-grouped figure IS contact-shaped
/// (`PHONE_RE` reads "90 000 - 110 000" as a phone number). A fabricated figure
/// mid-document therefore escaped the fabricated-metric check entirely.
#[test]
fn a_contact_shaped_body_paragraph_is_still_metric_checked() {
    let source = "Jane Doe\njane@example.com\n\nSUMMARY\n\n\
                  Backend engineer on payment platforms.\n\n\
                  EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    let fabricated = "Jane Doe\njane@example.com\n\nSUMMARY\n\n\
                      Grew the platform budget from 90 000 - 110 000 EUR inside one year\n\n\
                      EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    let report = report_for(fabricated, source, EN_JOB_AD, &[]);
    let evidence: Vec<&str> = fired(&report, FACTUAL_UNSOURCED_METRIC)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert!(
        evidence.contains(&"90 000") && evidence.contains(&"110 000"),
        "a body paragraph is not the contact band; got {evidence:?}"
    );
}

/// R8-F4 — the TRUTH side scanned the whole source with no band skip, so the
/// digits of the candidate's own phone number "sourced" an unrelated fabricated
/// figure: a bullet claiming 1 234 567 settlements was accepted because the
/// header says "+49 30 1234567".
#[test]
fn source_contact_digits_do_not_source_a_fabricated_metric() {
    let header = "Jane Doe\njane@example.com | +49 30 1234567 | 10115 Berlin\n\n";
    let source = format!("{header}EXPERIENCE\n\nAcme | 2021 - Present\n- Led the migration\n");
    let generated = format!(
        "{header}EXPERIENCE\n\nAcme | 2021 - Present\n\
         - Settled 1234567 payments in the first quarter after the rewrite\n"
    );
    let report = report_for(&generated, &source, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("1234567"));

    // …while the header's own digits are still not CLAIMS on either side: both
    // sides skip the same band, so a truthful document stays silent.
    silent(
        &report_for(&source, &source, EN_JOB_AD, &[]),
        FACTUAL_UNSOURCED_METRIC,
    );
}

/// R8-F5 — the stack-line exclusion keyed on the literal `"://"`, so a
/// scheme-less link line ("github.com/janedoe/ledger") was cut out of the
/// SOURCE link set. The generated document then carried a link the source
/// "did not have" and the candidate was told they had invented their own
/// repository URL.
#[test]
fn a_scheme_less_link_line_is_not_an_invented_link() {
    let source = "PROJECTS\n\n\
                  **Ledger CLI** · A double-entry bookkeeping tool\n\
                  github.com/janedoe/ledger\n\
                  Rust · SQLite · Clap\n";
    // The model wrote the same link with its scheme spelled out.
    let generated = "PROJECTS\n\n\
                     **Ledger CLI** · A double-entry bookkeeping tool\n\
                     https://github.com/janedoe/ledger\n\
                     Rust · SQLite · Clap\n";
    silent(
        &report_for(generated, source, EN_JOB_AD, &[]),
        FACTUAL_ALTERED_PROJECT_LINK,
    );

    // The guard the exclusion exists for stays green: a package-registry NAME
    // on a stack line is a technology, not a link, so dropping it while
    // tailoring is not an altered link.
    let stack = "PROJECTS\n\n\
                 **Ledger CLI** · https://github.com/janedoe/ledger\n\
                 Rust · crates.io · SQLite\n";
    let trimmed = "PROJECTS\n\n\
                   **Ledger CLI** · https://github.com/janedoe/ledger\n\
                   Rust · SQLite\n";
    silent(
        &report_for(trimmed, stack, EN_JOB_AD, &[]),
        FACTUAL_ALTERED_PROJECT_LINK,
    );

    // …and a genuinely different repository still fires.
    let altered = "PROJECTS\n\n\
                   **Ledger CLI** · A double-entry bookkeeping tool\n\
                   github.com/someone-else/ledger\n\
                   Rust · SQLite · Clap\n";
    fired(
        &report_for(altered, source, EN_JOB_AD, &[]),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
}

/// R8-F6 — `skill_not_demonstrated` filters its claims through
/// `function_words(lang)` but never asked whether that language HAS a curated
/// list, so for `fr`/`es`/`it`/`nl`/`pt` the filter is a no-op and ordinary
/// filler ("connaissances", "approfondies") was reported back as skills the
/// résumé never demonstrates. Same shape as R5-F5 in `ats.rs`, one check over.
#[test]
fn uncurated_language_skills_claims_go_quiet() {
    let generated = FR_RESUME.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · Terraform · Redis",
        "Connaissances approfondies en Rust, Python, Docker et Kubernetes",
    );
    let report = validate_content(&ContentInput {
        generated: &generated,
        source_resume: FR_RESUME,
        job_ad: FR_JOB_AD,
        top_requirements: &[],
        target_language: "fr",
        doc_kind: DocKind::Resume,
    });
    let claimed: Vec<String> = report
        .issues
        .iter()
        .filter(|i| i.code == CONSISTENCY_SKILL_NOT_DEMONSTRATED)
        .filter_map(|i| i.evidence.clone())
        .collect();
    assert!(
        claimed.is_empty(),
        "French filler is not a claimed skill, and nothing here can tell the two apart; \
         got {claimed:?}"
    );

    // A curated language still runs the check — suppression is about the LIST,
    // not about the shape of the line.
    assert!(
        de_skills_claims("Kenntnisse in Rust und Elasticsearch")
            .iter()
            .any(|c| c == "elasticsearch"),
        "German filler is FILTERED, so the unbacked skill beside it is still reported"
    );
}

/// R8-F7 — R6-F4's conceded boundary ("Frameworks: React" reports the label) is
/// also a false-positive channel: a category word is near-never demonstrated,
/// so the check reported the CATEGORY back as a skill the résumé fails to
/// evidence. A label is not a claim, wherever the line's shape put it.
#[test]
fn a_category_label_is_never_the_claim() {
    for line in [
        "Frameworks: Elasticsearch",
        "Frameworks: React Native",
        "Cloud: Elasticsearch",
        "Databases: Elasticsearch",
    ] {
        assert_eq!(
            en_skills_claims(line),
            Vec::<String>::new(),
            "{line:?}: the category label names no skill, and the item stays unpoliced"
        );
    }
    for line in [
        "Werkzeuge: Elasticsearch",
        "Datenbanken: Elasticsearch",
        "Programmiersprachen: Elasticsearch",
    ] {
        assert_eq!(
            de_skills_claims(line),
            Vec::<String>::new(),
            "{line:?}: a German category label names no skill either"
        );
    }
    // The head is still the claim when it names a SKILL rather than a category.
    assert_eq!(
        en_skills_claims("Elasticsearch: Advanced"),
        vec!["elasticsearch".to_string()],
        "a proficiency line still claims the skill in front of the colon"
    );
    assert_eq!(
        de_skills_claims("Englisch: verhandlungssicher in Wort und Schrift"),
        vec!["englisch".to_string()]
    );
}

// ── PR #963 round-9 findings ────────────────────────────────────────────────

/// R9-F2 — `topRequirementHits` is a bare count with no denominator, and a
/// requirement with no extractable keywords is dropped from the loop silently,
/// so "2" could mean 2 of 2, 2 of 10, or 2 out of a list where nothing else was
/// measurable. The panel cannot tell those apart, and neither could this test
/// suite.
#[test]
fn top_requirement_hits_carry_the_denominator_they_were_measured_against() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                  - Wrote Terraform modules for the AWS estate\n\
                  - Ran the PostgreSQL fleet through two major upgrades\n";
    let job = "Terraform AWS PostgreSQL Kubernetes platform engineer";
    let metrics = |reqs: &[String]| {
        serde_json::to_value(report_for(source, source, job, reqs).metrics)
            .expect("metrics must serialize")
    };
    let req = |s: &str| s.to_string();

    let both = metrics(&[req("Terraform modules"), req("PostgreSQL fleet")]);
    assert_eq!(both["topRequirementHits"], 2);
    assert_eq!(
        both["topRequirementsMeasured"], 2,
        "2 of 2 must be distinguishable from 2 of 10"
    );

    // A requirement with no extractable keywords ("5+ yrs" — every token is
    // under the kernel's length filter) is UNMEASURABLE, and an unmeasurable
    // requirement counts for neither side of the ratio.
    let mixed = metrics(&[
        req("Terraform modules"),
        req("PostgreSQL fleet"),
        req("5+ yrs"),
    ]);
    assert_eq!(mixed["topRequirementHits"], 2);
    assert_eq!(
        mixed["topRequirementsMeasured"], 2,
        "an unanswerable requirement is not a denominator"
    );

    // A requirement nothing evidences DOES count in the denominator — that is
    // the whole difference between "2 of 2" and "2 of 3".
    let with_gap = metrics(&[req("Terraform modules"), req("Kubernetes operators")]);
    assert_eq!(with_gap["topRequirementHits"], 1);
    assert_eq!(with_gap["topRequirementsMeasured"], 2);

    // The invariant the wire mirror depends on: the denominator is absent
    // EXACTLY when the hit count is, so the renderer needs one null check for
    // the pair. Both routes to "unmeasured" are covered — an empty requirements
    // list, and a cover letter (which never runs the alignment pass at all).
    for unmeasured in [
        metrics(&[]),
        serde_json::to_value(en_letter(EN_LETTER_GROUNDED).metrics).expect("metrics serialize"),
    ] {
        assert!(unmeasured["topRequirementHits"].is_null());
        assert!(unmeasured["topRequirementsMeasured"].is_null());
    }
}

/// R9-F3 — the TRUTH side skipped section 0 by POSITION, so a résumé that opens
/// with a profile paragraph BEFORE its first heading (an extremely common
/// layout) had every figure in that paragraph erased from the sourced set. The
/// same figure, restated in the generated document's headed summary, then read
/// as fabricated.
#[test]
fn a_profile_paragraph_before_the_first_heading_still_sources_its_metrics() {
    let source = "Jane Doe\njane@example.com\n\n\
                  Backend engineer who cut checkout latency by 40% on the payment platform.\n\n\
                  EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    let generated = "Jane Doe\njane@example.com\n\n\
                     SUMMARY\n\n\
                     Backend engineer; cut checkout latency by 40% on the payment platform.\n\n\
                     EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    silent(
        &report_for(generated, source, EN_JOB_AD, &[]),
        FACTUAL_UNSOURCED_METRIC,
    );

    // The check still bites: a figure the profile paragraph does NOT state is
    // still a fabrication.
    let invented = "Jane Doe\njane@example.com\n\n\
                    SUMMARY\n\n\
                    Backend engineer; cut checkout latency by 65% on the payment platform.\n\n\
                    EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    let report = report_for(invented, source, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("65%"));
}

/// R9-F4 — the trade R8-F4's symmetry fix made. A heading-less document (a
/// letter) has no positional band, so its sign-off is skipped by SHAPE — and
/// the shape test is `looks_like_header_phone`, whose own documented miss is a
/// bare US number with no `+`/parens. "555-123-4567" alone under "Best regards"
/// therefore reached the metric pass and became THREE `unsourced_metric`
/// Criticals about the candidate's own phone number.
#[test]
fn a_number_alone_on_a_line_is_a_phone_number_not_three_claims() {
    let body = "I put a Redis cache in front of the ledger service and checkout latency \
                went from 480ms to 90ms.";
    let signed = format!(
        "Jane Doe\njane.doe@example.com\n\n\
         Dear Hiring Manager,\n\n{body}\n\n\
         Best regards,\nJane Doe\n555-123-4567\n"
    );
    silent(&en_letter(&signed), FACTUAL_UNSOURCED_METRIC);

    // Direction 2 — R8-F4's property is kept: digits in PROSE are still
    // checked, and a figure the source never states is still a Critical.
    let fabricated = format!(
        "Jane Doe\njane.doe@example.com\n\n\
         Dear Hiring Manager,\n\n{body} I also settled 4500 disputes that quarter.\n\n\
         Best regards,\nJane Doe\n555-123-4567\n"
    );
    let report = en_letter(&fabricated);
    let evidence: Vec<&str> = fired(&report, FACTUAL_UNSOURCED_METRIC)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert_eq!(
        evidence,
        vec!["4500"],
        "the sign-off number is not a claim; the prose figure still is"
    );

    // Direction 3 — the same line in the SOURCE still cannot vouch for a body
    // metric that merely reuses its digits (R8-F4, via the shape this round
    // added rather than the position R9-F3 relaxed).
    let header = "Jane Doe\njane@example.com\n555-123-4567\n\n";
    let source = format!("{header}EXPERIENCE\n\nAcme | 2021 - Present\n- Led the migration\n");
    let generated = format!(
        "{header}EXPERIENCE\n\nAcme | 2021 - Present\n\
         - Settled 4567 disputes in the first quarter after the rewrite\n"
    );
    let report = report_for(&generated, &source, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("4567"));
}

/// R9-F5 — `generic_letter` obeyed the generated-vs-target language gate but
/// not the JOB-AD-vs-target one, which is the normal DACH case: an
/// English-language posting for a German-speaking role. The keyword
/// intersection then ran across two languages, came back at ~0, and told a
/// letter that names the posting's own subject matter that it "could have been
/// sent to anyone".
#[test]
fn a_posting_in_another_language_does_not_make_a_letter_generic() {
    let en_ad = "Operations Lead, Payment Disputes\n\n\
                 We are hiring an operations lead for our chargeback and dispute desk. You \
                 will own the refund workflow end to end, cut the average handling time per \
                 case, and report weekly to the finance team on recovered volume.\n";
    let de_ad = "Teamleitung Zahlungsreklamationen\n\n\
                 Wir suchen eine Teamleitung für unsere Rückbuchungsstelle. Sie verantworten \
                 den Erstattungsprozess von Anfang bis Ende, senken die Bearbeitungszeit je \
                 Fall und berichten wöchentlich an die Finanzabteilung über das \
                 zurückgeholte Volumen.\n";
    let letter = "Jana Mustermann\njana.mustermann@example.com\n\n\
                  Sehr geehrtes Team,\n\n\
                  Ihre Ausschreibung nennt die Rückbuchungsstelle und den Erstattungsprozess \
                  als Kern der Aufgabe. Genau dort habe ich die letzten vier Jahre \
                  gearbeitet. Ich habe die Bearbeitungszeit je Fall von elf auf vier Tage \
                  gesenkt und das wöchentliche Berichtswesen an die Finanzabteilung \
                  aufgebaut.\n\n\
                  Mit freundlichen Grüßen\nJana Mustermann\n";
    let de_letter_against = |job_ad: &str| {
        validate_content(&ContentInput {
            generated: letter,
            source_resume: DE_SOURCE,
            job_ad,
            top_requirements: &[],
            target_language: "de",
            doc_kind: DocKind::CoverLetter,
        })
    };

    // The control: against the SAME posting in the target language, this letter
    // is measurably specific. Only the ad's language differs between the two.
    silent(&de_letter_against(de_ad), VOICE_GENERIC_LETTER);
    let across_languages = de_letter_against(en_ad);
    silent(&across_languages, VOICE_GENERIC_LETTER);

    // The premise is scoped to the ad↔document INTERSECTION and nothing else:
    // coverage compares the two DOCUMENTS against the same ad, symmetrically,
    // so it stays a real measurement and must survive.
    assert!(
        across_languages.metrics.keyword_coverage.is_some(),
        "a coverage comparison between two documents is not a cross-language \
         intersection; only `generic_letter` is"
    );

    // …and the control on the control: divergence has to be RELIABLY detected.
    // A terse ad is a keyword soup `whatlang` reads as anything at all, so
    // suppressing on that guess would switch the check off for ordinary short
    // postings. Below the reliability bar the check runs exactly as it always
    // did — including, accepted and pinned, its cross-language noise.
    let terse = "Payment disputes chargeback refund workflow lead";
    fired(&de_letter_against(terse), VOICE_GENERIC_LETTER);
}

/// R10-F1 — `titled_entries` split the entry label with an ad-hoc
/// `split_once`, which leaves the LOCATION and DATE columns inside `company`.
/// Two unrelated employers that merely both say "Remote" (or both spell their
/// months out) were therefore the same employer, and the second entry's title
/// was reported as drift from the first's — a Critical-adjacent Warning on a
/// document byte-identical to its source.
///
/// `documents::evidence::GEOGRAPHY_TOKENS` cannot save this: "Remote" is not a
/// place, and no gazetteer covers every city. The fix is to stop asking the
/// question of the raw label — `split_entry` already knows which segment is the
/// company.
#[test]
fn title_drift_does_not_match_employers_on_a_shared_location_or_month() {
    // "Remote" is a column every second entry line carries.
    let remote = "EXPERIENCE\n\n\
                  Senior Engineer | Acme Payments | Remote | 2019 - 2022\n\
                  - Shipped Docker containers to production\n\n\
                  Product Manager | Globex Logistics | Remote | 2015 - 2018\n\
                  - Ran the reporting service\n";
    silent(
        &report_for(remote, remote, EN_JOB_AD, &[]),
        CONSISTENCY_TITLE_DRIFT,
    );

    // A spelled-out month survives the digit filter the shared-year case relies
    // on, so it matched two unrelated employers just as readily.
    let months = "EXPERIENCE\n\n\
                  Senior Engineer | Acme Payments | January 2019 - December 2022\n\
                  - Shipped Docker containers to production\n\n\
                  Product Manager | Globex Logistics | January 2015 - December 2018\n\
                  - Ran the reporting service\n";
    silent(
        &report_for(months, months, EN_JOB_AD, &[]),
        CONSISTENCY_TITLE_DRIFT,
    );

    // The guard: the check still does its job when the employer really IS the
    // same one and the generated document renamed the role.
    let source = "EXPERIENCE\n\n\
                  Senior Engineer | Acme Payments | Remote | 2019 - 2022\n\
                  - Shipped Docker containers to production\n";
    let renamed = "EXPERIENCE\n\n\
                   Product Manager | Acme Payments | Remote | 2019 - 2022\n\
                   - Shipped Docker containers to production\n";
    let report = report_for(renamed, source, EN_JOB_AD, &[]);
    let hits = fired(&report, CONSISTENCY_TITLE_DRIFT);
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("Senior Engineer → Product Manager")
    );
}

/// R10-F3 — the section-level carve-out (R5-F1) forgave a wholly-cut PROJECTS
/// section but not a single trimmed ENTRY, so a résumé trimmed from three
/// projects to two drew a `factual.altered_project_link` Critical per link on
/// the dropped entry. Same false-positive class, same commonest edit there is.
#[test]
fn a_trimmed_project_entry_is_not_an_altered_link() {
    let head = "EXPERIENCE\n\n\
                Acme Payments | 2021 - Present\n\
                - Shipped Docker containers to production\n\n\
                PROJECTS\n\n";
    let ledger = "**Ledger CLI** · https://github.com/janedoe/ledger\nRust · SQLite\n\n";
    let invoices =
        "**Invoice Parser** · https://github.com/janedoe/invoices\nPython · Tesseract\n\n";
    let routes = "**Route Planner** · https://github.com/janedoe/routes\nGo · PostgreSQL\n";

    let source = format!("{head}{ledger}{invoices}{routes}");
    let trimmed = format!("{head}{ledger}{invoices}");
    silent(
        &report_for(&trimmed, &source, EN_JOB_AD, &[]),
        FACTUAL_ALTERED_PROJECT_LINK,
    );

    // The guard, and the reason this is entry-scoped rather than switched off:
    // an entry the document KEPT must still match its own link exactly. A
    // changed link surfaces as one drop plus one invention.
    let rehosted =
        "**Invoice Parser** · https://github.com/someone-else/invoices\nPython · Tesseract\n\n";
    let altered = format!("{head}{ledger}{rehosted}{routes}");
    let report = report_for(&altered, &source, EN_JOB_AD, &[]);
    let evidence: Vec<&str> = fired(&report, FACTUAL_ALTERED_PROJECT_LINK)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert_eq!(
        evidence,
        vec![
            "https://github.com/janedoe/invoices",
            "https://github.com/someone-else/invoices"
        ],
        "a surviving entry's changed link is still a drop plus an invention"
    );

    // …and a link the source never carried at all is still an invention, even
    // when it arrives on an entry that is itself new.
    let invented = format!("{head}{ledger}{invoices}{routes}\
                            **Fleet Dashboard** · https://github.com/someone-else/fleet\nTypeScript · D3\n");
    let report = report_for(&invented, &source, EN_JOB_AD, &[]);
    let evidence: Vec<&str> = fired(&report, FACTUAL_ALTERED_PROJECT_LINK)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert_eq!(evidence, vec!["https://github.com/someone-else/fleet"]);
}

/// R10-F4 — [`is_bare_phone_line`] ran on BOTH sides of the metric comparison,
/// so the SOURCE lost every bare-numeric line from its sourced whitelist. A
/// figure the candidate wrote on a line of its own — the shape a table-extracted
/// PDF produces — could no longer vouch for its own restatement, which is an
/// ACCUSATION channel, and it broke the stated `MetricSide` invariant that the
/// source side reads strictly more than the claims side.
#[test]
fn a_figure_alone_on_a_source_line_still_sources_its_restatement() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                  - Refunds processed in the first quarter after the rewrite:\n\
                  1 200 000\n";
    let restated = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Processed 1,200,000 refunds in the first quarter after the rewrite\n";
    silent(
        &report_for(restated, source, EN_JOB_AD, &[]),
        FACTUAL_UNSOURCED_METRIC,
    );

    // The claims side is unchanged — a number-only line still makes no claim,
    // so a figure the source never states is still a Critical.
    let fabricated = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                      - Processed 9,400,000 refunds in the first quarter after the rewrite\n";
    let report = report_for(fabricated, source, EN_JOB_AD, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("9,400,000"));

    // The source side is widened in the BODY only, not wholesale: a bare number
    // in the contact band is still contact details and still cannot vouch for a
    // claim. That half lives in
    // `a_number_alone_on_a_line_is_a_phone_number_not_three_claims`
    // (direction 3), which a "claims side only" fix would have turned red —
    // which is why this rule is scoped by band rather than by side alone.
}

/// R11-F1 / R11-F5(b) — [`HEADER_PHONE_RE`]'s `[+(]` arm reads a parenthesized
/// DATE SPAN as an area code followed by digits: "(2019 - 2021)" is `(`, a
/// digit, eight more separator-or-digit characters and a digit.
/// `ats::is_contact_cluster` already refused a year-bearing line; the OTHER
/// caller, [`has_real_contact_match`], did not — so a pre-heading line carrying
/// a date span was struck out of the SOURCE's metric set, and restating its
/// figure came back as a fabrication Critical.
///
/// The guard therefore belongs to the shape test itself, not to one call site:
/// a date span is not a phone number wherever the question is asked.
#[test]
fn a_parenthesized_date_span_is_not_a_header_phone() {
    assert!(!looks_like_header_phone("Beratung (2019 - 2021)"));
    assert!(!has_real_contact_match("Beratung (2019 - 2021)"));
    // The two forms the shape test exists for still match.
    assert!(looks_like_header_phone("+49 30 1234567"));
    assert!(has_real_contact_match("Jane Doe | +49 (0)30 1234567"));
    assert!(has_real_contact_match("jane@example.com"));

    // End to end: the figure sits on a pre-heading line that also carries a
    // parenthesized span — the shape a freelance/contract summary has.
    let source = "Jane Doe\n\
                  jane@example.com | +49 30 1234567\n\n\
                  Contract work (2019 - 2021): 1 200 000 EUR in payment volume\n\n\
                  EXPERIENCE\n\n\
                  Senior Engineer | Acme Payments | 2021 - Present\n\
                  - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    let restated = "Jane Doe\n\
                    jane@example.com | +49 30 1234567\n\n\
                    EXPERIENCE\n\n\
                    Senior Engineer | Acme Payments | 2021 - Present\n\
                    - Moved 1 200 000 EUR of payment volume onto the new rails\n\
                    - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    silent(
        &report_for(restated, source, EN_JOB_AD, &[]),
        FACTUAL_UNSOURCED_METRIC,
    );

    // The guard: a genuine header line is still contact details, so its digits
    // still cannot source a body claim.
    let phone_source = "Jane Doe\n\
                        jane@example.com | +49 30 1234567\n\n\
                        EXPERIENCE\n\n\
                        Senior Engineer | Acme Payments | 2021 - Present\n\
                        - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    let borrowed = "Jane Doe\n\
                    jane@example.com | +49 30 1234567\n\n\
                    EXPERIENCE\n\n\
                    Senior Engineer | Acme Payments | 2021 - Present\n\
                    - Handled 1234567 disputes in the first quarter\n";
    fired(
        &report_for(borrowed, phone_source, EN_JOB_AD, &[]),
        FACTUAL_UNSOURCED_METRIC,
    );

    // The OTHER caller keeps the behaviour its own now-deleted `years_in`
    // clause bought: an employer line followed by a bare date column is not a
    // second contact block. This is the assertion that makes moving the guard
    // into the shape test provably behaviour-preserving for `ats`, rather than
    // just "the suite still passes".
    let date_column = "Jane Doe\n\
                       jane@example.com\n\n\
                       EXPERIENCE\n\n\
                       Acme Payments\n\
                       2018 - 2021\n\
                       - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    silent(
        &report_for(date_column, date_column, EN_JOB_AD, &[]),
        ATS_HEADER_IN_BODY,
    );
}

/// R11-F2 — `title_drift_issues` paired a generated entry with only the FIRST
/// source entry sharing company tokens, so two roles at the SAME employer — a
/// promotion, an internal move, the commonest thing on a résumé — reported the
/// second one as drift on a document byte-identical to its source.
#[test]
fn title_drift_tolerates_a_second_role_at_the_same_employer() {
    let resume = "EXPERIENCE\n\n\
                  Senior Engineer, Acme Corp (Jan 2021 - Mar 2023)\n\
                  - Shipped Docker containers to production\n\n\
                  Product Manager, Acme Corp (Jan 2018 - Dec 2020)\n\
                  - Ran the reporting service\n";
    silent(
        &report_for(resume, resume, EN_JOB_AD, &[]),
        CONSISTENCY_TITLE_DRIFT,
    );

    // The guard: a title the source never gave this employer STILL fires. The
    // rule is "disjoint from EVERY source title at that employer", not "skip
    // employers with more than one role".
    let invented = "EXPERIENCE\n\n\
                    Chief Revenue Officer, Acme Corp (Jan 2021 - Mar 2023)\n\
                    - Shipped Docker containers to production\n";
    let report = report_for(invented, resume, EN_JOB_AD, &[]);
    let hits = fired(&report, CONSISTENCY_TITLE_DRIFT);
    assert_eq!(hits.len(), 1, "one entry, one finding; got {hits:?}");
}

/// R11-F5(a) — `split_sections` inherits `export::parser`'s heading detection:
/// an EXACT `SECTION_NAMES` entry, an ATX marker, or ALL-CAPS. A Title-Case
/// heading that is not literally in that list is invisible, the whole résumé
/// collapses into ONE section, `has_headings` goes false, and `metric_lines`
/// then runs the COVER-LETTER rules (the 8-word body latch) over a résumé —
/// eating every short source line, figures included.
///
/// **The reviewer's own examples are not the failing ones**, and that is worth
/// keeping written down: "Berufserfahrung", "Ausbildung", "Formation" and
/// "Expérience professionnelle" are all in `SECTION_NAMES` and match
/// case-insensitively. What fails is every Title-Case heading OUTSIDE that
/// exact list — "Beruflicher Werdegang", "Berufliche Erfahrung", "Technische
/// Kenntnisse", "Kurzprofil", "Compétences techniques" — each of which
/// `documents::evidence::classify_section` already classifies correctly.
#[test]
fn a_title_cased_german_heading_still_sections_the_source() {
    let source = "Jana Mustermann\n\
                  jana.mustermann@example.com | +49 30 7654321\n\n\
                  Kurzprofil\n\n\
                  Verantwortetes Budget: 1 200 000 EUR\n\n\
                  Beruflicher Werdegang\n\n\
                  Senior Backend Engineer | Acme Payments | 2021 - Heute\n\
                  - Die Wartezeit an der Kasse von 480ms auf 90ms gesenkt\n";

    let sections = split_sections(source, DocKind::Resume);
    assert!(
        sections.len() > 1,
        "a Title-Case German heading must section the document; got {:?}",
        sections.iter().map(|s| &s.heading).collect::<Vec<_>>()
    );
    assert!(
        sections.iter().any(|s| s.kind == SectionKind::Experience),
        "\"Beruflicher Werdegang\" is an experience heading; got {:?}",
        sections.iter().map(|s| s.kind).collect::<Vec<_>>()
    );

    // The damage: the figure on the short pre-heading line is the candidate's
    // own, and restating it must not read as fabrication.
    let generated = "Jana Mustermann\n\
                     jana.mustermann@example.com | +49 30 7654321\n\n\
                     PROFIL\n\n\
                     Ein Budget von 1 200 000 EUR verantwortet und die Plattform aufgebaut\n\n\
                     BERUFSERFAHRUNG\n\n\
                     Senior Backend Engineer | Acme Payments | 2021 - Heute\n\
                     - Die Wartezeit an der Kasse von 480ms auf 90ms gesenkt\n";
    silent(
        &validate_content(&ContentInput {
            generated,
            source_resume: source,
            job_ad: DE_JOB_AD,
            top_requirements: &[],
            target_language: "de",
            doc_kind: DocKind::Resume,
        }),
        FACTUAL_UNSOURCED_METRIC,
    );

    // The negative twin, one line per guard — each candidate OPENS a block, so
    // the only thing refusing it is the guard named beside it. Every one of
    // these carries a heading stem the lexicon matches as a substring.
    let prose = "Jana Mustermann\n\
                 jana@example.com\n\n\
                 Ich habe umfangreiche Erfahrung mit Docker und Kubernetes gesammelt.\n\n\
                 Cloud Kubernetes Docker Redis Terraform\n\n\
                 Erfahrung 2019\n\n\
                 Kenntnisse: Rust, Python\n\n\
                 Rust Python Go\n\
                 Weitere Kenntnisse\n";
    assert_eq!(
        split_sections(prose, DocKind::Resume).len(),
        1,
        "a line that merely carries a heading word is not a heading — too long \
         (sentence), too many words, digit-bearing, punctuated, or mid-block; \
         got {:?}",
        split_sections(prose, DocKind::Resume)
            .iter()
            .map(|s| &s.heading)
            .collect::<Vec<_>>()
    );
}

// ── PR #963 round-12 findings ───────────────────────────────────────────────

/// R12-F1 — `consistency::skill_not_demonstrated` compares a document against
/// ITSELF, but it tokenized both sides with [`Analysis::tokens`], whose stemming
/// decision is `languages_align(job_ad, target_language)`. An English ad for a
/// German-language role (the ordinary DACH case) therefore switches stemming OFF
/// for a comparison the ad is not part of, and every German inflection pair —
/// "Abrechnungsschnittstelle" in the experience, "Abrechnungsschnittstellen"
/// under skills — reads as a skill the résumé never demonstrates.
///
/// The control is the same document against a GERMAN ad: only the AD's language
/// differs between the two runs, so the two answers must be identical.
#[test]
fn a_foreign_language_job_ad_does_not_break_the_skills_self_comparison() {
    let generated = DE_CLEAN.replace(
        "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis",
        "Rust · Python · Docker · Kubernetes · Abrechnungsschnittstellen · \
         Lagerstandorten · Kafka",
    );
    let against = |job_ad: &str| {
        validate_content(&ContentInput {
            generated: &generated,
            source_resume: DE_SOURCE,
            job_ad,
            top_requirements: &[],
            target_language: "de",
            doc_kind: DocKind::Resume,
        })
    };
    let undemonstrated = |report: &ContentReport| -> Vec<String> {
        report
            .issues
            .iter()
            .filter(|i| i.code == CONSISTENCY_SKILL_NOT_DEMONSTRATED)
            .filter_map(|i| i.evidence.clone())
            .collect()
    };

    // The control: with a German ad the pair is stemmed, so only the genuine
    // gap — a skill the experience never shows — is reported.
    assert_eq!(
        undemonstrated(&against(DE_JOB_AD)),
        vec!["kafka".to_string()],
        "the German inflections are demonstrated; only Kafka is not"
    );
    // The finding: the ad's language is not part of this comparison.
    assert_eq!(
        undemonstrated(&against(EN_JOB_AD)),
        vec!["kafka".to_string()],
        "a document-internal check must not change with the AD's language"
    );
}

/// R12-F2 — `ats::keyword_density` and `consistency::skill_not_demonstrated`
/// both filter their tokens through `function_words(ctx.lang)`, the TARGET
/// language — even when `content.language_mismatch` has already established
/// (with a reliable source control) that the document is written in some other
/// language. The filter is then a no-op against the words that are actually on
/// the page, so ordinary German prose is accused of keyword stuffing and German
/// filler is reported back as an undemonstrated skill, underneath the one
/// Critical that matters.
#[test]
fn a_wrong_language_document_is_not_measured_with_the_target_word_list() {
    // A German résumé that really IS stuffed (Kubernetes ×8) and really DOES
    // claim a skill it never shows (Kafka) — so both checks have something true
    // to say. Every "verantwortlich für" around them is ordinary German bullet
    // phrasing, and both of those words are in FUNCTION_WORDS_DE, where
    // `function_words("en")` cannot see them.
    let german = "Jana Mustermann\njana.mustermann@example.com\n\n\
                  PROFIL\n\n\
                  Backend-Entwicklerin mit acht Jahren Erfahrung im Zahlungsverkehr und im \
                  Aufbau von Container-Plattformen.\n\n\
                  BERUFSERFAHRUNG\n\n\
                  Senior Backend Engineer | Acme Payments | 2021 - Heute\n\
                  - Verantwortlich für den Betrieb der Zahlungsplattform auf Kubernetes\n\
                  - Verantwortlich für Kubernetes-Cluster, Kubernetes-Netzwerke und \
                  Kubernetes-Speicher\n\
                  - Verantwortlich für Kubernetes-Upgrades und für die Kubernetes-Bereitschaft\n\
                  - Verantwortlich für die Bereitstellung mit Kubernetes, Python und Terraform\n\n\
                  Backend Developer | Globex Logistics | 2018 - 2021\n\
                  - Verantwortlich für die Abrechnungsschnittstelle der Lagerstandorte\n\
                  - Verantwortlich für die Datenbanken und für die Wartung in Rust\n\
                  - Verantwortlich für die Migration und für den Betrieb\n\n\
                  KENNTNISSE\n\n\
                  Kenntnisse in Rust, Python, Kubernetes, Terraform und Kafka\n\n\
                  AUSBILDUNG\n\n\
                  BSc Informatik, TU Berlin, 2014 - 2018\n";

    // Read as English: the finding the user must act on is present and
    // Critical, and neither word-list-dependent check adds noise underneath it.
    let mismatched = en_resume(german, &[]);
    assert_eq!(
        fired(&mismatched, CONTENT_LANGUAGE_MISMATCH)[0].severity,
        Severity::Critical
    );
    silent(&mismatched, ATS_KEYWORD_DENSITY);
    silent(&mismatched, CONSISTENCY_SKILL_NOT_DEMONSTRATED);

    // The control — the SAME document, read as the German it is. Both checks
    // still bite, so the suppression is keyed on the mismatch rather than
    // switching the checks off, and the filler around the real findings is
    // filtered instead of counted.
    let matched = validate_content(&ContentInput {
        generated: german,
        source_resume: DE_SOURCE,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    let stuffed: Vec<&str> = fired(&matched, ATS_KEYWORD_DENSITY)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert!(
        stuffed.contains(&"kubernetes ×8"),
        "the real repetition is reported; got {stuffed:?}"
    );
    assert!(
        !stuffed
            .iter()
            .any(|e| e.starts_with("verantwortlich") || e.starts_with("für")),
        "German filler is FILTERED at a German target, not counted; got {stuffed:?}"
    );
    let undemonstrated: Vec<&str> = fired(&matched, CONSISTENCY_SKILL_NOT_DEMONSTRATED)
        .iter()
        .filter_map(|i| i.evidence.as_deref())
        .collect();
    assert_eq!(undemonstrated, vec!["kafka"]);
}

/// R12-F3 — the round-11 promotion gate was DOCUMENT-WIDE ("the parser found no
/// heading at all"), and `export::parser`'s `SECTION_NAMES` is not English-only:
/// "ausbildung", "kenntnisse", "formation" and "compétences" are all in it. One
/// conventional single-word heading therefore switched promotion off for the
/// whole document — including for the fix's own headline case, "Beruflicher
/// Werdegang", in the completely ordinary mixed résumé below.
#[test]
fn a_title_cased_heading_is_promoted_beside_a_parser_recognised_one() {
    // "Ausbildung" is a literal `SECTION_NAMES` entry; the other three headings
    // are Title-Case and outside that list.
    let resume = "Jana Mustermann\n\
                  jana.mustermann@example.com | +49 30 7654321\n\n\
                  Kurzprofil\n\n\
                  Backend-Entwicklerin mit acht Jahren Erfahrung im Zahlungsverkehr.\n\n\
                  Beruflicher Werdegang\n\n\
                  Senior Backend Engineer | Acme Payments | 2021 - Heute\n\
                  - Die Wartezeit an der Kasse von 480ms auf 90ms gesenkt\n\n\
                  Technische Kenntnisse\n\n\
                  Rust · Python · Docker · Kubernetes\n\n\
                  Ausbildung\n\n\
                  BSc Informatik, TU Berlin, 2014 - 2018\n";
    let kinds: Vec<SectionKind> = split_sections(resume, DocKind::Resume)
        .iter()
        .map(|s| s.kind)
        .collect();
    for expected in [
        SectionKind::Summary,
        SectionKind::Experience,
        SectionKind::Skills,
        SectionKind::Education,
    ] {
        assert!(
            kinds.contains(&expected),
            "{expected:?} must survive one parser-recognised sibling; got {kinds:?}"
        );
    }

    // The user-visible damage: a résumé that has all three standard sections
    // was told it was missing two of them, and its roles never counted.
    let report = validate_content(&ContentInput {
        generated: resume,
        source_resume: resume,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    silent(&report, ATS_MISSING_SECTION);
    assert_eq!(report.metrics.roles_output, 1, "the entry is an entry");

    // The negative twin, now in a WELL-HEADED document — the shape guards carry
    // the whole weight once the document-wide gate is gone, so each one is
    // exercised here on its own line, every candidate opening a block.
    let prose = "Jana Mustermann\n\
                 jana@example.com\n\n\
                 BERUFSERFAHRUNG\n\n\
                 Ich habe umfangreiche Erfahrung mit Docker und Kubernetes gesammelt.\n\n\
                 Cloud Kubernetes Docker Redis Terraform\n\n\
                 Erfahrung 2019\n\n\
                 Kenntnisse: Rust, Python\n\n\
                 Rust Python Go\n\
                 Weitere Kenntnisse\n";
    assert_eq!(
        split_sections(prose, DocKind::Resume).len(),
        2,
        "a line that merely carries a heading word is not a heading — too long \
         (sentence), too many words, digit-bearing, punctuated, unclassified, or \
         mid-block; got {:?}",
        split_sections(prose, DocKind::Resume)
            .iter()
            .map(|s| &s.heading)
            .collect::<Vec<_>>()
    );
}

/// R12-F4 — round 11 folded a year test into [`looks_like_header_phone`] to stop
/// a parenthesized DATE SPAN reading as an area code, and wrote off "a header
/// phone containing a 1900–2099 run" as a missed skip. It is not a missed skip,
/// it is an ACCUSATION channel: the source's contact band still drops such a
/// line (via `is_bare_phone_line`), while the letter that repeats the same
/// number keeps it (the shape test says it is not contact details) — so the
/// candidate's own phone digits come back as a fabricated metric.
#[test]
fn a_header_phone_carrying_a_year_run_is_still_contact_details() {
    assert!(looks_like_header_phone("+49 30 2019 1234"));
    assert!(has_real_contact_match(
        "Max Mustermann · Berlin · +49 30 2019 1234"
    ));
    // R11-F1 stays closed: a span is still not a phone number, on both callers.
    assert!(!looks_like_header_phone("Beratung (2019 - 2021)"));
    assert!(!has_real_contact_match(
        "Contract work (2019 - 2021): 1 200 000 EUR in payment volume"
    ));

    let source = "Max Mustermann\n\
                  max.mustermann@example.com\n\
                  +49 30 2019 1234\n\n\
                  EXPERIENCE\n\n\
                  Acme Payments | 2021 - Present\n\
                  - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    let letter = "Max Mustermann · Berlin · +49 30 2019 1234\n\n\
                  Dear Hiring Manager,\n\n\
                  I put a Redis cache in front of the ledger service and checkout latency \
                  went from 480ms to 90ms.\n\n\
                  Best regards,\nMax Mustermann\n";
    silent(
        &letter_report_for(letter, source, EN_JOB_AD),
        FACTUAL_UNSOURCED_METRIC,
    );

    // The invariant's own direction is unchanged: the source's contact band is
    // dropped from the truth set, so its digits still cannot vouch for a body
    // claim that merely reuses them.
    let borrowed = "Max Mustermann\n\
                    max.mustermann@example.com\n\
                    +49 30 2019 1234\n\n\
                    EXPERIENCE\n\n\
                    Acme Payments | 2021 - Present\n\
                    - Settled 1234 disputes in the first quarter after the rewrite\n";
    let borrowed_report = report_for(borrowed, source, EN_JOB_AD, &[]);
    let hits = fired(&borrowed_report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("1234"));

    // …and a letter figure the source never states is still a Critical, so the
    // skip exempts the letterhead rather than the letter.
    let fabricated = "Max Mustermann · Berlin · +49 30 2019 1234\n\n\
                      Dear Hiring Manager,\n\n\
                      I put a Redis cache in front of the ledger service and settled 9400 \
                      disputes in the same quarter.\n\n\
                      Best regards,\nMax Mustermann\n";
    let fabricated_report = letter_report_for(fabricated, source, EN_JOB_AD);
    let hits = fired(&fabricated_report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("9400"));
}

// ── PR #963 round-13 findings ───────────────────────────────────────────────

/// R13-F1 — round 12's per-line promotion reads an ordinary JOB TITLE as a
/// section heading. "Projektleiter" on its own line above the employer is
/// `LineKind::Text` (the parser's `JobTitle` arm needs the PREVIOUS line to
/// carry a date column, and here it is blank), it opens a block, it is one word,
/// it carries no digit and no punctuation — and `classify_section` matches the
/// `projekt` stem, so a `SectionKind::Projects` heading appears in the middle of
/// the experience section. Same for "Senior Project Manager".
///
/// The fixture carries BOTH shapes the guard reads, one per clause: two entry
/// lines `export::parser` recognises as `JobEntry`, and one it does not
/// ("Nordwind Systeme, Ingolstadt, 2016 - 2018" — a trailing date COLUMN, the
/// other way `documents::evidence` opens a role).
#[test]
fn a_job_title_above_its_employer_is_not_a_projects_heading() {
    let resume = "Jana Mustermann\n\
                  jana.mustermann@example.com | +49 30 7654321\n\n\
                  BERUFSERFAHRUNG\n\n\
                  Projektleiter\n\
                  Acme Payments · Berlin · 2021 - Heute\n\
                  - Die Wartezeit an der Kasse von 480ms auf 90ms gesenkt\n\n\
                  Senior Project Manager\n\
                  Globex Logistics · München · 2018 - 2021\n\
                  - Die Abrechnung für 40 Lagerstandorte automatisiert\n\n\
                  Projektassistenz\n\
                  Nordwind Systeme, Ingolstadt, 2016 - 2018\n\
                  - Die Rechnungsprüfung für zwei Werke übernommen\n\n\
                  KENNTNISSE\n\n\
                  Rust · Python · Docker · Kubernetes\n";

    let kinds: Vec<SectionKind> = split_sections(resume, DocKind::Resume)
        .iter()
        .map(|s| s.kind)
        .collect();
    assert!(
        !kinds.contains(&SectionKind::Projects),
        "a job title above its employer is not a Projects heading; got {kinds:?}"
    );

    // The user-visible damage: the two entries leave the experience section, so
    // the résumé counts no roles, and every bullet under the phantom heading is
    // graded as a malformed project card.
    let report = validate_content(&ContentInput {
        generated: resume,
        source_resume: resume,
        job_ad: DE_JOB_AD,
        top_requirements: &[],
        target_language: "de",
        doc_kind: DocKind::Resume,
    });
    assert_eq!(report.metrics.roles_output, 2, "two entries, two roles");
    silent(&report, CONSISTENCY_PROJECT_STRUCTURE);

    // The guard's own boundary, pinned so it cannot become "never promote": the
    // SAME word with a blank line under it is a heading over a block, not a
    // label on the line below, and it still promotes.
    let headed = resume.replace(
        "Projektleiter\nAcme Payments",
        "Projektleiter\n\nAcme Payments",
    );
    assert!(
        split_sections(&headed, DocKind::Resume)
            .iter()
            .any(|s| s.kind == SectionKind::Projects),
        "a candidate followed by a BLANK still reads as a heading; got {:?}",
        split_sections(&headed, DocKind::Resume)
            .iter()
            .map(|s| &s.heading)
            .collect::<Vec<_>>()
    );
}

/// R13-F2 — promotion is unconditionally live on cover letters. A letter never
/// has parser headings, so any short label line ("My Experience", "Kurzprofil")
/// promotes, `sections.len() > 1` flips `factual::metric_lines` from the LETTER
/// path to the résumé one, and section 0 — the whole opening of the letter — is
/// then skipped by POSITION on the claims side. Most of the letter stops being
/// checked for fabricated numbers at all.
#[test]
fn a_mini_heading_in_a_letter_does_not_exempt_its_opening() {
    let source = "Max Mustermann\n\
                  max.mustermann@example.com\n\n\
                  EXPERIENCE\n\n\
                  Acme Payments | 2021 - Present\n\
                  - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    let opening = "Max Mustermann\n\
                   max.mustermann@example.com\n\n\
                   Dear Hiring Manager,\n\n\
                   I rebuilt the settlement pipeline at Acme Payments and cleared a backlog \
                   of 4700 reconciliation cases in a single quarter.\n\n";
    let sign_off = "I would welcome the chance to do the same for your ledger team.\n\n\
                    Best regards,\nMax Mustermann\n";

    // The control: the same letter with no label line. The invented figure is a
    // Critical, as it has been since round 9.
    let plain = letter_report_for(&format!("{opening}{sign_off}"), source, EN_JOB_AD);
    assert_eq!(
        fired(&plain, FACTUAL_UNSOURCED_METRIC)[0]
            .evidence
            .as_deref(),
        Some("4700")
    );

    // The finding: one two-word label line above the sign-off, and the opening
    // paragraph stops being read at all.
    let labelled = letter_report_for(
        &format!("{opening}My Experience\n\n{sign_off}"),
        source,
        EN_JOB_AD,
    );
    assert_eq!(
        fired(&labelled, FACTUAL_UNSOURCED_METRIC)[0]
            .evidence
            .as_deref(),
        Some("4700"),
        "a label line inside a letter must not exempt the letter's opening"
    );
}

/// R13-F3 — `consistency::title_drift` stems both sides through
/// [`Analysis::tokens`], whose decision is `languages_align(job_ad, target)`. The
/// ad is not a party to a source-title↔generated-title comparison, so an English
/// ad for a German-language role switches stemming OFF and the ordinary
/// declension pair "Wissenschaftlicher Mitarbeiter" / "Wissenschaftliche
/// Mitarbeiterin" reads as two disjoint titles — a false drift Warning. Unstemmed
/// is the direction that FIRES more, which is what makes this an accusation
/// channel rather than a missed check.
///
/// The control is the same document pair against a GERMAN ad: only the AD's
/// language differs between the two runs, so the two answers must be identical.
#[test]
fn a_foreign_language_job_ad_does_not_break_the_title_comparison() {
    let source = DE_SOURCE.replace(
        "Senior Backend Engineer | Acme Payments",
        "Wissenschaftlicher Mitarbeiter | Acme Payments",
    );
    let generated = DE_CLEAN.replace(
        "Senior Backend Engineer | Acme Payments",
        "Wissenschaftliche Mitarbeiterin | Acme Payments",
    );
    let against = |job_ad: &str| {
        validate_content(&ContentInput {
            generated: &generated,
            source_resume: &source,
            job_ad,
            top_requirements: &[],
            target_language: "de",
            doc_kind: DocKind::Resume,
        })
    };
    let drift = |report: &ContentReport| -> Vec<String> {
        report
            .issues
            .iter()
            .filter(|i| i.code == CONSISTENCY_TITLE_DRIFT)
            .filter_map(|i| i.evidence.clone())
            .collect()
    };

    // The control: with a German ad the pair is stemmed, the two declensions
    // share "wissenschaftlich", and nothing is reported.
    assert!(
        drift(&against(DE_JOB_AD)).is_empty(),
        "a declension pair is not a title change; got {:?}",
        drift(&against(DE_JOB_AD))
    );
    // The finding: the ad's language is not part of this comparison.
    assert!(
        drift(&against(EN_JOB_AD)).is_empty(),
        "a document↔source comparison must not change with the AD's language; \
         got {:?}",
        drift(&against(EN_JOB_AD))
    );
}

/// R13-F3, the third `Analysis::tokens` document-internal caller. `duplicates`
/// compares two bullets of ONE document, so the posting is not a party to it
/// either — yet `duplicateRatio`, a reported metric, changed with the AD's
/// language. Un-stemming merges LESS here, so the direction is the opposite of
/// `title_drift`'s: a foreign-language ad HID a repeated bullet rather than
/// inventing one. It is still a measurement depending on an input it does not
/// measure, which is what the fix removes.
#[test]
fn a_foreign_language_job_ad_does_not_change_the_duplicate_ratio() {
    // A second bullet that says the first one again in different inflections —
    // the model padding a role, written the way German pads it.
    let generated = DE_CLEAN.replace(
        "- Den Wiederholungsplaner in Rust neu geschrieben",
        "- Betrieb der Docker-Container auf den Kubernetes-Clustern, die pro Sekunde 12000 \
         Anfragen beantworteten\n\
         - Den Wiederholungsplaner in Rust neu geschrieben",
    );
    let against = |job_ad: &str| {
        validate_content(&ContentInput {
            generated: &generated,
            source_resume: DE_SOURCE,
            job_ad,
            top_requirements: &[],
            target_language: "de",
            doc_kind: DocKind::Resume,
        })
    };

    let de = against(DE_JOB_AD);
    let en = against(EN_JOB_AD);
    // Not vacuous: the repetition IS reported, so the equality below is two
    // findings agreeing rather than two silences.
    assert_eq!(fired(&de, DUPLICATE_BULLET).len(), 1);
    assert_eq!(
        de.metrics.duplicate_ratio, en.metrics.duplicate_ratio,
        "a document-internal ratio must not change with the AD's language"
    );
    assert_eq!(
        fired(&en, DUPLICATE_BULLET)[0].evidence,
        fired(&de, DUPLICATE_BULLET)[0].evidence
    );
}

// ── PR #963 round-13, wave 2 (ats.rs) ───────────────────────────────────────

/// R13-W1 — the round-12 `i == 1` narrowing's own named false positive survives
/// at exactly that index. A conventional references block is a NAME on the
/// section's first line and that person's address under it: the heading goes to
/// `Section::heading` rather than into `lines`, `Blank` is filtered out, so the
/// reference lands at index 1 and satisfies every clause. The candidate is told
/// their document has two headers because they listed a referee.
///
/// What the Critical actually claims is that the candidate's OWN header appears
/// twice, so the discriminator is whose details these are.
#[test]
fn a_reference_block_is_not_a_second_contact_block() {
    let referees = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                    Acme | 2021 - Present\n- Led the migration\n\n\
                    REFERENCES\n\nMaria Lang\nmaria.lang@acme.example.com\n";
    silent(
        &report_for(referees, referees, EN_JOB_AD, &[]),
        ATS_HEADER_IN_BODY,
    );

    // A contact PERSON under a heading no list knows is the same shape and the
    // same non-defect.
    let ansprechpartner = "Jana Mustermann\njana@example.com\n\nBERUFSERFAHRUNG\n\n\
                           Acme | 2021 - Heute\n- Die Abrechnung betreut\n\n\
                           ANSPRECHPARTNER\n\nMaria Lang\nmaria.lang@acme.example.com\n";
    silent(
        &report_for(ansprechpartner, ansprechpartner, EN_JOB_AD, &[]),
        ATS_HEADER_IN_BODY,
    );

    // The genuine defect — the candidate's own header block, pasted a second
    // time — is still Critical.
    let repeated = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                    Acme | 2021 - Present\n- Led the migration\n\n\
                    CONTACT\n\nJane Doe\njane@example.com\n";
    let report = report_for(repeated, repeated, EN_JOB_AD, &[]);
    assert_eq!(
        fired(&report, ATS_HEADER_IN_BODY)[0].severity,
        Severity::Critical
    );
}

/// R13-W2 — `bullets_per_role` opens a role only on `LineKind::JobEntry` and
/// appends everything else to `roles.last_mut()`, which is the misattribution
/// class `documents::evidence` spent rounds 6–11 removing. Both read the same
/// `ParsedLine` stream: `extract_evidence` opens a role on a `Text`/`Contact`
/// line that ends in a date COLUMN, this one appends to the employer above. So
/// the second employer's bullets are counted against the first.
#[test]
fn bullets_are_counted_against_the_role_they_belong_to() {
    // Globex parses as a `JobEntry`; the Acme line does not (no two-space
    // column, no pipes, no parens) — it is the extracted-PDF comma form.
    let resume = "Jane Doe\njane@example.com\n\nEXPERIENCE\n\n\
                  Globex Logistics | 2021 - Present\n\
                  - Built the billing API\n\
                  - Owned the release train\n\n\
                  Acme Payments, Berlin, 2018 - 2021\n\
                  - Shipped Docker containers to production\n\
                  - Cut checkout latency from 480ms to 90ms\n\
                  - Rewrote the retry scheduler in Rust\n\
                  - Migrated the fleet to AWS\n\
                  - Described the deployment in Terraform\n\
                  - Ran the on-call rotation\n\
                  - Wrote the runbooks\n";
    let report = report_for(resume, resume, EN_JOB_AD, &[]);
    let hits = fired(&report, ATS_BULLET_COUNT);
    assert_eq!(hits.len(), 1, "one role is over the band; got {hits:?}");
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("Acme Payments, Berlin"),
        "the bullets belong to the employer they sit under, not the one above"
    );
    assert!(
        hits[0].message.contains("has 7 bullets"),
        "seven of the nine bullets are Acme's; got {:?}",
        hits[0].message
    );
}

/// R13-W3/W4 — `ats.bullet_count` passed the hardcoded English "Experience" as
/// the issue's `section`, which the panel renders verbatim as a GROUPING KEY. A
/// German résumé therefore showed two groups for one section: "BERUFSERFAHRUNG"
/// (from `long_bullet`, which reads the document's own heading) and an
/// untranslated "Experience" (from `bullet_count`).
#[test]
fn bullet_count_is_grouped_under_the_documents_own_heading() {
    // Eight bullets (over the band) and one of them over the character budget,
    // so both checks fire on the SAME section and their grouping keys are
    // directly comparable.
    let resume = "Jana Mustermann\njana@example.com\n\nBERUFSERFAHRUNG\n\n\
                  Acme Payments | 2021 - Heute\n\
                  - Die Wartezeit an der Kasse gesenkt\n\
                  - Die Abrechnung betreut\n\
                  - Die Flotte migriert\n\
                  - Die Bereitstellung beschrieben\n\
                  - Die Datenbanken gewartet\n\
                  - Die Migration begleitet\n\
                  - Die Rufbereitschaft übernommen\n\
                  - Die Wartezeit an der Kasse von 480ms auf 90ms gesenkt, indem ein \
                  Redis-Cache vorgeschaltet, die Abfragen zusammengefasst und die \
                  Verbindungen gebündelt wurden, was zusätzlich die Kosten im \
                  Rechenzentrum deutlich reduziert hat\n";
    let report = report_for(resume, resume, EN_JOB_AD, &[]);
    let grouping_keys: Vec<Option<&str>> = report
        .issues
        .iter()
        .filter(|i| i.code == ATS_BULLET_COUNT || i.code == ATS_LONG_BULLET)
        .map(|i| i.section.as_deref())
        .collect();
    assert_eq!(
        grouping_keys,
        vec![Some("BERUFSERFAHRUNG"), Some("BERUFSERFAHRUNG")],
        "one section, one group — the key is the document's own heading"
    );
}

// ── PR #963 round-14 findings ───────────────────────────────────────────────

/// R14-F2 — `SUFFIXED_NUMBER_RE` ran on the SOURCE side only, so a
/// magnitude-suffixed figure was never a CLAIM. `INTEGER_RE` sees only the
/// mantissa, and the mantissa is thrown away below three digits ("35k" → "35")
/// or on a decimal point ("3.5m" → "3.5"): a fabricated suffixed figure was
/// structurally invisible to `unsourced_metric`, not merely tolerated.
///
/// The mirror image of the same asymmetry is a false CRITICAL: "250k" left
/// "250" behind as a claim, which a source writing "250,000" never states.
#[test]
fn a_fabricated_suffixed_figure_is_an_unsourced_metric() {
    // The claims side must extract the EXPANDED value, the same language the
    // source side has always spoken.
    let claims: Vec<String> =
        factual::metrics_in(&resume_with_bullet("Onboarded 35k users"), DocKind::Resume)
            .into_iter()
            .map(|m| m.number)
            .collect();
    assert_eq!(
        claims,
        vec!["35000".to_string()],
        "one figure, expanded, and the mantissa is not a second claim"
    );

    let source = resume_with_bullet("Onboarded 4,000 users and booked 90,000 EUR in revenue");
    for (written, expanded) in [("35k", "35000"), ("3.5m", "3500000"), ("2bn", "2000000000")] {
        let fabricated = resume_with_bullet(&format!("Onboarded {written} users"));
        let report = report_for(&fabricated, &source, EN_JOB_AD, &[]);
        let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
        assert_eq!(
            hits.len(),
            1,
            "one invented figure is one Critical; got {hits:#?}"
        );
        assert_eq!(
            hits[0].evidence.as_deref(),
            Some(written),
            "the evidence quotes the span as written, not the expansion {expanded}"
        );
    }

    // …and the equivalence that makes the expansion worth having, in BOTH
    // directions: the same figure written the other way is not a fabrication.
    for (source_form, generated_form) in [
        ("10k requests", "10,000 requests"),
        ("10,000 requests", "10k requests"),
        ("250,000 requests", "250k requests"),
        ("1.2m requests", "1,200,000 requests"),
    ] {
        silent(
            &report_for(
                &resume_with_bullet(&format!("Handled {generated_form} per second at peak")),
                &resume_with_bullet(&format!("Handled {source_form} per second at peak")),
                EN_JOB_AD,
                &[],
            ),
            FACTUAL_UNSOURCED_METRIC,
        );
    }

    // The `\b` that keeps a millisecond figure out of the millions is still the
    // rule on both sides: `480ms` must not become 480 000 000.
    let latency = resume_with_bullet("Cut latency from 480ms to 90ms");
    silent(
        &report_for(&latency, &latency, EN_JOB_AD, &[]),
        FACTUAL_UNSOURCED_METRIC,
    );
}

/// R14-F3 — `canonical_link` dropped the scheme but not a leading `www.`, so
/// `https://www.github.com/janedoe/ledger` and `github.com/janedoe/ledger` —
/// the same resource, written the two ways résumés write it — keyed
/// differently and drew TWO `factual.altered_project_link` Criticals: the
/// source's link "missing or altered", plus the generated one "invented".
#[test]
fn a_www_prefix_is_the_same_link() {
    assert_eq!(
        factual::canonical_link("https://www.github.com/janedoe/ledger"),
        "github.com/janedoe/ledger"
    );
    assert_eq!(
        factual::canonical_link("WWW.GitHub.com/janedoe/ledger/"),
        factual::canonical_link("https://github.com/janedoe/ledger")
    );
    // Accented hosts, on the boundary this function has already panicked on
    // once: the `www.` strip is byte-compared, never byte-sliced blind.
    assert_eq!(
        factual::canonical_link("www.café-berlin.de"),
        factual::canonical_link("café-berlin.de"),
        "the same accented host written two ways is one key"
    );
    assert_eq!(
        factual::canonical_link("www.café-berlin.de"),
        "café-berlin.de"
    );
    assert_eq!(
        factual::canonical_link("HTTPS://WWW.Café-Berlin.DE/Über/"),
        "café-berlin.de/Über"
    );
    assert_eq!(factual::canonical_link("www.é.de"), "é.de");

    // `www` is a LABEL, not a prefix: a host that merely starts with those
    // three letters keeps them — including the multibyte twin, where a blind
    // four-byte slice would cut inside the char and abort the process.
    assert_eq!(factual::canonical_link("wwwé.de/x"), "wwwé.de/x");
    assert_eq!(
        factual::canonical_link("wwwx.example.com/x"),
        "wwwx.example.com/x"
    );
    // Only the FIRST label is stripped — `www.www.example.com` is a different
    // host from `www.example.com` and must stay one.
    assert_eq!(
        factual::canonical_link("www.www.example.com/x"),
        "www.example.com/x"
    );

    // End to end: the two spellings of one link, through `validate_content`.
    let source = "PROJECTS\n\n\
                  **Ledger CLI** · https://www.github.com/janedoe/ledger\n\
                  Rust · SQLite\n";
    let generated = "PROJECTS\n\n\
                     **Ledger CLI** · github.com/janedoe/ledger\n\
                     Rust · SQLite\n";
    silent(
        &report_for(generated, source, EN_JOB_AD, &[]),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
    // …and a genuinely different host is still Critical.
    let altered = "PROJECTS\n\n\
                   **Ledger CLI** · github.com/someone-else/ledger\n\
                   Rust · SQLite\n";
    fired(
        &report_for(altered, source, EN_JOB_AD, &[]),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
}

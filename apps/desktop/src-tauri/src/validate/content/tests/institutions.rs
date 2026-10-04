//! `factual.unsourced_institution`: an invented education section, and the cases that only
//! look like one.

use super::credential_calibration::institution_absent_by_value;
use super::credential_corpus::*;
use super::{support::*, *};

/// A2c positive: the source has no education section and names no institution
/// anywhere, and the output invented one. A Warning, not a Critical — see
/// `credentials::unsupported_institutions` for what was measured.
#[test]
fn an_education_section_invented_onto_a_source_that_has_none_is_a_warning() {
    let generated =
        "Jane Doe\n\nEDUCATION\n\nMSc Computer Science, Stanford University, 2012 - 2014\n";
    let report = report_against(generated, EN_SOURCE_FOUR_YEARS);
    assert_eq!(
        first_evidence(&report, FACTUAL_UNSOURCED_INSTITUTION),
        Some("Stanford University"),
        "the finding must fire and quote the institution, never the degree; report \
         carried {:?}",
        codes(&report)
    );
    // The SEVERITY of this finding, not `report.ok` — the same document also
    // drops both of the source's roles, so the report is blocked for a reason
    // that has nothing to do with education. Asserting `ok` here would pass on
    // the strength of the wrong Critical.
    assert_eq!(
        fired(&report, FACTUAL_UNSOURCED_INSTITUTION)[0].severity,
        Severity::Warning,
        "an invented institution is advisory: the value comparison that would justify a \
         Critical measured a false positive on truthful cross-language output"
    );
}

/// A2c negative and mutation check in one: the source DOES name a place of
/// study, so nothing is reported however different the institution is.
///
/// Drop the "the source names no institution at all" guard and this goes red,
/// because `TU Berlin` and `Stanford University` share nothing — which is
/// precisely the value comparison that was rejected.
#[test]
fn an_institution_is_never_compared_by_value_against_a_source_that_has_one() {
    let generated =
        "Jane Doe\n\nEDUCATION\n\nMSc Computer Science, Stanford University, 2012 - 2014\n";
    let report = report_against(generated, EN_SOURCE);
    silent(&report, FACTUAL_UNSOURCED_INSTITUTION);
}

/// A2c cross-language, and the measurement that scoped the check down: the
/// German rendering of an English source's institution is CORRECT output, and
/// the value comparison calls it an invention.
///
/// Both halves are asserted. The shipped check stays silent; the rejected one
/// fires — so this fails if the value comparison is ever re-adopted, and also
/// if it silently stops being wrong (in which case the scoping decision is the
/// thing to revisit, deliberately).
#[test]
fn institution_value_comparison_fires_on_a_correctly_translated_institution() {
    let source = "Jane Doe\n\nEDUCATION\n\n\
         BSc Computer Science, Technical University of Munich, 2014 - 2018\n";
    let generated = "Jana Mustermann\n\nAUSBILDUNG\n\n\
         BSc Informatik, Technische Universität München, 2014 - 2018\n";

    silent(
        &report_against(generated, source),
        FACTUAL_UNSOURCED_INSTITUTION,
    );
    assert!(
        institution_absent_by_value("Technische Universität München", source),
        "the rejected value comparison is recorded as firing here; if it no longer does, \
         the reason A2c ships as an absence check has changed and must be restated"
    );
}

/// Both clean fixtures are pinned to a COMPLETELY empty report elsewhere in
/// this file, which already covers the credential family there. This pins the
/// other end: the paraphrased pair, whose sources state a tenure in words and
/// whose output re-states it in different ones.
#[test]
fn the_paraphrased_fixtures_credentials_survive_being_reworded() {
    let de = report_in("de", DE_PARAPHRASED, DE_SOURCE, DE_JOB_AD);
    let en = report_against(EN_PARAPHRASED, EN_SOURCE);
    for report in [&de, &en] {
        for code in [
            FACTUAL_INFLATED_EXPERIENCE,
            FACTUAL_UNSOURCED_CERTIFICATION,
            FACTUAL_UNSOURCED_INSTITUTION,
        ] {
            silent(report, code);
        }
    }
}

/// An education history the section classifier does not recognise, whose
/// institution carries no marker word either, is still an education history.
///
/// `IIT Delhi` misses `INSTITUTION_MARKER_RE` and `QUALIFICATIONS` misses
/// `classify_section`, so the guard saw a source with no education at all and
/// warned about a truthful entry. The DEGREE token is the third layer.
///
/// Mutation check: drop `names_a_degree` from the guard and this goes red.
#[test]
fn a_degree_token_counts_as_education_when_the_institution_has_no_marker_word() {
    let source = "Ravi Menon\n\nEXPERIENCE\n\n\
         Backend Engineer | Acme Payments | 2019 - Present\n\
         - Built the settlement service in Go\n\n\
         QUALIFICATIONS\n\n\
         B.Tech Computer Science, IIT Delhi, 2012 - 2016\n";
    let generated = "Ravi Menon\n\nEDUCATION\n\n\
         B.Tech Computer Science, Indian Institute of Technology Delhi, 2012 - 2016\n";
    silent(
        &report_against(generated, source),
        FACTUAL_UNSOURCED_INSTITUTION,
    );
    // The control: a source with neither a degree nor an institution still
    // reports the invented one.
    assert!(
        codes(&report_against(generated, EN_SOURCE_FOUR_YEARS))
            .contains(&FACTUAL_UNSOURCED_INSTITUTION),
        "the degree token is the discriminator, not silence"
    );
}

/// A `Certified Scrum Master` credential is not a Master's DEGREE — reading it
/// as one lets a source with no education section at all satisfy
/// `source_has_education`, which silences the whole institution-invention
/// check for a document that invented one whole.
///
/// Mutation check: drop the "scrum master" exclusion from `names_a_degree`
/// and this goes red — the invented `TU Berlin` stops being reported.
#[test]
fn a_scrum_master_credential_is_not_a_masters_degree() {
    let source = "Jane Doe\n\nCERTIFICATIONS\n\nCertified Scrum Master\n\n\
         EXPERIENCE\n\nBackend Engineer | Acme | 2019 - Present\n\
         - Built the ledger service\n";
    let generated = "Jane Doe\n\nEDUCATION\n\nBSc Computer Science, TU Berlin, 2014 - 2018\n";
    assert!(
        codes(&report_against(generated, source)).contains(&FACTUAL_UNSOURCED_INSTITUTION),
        "the source names no education at all; a Scrum Master credential must not spare an \
         invented institution"
    );
}

/// The same invented institution named on two Education lines is one finding,
/// the same rule [`credentials::inflated_years_claims`] and
/// [`credentials::unsupported_certs`] already apply to their own duplicates.
///
/// Mutation check: drop the dedup filter from `unsupported_institutions` and
/// this goes red — the same `TU Berlin` is reported twice.
#[test]
fn an_institution_named_twice_is_reported_once() {
    let source = "Jane Doe\n\nEXPERIENCE\n\nBackend Engineer | Acme | 2019 - Present\n\
         - Built the ledger service\n";
    let generated = "Jane Doe\n\nEDUCATION\n\n\
         BSc Computer Science, TU Berlin, 2014 - 2018\n\
         MSc Software Engineering, TU Berlin, 2018 - 2020\n";
    let generated_sections = split_sections(generated, DocKind::Resume);
    let source_sections = split_sections(source, DocKind::Resume);
    let found =
        credentials::unsupported_institutions(&generated_sections, source, &source_sections);
    assert_eq!(
        found.len(),
        1,
        "the same institution named twice is one invention, not two: {found:?}"
    );
}

/// The institution arm is skipped for a LETTER — a letter has no education
/// section to read, so naming one there must never fire
/// `factual.unsourced_institution`, however unsourced the school.
///
/// Without this, `credentials::validate`'s `ctx.input.doc_kind == DocKind::Resume`
/// gate could be deleted and every OTHER letter test in this file would still
/// pass, which is a gate no test actually pins.
#[test]
fn a_letter_naming_an_institution_is_never_checked_for_one() {
    let source = "Jane Doe\n\nEXPERIENCE\n\nBackend Engineer | Acme | 2019 - Present\n\
         - Built the ledger service\n";
    let generated = "Dear Hiring Manager,\n\n\
         I have since built payment platforms.\n\n\
         EDUCATION\n\nBSc Computer Science, TU Berlin, 2014 - 2018\n\n\
         Sincerely,\nJane Doe\n";
    silent(
        &letter_report_for(generated, source, EN_JOB_AD),
        FACTUAL_UNSOURCED_INSTITUTION,
    );
}

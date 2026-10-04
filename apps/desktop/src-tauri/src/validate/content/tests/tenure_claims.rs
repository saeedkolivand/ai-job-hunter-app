//! `factual.inflated_experience`: which sentences are a tenure claim and what the source
//! supports.

use super::credential_corpus::*;
use super::{support::*, *};

/// A2a positive: the audit's own example. Four years of dated employment, a
/// summary claiming eight, and nothing in the source that says eight.
#[test]
fn a_tenure_the_source_dates_cannot_reach_is_reported_and_quotes_the_span() {
    let report = report_against(
        &summary_claiming("Backend engineer with 8+ years of experience in payments."),
        EN_SOURCE_FOUR_YEARS,
    );
    assert_eq!(
        first_evidence(&report, FACTUAL_INFLATED_EXPERIENCE),
        Some("8+ years"),
        "the finding must fire and quote the span the document wrote; report carried {:?}",
        codes(&report)
    );
    // A WARNING, not a Critical — see the code's registration in `mod.rs` for
    // the measurement that permitted a Critical and the argument that declined
    // it.
    assert_eq!(
        fired(&report, FACTUAL_INFLATED_EXPERIENCE)[0].severity,
        Severity::Warning
    );
    // The SEVERITY, not `report.ok`: this fixture is a bare summary, so it also
    // drops both of the source's roles. Asserting `ok` here would be asserting
    // something about `factual.dropped_role`.
}

/// A2a boundary, anchored to an ABSOLUTE expected number rather than to the
/// allowance the check itself computes.
///
/// The source covers 2016–2020: four years by date subtraction, five once
/// `credentials::CAREER_SPAN_SLACK_YEARS` pays for the months a year column
/// does not carry. So 5 is the last claim that passes and 6 is the first that
/// fails — spelled out here, not computed, because a test that recomputes the
/// boundary from the same constant passes however wrong the constant is.
#[test]
fn the_tenure_allowance_is_the_dated_span_plus_exactly_one_year() {
    for (years, should_fire) in [(4, false), (5, false), (6, true), (9, true)] {
        let report = report_against(
            &summary_claiming(&format!(
                "Backend engineer with {years} years of experience in payments."
            )),
            EN_SOURCE_FOUR_YEARS,
        );
        assert_eq!(
            codes(&report).contains(&FACTUAL_INFLATED_EXPERIENCE),
            should_fire,
            "a claim of {years} years against a 2016-2020 source (4 dated + 1 slack = 5 \
             allowed) should {}fire; report carried {:?}",
            if should_fire { "" } else { "not " },
            codes(&report)
        );
    }
}

/// A2a negative, and the reason the rule is "exceeds" rather than "matches":
/// a candidate who rounds DOWN has not fabricated anything.
#[test]
fn understating_a_tenure_is_never_a_fabrication() {
    let report = report_against(
        &summary_claiming("Backend engineer with 3 years of experience in payments."),
        EN_SOURCE_FOUR_YEARS,
    );
    silent(&report, FACTUAL_INFLATED_EXPERIENCE);
}

/// A2a negative: the source's own stated tenure is evidence even when its dates
/// are shorter. A résumé that says "eight years" and only dates the last two
/// roles is an ordinary résumé, not a fabrication.
#[test]
fn a_tenure_the_source_itself_states_is_supported_whatever_its_dates_cover() {
    let source = format!("Jane Doe\n\nSUMMARY\n\nEight years in payments.\n{EN_SOURCE_FOUR_YEARS}");
    let report = report_against(
        &summary_claiming("Backend engineer with 8 years of experience in payments."),
        &source,
    );
    silent(&report, FACTUAL_INFLATED_EXPERIENCE);
}

/// A2a cross-language — the case this check exists to survive. English source,
/// German output, the SAME number: a value comparison on words would see
/// nothing in common, and a comparison on the number sees 8 = 8.
#[test]
fn a_german_tenure_generated_from_an_english_source_compares_on_the_number() {
    let truthful = "Jana Mustermann\n\nPROFIL\n\nAcht Jahre Erfahrung im Zahlungsverkehr.\n";
    let inflated = "Jana Mustermann\n\nPROFIL\n\nVierzehn Jahre Erfahrung im Zahlungsverkehr.\n";
    let source = format!("Jane Doe\n\nSUMMARY\n\nEight years in payments.\n{EN_SOURCE_FOUR_YEARS}");

    silent(
        &report_against(truthful, &source),
        FACTUAL_INFLATED_EXPERIENCE,
    );
    assert!(
        codes(&report_against(inflated, &source)).contains(&FACTUAL_INFLATED_EXPERIENCE),
        "a German claim of fourteen years against an English source stating eight must \
         still be caught — the number is what crosses the language boundary"
    );
}

/// A2a mutation check: delete the FEATURE, not the constant.
///
/// Without the experience-context requirement, every `<number> years` in the
/// document is a tenure claim — and an ordinary bullet about a 20-year-old
/// system becomes a Critical. This assertion is what goes red if the context
/// gate is removed, so it is the gate's proof of work rather than a second
/// reading of it.
#[test]
fn a_number_of_years_with_no_experience_context_is_not_a_tenure_claim() {
    let generated = "Jane Doe\n\nEXPERIENCE\n\n\
         Backend Developer | Globex Logistics | 2018 - 2020\n\
         - Replaced a 20 year old COBOL settlement batch with a Rust service\n\
         - Cut the 12 years of accumulated schema drift down to one migration\n";
    let report = report_against(generated, EN_SOURCE_FOUR_YEARS);
    silent(&report, FACTUAL_INFLATED_EXPERIENCE);
}

/// A summary sentence, as this repo's own truthful fixtures actually write one.
///
/// None of the first six carries an experience word, and an inflated résumé
/// writes the same sentence with a bigger number — so a gate that admits only
/// "N years of experience" cannot see the class at all. The last three are the
/// register that gate exists to keep out, and they must stay out.
///
/// Mutation check: delete the summary-shape half of `is_tenure_context` and the
/// first six go red; delete the clause/follower test inside it and the last
/// three go red.
#[test]
fn the_summary_shapes_a_real_resume_writes_are_read_as_tenure_claims() {
    let claims = |sentence: &str| {
        let doc = format!("Jane Doe\n\nSUMMARY\n\n{sentence}\n");
        credentials::years_claims(&split_sections(&doc, DocKind::Resume))
            .iter()
            .map(|c| c.years)
            .collect::<Vec<_>>()
    };

    // Verbatim from `en_generated_clean.txt`, `en_generated_paraphrased.txt`,
    // `de_generated_paraphrased.txt` and `tests/corpus/synthetic_swe.txt`.
    for (sentence, expected) in [
        (
            "Eight years of backend work, most of it on payment systems and the container \
             platforms behind them.",
            8,
        ),
        (
            "Backend engineer, eight years across payment systems and container platforms.",
            8,
        ),
        (
            "Backend-Entwicklerin, acht Jahre im Zahlungsverkehr und im Aufbau von \
             Container-Plattformen.",
            8,
        ),
        (
            "Backend engineer with 8 years building distributed services for high-traffic \
             platforms.",
            8,
        ),
        ("Twelve years leading platform teams.", 12),
        ("Ingénieure backend avec 15 années dans les paiements.", 15),
    ] {
        assert_eq!(
            claims(sentence),
            vec![expected],
            "this is a tenure claim: {sentence}"
        );
    }

    // …and the register that must NOT be read as one, in the same section.
    for sentence in [
        "Backend engineer who replaced a 30 year old mainframe.",
        "Backend engineer who retired 12 years of accumulated schema drift.",
        "Backend engineer who cut a 40 year legacy batch to minutes.",
    ] {
        assert!(
            claims(sentence).is_empty(),
            "this is an achievement, not a tenure: {sentence}"
        );
    }
}

/// A summary sentence about a SYSTEM is not a claim about a person.
///
/// Fifteen of fifteen fired when the shape rule asked only that the span open a
/// clause: every one of these is true, and the finding told the user to correct
/// it. The discriminator is the SUBJECT — `platform | team | codebase | stack |
/// ledger | service | mainframe` against `engineer | developer | designer |
/// Ingenieurin` — and it separates completely, which no follower vocabulary
/// does (`of | in | on | at` follow any counted noun phrase).
///
/// Mutation check: drop the `names_a_role` term from `is_tenure_context` and
/// every line here goes red.
#[test]
fn a_summary_sentence_about_a_system_is_not_a_tenure_claim() {
    for sentence in [
        "Rebuilt a platform with 15 years of accumulated technical debt.",
        "Joined a team with 12 years of shipping history behind it.",
        "Inherited a codebase with 20 years in production.",
        "Owns a service with 30 years at the same bank.",
        "Modernised a stack with 18 years of accumulated patches.",
        "Replaced a ledger with 25 years of transaction history.",
        "Retired a mainframe with 40 years in production.",
        "Migrated a warehouse with 22 years of order history.",
        "Supported a gateway with 16 years of uptime.",
        "Eine Plattform mit 15 Jahren im Betrieb übernommen.",
        "Repris une plateforme avec 15 années de dette technique.",
        "Heredó una plataforma con 15 años de deuda técnica.",
        "Ereditato una piattaforma con 15 anni di debito tecnico.",
        "Een platform met 15 jaar aan technische schuld overgenomen.",
        "Herdou uma plataforma com 15 anos de dívida técnica.",
    ] {
        let doc = format!("Jane Doe\n\nSUMMARY\n\n{sentence}\n");
        let claims = credentials::years_claims(&split_sections(&doc, DocKind::Resume));
        assert!(
            claims.is_empty(),
            "this is a sentence about a system, not a tenure: {sentence} (read {:?})",
            claims.iter().map(|c| c.years).collect::<Vec<_>>()
        );
    }
}

/// A DERIVATIONAL noun built on a role noun is not the role itself:
/// `architecture` contains `architect`, `engineering` contains `engineer`,
/// `expertise` contains `expert` — none of them name a PERSON, and a bare
/// `contains` used to let all three satisfy `names_a_role` on the SUBJECT
/// test, which is exactly the register that test was built to reject (see the
/// doc above).
///
/// Mutation check: revert `role_noun_matches` to `t.contains(role)` and every
/// line here goes red.
#[test]
fn a_derivational_noun_does_not_name_the_role_it_derives_from() {
    for sentence in [
        "Rebuilt a platform architecture with 15 years of accumulated technical debt.",
        "Modernised the release engineering practice with 12 years of accumulated scripts.",
        "Deep expertise with 15 years of undocumented legacy quirks.",
    ] {
        let doc = format!("Jane Doe\n\nSUMMARY\n\n{sentence}\n");
        let claims = credentials::years_claims(&split_sections(&doc, DocKind::Resume));
        assert!(
            claims.is_empty(),
            "a derivational noun is not a role, so this is not a tenure claim: {sentence} \
             (read {:?})",
            claims.iter().map(|c| c.years).collect::<Vec<_>>()
        );
    }
}

/// The subject may be a two-word job title, and the head noun comes last in
/// some languages and first in others.
#[test]
fn a_two_word_job_title_still_carries_its_tenure() {
    for sentence in [
        "Backend engineer with 8 years building distributed services.",
        "Senior Software Engineer with 12 years across payment systems.",
        "Ingénieure backend avec 15 années dans les paiements.",
        "Product Designer, twelve years of design systems.",
        "Backend-Entwicklerin, acht Jahre im Zahlungsverkehr.",
    ] {
        let doc = format!("Jane Doe\n\nSUMMARY\n\n{sentence}\n");
        assert!(
            !credentials::years_claims(&split_sections(&doc, DocKind::Resume)).is_empty(),
            "a person states their tenure here: {sentence}"
        );
    }
}

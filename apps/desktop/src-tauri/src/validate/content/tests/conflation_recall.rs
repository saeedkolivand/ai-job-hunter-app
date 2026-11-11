//! The other half of `conflation_faithful.rs`: the gates added to stop false
//! accusations must not open false negatives. Each case here is a fabrication
//! that an over-eager gate once let through.

use super::{support::*, *};

fn count(report: &ContentReport, code: &str) -> usize {
    report.issues.iter().filter(|i| i.code == code).count()
}

fn en_with(from: &str, to: &str) -> String {
    assert!(EN_CLEAN.contains(from), "fixture drifted: {from}");
    EN_CLEAN.replace(from, to)
}

const ACME_LAST_BULLET: &str =
    "- Rewrote the retry scheduler in Rust, and failed settlements fell by 35%";

fn de_with(from: &str, to: &str) -> String {
    assert!(DE_CLEAN.contains(from), "fixture drifted: {from}");
    DE_CLEAN.replace(from, to)
}

#[test]
fn a_location_copied_from_another_role_is_critical() {
    // Acme carries "Berlin"; Globex carries no location; the output gives
    // Globex one. A place found only under another role is no evidence.
    let source = EN_SOURCE.replace(
        "Senior Backend Engineer | Acme Payments | 2021 - Present",
        "Senior Backend Engineer | Acme Payments | Berlin | 2021 - Present",
    );
    let generated = en_with(
        "Backend Developer | Globex Logistics | 2018 - 2021",
        "Backend Developer | Globex Logistics | Berlin | 2018 - 2021",
    );
    let report = report_against(&generated, &source);
    assert_eq!(
        count(&report, FACTUAL_INVENTED_LOCATION),
        1,
        "{:#?}",
        report.issues
    );
    assert!(!report.ok);
}

#[test]
fn a_fabricated_term_survives_an_unreadable_source_language() {
    // A verb-less noun pile is the shape whatlang misreads; an unreadable
    // language counts as comparable, so the fabrication stays Critical.
    let source = "Jane Doe\n\nEXPERIENCE\n\n\
        Backend Engineer | Acme Payments | 2021 - Present\n\
        - Ledger service, Rust, Redis cache, checkout latency\n\
        - Kubernetes cluster, Docker workloads, PostgreSQL\n\n\
        SKILLS\n\nRust · Redis · Docker · Kubernetes · PostgreSQL\n";
    let generated = "Jane Doe\n\nEXPERIENCE\n\n\
        Backend Engineer | Acme Payments | 2021 - Present\n\
        - Ledger service, Rust, Redis cache, checkout latency\n\
        - Kubernetes cluster, Docker workloads, PostgreSQL\n\
        - GraphQL gateway in front of the ledger service\n\n\
        SKILLS\n\nRust · Redis · Docker · Kubernetes · PostgreSQL\n";
    let ad = "We run Rust, Kubernetes and GraphQL.";
    let report = report_for(generated, source, ad, &[]);
    assert_eq!(
        count(&report, FACTUAL_UNSUPPORTED_ROLE_TERM),
        1,
        "{:#?}",
        report.issues
    );
}

#[test]
fn conflation_stays_critical_across_languages() {
    // English source, German output: Globex's tools under Acme need no
    // translation to be recognised as the wrong entry's.
    let generated = de_with(
        "- Den Wiederholungsplaner in Rust neu geschrieben; fehlgeschlagene Abrechnungen sanken um 35%",
        "- Den Wiederholungsplaner in Rust neu geschrieben; fehlgeschlagene Abrechnungen sanken um 35%\n\
         - Die Flotte mit PostgreSQL und Terraform auf AWS bereitgestellt",
    );
    let report = report_in("de", &generated, EN_SOURCE, DE_JOB_AD);
    assert!(
        count(&report, FACTUAL_CONFLATED_ROLE_TERM) >= 1,
        "{:#?}",
        report.issues
    );
}

#[test]
fn a_declared_tool_at_the_start_of_a_sentence_is_still_conflation() {
    let generated = en_with(
        ACME_LAST_BULLET,
        &format!("{ACME_LAST_BULLET}\n- Terraform modules provisioned the checkout cluster"),
    );
    let report = report_against(&generated, EN_SOURCE);
    assert_eq!(
        count(&report, FACTUAL_CONFLATED_ROLE_TERM),
        1,
        "{:#?}",
        report.issues
    );
}

#[test]
fn a_declared_tool_in_a_german_target_is_still_conflation() {
    let generated = de_with(
        "- Den Wiederholungsplaner in Rust neu geschrieben; fehlgeschlagene Abrechnungen sanken um 35%",
        "- Den Wiederholungsplaner in Rust neu geschrieben; fehlgeschlagene Abrechnungen sanken um 35%\n\
         - Die Terraform-Module für den Cluster geschrieben",
    );
    let report = report_in("de", &generated, DE_SOURCE, DE_JOB_AD);
    assert_eq!(
        count(&report, FACTUAL_CONFLATED_ROLE_TERM),
        1,
        "{:#?}",
        report.issues
    );
}

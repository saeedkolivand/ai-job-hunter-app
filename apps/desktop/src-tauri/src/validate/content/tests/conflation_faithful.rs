//! Faithful inputs for the entry-grounding checks: documents that say exactly
//! what their source says (translated, restated, differently spelled) and must
//! never read as a fabrication. Each one was a measured false Critical.

use super::{support::*, *};

const ANY_ROLE_CRITICAL: &[&str] = &[
    FACTUAL_UNSUPPORTED_ROLE_TERM,
    FACTUAL_CONFLATED_ROLE_TERM,
    FACTUAL_INVENTED_LOCATION,
];

fn no_role_critical(report: &ContentReport) {
    for code in ANY_ROLE_CRITICAL {
        silent(report, code);
    }
    assert!(report.ok, "{:#?}", report.issues);
}

fn count(report: &ContentReport, code: &str) -> usize {
    report.issues.iter().filter(|i| i.code == code).count()
}

const CAPS_SOURCE: &str = "Jane Doe\n\nEXPERIENCE\n\n\
    Engineer | Acme Payments | 2020 - 2024\n\
    - Shipped the ledger service in Rust\n\
    - Cut checkout latency with a cache\n\n\
    Engineer | Digital Logistics | 2016 - 2020\n\
    - Built the billing service in Rust\n\n\
    SKILLS\n\nRust · Mentoring · Testing · Kubernetes\n";

fn en_with(from: &str, to: &str) -> String {
    assert!(EN_CLEAN.contains(from), "fixture drifted: {from}");
    EN_CLEAN.replace(from, to)
}

#[test]
fn a_translated_bullet_is_not_a_fabrication() {
    // German source, English output: "API" is the translation of
    // "Abrechnungsschnittstelle", and there is no source spelling to compare to.
    let generated = en_with(
        "- 40 warehouse sites bill through an API I wrote in Python, backed by PostgreSQL",
        "- Built the billing API in Python and PostgreSQL for 40 warehouse sites",
    );
    // The German source also lists a third employer; keep it so only the
    // translated bullet is under test.
    let generated = generated.replace(
        "\nPROJECTS",
        "\nIT-Beraterin | IBM Deutschland GmbH | 2015 - 2018\n\
         - Supported the billing services of two plants\n\nPROJECTS",
    );
    let ad = format!("{EN_JOB_AD}\nYou will own our billing API.\n");
    let report = report_in("en", &generated, DE_SOURCE, &ad);
    no_role_critical(&report);
    silent(&report, FACTUAL_UNSUPPORTED_BULLET);
}

#[test]
fn a_declared_soft_skill_does_not_make_its_verb_a_tool_claim() {
    let generated = "Jane Doe\n\nEXPERIENCE\n\n\
        Engineer | Acme Payments | 2020 - 2024\n\
        - Mentored three junior engineers on the ledger service\n\
        - Tested the cache in front of the ledger service\n\
        - Shipped the ledger service in Rust\n\n\
        Engineer | Digital Logistics | 2016 - 2020\n\
        - Built the billing service in Rust\n\n\
        SKILLS\n\nRust · Mentoring · Testing · Kubernetes\n";
    let report = report_against(generated, CAPS_SOURCE);
    no_role_critical(&report);
}

#[test]
fn a_declared_tool_written_as_a_name_is_still_conflation() {
    let generated = "Jane Doe\n\nEXPERIENCE\n\n\
        Engineer | Acme Payments | 2020 - 2024\n\
        - Shipped the ledger service in Rust on Kubernetes\n\n\
        Engineer | Digital Logistics | 2016 - 2020\n\
        - Built the billing service in Rust\n\n\
        SKILLS\n\nRust · Mentoring · Testing · Kubernetes\n";
    let report = report_against(generated, CAPS_SOURCE);
    assert_eq!(count(&report, FACTUAL_CONFLATED_ROLE_TERM), 1);
}

fn located(source_place: &str, generated_place: &str) -> ContentReport {
    let header = "Backend Developer | Globex Logistics | 2018 - 2021";
    let source = EN_SOURCE.replace(
        header,
        &format!("Backend Developer | Globex Logistics | {source_place} | 2018 - 2021"),
    );
    let generated = en_with(
        header,
        &format!("Backend Developer | Globex Logistics | {generated_place} | 2018 - 2021"),
    );
    report_against(&generated, &source)
}

#[test]
fn a_country_added_to_a_known_city_is_not_invented() {
    no_role_critical(&located("Berlin", "Berlin, Germany"));
    silent(
        &located("Berlin", "Berlin, Germany"),
        FACTUAL_UNSUPPORTED_LOCATION,
    );
}

#[test]
fn remote_and_eu_spellings_are_the_same_place() {
    let report = located("Remote, EU", "Remote (Europe)");
    no_role_critical(&report);
    silent(&report, FACTUAL_UNSUPPORTED_LOCATION);
}

#[test]
fn a_place_given_elsewhere_in_the_source_only_warns() {
    let source = EN_SOURCE.replace(
        "jane.doe@example.com | +49 30 1234567",
        "Berlin | jane.doe@example.com | +49 30 1234567",
    );
    let generated = en_with(
        "Backend Developer | Globex Logistics | 2018 - 2021",
        "Backend Developer | Globex Logistics | Berlin | 2018 - 2021",
    );
    let report = report_against(&generated, &source);
    assert_eq!(
        count(&report, FACTUAL_UNSUPPORTED_LOCATION),
        1,
        "{:#?}",
        report.issues
    );
    silent(&report, FACTUAL_INVENTED_LOCATION);
    let warning = report
        .issues
        .iter()
        .find(|i| i.code == FACTUAL_UNSUPPORTED_LOCATION)
        .unwrap();
    assert!(!warning.message.contains("  "), "{}", warning.message);
    assert!(report.ok);
}

#[test]
fn a_letter_sentence_with_a_common_word_employer_is_not_tied() {
    for sentence in [
        "I enjoy building digital payment systems on Kubernetes every day.",
        "Most of my work is software solutions on Kubernetes.",
    ] {
        let letter = format!("Dear Hiring Manager,\n\n{sentence}\n\nBest regards,\nJane Doe\n");
        let source = CAPS_SOURCE.replace("Acme Payments", "Software Solutions");
        let report = letter_report_for(&letter, &source, EN_JOB_AD);
        for code in ANY_ROLE_CRITICAL {
            silent(&report, code);
        }
    }
}

#[test]
fn a_letter_sentence_naming_the_full_employer_is_still_checked() {
    let letter = "Dear Hiring Manager,\n\nAt Digital Logistics I ran Kubernetes for the billing team.\n\nBest regards,\nJane Doe\n";
    let report = letter_report_for(letter, CAPS_SOURCE, EN_JOB_AD);
    assert_eq!(
        count(&report, FACTUAL_CONFLATED_ROLE_TERM),
        1,
        "{:#?}",
        report.issues
    );
}

#[test]
fn practice_words_in_a_skills_list_are_not_tool_claims_in_other_forms() {
    // "Testing · Security · Monitoring · Design · Agile" are practices, not
    // tools: rewording a bullet around them is faithful.
    let source = "Jane Doe\n\nEXPERIENCE\n\n\
        Backend Engineer | Acme Payments | 2020 - 2024\n\
        - Verified the checkout cache under load before each release\n\
        - Hardened the ledger service against fraud\n\
        - Set up dashboards and alerts for the ledger service in Grafana\n\n\
        Engineer | Globex Logistics | 2016 - 2020\n\
        - Built the billing service in Rust\n\n\
        SKILLS\n\nTesting · Security · Monitoring · Design · Agile · Rust · Grafana\n";
    let generated = "Jane Doe\n\nEXPERIENCE\n\n\
        Backend Engineer | Acme Payments | 2020 - 2024\n\
        - Testing the checkout cache under load before each release\n\
        - Improved the security of the ledger service against fraud\n\
        - Set up monitoring dashboards and alerts for the ledger service\n\
        - Worked in an agile team on the ledger service design\n\n\
        Engineer | Globex Logistics | 2016 - 2020\n\
        - Built the billing service in Rust\n\n\
        SKILLS\n\nTesting · Security · Monitoring · Design · Agile · Rust · Grafana\n";
    no_role_critical(&report_against(generated, source));
}

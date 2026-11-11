//! Entry-level grounding: a claim is checked against the entry it is attached
//! to. Defect documents are the clean fixture with exactly one edit; the
//! paraphrased and spelling-variant documents must stay silent.

use super::{support::*, *};

const NEW_CODES: &[&str] = &[
    FACTUAL_UNSUPPORTED_TERM,
    FACTUAL_UNSUPPORTED_ROLE_TERM,
    FACTUAL_CONFLATED_TERM,
    FACTUAL_CONFLATED_ROLE_TERM,
    FACTUAL_INVENTED_LOCATION,
    FACTUAL_UNSUPPORTED_BULLET,
    FACTUAL_UNSUPPORTED_OUTCOME,
];

const GLOBEX_BULLET: &str =
    "- Took the fleet to AWS, with the whole deployment pipeline written as Terraform";

fn ad_with_extra() -> String {
    format!("{EN_JOB_AD}\nYou will also use GraphQL and Jest.\n")
}

fn edited(from: &str, to: &str) -> String {
    assert!(EN_CLEAN.contains(from), "fixture drifted: {from}");
    EN_CLEAN.replace(from, to)
}

fn only(report: &ContentReport, code: &str) -> Vec<ContentIssue> {
    report
        .issues
        .iter()
        .filter(|i| i.code == code)
        .cloned()
        .collect()
}

#[test]
fn the_clean_and_paraphrased_fixtures_raise_no_grounding_finding() {
    let reports = [
        en_resume(EN_CLEAN, &en_requirements()),
        en_resume(EN_PARAPHRASED, &en_requirements()),
        report_in("de", DE_CLEAN, DE_SOURCE, DE_JOB_AD),
        report_in("de", DE_PARAPHRASED, DE_SOURCE, DE_JOB_AD),
        en_letter(EN_LETTER_GROUNDED),
    ];
    for report in &reports {
        for code in NEW_CODES {
            silent(report, code);
        }
        assert!(report.ok, "{:#?}", report.issues);
    }
}

#[test]
fn a_job_ad_term_the_source_never_has_is_critical_inside_a_role() {
    let generated = edited(
        GLOBEX_BULLET,
        &format!("{GLOBEX_BULLET}, tested with GraphQL"),
    );
    let report = report_for(&generated, EN_SOURCE, &ad_with_extra(), &[]);
    let found = only(&report, FACTUAL_UNSUPPORTED_ROLE_TERM);
    assert_eq!(found.len(), 1, "{:#?}", report.issues);
    assert_eq!(found[0].severity, Severity::Critical);
    assert!(found[0].evidence.as_deref().unwrap().contains("GraphQL"));
    assert!(!report.ok);
}

#[test]
fn the_same_term_in_the_summary_only_warns() {
    let generated = edited(
        "Eight years of backend work",
        "Eight years of backend work with GraphQL",
    );
    let report = report_for(&generated, EN_SOURCE, &ad_with_extra(), &[]);
    assert_eq!(only(&report, FACTUAL_UNSUPPORTED_TERM).len(), 1);
    silent(&report, FACTUAL_UNSUPPORTED_ROLE_TERM);
    assert!(report.ok);
}

#[test]
fn a_term_outside_the_job_ad_is_not_this_checks_to_judge() {
    let generated = edited(
        GLOBEX_BULLET,
        &format!("{GLOBEX_BULLET}, tested with GraphQL"),
    );
    let report = report_for(&generated, EN_SOURCE, EN_JOB_AD, &[]);
    silent(&report, FACTUAL_UNSUPPORTED_ROLE_TERM);
    silent(&report, FACTUAL_UNSUPPORTED_TERM);
}

#[test]
fn a_skills_list_term_credited_to_one_employer_is_conflation() {
    let generated = edited(
        GLOBEX_BULLET,
        &format!("{GLOBEX_BULLET}, with a Redis layer"),
    );
    let report = report_against(&generated, EN_SOURCE);
    let found = only(&report, FACTUAL_CONFLATED_ROLE_TERM);
    assert_eq!(found.len(), 1, "{:#?}", report.issues);
    assert_eq!(found[0].severity, Severity::Critical);
    // Redis is in the source (Acme's entry and the skills list): a whole-resume
    // check alone would have passed it.
    silent(&report, FACTUAL_UNSUPPORTED_ROLE_TERM);
}

#[test]
fn a_term_moved_into_a_project_entry_only_warns() {
    let generated = edited(
        "Rust · SQLite · Clap\nDouble-entry",
        "Rust · SQLite · Clap\nRedis-backed double-entry",
    );
    let report = report_against(&generated, EN_SOURCE);
    assert_eq!(
        only(&report, FACTUAL_CONFLATED_TERM).len(),
        1,
        "{:#?}",
        report.issues
    );
    silent(&report, FACTUAL_CONFLATED_ROLE_TERM);
}

#[test]
fn the_skills_section_is_judged_against_the_whole_resume() {
    let generated = edited("Terraform · Redis", "Terraform · Redis · GraphQL");
    let report = report_for(&generated, EN_SOURCE, &ad_with_extra(), &[]);
    assert_eq!(only(&report, FACTUAL_UNSUPPORTED_TERM).len(), 1);
    silent(&report, FACTUAL_CONFLATED_TERM);
    silent(&report, FACTUAL_CONFLATED_ROLE_TERM);
}

#[test]
fn spelling_variants_of_a_supported_tool_are_the_same_tool() {
    let source = "Jane Doe\n\nEXPERIENCE\n\nEngineer | Acme Payments | 2020 - 2024\n\
                  - Built the web client in React and the services in Node\n\
                  - Wired CI/CD for the team\n\nSKILLS\n\nReact · Node · CI/CD\n";
    let generated = "Jane Doe\n\nEXPERIENCE\n\nEngineer | Acme Payments | 2020 - 2024\n\
                     - Built the web client in React.js and the services in Node.js\n\
                     - Wired CI-CD for the team\n\nSKILLS\n\nReact.js · Node.js · CI/CD\n";
    let ad = "We use React.js, Node.js and CI/CD every day.";
    let report = report_for(generated, source, ad, &[]);
    for code in NEW_CODES {
        silent(&report, code);
    }
}

#[test]
fn a_location_the_source_never_gave_the_role_is_flagged() {
    let generated = edited(
        "Backend Developer | Globex Logistics | 2018 - 2021",
        "Backend Developer | Globex Logistics | Berlin | 2018 - 2021",
    );
    let report = report_against(&generated, EN_SOURCE);
    let found = only(&report, FACTUAL_INVENTED_LOCATION);
    assert_eq!(found.len(), 1, "{:#?}", report.issues);
    assert_eq!(found[0].evidence.as_deref(), Some("Berlin"));
}

#[test]
fn a_location_the_source_gives_in_another_language_is_not_invented() {
    let source = EN_SOURCE.replace(
        "Backend Developer | Globex Logistics | 2018 - 2021",
        "Backend Developer | Globex Logistics | Deutschland | 2018 - 2021",
    );
    let generated = edited(
        "Backend Developer | Globex Logistics | 2018 - 2021",
        "Backend Developer | Globex Logistics | Germany | 2018 - 2021",
    );
    silent(
        &report_against(&generated, &source),
        FACTUAL_INVENTED_LOCATION,
    );
}

#[test]
fn a_bullet_with_nothing_in_common_with_its_entry_warns() {
    let generated = edited(
        GLOBEX_BULLET,
        &format!(
            "{GLOBEX_BULLET}\n- Designed an onboarding programme for new engineers across teams"
        ),
    );
    let report = report_against(&generated, EN_SOURCE);
    assert_eq!(
        only(&report, FACTUAL_UNSUPPORTED_BULLET).len(),
        1,
        "{:#?}",
        report.issues
    );
    assert!(report.ok);
}

#[test]
fn an_invented_outcome_clause_warns_but_a_restated_one_does_not() {
    let plain = "Rewrote the retry scheduler in Rust, and failed settlements fell by 35%";
    let invented = edited(
        plain,
        "Rewrote the retry scheduler in Rust, reducing bugs in production",
    );
    let report = report_against(&invented, EN_SOURCE);
    assert_eq!(
        only(&report, FACTUAL_UNSUPPORTED_OUTCOME).len(),
        1,
        "{:#?}",
        report.issues
    );
    let restated = edited(
        plain,
        "Rewrote the retry scheduler in Rust, which cut failed settlements by 35%",
    );
    silent(
        &report_against(&restated, EN_SOURCE),
        FACTUAL_UNSUPPORTED_OUTCOME,
    );
}

#[test]
fn a_letter_sentence_tying_a_tool_to_the_wrong_employer_is_critical() {
    let letter = EN_LETTER_GROUNDED.replace(
        "built the billing API in Python and PostgreSQL,",
        "built the billing API in Python and PostgreSQL on Redis,",
    );
    assert_ne!(letter, EN_LETTER_GROUNDED);
    let report = en_letter(&letter);
    assert_eq!(
        only(&report, FACTUAL_CONFLATED_ROLE_TERM).len(),
        1,
        "{:#?}",
        report.issues
    );
    assert!(!report.ok);
}

#[test]
fn a_letter_sentence_naming_no_employer_is_left_alone() {
    let letter =
        EN_LETTER_GROUNDED.replace("Docker and Kubernetes I know", "Docker and Redis I know");
    assert_ne!(letter, EN_LETTER_GROUNDED);
    silent(&en_letter(&letter), FACTUAL_CONFLATED_ROLE_TERM);
}

#[test]
fn the_thresholds_are_pinned() {
    assert_eq!(factual::MIN_BULLET_TOKENS, 4);
    assert_eq!(factual::MIN_BULLET_OVERLAP, 2);
    assert_eq!(factual::MIN_OUTCOME_TOKENS, 2);
}

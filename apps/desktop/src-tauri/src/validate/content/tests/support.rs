//! Fixtures and helpers shared by every topic file under this module.

use super::*;

pub(super) const EN_SOURCE: &str = include_str!("../fixtures/en_source_resume.txt");

pub(super) const EN_JOB_AD: &str = include_str!("../fixtures/en_job_ad.txt");

pub(super) const EN_CLEAN: &str = include_str!("../fixtures/en_generated_clean.txt");

pub(super) const EN_PARAPHRASED: &str = include_str!("../fixtures/en_generated_paraphrased.txt");

pub(super) const DE_PARAPHRASED: &str = include_str!("../fixtures/de_generated_paraphrased.txt");

pub(super) const EN_FABRICATED_METRIC: &str =
    include_str!("../fixtures/en_generated_fabricated_metric.txt");

pub(super) const EN_DROPPED_ROLE: &str = include_str!("../fixtures/en_generated_dropped_role.txt");

pub(super) const EN_ALTERED_LINK: &str =
    include_str!("../fixtures/en_generated_altered_project_link.txt");

pub(super) const EN_DUPLICATES: &str =
    include_str!("../fixtures/en_generated_duplicate_bullets.txt");

pub(super) const EN_WRONG_LANGUAGE: &str =
    include_str!("../fixtures/en_generated_wrong_language.txt");

pub(super) const EN_EXPERIENCE_DRIFTED_ITALIAN: &str =
    include_str!("../fixtures/en_generated_experience_drifted_italian.txt");

pub(super) const EN_PROJECTS_TIER2: &str =
    include_str!("../fixtures/en_generated_projects_tier2.txt");

pub(super) const EN_PROJECTS_TIER3: &str =
    include_str!("../fixtures/en_generated_projects_tier3.txt");

pub(super) const EN_PROJECTS_BROKEN: &str =
    include_str!("../fixtures/en_generated_projects_broken.txt");

pub(super) const EN_LETTER_AI_TELLS: &str = include_str!("../fixtures/en_letter_ai_tells.txt");

pub(super) const EN_LETTER_GROUNDED: &str = include_str!("../fixtures/en_letter_grounded.txt");

pub(super) const DE_SOURCE: &str = include_str!("../fixtures/de_source_resume.txt");

pub(super) const DE_JOB_AD: &str = include_str!("../fixtures/de_job_ad.txt");

pub(super) const DE_CLEAN: &str = include_str!("../fixtures/de_generated_clean.txt");

pub(super) fn en_requirements() -> Vec<String> {
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

/// The inputs most tests share: a résumé, English, no top requirements. A test
/// that needs anything else overrides that field at its own site.
pub(super) fn content_input<'a>(
    generated: &'a str,
    source_resume: &'a str,
    job_ad: &'a str,
) -> ContentInput<'a> {
    ContentInput {
        generated,
        source_resume,
        job_ad,
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    }
}

pub(super) fn en_resume(generated: &str, requirements: &[String]) -> ContentReport {
    report_for(generated, EN_SOURCE, EN_JOB_AD, requirements)
}

pub(super) fn en_letter(generated: &str) -> ContentReport {
    letter_report_for(generated, EN_SOURCE, EN_JOB_AD)
}

pub(super) fn report_for(
    generated: &str,
    source: &str,
    job_ad: &str,
    reqs: &[String],
) -> ContentReport {
    validate_content(&ContentInput {
        top_requirements: reqs,
        ..content_input(generated, source, job_ad)
    })
}

/// [`report_for`] under the shared English posting and no requirements: the
/// question "does `generated` still say what `source` says?".
pub(super) fn report_against(generated: &str, source: &str) -> ContentReport {
    report_for(generated, source, EN_JOB_AD, &[])
}

/// A résumé validated against a target language other than English.
pub(super) fn report_in(lang: &str, generated: &str, source: &str, job_ad: &str) -> ContentReport {
    validate_content(&ContentInput {
        target_language: lang,
        ..content_input(generated, source, job_ad)
    })
}

pub(super) fn letter_report_for(generated: &str, source: &str, job_ad: &str) -> ContentReport {
    letter_in("en", generated, source, job_ad)
}

/// A cover letter validated against a target language.
pub(super) fn letter_in(lang: &str, generated: &str, source: &str, job_ad: &str) -> ContentReport {
    validate_content(&ContentInput {
        target_language: lang,
        doc_kind: DocKind::CoverLetter,
        ..content_input(generated, source, job_ad)
    })
}

pub(super) fn codes(report: &ContentReport) -> Vec<&str> {
    report.issues.iter().map(|i| i.code).collect()
}

/// Assert `code` did NOT fire — the half of a boundary test that proves the
/// threshold is doing work rather than always firing.
#[track_caller]
pub(super) fn silent(report: &ContentReport, code: &str) {
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
pub(super) fn first_evidence<'a>(report: &'a ContentReport, code: &str) -> Option<&'a str> {
    report
        .issues
        .iter()
        .find(|i| i.code == code)
        .and_then(|i| i.evidence.as_deref())
}

/// Assert `code` fired, and return its issues.
#[track_caller]
pub(super) fn fired<'a>(report: &'a ContentReport, code: &str) -> Vec<&'a ContentIssue> {
    let hits: Vec<&ContentIssue> = report.issues.iter().filter(|i| i.code == code).collect();
    assert!(
        !hits.is_empty(),
        "expected {code} to fire; report carried {:?}",
        codes(report)
    );
    hits
}

/// The evidence of every `code` issue, borrowed (empty when it never fired).
pub(super) fn evidence_of<'a>(report: &'a ContentReport, code: &str) -> Vec<&'a str> {
    report
        .issues
        .iter()
        .filter(|i| i.code == code)
        .filter_map(|i| i.evidence.as_deref())
        .collect()
}

/// [`evidence_of`], owned — for the tests that compare against `String`s.
pub(super) fn evidence_strings(report: &ContentReport, code: &str) -> Vec<String> {
    evidence_of(report, code)
        .into_iter()
        .map(String::from)
        .collect()
}

/// Assert `code` fired, and return the evidence of its issues.
#[track_caller]
pub(super) fn fired_evidence<'a>(report: &'a ContentReport, code: &str) -> Vec<&'a str> {
    fired(report, code);
    evidence_of(report, code)
}

/// Every Critical in `report`.
pub(super) fn criticals_of(report: &ContentReport) -> Vec<&ContentIssue> {
    report
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .collect()
}

/// A tell-tale letter in both apostrophe shapes a model writes: the typographic
/// U+2019 as given, and its ASCII U+0027 twin.
pub(super) fn apostrophe_shapes(typographic: &str) -> [(&'static str, String); 2] {
    [
        ("typographic U+2019", typographic.to_string()),
        ("ASCII U+0027", typographic.replace('\u{2019}', "'")),
    ]
}

/// The skills line of the clean fixtures, which several tests rewrite.
pub(super) const SKILLS_LINE: &str =
    "Rust · Python · Docker · Kubernetes · PostgreSQL · AWS · Terraform · Redis";

/// A lowercase tool list `whatlang` confidently misreads as some other language.
pub(super) const LOWERCASE_TOOL_LIST: &str =
    "pandas numpy scikit-learn pytest git bash npm nginx dbt kubectl \
    docker kubernetes terraform ansible jenkins grafana prometheus redis postgresql \
    elasticsearch java spring hibernate maven gradle jira confluence";

pub(super) const FR_RESUME: &str = "\
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

pub(super) const FR_JOB_AD: &str = "\
Nous recherchons une ingenieure backend pour notre plateforme de paiement.
Vous travaillerez avec Rust, Python, Docker et Kubernetes dans une equipe
distribuee. Une experience avec PostgreSQL, Redis et Terraform est demandee
pour ce poste base a Lyon.
";

/// A one-role résumé around `bullet`, for the metric and date checks.
pub(super) fn resume_with_bullet(bullet: &str) -> String {
    format!("EXPERIENCE\n\nAcme Payments | 2021 - Present\n- {bullet}\n")
}

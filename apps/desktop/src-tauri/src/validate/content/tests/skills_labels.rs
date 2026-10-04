//! `consistency.skill_not_demonstrated`: what a skills line claims once its category labels
//! and proficiency grades are set aside.

use super::{support::*, *};

/// The skills-label strip must BITE at its boundary and stop there: a short
/// `Category:` head is dropped, a long one is prose whose content still counts
/// as a claim. Pinning the number without pinning its effect is how a threshold
/// silently starts eating real skills.
#[test]
fn skills_label_strip_boundary_behaves() {
    // A three-word label is stripped, so nothing in front of the colon is a
    // claim …
    let labelled = EN_CLEAN.replace(
        SKILLS_LINE,
        "Everyday programming languages: Rust, Python\n\
         Docker, Kubernetes, PostgreSQL, AWS, Terraform, Redis",
    );
    silent(
        &report_against(&labelled, EN_SOURCE),
        CONSISTENCY_SKILL_NOT_DEMONSTRATED,
    );

    // … while a head too long to be a label keeps counting: this one really
    // does claim a skill nothing demonstrates, and must still be reported.
    let prose = EN_CLEAN.replace(
        SKILLS_LINE,
        "Shipped four platform services on Elasticsearch: Rust, Python, Docker, \
         Kubernetes, PostgreSQL, AWS, Terraform, Redis",
    );
    let report = report_against(&prose, EN_SOURCE);
    let hits = fired(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
    assert!(
        hits.iter()
            .any(|i| i.evidence.as_deref() == Some("elasticsearch")),
        "a claim in front of a long head is still a claim; got {:?}",
        hits.iter().map(|i| &i.evidence).collect::<Vec<_>>()
    );
}

/// R5-F3 — a labelled skills line ("Languages: Rust, Python") is the commonest
/// skills layout there is, and every one of its CATEGORY LABELS was read as a
/// claimed skill: "languages", "frameworks", "databases" and "tooling" all
/// reported as skills the résumé never demonstrates.
///
/// The German half of the same shape — the label is a German noun, so an
/// English-only stopword list cannot be what saves it.
#[test]
fn skills_category_labels_are_not_claimed_skills_in_either_language() {
    let en = EN_CLEAN.replace(
        SKILLS_LINE,
        "Languages: Rust, Python\nFrameworks: Docker, Kubernetes\n\
         Databases: PostgreSQL, Redis\nTooling: AWS, Terraform",
    );
    let de = DE_CLEAN.replace(
        SKILLS_LINE,
        "Programmiersprachen: Rust, Python\n\
         Werkzeuge: Docker, Kubernetes, AWS, Terraform\n\
         Datenbanken: PostgreSQL, Redis",
    );
    for (report, why) in [
        (
            report_for(&en, EN_SOURCE, EN_JOB_AD, &en_requirements()),
            "a category label names no skill",
        ),
        (
            report_in("de", &de, DE_SOURCE, DE_JOB_AD),
            "a German category label names no skill either",
        ),
    ] {
        let claimed = evidence_strings(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
        assert!(claimed.is_empty(), "{why}; got {claimed:?}");
    }
}

/// R6-F4 — the `Category:` strip fired on any head of three words or fewer, so
/// a PROFICIENCY line ("Python: Advanced", "Deutsch: Muttersprache") lost the
/// SKILL and kept the level word: "advanced" was reported as a skill nothing
/// demonstrates while Python itself was never checked at all.
#[test]
fn a_proficiency_line_claims_the_skill_not_the_level() {
    let generated = EN_CLEAN.replace(
        SKILLS_LINE,
        "Python: Advanced\nDocker: Expert\nElasticsearch: Advanced",
    );
    let report = report_against(&generated, EN_SOURCE);
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
        SKILLS_LINE,
        "Python: Fortgeschritten\nDocker: Experte\nDeutsch: Muttersprache\n\
         Elasticsearch: Grundkenntnisse",
    );
    let report = report_in("de", &generated, DE_SOURCE, DE_JOB_AD);
    let claimed = evidence_strings(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
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
    // A list tail is a CATEGORY label: the head is dropped, the items are the
    // claims (R5-F3's fix, unchanged).
    assert_eq!(
        en_skills_claims("Languages: Rust, Python"),
        Vec::<String>::new()
    );
    assert_eq!(
        en_skills_claims("Languages: Rust, Elasticsearch"),
        vec!["elasticsearch".to_string()]
    );
    // (R6's "a multi-word tail is still a list" row moved to R7-F2's
    // `a_space_separated_category_now_reads_as_a_grade` — word count is no
    // longer a list signal, because multi-word GRADES are commoner.)
    //
    // A tail with no separator is a GRADE: the head is the claim.
    assert_eq!(en_skills_claims("Python: Advanced"), Vec::<String>::new());
    // …which is exactly why a single-item CATEGORY reads as a grade too. The
    // accepted cost of the rule, pinned so it cannot change unnoticed: the item
    // is not policed. The LABEL is not reported in its place either — that was
    // R8-F7, and `a_category_label_is_never_the_claim` owns the rows for it.
    assert_eq!(
        en_skills_claims("Frameworks: Elasticsearch"),
        Vec::<String>::new()
    );
}

/// The EN half of the R7-F2 rows, and the concession that comes with the fix.
/// Shares `skills`' shape with the R6-F4 test on purpose: same fixture, same
/// extraction, so the two discriminator rules are compared on one surface.
fn en_skills_claims(lines: &str) -> Vec<String> {
    let generated = EN_CLEAN.replace(SKILLS_LINE, lines);
    sorted_claims(&report_against(&generated, EN_SOURCE))
}

fn de_skills_claims(lines: &str) -> Vec<String> {
    let generated = DE_CLEAN.replace(SKILLS_LINE, lines);
    sorted_claims(&report_in("de", &generated, DE_SOURCE, DE_JOB_AD))
}

fn sorted_claims(report: &ContentReport) -> Vec<String> {
    let mut claimed = evidence_strings(report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
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
    let report = report_in("fr", &generated, FR_RESUME, FR_JOB_AD);
    let claimed = evidence_strings(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
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

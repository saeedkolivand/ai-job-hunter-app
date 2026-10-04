//! `classify_section`: which kind of section a heading names, across the languages the résumé
//! pipeline supports.

use super::*;

/// Every heading in `headings` must classify as `want`; `why` completes the failure message.
fn assert_classifies(headings: &[&str], want: SectionKind, why: &str) {
    for heading in headings {
        assert_eq!(classify_section(heading), want, "{heading:?} {why}");
    }
}

/// `classify_section` matches substrings, and "formation" hides inside
/// "information" (as do `formación`/`formação`/`formazione` inside
/// `información`/`informação`/`informazione`). So the "PERSONAL INFORMATION"
/// heading that heads half the CVs in Europe classified as EDUCATION — and,
/// with the Contact arm above, filed the candidate's phone number and email
/// as a degree.
#[test]
fn personal_information_headings_are_not_education() {
    for heading in [
        "PERSONAL INFORMATION",
        "Personal Information",
        "Información personal",
        "Informação pessoal",
        "Informazioni personali",
    ] {
        assert_ne!(
            classify_section(heading),
            SectionKind::Education,
            "{heading:?} is a contact block, not education"
        );
    }
    // The real headings these stems exist for still classify.
    assert_classifies(
        &[
            "FORMATION",
            "Formation académique",
            "Formations",
            "Formación académica",
            "Formação acadêmica",
            "Formazione",
            "Ausbildung",
            "Weiterbildung",
            "EDUCATION",
        ],
        SectionKind::Education,
        "names an education section",
    );
}

#[test]
fn italian_skills_heading_is_classified() {
    assert_eq!(classify_section("COMPETENZE"), SectionKind::Skills);
    assert_eq!(classify_section("Competenze tecniche"), SectionKind::Skills);
}

/// R6-F6 — "Beruflicher Werdegang" is a standard German experience heading and
/// classified as `Other`, so every bullet under it was discarded and the prompt
/// was told the candidate had no experience.
#[test]
fn german_career_headings_classify_as_experience() {
    assert_classifies(
        &[
            "Beruflicher Werdegang",
            "BERUFLICHER WERDEGANG",
            "Werdegang",
            "Erfahrung",
            "Berufliche Erfahrung",
            "Erfahrungen",
            // …and the spellings that already worked must keep working.
            "BERUFSERFAHRUNG",
            "Arbeitserfahrung",
        ],
        SectionKind::Experience,
        "names an experience section",
    );
}

/// R8-F1 — `EXPERIENCE_HEADINGS` carries the bare substring `career`, and the
/// experience test runs BEFORE the summary one, so "Career Summary" (and
/// "Career Objective", and "Career Profile") classified as EXPERIENCE. The
/// prose under a summary heading then reached the experience arm and became
/// work bullets under an invented, unattributed role.
#[test]
fn a_career_summary_heading_is_a_summary_not_experience() {
    assert_classifies(
        &[
            "Career Summary",
            "CAREER OBJECTIVE",
            "Career Profile",
            "Career Objective Statement",
        ],
        SectionKind::Summary,
        "names a summary, not a work history",
    );
    // …and a career heading that names no summary is still Experience.
    for heading in ["Career History", "CAREER", "Career Highlights"] {
        assert_eq!(
            classify_section(heading),
            SectionKind::Experience,
            "{heading:?} still names a work history"
        );
    }
}

/// R14-F1 — every experience stem outranked SKILLS unconditionally, so the
/// combined heading a real résumé writes over a SKILLS MATRIX ("Skills and
/// Experience", "Technical Skills & Experience", "Kenntnisse und Erfahrungen")
/// classified as Experience and filed every skill line as a work bullet under a
/// role that never existed.
///
/// The resolution is asymmetric on purpose, and the asymmetry is the whole
/// finding: the stems that name a work history *unambiguously* (they carry
/// "work"/"employment"/"Beruf"/"Arbeit") keep Experience whatever else the
/// heading says, while the BARE word for "experience" yields.
#[test]
fn a_combined_skills_and_experience_heading_is_a_skills_section() {
    assert_classifies(
        &[
            "Skills and Experience",
            "SKILLS & EXPERIENCE",
            "Technical Skills & Experience",
            "Kenntnisse und Erfahrungen",
            "Fähigkeiten und Erfahrung",
            "Compétences et expérience",
        ],
        SectionKind::Skills,
        "heads a skills matrix, not a work history",
    );

    // …and the bare experience heading is untouched: no skills word, no change.
    assert_classifies(
        &[
            "Experience",
            "PROFESSIONAL EXPERIENCE",
            "Berufserfahrung",
            "BERUFSERFAHRUNG",
            "Erfahrung",
            "Erfahrungen",
            "Berufliche Erfahrung",
            "Beruflicher Werdegang",
            "Expérience professionnelle",
        ],
        SectionKind::Experience,
        "still names a work history",
    );

    // A heading that says WORK keeps its work history even beside a skills
    // word — the carve-out that stops this fix from deleting a section.
    assert_classifies(
        &[
            "Work Experience and Skills",
            "Professional Experience & Key Skills",
            "Berufserfahrung und Kenntnisse",
            "Employment History and Competencies",
        ],
        SectionKind::Experience,
        "names a work history that a skills word does not override",
    );

    // The round-8 summary precedent is unchanged, and the two ambiguity rules
    // compose rather than fight.
    assert_eq!(classify_section("Career Summary"), SectionKind::Summary);
    assert_eq!(classify_section("Career History"), SectionKind::Experience);
}

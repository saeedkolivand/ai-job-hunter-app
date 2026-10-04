//! `split_sections`: which lines are headings, and which only look like one.

use super::{support::*, *};

fn kinds_of(text: &str) -> Vec<SectionKind> {
    split_sections(text, DocKind::Resume)
        .iter()
        .map(|s| s.kind)
        .collect()
}

fn headings_of(text: &str) -> Vec<Option<String>> {
    split_sections(text, DocKind::Resume)
        .into_iter()
        .map(|s| s.heading)
        .collect()
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
        &report_in("de", generated, source, DE_JOB_AD),
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
        headings_of(prose)
    );
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
    let kinds = kinds_of(resume);
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
    let report = report_in("de", resume, resume, DE_JOB_AD);
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
        headings_of(prose)
    );
}

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

    let kinds = kinds_of(resume);
    assert!(
        !kinds.contains(&SectionKind::Projects),
        "a job title above its employer is not a Projects heading; got {kinds:?}"
    );

    // The user-visible damage: the two entries leave the experience section, so
    // the résumé counts no roles, and every bullet under the phantom heading is
    // graded as a malformed project card.
    let report = report_in("de", resume, resume, DE_JOB_AD);
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
        headings_of(&headed)
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

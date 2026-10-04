//! `ats`: keyword density, missing and empty sections, and bullet counts and lengths.

use super::{support::*, *};

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
    silent(&report_against(&at_limit, &at_limit), ATS_KEYWORD_DENSITY);
    let over = doc(ats::MAX_KEYWORD_OCCURRENCES + 1);
    let report = report_against(&over, &over);
    let hits = fired(&report, ATS_KEYWORD_DENSITY);
    assert!(hits[0]
        .evidence
        .as_deref()
        .is_some_and(|e| e.starts_with("kubernetes ×")));
}

#[test]
fn missing_section_warns_once_per_absent_standard_section() {
    let only_experience = "EXPERIENCE\n\nAcme | 2021 - Present\n- Shipped the ledger service\n";
    let report = report_against(only_experience, only_experience);
    let named = fired_evidence(&report, ATS_MISSING_SECTION);
    assert_eq!(named, vec!["Education", "Skills"]);
}

/// Bug 2 (PR#998 regression): the draft prompt's old "order the sections
/// EXACTLY as … do not drop" phrasing read as a manifest, and the model
/// obeyed by writing a heading with nothing under it.
///
/// A WARNING, deliberately — see `empty_section_issues`'s doc comment. A
/// Critical would route this to `repair`, whose remedy is to regenerate the
/// section, which for an empty `PROJECTS` means inventing projects the
/// candidate does not have. Removal is the correct remedy and
/// `model::adapter::push_nonempty_section` already performs it structurally;
/// this issue reports a defect that is handled, it does not trigger the fix.
#[test]
fn empty_section_heading_is_reported_but_never_routed_to_repair() {
    let generated = "EXPERIENCE\n\nAcme | 2021 - Present\n- Shipped the ledger service\n\n\
                      PROJECTS\n\n\
                      SKILLS\n\nRust, Go\n\n\
                      EDUCATION\n\nBSc, TU Berlin, 2018\n";
    let report = report_against(generated, generated);
    let hits = fired(&report, ATS_EMPTY_SECTION);
    assert_eq!(
        hits[0].severity,
        Severity::Warning,
        "a Critical here would make `repair` regenerate the empty section, i.e.          invent its content — removal is the remedy, and adapter already does it"
    );
    assert_eq!(hits[0].section.as_deref(), Some("PROJECTS"));
}

/// The false-positive risk named alongside the guard: a section that is
/// merely TERSE — one real line, not zero — must not be mistaken for an empty
/// one. Otherwise a legitimate one-entry Publications/Awards section would be
/// flagged right alongside a genuinely empty one.
#[test]
fn a_terse_one_line_section_does_not_trip_the_empty_section_critical() {
    let generated = "EXPERIENCE\n\nAcme | 2021 - Present\n- Shipped the ledger service\n\n\
                      PUBLICATIONS\n\nDoe, J. (2022). A short paper.\n\n\
                      SKILLS\n\nRust, Go\n";
    let report = report_against(generated, generated);
    silent(&report, ATS_EMPTY_SECTION);
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
    silent(&report_against(&at_limit, &at_limit), ATS_LONG_BULLET);
    let over = doc(ats::MAX_BULLET_CHARS + 1);
    fired(&report_against(&over, &over), ATS_LONG_BULLET);
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
    silent(&report_against(&at_max, &at_max), ATS_BULLET_COUNT);

    let too_many = doc(ats::MAX_BULLETS_PER_ROLE + 1);
    fired(&report_against(&too_many, &too_many), ATS_BULLET_COUNT);

    let empty_role = doc(0);
    let report = report_against(&empty_role, &empty_role);
    let hits = fired(&report, ATS_BULLET_COUNT);
    assert!(
        hits[0].message.contains("0 bullets"),
        "a role with no results must be reported too; got {:?}",
        hits[0].message
    );
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
    let report = report_in("de", resume, resume, DE_JOB_AD);
    let stuffing = evidence_of(&report, ATS_KEYWORD_DENSITY);
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
        &report_in("de", &stuffed, &stuffed, DE_JOB_AD),
        ATS_KEYWORD_DENSITY,
    );
}

/// R5-F5 — the keyword-density ceiling counts every token the ENGLISH stopword
/// list does not drop. Only `en` and `de` have a curated function-word list, so
/// ordinary French prose (`pour`, `avec`, `dans`, `responsable`) was counted as
/// repeated keywords and a truthful French résumé was accused of stuffing.
#[test]
fn ordinary_french_prose_is_not_keyword_stuffing() {
    let report = report_in("fr", FR_RESUME, FR_RESUME, FR_JOB_AD);
    let density = evidence_strings(&report, ATS_KEYWORD_DENSITY);
    assert!(
        density.is_empty(),
        "French function words are not stuffed keywords; got {density:?}"
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
    let matched = report_in("de", german, DE_SOURCE, DE_JOB_AD);
    let stuffed = fired_evidence(&matched, ATS_KEYWORD_DENSITY);
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
    let undemonstrated = fired_evidence(&matched, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
    assert_eq!(undemonstrated, vec!["kafka"]);
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
    let report = report_against(resume, resume);
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
    let report = report_against(resume, resume);
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

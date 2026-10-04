//! `consistency`: title drift and skills, including the checks that compare a document
//! against itself and so must not depend on the posting's language.

use super::{support::*, *};

/// Two unrelated employers whose spans merely touch the same year are not the
/// same employer. `titled_entries` keeps the date span inside its company
/// string, so "Globex … | 2018 - 2021" and "Initech … | 2015 - 2018" shared the
/// token "2018" and the second entry's title was reported as drift from the
/// first's.
#[test]
fn title_drift_does_not_match_employers_on_a_shared_year() {
    let doc = "EXPERIENCE\n\n\
               Backend Developer | Globex Logistics | 2018 - 2021\n\
               - Built the billing API in Python\n\n\
               IT Consultant | Initech Systems | 2015 - 2018\n\
               - Ran the reporting service\n";
    silent(&report_against(doc, doc), CONSISTENCY_TITLE_DRIFT);
}

/// M5 — a Snowball stem must never reach the user. "kubernet is listed under
/// skills but never appears in your experience" is a finding nobody can act on.
#[test]
fn user_facing_messages_carry_readable_words_not_stems() {
    let doc = "EXPERIENCE\n\nAcme | 2021 - Present\n- Shipped Docker containers to production\n\n\
               SKILLS\n\nDocker · Kubernetes\n";
    let report = report_against(doc, doc);
    let hits = fired(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("kubernetes"),
        "the evidence must be the readable word, not the stem"
    );
    assert!(
        hits[0].message.contains("\"kubernetes\""),
        "the message must quote the readable word too; got {:?}",
        hits[0].message
    );
}

/// A different job title at the same employer. A promotion that SHARES a word
/// ("Senior Engineer" → "Staff Engineer") is not drift — only a wholly
/// different role is.
#[test]
fn title_drift_warns_on_an_unrelated_title_at_the_same_employer() {
    let source = "EXPERIENCE\n\nSenior Engineer | Acme Payments | 2021 - Present\n\
                  - Shipped the ledger service\n";
    let drifted = "EXPERIENCE\n\nProduct Manager | Acme Payments | 2021 - Present\n\
                   - Shipped the ledger service\n";
    let report = report_against(drifted, source);
    let hits = fired(&report, CONSISTENCY_TITLE_DRIFT);
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("Senior Engineer → Product Manager")
    );

    let promoted = "EXPERIENCE\n\nStaff Engineer | Acme Payments | 2021 - Present\n\
                    - Shipped the ledger service\n";
    silent(&report_against(promoted, source), CONSISTENCY_TITLE_DRIFT);
}

#[test]
fn skill_not_demonstrated_warns_on_a_claim_nothing_backs() {
    let doc = "EXPERIENCE\n\nAcme | 2021 - Present\n- Shipped Docker containers to production\n\n\
               SKILLS\n\nDocker · Kubernetes\n";
    let report = report_against(doc, doc);
    let hits = fired(&report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);
    assert_eq!(hits.len(), 1, "only the unbacked skill; got {hits:#?}");
    assert!(hits[0]
        .evidence
        .as_deref()
        .is_some_and(|e| e.starts_with("kubernet")));
}

/// R4-F3 — `titled_entries` stripped digits from its identity tokens but not
/// legal forms or geography, so two UNRELATED employers that merely share
/// "GmbH" (or a city) matched as the same employer and their titles were
/// compared. Reproduced on a document that is byte-identical to its source:
/// the second entry's title was reported as drift from the FIRST entry's.
#[test]
fn title_drift_does_not_match_employers_on_a_shared_legal_form_or_city() {
    for shared in ["GmbH", ", Berlin"] {
        let doc = format!(
            "EXPERIENCE\n\n\
             Senior Engineer | Northwind Systems {shared} | 2018 - 2021\n\
             - Ran the integration platform\n\n\
             Product Manager | Vitesse Logistics {shared} | 2015 - 2018\n\
             - Owned the roadmap for the carrier portal\n"
        );
        silent(&report_against(&doc, &doc), CONSISTENCY_TITLE_DRIFT);
    }
}

/// R10-F1 — `titled_entries` split the entry label with an ad-hoc
/// `split_once`, which leaves the LOCATION and DATE columns inside `company`.
/// Two unrelated employers that merely both say "Remote" (or both spell their
/// months out) were therefore the same employer, and the second entry's title
/// was reported as drift from the first's — a Critical-adjacent Warning on a
/// document byte-identical to its source.
///
/// `documents::evidence::GEOGRAPHY_TOKENS` cannot save this: "Remote" is not a
/// place, and no gazetteer covers every city. The fix is to stop asking the
/// question of the raw label — `split_entry` already knows which segment is the
/// company.
#[test]
fn title_drift_does_not_match_employers_on_a_shared_location_or_month() {
    // "Remote" is a column every second entry line carries.
    let remote = "EXPERIENCE\n\n\
                  Senior Engineer | Acme Payments | Remote | 2019 - 2022\n\
                  - Shipped Docker containers to production\n\n\
                  Product Manager | Globex Logistics | Remote | 2015 - 2018\n\
                  - Ran the reporting service\n";
    silent(&report_against(remote, remote), CONSISTENCY_TITLE_DRIFT);

    // A spelled-out month survives the digit filter the shared-year case relies
    // on, so it matched two unrelated employers just as readily.
    let months = "EXPERIENCE\n\n\
                  Senior Engineer | Acme Payments | January 2019 - December 2022\n\
                  - Shipped Docker containers to production\n\n\
                  Product Manager | Globex Logistics | January 2015 - December 2018\n\
                  - Ran the reporting service\n";
    silent(&report_against(months, months), CONSISTENCY_TITLE_DRIFT);

    // The guard: the check still does its job when the employer really IS the
    // same one and the generated document renamed the role.
    let source = "EXPERIENCE\n\n\
                  Senior Engineer | Acme Payments | Remote | 2019 - 2022\n\
                  - Shipped Docker containers to production\n";
    let renamed = "EXPERIENCE\n\n\
                   Product Manager | Acme Payments | Remote | 2019 - 2022\n\
                   - Shipped Docker containers to production\n";
    let report = report_against(renamed, source);
    let hits = fired(&report, CONSISTENCY_TITLE_DRIFT);
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("Senior Engineer → Product Manager")
    );
}

/// R11-F2 — `title_drift_issues` paired a generated entry with only the FIRST
/// source entry sharing company tokens, so two roles at the SAME employer — a
/// promotion, an internal move, the commonest thing on a résumé — reported the
/// second one as drift on a document byte-identical to its source.
#[test]
fn title_drift_tolerates_a_second_role_at_the_same_employer() {
    let resume = "EXPERIENCE\n\n\
                  Senior Engineer, Acme Corp (Jan 2021 - Mar 2023)\n\
                  - Shipped Docker containers to production\n\n\
                  Product Manager, Acme Corp (Jan 2018 - Dec 2020)\n\
                  - Ran the reporting service\n";
    silent(&report_against(resume, resume), CONSISTENCY_TITLE_DRIFT);

    // The guard: a title the source never gave this employer STILL fires. The
    // rule is "disjoint from EVERY source title at that employer", not "skip
    // employers with more than one role".
    let invented = "EXPERIENCE\n\n\
                    Chief Revenue Officer, Acme Corp (Jan 2021 - Mar 2023)\n\
                    - Shipped Docker containers to production\n";
    let report = report_against(invented, resume);
    let hits = fired(&report, CONSISTENCY_TITLE_DRIFT);
    assert_eq!(hits.len(), 1, "one entry, one finding; got {hits:?}");
}

/// R12-F1 — `consistency::skill_not_demonstrated` compares a document against
/// ITSELF, but it tokenized both sides with [`Analysis::tokens`], whose stemming
/// decision is `languages_align(job_ad, target_language)`. An English ad for a
/// German-language role (the ordinary DACH case) therefore switches stemming OFF
/// for a comparison the ad is not part of, and every German inflection pair —
/// "Abrechnungsschnittstelle" in the experience, "Abrechnungsschnittstellen"
/// under skills — reads as a skill the résumé never demonstrates.
///
/// The control is the same document against a GERMAN ad: only the AD's language
/// differs between the two runs, so the two answers must be identical.
#[test]
fn a_foreign_language_job_ad_does_not_break_the_skills_self_comparison() {
    let generated = DE_CLEAN.replace(
        SKILLS_LINE,
        "Rust · Python · Docker · Kubernetes · Abrechnungsschnittstellen · \
         Lagerstandorten · Kafka",
    );
    let against = |job_ad: &str| report_in("de", &generated, DE_SOURCE, job_ad);
    let undemonstrated =
        |report: &ContentReport| evidence_strings(report, CONSISTENCY_SKILL_NOT_DEMONSTRATED);

    // The control: with a German ad the pair is stemmed, so only the genuine
    // gap — a skill the experience never shows — is reported.
    assert_eq!(
        undemonstrated(&against(DE_JOB_AD)),
        vec!["kafka".to_string()],
        "the German inflections are demonstrated; only Kafka is not"
    );
    // The finding: the ad's language is not part of this comparison.
    assert_eq!(
        undemonstrated(&against(EN_JOB_AD)),
        vec!["kafka".to_string()],
        "a document-internal check must not change with the AD's language"
    );
}

/// R13-F3 — `consistency::title_drift` stems both sides through
/// [`Analysis::tokens`], whose decision is `languages_align(job_ad, target)`. The
/// ad is not a party to a source-title↔generated-title comparison, so an English
/// ad for a German-language role switches stemming OFF and the ordinary
/// declension pair "Wissenschaftlicher Mitarbeiter" / "Wissenschaftliche
/// Mitarbeiterin" reads as two disjoint titles — a false drift Warning. Unstemmed
/// is the direction that FIRES more, which is what makes this an accusation
/// channel rather than a missed check.
///
/// The control is the same document pair against a GERMAN ad: only the AD's
/// language differs between the two runs, so the two answers must be identical.
#[test]
fn a_foreign_language_job_ad_does_not_break_the_title_comparison() {
    let source = DE_SOURCE.replace(
        "Senior Backend Engineer | Acme Payments",
        "Wissenschaftlicher Mitarbeiter | Acme Payments",
    );
    let generated = DE_CLEAN.replace(
        "Senior Backend Engineer | Acme Payments",
        "Wissenschaftliche Mitarbeiterin | Acme Payments",
    );
    let against = |job_ad: &str| report_in("de", &generated, &source, job_ad);
    let drift = |report: &ContentReport| evidence_strings(report, CONSISTENCY_TITLE_DRIFT);

    // The control: with a German ad the pair is stemmed, the two declensions
    // share "wissenschaftlich", and nothing is reported.
    assert!(
        drift(&against(DE_JOB_AD)).is_empty(),
        "a declension pair is not a title change; got {:?}",
        drift(&against(DE_JOB_AD))
    );
    // The finding: the ad's language is not part of this comparison.
    assert!(
        drift(&against(EN_JOB_AD)).is_empty(),
        "a document↔source comparison must not change with the AD's language; \
         got {:?}",
        drift(&against(EN_JOB_AD))
    );
}

/// R13-F3, the third `Analysis::tokens` document-internal caller. `duplicates`
/// compares two bullets of ONE document, so the posting is not a party to it
/// either — yet `duplicateRatio`, a reported metric, changed with the AD's
/// language. Un-stemming merges LESS here, so the direction is the opposite of
/// `title_drift`'s: a foreign-language ad HID a repeated bullet rather than
/// inventing one. It is still a measurement depending on an input it does not
/// measure, which is what the fix removes.
#[test]
fn a_foreign_language_job_ad_does_not_change_the_duplicate_ratio() {
    // A second bullet that says the first one again in different inflections —
    // the model padding a role, written the way German pads it.
    let generated = DE_CLEAN.replace(
        "- Den Wiederholungsplaner in Rust neu geschrieben",
        "- Betrieb der Docker-Container auf den Kubernetes-Clustern, die pro Sekunde 12000 \
         Anfragen beantworteten\n\
         - Den Wiederholungsplaner in Rust neu geschrieben",
    );
    let against = |job_ad: &str| report_in("de", &generated, DE_SOURCE, job_ad);

    let de = against(DE_JOB_AD);
    let en = against(EN_JOB_AD);
    // Not vacuous: the repetition IS reported, so the equality below is two
    // findings agreeing rather than two silences.
    assert_eq!(fired(&de, DUPLICATE_BULLET).len(), 1);
    assert_eq!(
        de.metrics.duplicate_ratio, en.metrics.duplicate_ratio,
        "a document-internal ratio must not change with the AD's language"
    );
    assert_eq!(
        fired(&en, DUPLICATE_BULLET)[0].evidence,
        fired(&de, DUPLICATE_BULLET)[0].evidence
    );
}

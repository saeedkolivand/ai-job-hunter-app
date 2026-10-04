//! `factual.altered_project_link` and the projects section: dropped, changed and invented
//! links, and the tailoring cuts that are none of them.

use super::{support::*, *};

/// A changed link surfaces as BOTH halves: the candidate's own URL is gone and
/// an unknown one appeared. Both are Critical — a reviewer following the wrong
/// link is the failure this prevents.
#[test]
fn altered_project_link_fires_for_the_drop_and_the_invention() {
    let report = en_resume(EN_ALTERED_LINK, &en_requirements());
    let hits = fired(&report, FACTUAL_ALTERED_PROJECT_LINK);
    assert_eq!(hits.len(), 2, "one drop + one invention; got {hits:#?}");
    assert!(hits.iter().all(|i| i.severity == Severity::Critical));
    let evidence: Vec<&str> = hits.iter().filter_map(|i| i.evidence.as_deref()).collect();
    assert!(
        evidence.contains(&"https://github.com/janedoe/ledger"),
        "the dropped source link must be named; got {evidence:?}"
    );
    assert!(
        evidence.contains(&"https://github.com/jane-doe/ledger-cli"),
        "the invented link must be named; got {evidence:?}"
    );
    // The untouched link must NOT be reported.
    assert!(
        !evidence.contains(&"https://ledger.example.dev"),
        "an unchanged link must never fire; got {evidence:?}"
    );
}

/// Audit finding #4 (HIGH) — `Analysis::section_of_kind` used to be
/// first-match-only, so an invented link that landed in a SECOND Projects
/// section was invisible to `factual::project_link_issues` (Critical-severity)
/// even though the SAME link in the first section fires cleanly.
///
/// Mutation check: change `project_link_issues` back to
/// `ctx.section_of_kind(SectionKind::Projects)` (single section) and this goes
/// red — verified (zero `factual.altered_project_link` hits for the second-
/// section case), then restored to `generated_sections_of_kind` and
/// re-verified green.
#[test]
fn an_invented_link_in_a_second_projects_section_still_fires() {
    let source = "PROJECTS\n\n\
                  **Ledger CLI** · https://github.com/janedoe/ledger\n\
                  Rust · SQLite\n";
    // Two PROJECTS sections — the shape a repair round or an imported résumé
    // can leave. The first is untouched; the invented link sits ONLY in the
    // second.
    let generated = "PROJECTS\n\n\
                     **Ledger CLI** · https://github.com/janedoe/ledger\n\
                     Rust · SQLite\n\n\
                     PROJECTS\n\n\
                     **Side Tracker** · https://github.com/janedoe/side-tracker\n\
                     Python · Flask\n";
    let report = report_against(generated, source);
    let hits = fired(&report, FACTUAL_ALTERED_PROJECT_LINK);
    assert!(
        hits.iter()
            .any(|i| i.evidence.as_deref() == Some("https://github.com/janedoe/side-tracker")),
        "an invented link in the SECOND Projects section must fire, not just \
         the first; got {hits:#?}"
    );
    assert!(hits.iter().all(|i| i.severity == Severity::Critical));
}

/// **PR #1003 finding 3 (MAJOR).** `generated_urls` unions every generated
/// Projects section's links (the test above proves that union is needed) —
/// but the invention loop pushed one issue per OCCURRENCE, unlike the rest of
/// this function, which already keys everything else off the canonical link.
/// The SAME invented URL appearing in two generated Projects sections (a
/// duplicate the model produced, or one already present in an imported
/// résumé) used to read as two identical Criticals for one fabrication.
///
/// Mutation check: drop the `reported_invented.insert(key)` half of the
/// invention loop's condition and this goes red — two identical Criticals for
/// `weekend-tracker`.
#[test]
fn the_same_invented_link_in_two_generated_projects_sections_reports_once() {
    let source = "PROJECTS\n\n\
                  **Ledger CLI** · https://github.com/janedoe/ledger\n\
                  Rust · SQLite\n";
    // The genuine link survives unaltered in the first section; the SAME
    // invented link appears once in EACH of the two generated sections.
    let generated = "PROJECTS\n\n\
                     **Ledger CLI** · https://github.com/janedoe/ledger\n\
                     Rust · SQLite\n\n\
                     **Weekend Tracker** · https://github.com/janedoe/weekend-tracker\n\
                     Swift · CoreData\n\n\
                     SIDE PROJECTS\n\n\
                     **Weekend Tracker** · https://github.com/janedoe/weekend-tracker\n\
                     Swift · CoreData\n";
    let report = report_against(generated, source);
    let hits = fired(&report, FACTUAL_ALTERED_PROJECT_LINK);
    assert_eq!(
        hits.len(),
        1,
        "the same invented link in two generated Projects sections must \
         report once, not once per occurrence; got {hits:#?}"
    );
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("https://github.com/janedoe/weekend-tracker")
    );
}

/// **Confirmation-review finding 3, test C.** `project_link_issues` used to
/// widen its GENERATED side to every Projects section (the test above) but
/// left the SOURCE side reading only the first — so a source résumé with
/// both a "PROJECTS" and a "SIDE PROJECTS" section accused the candidate of
/// inventing their OWN link, unclearably, whenever it lived in the second
/// section: `criticals_by_section` routes the finding to the FIRST Projects
/// section, which never contained it.
///
/// Both source sections classify `SectionKind::Projects` — `SECTION_NAMES`
/// recognises "projects" and "side projects" as separate exact headings —
/// and the generated document here carries BOTH links unaltered.
///
/// Mutation check: change `project_link_issues`'s source side back to
/// `ctx.section_of_kind(SectionKind::Projects)` (first section only) and
/// this goes red — the Weekend Tracker link, sourced only from the second
/// section, reads as invented.
#[test]
fn a_link_sourced_only_from_a_second_side_projects_section_is_not_flagged_as_invented() {
    let source = "PROJECTS\n\n\
                  **Ledger CLI** · https://github.com/janedoe/ledger\n\
                  Rust · SQLite\n\n\
                  SIDE PROJECTS\n\n\
                  **Weekend Tracker** · https://github.com/janedoe/weekend-tracker\n\
                  Swift · CoreData\n";
    let generated = "PROJECTS\n\n\
                     **Ledger CLI** · https://github.com/janedoe/ledger\n\
                     Rust · SQLite\n\n\
                     **Weekend Tracker** · https://github.com/janedoe/weekend-tracker\n\
                     Swift · CoreData\n";
    let report = report_against(generated, source);
    silent(&report, FACTUAL_ALTERED_PROJECT_LINK);
}

/// The two DEGRADED project tiers are legal — the source simply had less data.
/// Neither may warn.
#[test]
fn degraded_project_tiers_are_accepted() {
    for (name, fixture) in [
        ("name+links+stack", EN_PROJECTS_TIER2),
        ("compact", EN_PROJECTS_TIER3),
    ] {
        let report = en_resume(fixture, &en_requirements());
        assert!(
            !codes(&report).contains(&CONSISTENCY_PROJECT_STRUCTURE),
            "the {name} tier is an accepted degradation; got {:?}",
            codes(&report)
        );
    }
}

#[test]
fn project_outside_the_three_tiers_warns() {
    let report = en_resume(EN_PROJECTS_BROKEN, &en_requirements());
    let hits = fired(&report, CONSISTENCY_PROJECT_STRUCTURE);
    assert_eq!(hits[0].severity, Severity::Warning);
    assert_eq!(hits[0].section.as_deref(), Some("Projects"));
}

/// `EN_CLEAN` with its whole `PROJECTS` block cut out — the shape a length trim
/// produces. Built here rather than as a fixture file so it stays exactly one
/// edit from the clean fixture no matter how that fixture evolves.
fn en_clean_without_projects() -> String {
    let (head, rest) = EN_CLEAN
        .split_once("PROJECTS")
        .expect("the clean fixture has a PROJECTS section");
    let (_, tail) = rest
        .split_once("SKILLS")
        .expect("SKILLS follows PROJECTS in the clean fixture");
    format!("{head}SKILLS{tail}")
}

/// R5-F1 — dropping the Projects section outright is a legitimate tailoring
/// decision (the commonest one: a length trim), not an altered link. Comparing
/// an ABSENT section against the source's links raised one
/// `factual.altered_project_link` **Critical per source link** on a document
/// that changed nothing else — three Criticals for one editorial choice.
#[test]
fn a_dropped_projects_section_is_not_an_altered_link() {
    let generated = en_clean_without_projects();
    assert!(
        !generated.contains("ledger.example.dev"),
        "the section really is gone"
    );
    let report = report_for(&generated, EN_SOURCE, EN_JOB_AD, &en_requirements());
    silent(&report, FACTUAL_ALTERED_PROJECT_LINK);

    // …while a document that KEEPS the section and rewrites a link still fires.
    fired(
        &en_resume(EN_ALTERED_LINK, &en_requirements()),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
}

/// R8-F5 — the stack-line exclusion keyed on the literal `"://"`, so a
/// scheme-less link line ("github.com/janedoe/ledger") was cut out of the
/// SOURCE link set. The generated document then carried a link the source
/// "did not have" and the candidate was told they had invented their own
/// repository URL.
#[test]
fn a_scheme_less_link_line_is_not_an_invented_link() {
    let source = "PROJECTS\n\n\
                  **Ledger CLI** · A double-entry bookkeeping tool\n\
                  github.com/janedoe/ledger\n\
                  Rust · SQLite · Clap\n";
    // The model wrote the same link with its scheme spelled out.
    let generated = "PROJECTS\n\n\
                     **Ledger CLI** · A double-entry bookkeeping tool\n\
                     https://github.com/janedoe/ledger\n\
                     Rust · SQLite · Clap\n";
    silent(
        &report_against(generated, source),
        FACTUAL_ALTERED_PROJECT_LINK,
    );

    // The guard the exclusion exists for stays green: a package-registry NAME
    // on a stack line is a technology, not a link, so dropping it while
    // tailoring is not an altered link.
    let stack = "PROJECTS\n\n\
                 **Ledger CLI** · https://github.com/janedoe/ledger\n\
                 Rust · crates.io · SQLite\n";
    let trimmed = "PROJECTS\n\n\
                   **Ledger CLI** · https://github.com/janedoe/ledger\n\
                   Rust · SQLite\n";
    silent(
        &report_against(trimmed, stack),
        FACTUAL_ALTERED_PROJECT_LINK,
    );

    // …and a genuinely different repository still fires.
    let altered = "PROJECTS\n\n\
                   **Ledger CLI** · A double-entry bookkeeping tool\n\
                   github.com/someone-else/ledger\n\
                   Rust · SQLite · Clap\n";
    fired(
        &report_against(altered, source),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
}

/// R10-F3 — the section-level carve-out (R5-F1) forgave a wholly-cut PROJECTS
/// section but not a single trimmed ENTRY, so a résumé trimmed from three
/// projects to two drew a `factual.altered_project_link` Critical per link on
/// the dropped entry. Same false-positive class, same commonest edit there is.
#[test]
fn a_trimmed_project_entry_is_not_an_altered_link() {
    let head = "EXPERIENCE\n\n\
                Acme Payments | 2021 - Present\n\
                - Shipped Docker containers to production\n\n\
                PROJECTS\n\n";
    let ledger = "**Ledger CLI** · https://github.com/janedoe/ledger\nRust · SQLite\n\n";
    let invoices =
        "**Invoice Parser** · https://github.com/janedoe/invoices\nPython · Tesseract\n\n";
    let routes = "**Route Planner** · https://github.com/janedoe/routes\nGo · PostgreSQL\n";

    let source = format!("{head}{ledger}{invoices}{routes}");
    let trimmed = format!("{head}{ledger}{invoices}");
    silent(
        &report_against(&trimmed, &source),
        FACTUAL_ALTERED_PROJECT_LINK,
    );

    // The guard, and the reason this is entry-scoped rather than switched off:
    // an entry the document KEPT must still match its own link exactly. A
    // changed link surfaces as one drop plus one invention.
    let rehosted =
        "**Invoice Parser** · https://github.com/someone-else/invoices\nPython · Tesseract\n\n";
    let altered = format!("{head}{ledger}{rehosted}{routes}");
    let report = report_against(&altered, &source);
    let evidence = fired_evidence(&report, FACTUAL_ALTERED_PROJECT_LINK);
    assert_eq!(
        evidence,
        vec![
            "https://github.com/janedoe/invoices",
            "https://github.com/someone-else/invoices"
        ],
        "a surviving entry's changed link is still a drop plus an invention"
    );

    // …and a link the source never carried at all is still an invention, even
    // when it arrives on an entry that is itself new.
    let invented = format!("{head}{ledger}{invoices}{routes}\
                            **Fleet Dashboard** · https://github.com/someone-else/fleet\nTypeScript · D3\n");
    let report = report_against(&invented, &source);
    let evidence = fired_evidence(&report, FACTUAL_ALTERED_PROJECT_LINK);
    assert_eq!(evidence, vec!["https://github.com/someone-else/fleet"]);
}

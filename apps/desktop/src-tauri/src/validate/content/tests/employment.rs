//! Employment history: date spans (`factual.unsupported_date`, `consistency.date_order`) and
//! `factual.dropped_role` — which employers must survive tailoring, and which entries the
//! check can even see.

use super::{support::*, *};

/// An open-ended source span resolved to a concrete end year is the SAME fact,
/// not a fabricated date — the carve-out that keeps this check usable.
#[test]
fn resolving_an_open_ended_span_is_not_an_unsupported_date() {
    let source = "EXPERIENCE\n\nAcme Payments | 2021 - Present\n- Shipped the ledger\n";
    let generated = "EXPERIENCE\n\nAcme Payments | 2021 - 2026\n- Shipped the ledger\n";
    let report = report_against(generated, source);
    assert!(
        !codes(&report).contains(&FACTUAL_UNSUPPORTED_DATE),
        "a later end year against an open-ended source span is legitimate; got {:?}",
        codes(&report)
    );

    // An INVENTED EARLIER year has no such explanation.
    let backdated = "EXPERIENCE\n\nAcme Payments | 2015 - Present\n- Shipped the ledger\n";
    let report = report_against(backdated, source);
    let hits = fired(&report, FACTUAL_UNSUPPORTED_DATE);
    assert_eq!(hits[0].evidence.as_deref(), Some("2015"));
    assert_eq!(hits[0].severity, Severity::Critical);
}

/// H4 — "still there" is spelled in more than one way. A source that writes
/// `seit 2021`, `since 2021` or a bare `2021 –` carries no `Present` marker, and
/// the old `|| !source_open_ended` arm turned every one of those into a Critical
/// the moment the output resolved the span to a concrete year.
#[test]
fn open_ended_spans_without_a_present_marker_resolve_cleanly() {
    use crate::documents::evidence::is_open_ended;
    for span in ["2021 - Present", "seit 2021", "since 2021", "2021 –"] {
        assert!(is_open_ended(span), "{span:?} is an open-ended span");
    }
    for closed in ["2018 - 2021", "Jan 2018 to Mar 2021", "presented in 2019"] {
        assert!(!is_open_ended(closed), "{closed:?} has an end");
    }

    for source_span in ["seit 2021", "since 2021", "2021 –"] {
        let source = format!(
            "EXPERIENCE\n\nSenior Engineer | Acme Payments | {source_span}\n\
             - Shipped the ledger service\n"
        );
        let resolved = "EXPERIENCE\n\nSenior Engineer | Acme Payments | 2021 - 2026\n\
                        - Shipped the ledger service\n";
        silent(&report_against(resolved, &source), FACTUAL_UNSUPPORTED_DATE);
    }

    // An invented EARLIER year still has no explanation.
    let source = "EXPERIENCE\n\nSenior Engineer | Acme Payments | seit 2021\n\
                  - Shipped the ledger service\n";
    let backdated = "EXPERIENCE\n\nSenior Engineer | Acme Payments | 2015 - 2026\n\
                     - Shipped the ledger service\n";
    let report = report_against(backdated, source);
    let hits = fired(&report, FACTUAL_UNSUPPORTED_DATE);
    assert_eq!(hits[0].evidence.as_deref(), Some("2015"));
}

/// H5 — every present-tense marker hides inside an ordinary word. Matched as
/// substrings, "presented", "knowledge", "currently" and "actually" all turned
/// an ordinary bullet into a date context, and any year in it into a Critical.
#[test]
fn present_markers_only_match_whole_words() {
    use crate::documents::evidence::looks_like_date_span;
    for prose in [
        "presented the roadmap to the board",
        "knowledge sharing across the team",
        "currently owned by the platform group",
        "actually shipped ahead of schedule",
    ] {
        assert!(
            !looks_like_date_span(prose),
            "{prose:?} is prose, not a date span"
        );
    }
    assert!(looks_like_date_span("2021 - Present"));
    assert!(looks_like_date_span("2021 - Heute"));
    // A bare marker with no year anywhere to anchor it is NOT a date span —
    // the same non-answer DATE_ONLY_MARKERS already gives for a bare "Today"
    // (see `documents::evidence::is_open_ended`'s doc comment). Requiring a
    // year is exactly what stops an unrelated present-tense word from turning
    // an ordinary sentence into a date context, so this is an intentional
    // behavior change, not a silently accepted regression.
    assert!(!looks_like_date_span("Heute"));

    // End to end: a truthful bullet with a year the source does not carry, in a
    // line whose only "date marker" is the word "Presented".
    let source = "EXPERIENCE\n\nAcme Payments | 2020 - 2021\n\
                  - Shipped the ledger service\n";
    let generated = "EXPERIENCE\n\nAcme Payments | 2020 - 2021\n\
                     - Presented the 2019 roadmap review to the board\n";
    silent(&report_against(generated, source), FACTUAL_UNSUPPORTED_DATE);
}

/// H5b — a present-tense word standing on its own, many words from an
/// unrelated year, is not a date context either — reproduces the reported
/// defect end to end. `unsupported_date_issues` used to decide a line was a
/// "date context" the moment it carried a bare `PRESENT_MARKERS` word
/// ANYWHERE, so an ordinary truthful bullet naming its own year (not one
/// buried inside another word, unlike H5's "Presented") still tripped a false
/// Critical. Each pair below is the SAME document, one word apart: the
/// marker-word version and a control with just that word removed must both
/// resolve silently, and both marker positions (before the year, after it)
/// are covered because the old bug fired on either.
#[test]
fn present_tense_prose_far_from_a_year_is_not_a_date_context() {
    let source = "EXPERIENCE\n\nAcme Payments | 2020 - 2024\n\
                  - Ran the settlement platform on Kubernetes\n";

    // Marker BEFORE the year — the task's own reproduction.
    let with_marker = "EXPERIENCE\n\nAcme Payments | 2020 - 2024\n\
                       - Reduced actual costs by 20% in 2023\n";
    silent(
        &report_against(with_marker, source),
        FACTUAL_UNSUPPORTED_DATE,
    );
    let control = "EXPERIENCE\n\nAcme Payments | 2020 - 2024\n\
                   - Reduced costs by 20% in 2023\n";
    silent(&report_against(control, source), FACTUAL_UNSUPPORTED_DATE);

    // Marker AFTER the year, several words away.
    let ongoing = "EXPERIENCE\n\nAcme Payments | 2020 - 2024\n\
                   - Cut spend 30% below the 2023 baseline while keeping the \
                     ongoing migration on schedule\n";
    silent(&report_against(ongoing, source), FACTUAL_UNSUPPORTED_DATE);
    let ongoing_control = "EXPERIENCE\n\nAcme Payments | 2020 - 2024\n\
                           - Cut spend 30% below the 2023 baseline while \
                             keeping the migration on schedule\n";
    silent(
        &report_against(ongoing_control, source),
        FACTUAL_UNSUPPORTED_DATE,
    );
}

#[test]
fn date_order_warns_when_a_span_runs_backwards() {
    let source = "EXPERIENCE\n\nAcme | 2018 - 2021\n- Shipped the ledger service\n";
    let backwards = "EXPERIENCE\n\nAcme | 2021 - 2018\n- Shipped the ledger service\n";
    let report = report_against(backwards, source);
    let hits = fired(&report, CONSISTENCY_DATE_ORDER);
    assert_eq!(hits[0].severity, Severity::Warning);
    silent(&report_against(source, source), CONSISTENCY_DATE_ORDER);
}

/// R6-F3 — `date_order_issues` read EVERY line, took any two years on it as a
/// span, and reported "the end date is before the start" whenever the second
/// was smaller. An ordinary bullet comparing this year against an older
/// baseline was therefore accused of carrying a swapped date span.
#[test]
fn two_years_compared_in_prose_are_not_a_swapped_date_span() {
    for prose in [
        "Cut the incident count to 2024 levels from the 2019 baseline",
        "Migrated every service written between 2024 and 2019 onto one platform",
    ] {
        let doc = resume_with_bullet(prose);
        silent(&report_against(&doc, &doc), CONSISTENCY_DATE_ORDER);
    }

    // A genuinely swapped span still fires, in both entry shapes …
    let backwards = "EXPERIENCE\n\nAcme | 2021 - 2018\n- Shipped the ledger service\n";
    let report = report_against(backwards, backwards);
    let hits = fired(&report, CONSISTENCY_DATE_ORDER);
    assert_eq!(hits[0].evidence.as_deref(), Some("2021 - 2018"));
    let two_space = "EXPERIENCE\n\nAcme Payments    Jan 2021 - Mar 2018\n\
                     - Shipped the ledger service\n";
    fired(
        &report_against(two_space, two_space),
        CONSISTENCY_DATE_ORDER,
    );
    // … and an ordinary forward span stays quiet.
    let forward = "EXPERIENCE\n\nAcme | 2018 - 2021\n- Shipped the ledger service\n";
    silent(&report_against(forward, forward), CONSISTENCY_DATE_ORDER);
}

/// **Confirmation-review finding 3, test B.** `generated_experience_lower`
/// used to fall back to searching the WHOLE document once the Experience
/// section's own text was empty — exactly what a heading-only (wiped)
/// Experience section always produces. A Summary sentence that merely NAMES
/// a former employer then read as evidence the employer "survived", even
/// though every role naming it is gone: measured on commit 8c74ccd1's own
/// branch as `factual.dropped_role = 0`, `ok = true` on a résumé whose entire
/// employment history had been deleted, and never pinned by a test.
///
/// `EN_SOURCE` already has "Globex Logistics" as a real employer (see
/// `dropped_role_is_critical_and_names_the_employer` above); this reuses it
/// rather than a company name unproven against `distinctive_tokens`/
/// `survival_tokens`.
///
/// Mutation check: revert `generated_experience_lower` to the
/// text-emptiness fallback (drop the `has_experience` gate) and this goes
/// red — Globex Logistics is found via the Summary sentence and the
/// Critical never fires.
#[test]
fn a_wiped_experience_section_still_flags_a_dropped_role_named_only_in_the_summary() {
    let generated = "Jane Doe\n\
         jane.doe@example.com | +49 30 1234567 | github.com/janedoe\n\n\
         SUMMARY\n\n\
         Backend engineer, most recently leading payments reliability work \
         after Globex Logistics.\n\n\
         EXPERIENCE\n\n";
    let report = en_resume(generated, &en_requirements());
    let hits = fired(&report, FACTUAL_DROPPED_ROLE);
    assert!(
        hits.iter()
            .any(|i| i.evidence.as_deref().is_some_and(|e| e.contains("Globex"))),
        "Globex Logistics is named only in the Summary, not in the (wiped) \
         Experience section — the old text-emptiness fallback searched the \
         whole document and let this slip through silently; got {hits:#?}"
    );
}

/// H2 — a shortened company name is normal tailoring, not a dropped role.
/// "IBM Deutschland GmbH" has exactly one 4+ character token that is not a legal
/// form — "deutschland" — so writing the employer as "IBM" made the entry look
/// like it had vanished.
#[test]
fn shortened_company_names_are_not_dropped_roles() {
    let source = "EXPERIENCE\n\n\
                  Software Engineer | IBM Deutschland GmbH | 2018 - 2021\n\
                  - Built the billing service in Java\n\n\
                  Senior Developer | SAP Deutschland SE | 2015 - 2018\n\
                  - Ran the integration platform\n";
    let shortened = "EXPERIENCE\n\n\
                     Software Engineer | IBM | 2018 - 2021\n\
                     - Built the billing service in Java\n\n\
                     Senior Developer | SAP | 2015 - 2018\n\
                     - Ran the integration platform\n";
    silent(&report_against(shortened, source), FACTUAL_DROPPED_ROLE);

    // A genuinely dropped employer still fires — and geography alone does NOT
    // count as evidence that it survived.
    let dropped = "EXPERIENCE\n\n\
                   Software Engineer | IBM | 2018 - 2021\n\
                   - Built the billing service in Java\n\
                   - Worked with colleagues across Deutschland\n";
    let report = report_against(dropped, source);
    let hits = fired(&report, FACTUAL_DROPPED_ROLE);
    assert_eq!(hits.len(), 1, "only the SAP entry is gone; got {hits:#?}");
    assert!(hits[0]
        .evidence
        .as_deref()
        .is_some_and(|e| e.contains("SAP")));

    // A two-character token still needs a word boundary: "SAP" is not evidenced
    // by "sapphire".
    let lookalike = "EXPERIENCE\n\n\
                     Software Engineer | IBM | 2018 - 2021\n\
                     - Built the sapphire billing service in Java\n";
    fired(&report_against(lookalike, source), FACTUAL_DROPPED_ROLE);
}

/// Audit finding #1 (CRITICAL) — `company_survives` used to substring-search
/// the WHOLE document, so a Summary sentence that merely NAMES a former
/// employer let a repair round delete the entry entirely and still read as
/// "survived". Scoping the search to the generated EXPERIENCE section(s)
/// closes that hole; the Summary mention below must not spare the dropped
/// entry.
///
/// Mutation check: revert `dropped_role_issues` to search
/// `ctx.input.generated.to_lowercase()` instead of
/// `generated_experience_lower(&ctx.generated_sections)` and this goes red —
/// verified (the fixed-fixture form fired, the whole-document form was
/// silent), then restored to the scoped form and re-verified green.
#[test]
fn a_company_named_only_in_the_summary_does_not_spare_a_dropped_experience_entry() {
    let source = "EXPERIENCE\n\n\
                  Software Engineer | Acme Payments | 2019 - 2021\n\
                  - Built the payments core\n\n\
                  Senior Engineer | Globex Logistics | 2021 - 2023\n\
                  - Ran the routing platform\n";
    // A destructive repair round deleted the Globex Logistics entry from
    // EXPERIENCE, but the Summary — written before the round, untouched by
    // it — still names both employers.
    let generated = "SUMMARY\n\n\
                     Engineer with experience at Acme Payments and Globex Logistics.\n\n\
                     EXPERIENCE\n\n\
                     Software Engineer | Acme Payments | 2019 - 2021\n\
                     - Built the payments core\n";
    let report = report_against(generated, source);
    let hits = fired(&report, FACTUAL_DROPPED_ROLE);
    assert!(
        hits[0]
            .evidence
            .as_deref()
            .is_some_and(|e| e.contains("Globex")),
        "the evidence must name the dropped employer even though a sibling \
         section still mentions it; got {:?}",
        hits[0].evidence
    );
    assert_eq!(report.metrics.roles_source, 2);
    assert_eq!(report.metrics.roles_output, 1);
}

/// F5 — `dropped_role_issues` searches the whole generated document once per
/// SOURCE entry, and `title_drift_issues` compares every generated entry
/// against every source entry: both are O(entries × document) over inputs that
/// admit 200KB each. The sibling near-duplicate scan was explicitly capped for
/// the same risk class ([`duplicates::MAX_DUP_BULLETS`]); this pins the
/// analogous entry cap and proves it BITES without disabling either check.
#[test]
fn entry_scans_are_capped_like_the_duplicate_scan() {
    assert_eq!(factual::MAX_SCANNED_ENTRIES, 200);
    let over = factual::MAX_SCANNED_ENTRIES + 50;
    let doc = |title: &str| {
        let mut out = String::from("EXPERIENCE\n\n");
        for i in 0..over {
            out.push_str(&format!(
                "{title} | Zetacorp{i} Systems | 2010 - 2011\n- Shipped release {i}\n\n"
            ));
        }
        out
    };
    let source = doc("Backend Engineer");
    let count = |issues: Vec<ContentIssue>, code: &str| {
        issues.into_iter().filter(|i| i.code == code).count()
    };

    // `factual.dropped_role`: the generated document mentions none of them.
    let none_survive = "EXPERIENCE\n\nBackend Engineer | Different Employer | 2020 - 2021\n\
                        - Shipped one thing\n";
    let input = content_input(none_survive, &source, EN_JOB_AD);
    assert_eq!(
        count(
            factual::validate(&Analysis::new(&input)),
            FACTUAL_DROPPED_ROLE
        ),
        factual::MAX_SCANNED_ENTRIES,
        "the dropped-role scan must stop AT the cap — and not before it"
    );

    // `consistency.title_drift`: the same employers, a wholly different title.
    let drifted = doc("Pastry Chef");
    let input = content_input(&drifted, &source, EN_JOB_AD);
    assert_eq!(
        count(
            consistency::validate(&Analysis::new(&input)),
            CONSISTENCY_TITLE_DRIFT
        ),
        factual::MAX_SCANNED_ENTRIES,
        "the title-drift scan must stop AT the cap — and not before it"
    );
}

/// R4-F1 — an employer whose name reduces to NOTHING but geography and a legal
/// form ("Deutschland GmbH", "Global Group", a bare "Berlin") is still
/// CHECKABLE (`distinctive_tokens` keeps "deutschland"/"global"/"berlin") but
/// has an EMPTY survival-token list, so `company_survives` answered false
/// whatever the document said. The result was an unavoidable Critical claiming
/// the role had vanished while the employer sat VERBATIM in the output.
///
/// Same family as `shortened_company_names_are_not_dropped_roles`: no possible
/// evidence means no accusation.
#[test]
fn companies_made_only_of_geography_and_legal_form_are_never_dropped_roles() {
    for company in ["Deutschland GmbH", "Global Group", "Berlin"] {
        let doc = format!(
            "EXPERIENCE\n\n\
             Software Engineer | {company} | 2018 - 2021\n\
             - Built the billing service in Java\n"
        );
        // Source and output are byte-identical: there is nothing to accuse.
        silent(&report_against(&doc, &doc), FACTUAL_DROPPED_ROLE);
    }

    // The check still works where evidence is POSSIBLE: one identity token
    // ("acme"), and the output never says it.
    let source = "EXPERIENCE\n\n\
                  Software Engineer | Acme Deutschland GmbH | 2018 - 2021\n\
                  - Built the billing service in Java\n";
    let dropped = "EXPERIENCE\n\n\
                   Software Engineer | Globex | 2018 - 2021\n\
                   - Built the billing service in Java\n";
    fired(&report_against(dropped, source), FACTUAL_DROPPED_ROLE);
}

/// Regression: scoping the survival search to `SectionKind::Experience` turned
/// "this résumé has no section my classifier calls Experience" into "these jobs
/// were deleted". `classify_section` does not know `Work History` or `Selected
/// Roles`, so a truthful résumé using either earned two unclearable Criticals —
/// unclearable because `criticals_by_section` routes them to
/// `SectionKey::Experience`, which `find` cannot locate, so no repair runs.
#[test]
fn an_unrecognised_experience_heading_is_not_read_as_deleted_jobs() {
    let src = "Jane Doe\n\nWORK EXPERIENCE\nAcme Payments | 2021 - Present\n\
               - Led the settlement migration to Rust across fourteen markets\n\n\
               Globex Logistics | 2018 - 2021\n\
               - Built a distributed scheduler handling four million jobs a day\n";
    for heading in ["WORK EXPERIENCE", "WORK HISTORY", "SELECTED ROLES"] {
        let generated = src.replace("WORK EXPERIENCE", heading);
        let report = report_against(&generated, src);
        let dropped = codes(&report)
            .iter()
            .filter(|c| **c == FACTUAL_DROPPED_ROLE)
            .count();
        assert_eq!(
            dropped, 0,
            "heading {heading:?}: both roles are present verbatim, so nothing was \
             dropped; got {dropped} factual.dropped_role"
        );
    }
}

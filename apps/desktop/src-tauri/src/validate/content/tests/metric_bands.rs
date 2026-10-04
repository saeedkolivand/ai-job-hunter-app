//! The contact band and the phone shape: which lines may neither vouch for a metric nor
//! be checked as one.

use super::{support::*, *};

/// R8 follow-up — a heading-less document (a cover letter) has no section band,
/// so `metric_lines` skips its contact lines by SHAPE. That shape test was
/// `export::parser`'s loose phone rule (any 7+ run of digits, spaces and
/// hyphens), so a letter paragraph quoting a rate range exempted itself from the
/// fabricated-metric check — the letter-path twin of R8-F3, left open because
/// the helper lived outside that round's file set.
#[test]
fn a_letter_paragraph_quoting_a_range_is_still_metric_checked() {
    let letter = "Jane Doe\njane.doe@example.com\n\n\
                  Dear Hiring Manager,\n\n\
                  Last year I renegotiated our agency rates from 150 - 200 EUR an hour \
                  across twelve suppliers.\n\n\
                  Best regards,\nJane Doe\n";
    let report = en_letter(letter);
    let evidence = fired_evidence(&report, FACTUAL_UNSOURCED_METRIC);
    assert!(
        evidence.contains(&"150") && evidence.contains(&"200"),
        "a letter's body prose is not its letterhead; got {evidence:?}"
    );

    // The guard that keeps the shape test on this path at all: a letter carries
    // contact details in its SIGN-OFF as well as its letterhead, and neither is
    // a claim of impact. Both must stay skipped.
    let signed = "Jane Doe\njane.doe@example.com\n\n\
                  Dear Hiring Manager,\n\n\
                  I put a Redis cache in front of the ledger service and checkout latency \
                  went from 480ms to 90ms.\n\n\
                  Best regards,\nJane Doe\n+49 30 1234567\n";
    silent(&en_letter(signed), FACTUAL_UNSOURCED_METRIC);
}

/// The two surfaces that ask "is this line a phone number?" must agree, because
/// they disagreed for a whole round: `ats::header_in_body` took the strict shape
/// and the metric band skip kept the parser's loose one.
#[test]
fn the_header_phone_shape_is_one_rule_for_both_surfaces() {
    for phone in [
        "+49 30 1234567",
        "+49 (0)30 1234567",
        "(030) 12345678",
        "0176 12345678",
        "+1 (555) 123-4567",
    ] {
        assert!(
            looks_like_header_phone(phone),
            "{phone:?} is a phone number"
        );
        assert!(has_real_contact_match(phone), "{phone:?} is a contact line");
    }
    for prose in [
        "Renegotiated agency rates from 150 - 200 EUR per hour",
        "Das Jahresbudget von 90 000 - 110 000 EUR gesteuert",
        "Acme Payments | 2018 - 2021",
        "Processed 4500 orders a day",
    ] {
        assert!(
            !looks_like_header_phone(prose),
            "{prose:?} is prose, not a phone number"
        );
        assert!(
            !has_real_contact_match(prose),
            "{prose:?} must not exempt itself from the metric pass"
        );
    }
    // An address still counts, and still only when it is a real one.
    assert!(has_real_contact_match("jane.doe@example.com"));
    assert!(!has_real_contact_match("owned the @payments rotation"));
}

/// R8-F3 — the metric pass exempted ANY line the parser marked `Contact`, and a
/// wrapped body paragraph carrying a European-grouped figure IS contact-shaped
/// (`PHONE_RE` reads "90 000 - 110 000" as a phone number). A fabricated figure
/// mid-document therefore escaped the fabricated-metric check entirely.
#[test]
fn a_contact_shaped_body_paragraph_is_still_metric_checked() {
    let source = "Jane Doe\njane@example.com\n\nSUMMARY\n\n\
                  Backend engineer on payment platforms.\n\n\
                  EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    let fabricated = "Jane Doe\njane@example.com\n\nSUMMARY\n\n\
                      Grew the platform budget from 90 000 - 110 000 EUR inside one year\n\n\
                      EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    let report = report_against(fabricated, source);
    let evidence = fired_evidence(&report, FACTUAL_UNSOURCED_METRIC);
    assert!(
        evidence.contains(&"90 000") && evidence.contains(&"110 000"),
        "a body paragraph is not the contact band; got {evidence:?}"
    );
}

/// R8-F4 — the TRUTH side scanned the whole source with no band skip, so the
/// digits of the candidate's own phone number "sourced" an unrelated fabricated
/// figure: a bullet claiming 1 234 567 settlements was accepted because the
/// header says "+49 30 1234567".
#[test]
fn source_contact_digits_do_not_source_a_fabricated_metric() {
    let header = "Jane Doe\njane@example.com | +49 30 1234567 | 10115 Berlin\n\n";
    let source = format!("{header}EXPERIENCE\n\nAcme | 2021 - Present\n- Led the migration\n");
    let generated = format!(
        "{header}EXPERIENCE\n\nAcme | 2021 - Present\n\
         - Settled 1234567 payments in the first quarter after the rewrite\n"
    );
    let report = report_against(&generated, &source);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("1234567"));

    // …while the header's own digits are still not CLAIMS on either side: both
    // sides skip the same band, so a truthful document stays silent.
    silent(&report_against(&source, &source), FACTUAL_UNSOURCED_METRIC);
}

/// R9-F3 — the TRUTH side skipped section 0 by POSITION, so a résumé that opens
/// with a profile paragraph BEFORE its first heading (an extremely common
/// layout) had every figure in that paragraph erased from the sourced set. The
/// same figure, restated in the generated document's headed summary, then read
/// as fabricated.
#[test]
fn a_profile_paragraph_before_the_first_heading_still_sources_its_metrics() {
    let source = "Jane Doe\njane@example.com\n\n\
                  Backend engineer who cut checkout latency by 40% on the payment platform.\n\n\
                  EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    let generated = "Jane Doe\njane@example.com\n\n\
                     SUMMARY\n\n\
                     Backend engineer; cut checkout latency by 40% on the payment platform.\n\n\
                     EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    silent(&report_against(generated, source), FACTUAL_UNSOURCED_METRIC);

    // The check still bites: a figure the profile paragraph does NOT state is
    // still a fabrication.
    let invented = "Jane Doe\njane@example.com\n\n\
                    SUMMARY\n\n\
                    Backend engineer; cut checkout latency by 65% on the payment platform.\n\n\
                    EXPERIENCE\n\nAcme | 2021 - Present\n- Led the ledger migration\n";
    let report = report_against(invented, source);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("65%"));
}

/// R9-F4 — the trade R8-F4's symmetry fix made. A heading-less document (a
/// letter) has no positional band, so its sign-off is skipped by SHAPE — and
/// the shape test is `looks_like_header_phone`, whose own documented miss is a
/// bare US number with no `+`/parens. "555-123-4567" alone under "Best regards"
/// therefore reached the metric pass and became THREE `unsourced_metric`
/// Criticals about the candidate's own phone number.
#[test]
fn a_number_alone_on_a_line_is_a_phone_number_not_three_claims() {
    let body = "I put a Redis cache in front of the ledger service and checkout latency \
                went from 480ms to 90ms.";
    let signed = format!(
        "Jane Doe\njane.doe@example.com\n\n\
         Dear Hiring Manager,\n\n{body}\n\n\
         Best regards,\nJane Doe\n555-123-4567\n"
    );
    silent(&en_letter(&signed), FACTUAL_UNSOURCED_METRIC);

    // Direction 2 — R8-F4's property is kept: digits in PROSE are still
    // checked, and a figure the source never states is still a Critical.
    let fabricated = format!(
        "Jane Doe\njane.doe@example.com\n\n\
         Dear Hiring Manager,\n\n{body} I also settled 4500 disputes that quarter.\n\n\
         Best regards,\nJane Doe\n555-123-4567\n"
    );
    let report = en_letter(&fabricated);
    let evidence = fired_evidence(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(
        evidence,
        vec!["4500"],
        "the sign-off number is not a claim; the prose figure still is"
    );

    // Direction 3 — the same line in the SOURCE still cannot vouch for a body
    // metric that merely reuses its digits (R8-F4, via the shape this round
    // added rather than the position R9-F3 relaxed).
    let header = "Jane Doe\njane@example.com\n555-123-4567\n\n";
    let source = format!("{header}EXPERIENCE\n\nAcme | 2021 - Present\n- Led the migration\n");
    let generated = format!(
        "{header}EXPERIENCE\n\nAcme | 2021 - Present\n\
         - Settled 4567 disputes in the first quarter after the rewrite\n"
    );
    let report = report_against(&generated, &source);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("4567"));
}

/// R10-F4 — [`is_bare_phone_line`] ran on BOTH sides of the metric comparison,
/// so the SOURCE lost every bare-numeric line from its sourced whitelist. A
/// figure the candidate wrote on a line of its own — the shape a table-extracted
/// PDF produces — could no longer vouch for its own restatement, which is an
/// ACCUSATION channel, and it broke the stated `MetricSide` invariant that the
/// source side reads strictly more than the claims side.
#[test]
fn a_figure_alone_on_a_source_line_still_sources_its_restatement() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                  - Refunds processed in the first quarter after the rewrite:\n\
                  1 200 000\n";
    let restated = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Processed 1,200,000 refunds in the first quarter after the rewrite\n";
    silent(&report_against(restated, source), FACTUAL_UNSOURCED_METRIC);

    // The claims side is unchanged — a number-only line still makes no claim,
    // so a figure the source never states is still a Critical.
    let fabricated = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                      - Processed 9,400,000 refunds in the first quarter after the rewrite\n";
    let report = report_against(fabricated, source);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("9,400,000"));

    // The source side is widened in the BODY only, not wholesale: a bare number
    // in the contact band is still contact details and still cannot vouch for a
    // claim. That half lives in
    // `a_number_alone_on_a_line_is_a_phone_number_not_three_claims`
    // (direction 3), which a "claims side only" fix would have turned red —
    // which is why this rule is scoped by band rather than by side alone.
}

/// R11-F1 / R11-F5(b) — [`HEADER_PHONE_RE`]'s `[+(]` arm reads a parenthesized
/// DATE SPAN as an area code followed by digits: "(2019 - 2021)" is `(`, a
/// digit, eight more separator-or-digit characters and a digit.
/// `ats::is_contact_cluster` already refused a year-bearing line; the OTHER
/// caller, [`has_real_contact_match`], did not — so a pre-heading line carrying
/// a date span was struck out of the SOURCE's metric set, and restating its
/// figure came back as a fabrication Critical.
///
/// The guard therefore belongs to the shape test itself, not to one call site:
/// a date span is not a phone number wherever the question is asked.
#[test]
fn a_parenthesized_date_span_is_not_a_header_phone() {
    assert!(!looks_like_header_phone("Beratung (2019 - 2021)"));
    assert!(!has_real_contact_match("Beratung (2019 - 2021)"));
    // The two forms the shape test exists for still match.
    assert!(looks_like_header_phone("+49 30 1234567"));
    assert!(has_real_contact_match("Jane Doe | +49 (0)30 1234567"));
    assert!(has_real_contact_match("jane@example.com"));

    // End to end: the figure sits on a pre-heading line that also carries a
    // parenthesized span — the shape a freelance/contract summary has.
    let source = "Jane Doe\n\
                  jane@example.com | +49 30 1234567\n\n\
                  Contract work (2019 - 2021): 1 200 000 EUR in payment volume\n\n\
                  EXPERIENCE\n\n\
                  Senior Engineer | Acme Payments | 2021 - Present\n\
                  - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    let restated = "Jane Doe\n\
                    jane@example.com | +49 30 1234567\n\n\
                    EXPERIENCE\n\n\
                    Senior Engineer | Acme Payments | 2021 - Present\n\
                    - Moved 1 200 000 EUR of payment volume onto the new rails\n\
                    - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    silent(&report_against(restated, source), FACTUAL_UNSOURCED_METRIC);

    // The guard: a genuine header line is still contact details, so its digits
    // still cannot source a body claim.
    let phone_source = "Jane Doe\n\
                        jane@example.com | +49 30 1234567\n\n\
                        EXPERIENCE\n\n\
                        Senior Engineer | Acme Payments | 2021 - Present\n\
                        - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    let borrowed = "Jane Doe\n\
                    jane@example.com | +49 30 1234567\n\n\
                    EXPERIENCE\n\n\
                    Senior Engineer | Acme Payments | 2021 - Present\n\
                    - Handled 1234567 disputes in the first quarter\n";
    fired(
        &report_against(borrowed, phone_source),
        FACTUAL_UNSOURCED_METRIC,
    );

    // The OTHER caller keeps the behaviour its own now-deleted `years_in`
    // clause bought: an employer line followed by a bare date column is not a
    // second contact block. This is the assertion that makes moving the guard
    // into the shape test provably behaviour-preserving for `ats`, rather than
    // just "the suite still passes".
    let date_column = "Jane Doe\n\
                       jane@example.com\n\n\
                       EXPERIENCE\n\n\
                       Acme Payments\n\
                       2018 - 2021\n\
                       - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    silent(
        &report_against(date_column, date_column),
        ATS_HEADER_IN_BODY,
    );
}

/// R12-F4 — round 11 folded a year test into [`looks_like_header_phone`] to stop
/// a parenthesized DATE SPAN reading as an area code, and wrote off "a header
/// phone containing a 1900–2099 run" as a missed skip. It is not a missed skip,
/// it is an ACCUSATION channel: the source's contact band still drops such a
/// line (via `is_bare_phone_line`), while the letter that repeats the same
/// number keeps it (the shape test says it is not contact details) — so the
/// candidate's own phone digits come back as a fabricated metric.
#[test]
fn a_header_phone_carrying_a_year_run_is_still_contact_details() {
    assert!(looks_like_header_phone("+49 30 2019 1234"));
    assert!(has_real_contact_match(
        "Max Mustermann · Berlin · +49 30 2019 1234"
    ));
    // R11-F1 stays closed: a span is still not a phone number, on both callers.
    assert!(!looks_like_header_phone("Beratung (2019 - 2021)"));
    assert!(!has_real_contact_match(
        "Contract work (2019 - 2021): 1 200 000 EUR in payment volume"
    ));

    let source = "Max Mustermann\n\
                  max.mustermann@example.com\n\
                  +49 30 2019 1234\n\n\
                  EXPERIENCE\n\n\
                  Acme Payments | 2021 - Present\n\
                  - Cut checkout latency from 480ms to 90ms with a Redis cache\n";
    let letter = "Max Mustermann · Berlin · +49 30 2019 1234\n\n\
                  Dear Hiring Manager,\n\n\
                  I put a Redis cache in front of the ledger service and checkout latency \
                  went from 480ms to 90ms.\n\n\
                  Best regards,\nMax Mustermann\n";
    silent(
        &letter_report_for(letter, source, EN_JOB_AD),
        FACTUAL_UNSOURCED_METRIC,
    );

    // The invariant's own direction is unchanged: the source's contact band is
    // dropped from the truth set, so its digits still cannot vouch for a body
    // claim that merely reuses them.
    let borrowed = "Max Mustermann\n\
                    max.mustermann@example.com\n\
                    +49 30 2019 1234\n\n\
                    EXPERIENCE\n\n\
                    Acme Payments | 2021 - Present\n\
                    - Settled 1234 disputes in the first quarter after the rewrite\n";
    let borrowed_report = report_against(borrowed, source);
    let hits = fired(&borrowed_report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("1234"));

    // …and a letter figure the source never states is still a Critical, so the
    // skip exempts the letterhead rather than the letter.
    let fabricated = "Max Mustermann · Berlin · +49 30 2019 1234\n\n\
                      Dear Hiring Manager,\n\n\
                      I put a Redis cache in front of the ledger service and settled 9400 \
                      disputes in the same quarter.\n\n\
                      Best regards,\nMax Mustermann\n";
    let fabricated_report = letter_report_for(fabricated, source, EN_JOB_AD);
    let hits = fired(&fabricated_report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("9400"));
}

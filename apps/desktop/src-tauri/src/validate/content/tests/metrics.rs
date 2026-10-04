//! `factual.unsourced_metric`: which figures are extracted from a document and how a
//! source vouches for them.

use super::{support::*, *};

/// The normalized numbers of the `kind` metrics `doc` claims.
fn numbers_of(doc: &str, kind: factual::MetricKind) -> Vec<String> {
    factual::metrics_in(doc, DocKind::Resume)
        .into_iter()
        .filter(|m| m.kind == kind)
        .map(|m| m.number)
        .collect()
}

/// Years are never metrics. A résumé is full of them and a fabricated-metric
/// Critical on "2021" would be unusable.
#[test]
fn years_are_never_treated_as_metrics() {
    let metrics = factual::metrics_in(
        "EXPERIENCE\n\nAcme | 2021 - 2024\n- Shipped in 1999\n",
        DocKind::Resume,
    );
    assert!(
        metrics.is_empty(),
        "1900–2099 must be excluded from metric extraction; got {metrics:?}"
    );
    // Just outside the window, a four-digit run IS a quantity.
    let quantity = factual::metrics_in(
        "EXPERIENCE\n\nAcme\n- Processed 4500 orders a day\n",
        DocKind::Resume,
    );
    assert_eq!(quantity.len(), 1, "got {quantity:?}");
    assert_eq!(quantity[0].number, "4500");
}

/// Phone numbers and postal codes live in the contact band and are not claims
/// of impact.
#[test]
fn contact_band_digits_are_not_metrics() {
    let text = "Jane Doe\njane@example.com | +49 30 1234567 | 10115 Berlin\n\n\
                EXPERIENCE\n\nAcme\n- Led the migration\n";
    assert!(
        factual::metrics_in(text, DocKind::Resume).is_empty(),
        "the header band must be skipped entirely; got {:?}",
        factual::metrics_in(text, DocKind::Resume)
    );
}

/// M1 — a restated number is not a fabricated one. `10k` and `10,000` are the
/// same figure; so are "doubled" and "2x".
#[test]
fn restated_numbers_are_not_fabricated_metrics() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                  - Handled 10k requests per second at peak\n\
                  - Doubled throughput on the ledger service\n";
    let restated = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Handled 10,000 requests per second at peak\n\
                    - Lifted throughput 2x on the ledger service\n";
    silent(&report_against(restated, source), FACTUAL_UNSOURCED_METRIC);

    // Millisecond figures must not read as millions: `480ms` is 480, not
    // 480 000 000, and inventing that expansion would silence a real check.
    let latency = "EXPERIENCE\n\nAcme | 2021 - Present\n- Cut latency from 480ms to 90ms\n";
    let invented = "EXPERIENCE\n\nAcme | 2021 - Present\n- Cut latency from 480ms to 250ms\n";
    let report = report_against(invented, latency);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("250"));
}

/// M2 — the metric check skipped anything `is_contact_shaped` accepted, which
/// includes any line with two `·` separators or the word "website". A body
/// bullet was therefore exempt from the fabricated-metric Critical entirely.
#[test]
fn a_separator_laden_body_bullet_stays_metric_checkable() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n- Rebuilt the marketing website\n";
    let evasive = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                   - Rebuilt the marketing website · cut page weight · shipped 12000 pages\n";
    let report = report_against(evasive, source);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("12000"));

    // The header band is still skipped: a phone number is not a claim of impact.
    let header = "Jane Doe\njane@example.com | +49 30 1234567 | 10115 Berlin\n\n\
                  EXPERIENCE\n\nAcme | 2021 - Present\n- Led the migration\n";
    silent(&report_against(header, header), FACTUAL_UNSOURCED_METRIC);
}

/// A technology in neither the source résumé nor the ad. Only vocabulary the
/// keyword kernel itself recognises is policed, so ordinary rephrasing is safe.
#[test]
fn unsourced_term_warns_on_an_ungrounded_technology() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n- Built the billing service in Python\n";
    let job = "Backend engineer, Python and PostgreSQL.";
    let invented = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Built the billing service in Python and TensorFlow\n";
    let report = report_for(invented, source, job, &[]);
    let hits = fired(&report, FACTUAL_UNSOURCED_TERM);
    assert_eq!(hits[0].evidence.as_deref(), Some("tensorflow"));
    assert_eq!(hits[0].severity, Severity::Warning);
    // Rewording ordinary prose is not a factual claim.
    let reworded = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                    - Owned the invoicing service, written in Python\n";
    silent(
        &report_for(reworded, source, job, &[]),
        FACTUAL_UNSOURCED_TERM,
    );
}

/// F4 — `PERCENT_RE` and `INTEGER_RE` both capture a fabricated figure of 100
/// or more written with a percent sign ("150%" and "150"), and the dedupe key
/// was the RAW matched text, so one invented number produced two Criticals on
/// the same span. Sourcing is decided on the normalized number, so the dedupe
/// has to be too.
#[test]
fn one_fabricated_percentage_produces_exactly_one_critical() {
    let source = "EXPERIENCE\n\nAcme | 2021 - Present\n- Cut checkout latency from 480ms to 90ms\n";
    let three_digit =
        "EXPERIENCE\n\nAcme | 2021 - Present\n- Grew signups 150% after the rewrite\n";
    let report = report_against(three_digit, source);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(
        hits.len(),
        1,
        "one invented figure is one finding, not one per regex that saw it; got {hits:#?}"
    );
    assert_eq!(
        hits[0].evidence.as_deref(),
        Some("150%"),
        "the evidence must still quote the span exactly as written"
    );

    // A two-digit percentage never had the collision (`INTEGER_RE` needs three
    // significant digits) and must keep firing exactly once — the dedupe change
    // must not swallow a second, genuinely DIFFERENT number.
    let two_numbers = "EXPERIENCE\n\nAcme | 2021 - Present\n\
                       - Grew signups 72% and cut refunds 150% after the rewrite\n";
    let report = report_against(two_numbers, source);
    let evidence = fired_evidence(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(
        evidence,
        vec!["72%", "150%"],
        "two different invented figures are two findings"
    );
}

#[test]
fn number_normalization_is_locale_neutral() {
    // Thousands grouping, three conventions, one answer.
    assert_eq!(factual::normalize_number("1,200"), "1200");
    assert_eq!(factual::normalize_number("1.200"), "1200");
    assert_eq!(factual::normalize_number("1\u{202F}200"), "1200");
    // Decimals, both conventions, one answer.
    assert_eq!(factual::normalize_number("3.5"), "3.5");
    assert_eq!(factual::normalize_number("3,5"), "3.5");
    assert_eq!(factual::normalize_number("42"), "42");
}

/// R6-F1 — `MULTIPLIER_RE` ended in `(?:\b|$)`, and `×` is not a word
/// character: a boundary after it needs a WORD character on the other side, so
/// only `3×5` or a line ENDING in `3×` ever matched. Every mid-sentence
/// typographic multiplier was invisible to the metric pass — never extracted,
/// never cross-checked against the source.
#[test]
fn a_typographic_multiplier_is_extracted_mid_sentence() {
    let doc = resume_with_bullet("Grew throughput 3× while holding the error budget flat");
    let multipliers = numbers_of(&doc, factual::MetricKind::Multiplier);
    assert_eq!(
        multipliers,
        vec!["3".to_string()],
        "a mid-sentence `3×` is a multiplier claim"
    );

    // The ASCII spelling is unchanged, mid-sentence and at end of line …
    for line in [
        "Grew throughput 3x while holding the error budget flat",
        "Grew throughput 3x",
        "Grew throughput 3×",
    ] {
        let doc = resume_with_bullet(line);
        assert!(
            factual::metrics_in(&doc, DocKind::Resume)
                .iter()
                .any(|m| m.kind == factual::MetricKind::Multiplier && m.number == "3"),
            "{line:?} states a multiplier"
        );
    }
    // … and `x` inside a word is still not a multiplier.
    let doc = resume_with_bullet("Ran the 3xtra pipeline for the payments team every night");
    assert!(
        !factual::metrics_in(&doc, DocKind::Resume)
            .iter()
            .any(|m| m.kind == factual::MetricKind::Multiplier),
        "`x` inside a word is not a multiplier"
    );

    // End to end: an invented multiplier is a Critical, like every other
    // fabricated figure.
    let source = resume_with_bullet("Improved throughput after rewriting the retry scheduler");
    let invented = resume_with_bullet("Improved throughput 5× after rewriting the retry scheduler");
    let report = report_against(&invented, &source);
    let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
    assert_eq!(hits[0].evidence.as_deref(), Some("5×"));
}

/// R6-F2 — `INTEGER_RE` accepted `.`, `,`, NBSP and NNBSP as digit grouping but
/// not the ASCII space (nor the Swiss apostrophe), while `normalize_number`'s
/// doc and body both promise "1 200" normalizes to `1200`. So a space-grouped
/// figure split into two numbers, and a document truthfully restating its own
/// source's "1 200" as "1,200" was accused of fabricating it.
#[test]
fn a_space_grouped_figure_is_one_number_not_two() {
    for (written, expected) in [
        ("1 200", "1200"),
        ("1\u{202F}200", "1200"),
        ("1\u{00A0}200", "1200"),
        ("1'200", "1200"),
        ("1\u{2019}200", "1200"),
        ("1,200", "1200"),
        ("1.200", "1200"),
        ("40 000", "40000"),
    ] {
        // Normalization always promised this …
        assert_eq!(factual::normalize_number(written), expected);
        // … and extraction must hand it the whole figure.
        let doc = resume_with_bullet(&format!("Processed {written} refunds a day"));
        let numbers = numbers_of(&doc, factual::MetricKind::LargeInteger);
        assert!(
            numbers.contains(&expected.to_string()),
            "{written:?} is one figure, not two; got {numbers:?}"
        );
    }

    // Two separate small numbers stay separate — the space is grouping only
    // when exactly three digits follow it.
    let doc = resume_with_bullet("Ran 5 12 hour shifts across the payments on-call rota");
    assert!(
        !factual::metrics_in(&doc, DocKind::Resume)
            .iter()
            .any(|m| m.number == "512"),
        "a space with two digits after it is not grouping; got {:?}",
        factual::metrics_in(&doc, DocKind::Resume)
    );

    // End to end: restating a space-grouped figure in another convention is
    // not a fabrication.
    for source_form in ["1 200", "1'200"] {
        let source = resume_with_bullet(&format!("Processed {source_form} refunds a day"));
        let restated = resume_with_bullet("Processed 1,200 refunds a day");
        silent(
            &report_against(&restated, &source),
            FACTUAL_UNSOURCED_METRIC,
        );
    }
}

/// R14-F2 — `SUFFIXED_NUMBER_RE` ran on the SOURCE side only, so a
/// magnitude-suffixed figure was never a CLAIM. `INTEGER_RE` sees only the
/// mantissa, and the mantissa is thrown away below three digits ("35k" → "35")
/// or on a decimal point ("3.5m" → "3.5"): a fabricated suffixed figure was
/// structurally invisible to `unsourced_metric`, not merely tolerated.
///
/// The mirror image of the same asymmetry is a false CRITICAL: "250k" left
/// "250" behind as a claim, which a source writing "250,000" never states.
#[test]
fn a_fabricated_suffixed_figure_is_an_unsourced_metric() {
    // The claims side must extract the EXPANDED value, the same language the
    // source side has always spoken.
    let claims: Vec<String> =
        factual::metrics_in(&resume_with_bullet("Onboarded 35k users"), DocKind::Resume)
            .into_iter()
            .map(|m| m.number)
            .collect();
    assert_eq!(
        claims,
        vec!["35000".to_string()],
        "one figure, expanded, and the mantissa is not a second claim"
    );

    let source = resume_with_bullet("Onboarded 4,000 users and booked 90,000 EUR in revenue");
    for (written, expanded) in [("35k", "35000"), ("3.5m", "3500000"), ("2bn", "2000000000")] {
        let fabricated = resume_with_bullet(&format!("Onboarded {written} users"));
        let report = report_against(&fabricated, &source);
        let hits = fired(&report, FACTUAL_UNSOURCED_METRIC);
        assert_eq!(
            hits.len(),
            1,
            "one invented figure is one Critical; got {hits:#?}"
        );
        assert_eq!(
            hits[0].evidence.as_deref(),
            Some(written),
            "the evidence quotes the span as written, not the expansion {expanded}"
        );
    }

    // …and the equivalence that makes the expansion worth having, in BOTH
    // directions: the same figure written the other way is not a fabrication.
    for (source_form, generated_form) in [
        ("10k requests", "10,000 requests"),
        ("10,000 requests", "10k requests"),
        ("250,000 requests", "250k requests"),
        ("1.2m requests", "1,200,000 requests"),
    ] {
        silent(
            &report_against(
                &resume_with_bullet(&format!("Handled {generated_form} per second at peak")),
                &resume_with_bullet(&format!("Handled {source_form} per second at peak")),
            ),
            FACTUAL_UNSOURCED_METRIC,
        );
    }

    // The `\b` that keeps a millisecond figure out of the millions is still the
    // rule on both sides: `480ms` must not become 480 000 000.
    let latency = resume_with_bullet("Cut latency from 480ms to 90ms");
    silent(
        &report_against(&latency, &latency),
        FACTUAL_UNSOURCED_METRIC,
    );
}

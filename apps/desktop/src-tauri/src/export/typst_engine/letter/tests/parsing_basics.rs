//! Core parse_cover_letter coverage: paragraphs, EN/DE fields, market geometry, rich text, markdown stripping, ATS flag.

use super::super::model::{A4_H, A4_W, LETTER_H, LETTER_W};
use super::super::{parse_cover_letter, strip_md_links};
use super::support::*;

#[test]
fn body_paragraphs_survive_a_letter_with_no_letterhead() {
    // The agent's `draft_cover_letter` prompt asks for the finished letter
    // only — "no preamble" — so the draft opens straight at the salutation
    // with no name/contact echo to consume the three header skips. The skip
    // arm then ate the BODY's blank lines and the whole letter collapsed into
    // a single run-on paragraph.
    let letter = "Dear Hiring Manager,\n\nFirst paragraph.\n\nSecond paragraph.\n\nThird paragraph.\n\nSincerely,\n\nJane Smith\n";
    let model = parse_cover_letter(
        letter,
        None,
        Some("Jane Smith"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert_eq!(
        model.body.len(),
        3,
        "each blank-line-separated paragraph must stay separate; got {:?}",
        model.body
    );
    assert_eq!(model.salutation.as_deref(), Some("Dear Hiring Manager,"));
}

#[test]
fn body_paragraphs_survive_a_letter_with_only_a_date_and_salutation() {
    // Two leading pre-body lines instead of three — still not enough to burn
    // the skip budget before the body starts.
    let letter = "12 March 2025\n\nDear Hiring Manager,\n\nFirst paragraph.\n\nSecond paragraph.\n\nSincerely,\n\nJane Smith\n";
    let model = parse_cover_letter(
        letter,
        None,
        Some("Jane Smith"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert_eq!(model.body.len(), 2, "got {:?}", model.body);
}

#[test]
fn parses_english_letter_all_fields() {
    let model = parse_cover_letter(
        EN_LETTER,
        None,
        Some("Jane Smith"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert_eq!(model.letterhead.name, "Jane Smith");
    assert!(
        model.date.as_deref().unwrap_or("").contains("2025"),
        "date should be captured; got {:?}",
        model.date
    );
    assert!(
        !model.recipient_lines.is_empty(),
        "recipient block should be present"
    );
    assert!(
        model.recipient_lines.iter().any(|l| l.contains("Acme")),
        "recipient lines should include company name"
    );
    assert!(
        model.subject.is_none(),
        "US market letter should have no subject line"
    );
    assert!(
        model
            .salutation
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .starts_with("dear"),
        "salutation should start with 'Dear'; got {:?}",
        model.salutation
    );
    assert!(!model.body.is_empty(), "body paragraphs must not be empty");
    assert!(
        model
            .signoff
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .starts_with("sincerely"),
        "signoff should be 'Sincerely'; got {:?}",
        model.signoff
    );
    assert_eq!(model.signature_name, "Jane Smith");
    assert!(
        model
            .signature_title
            .as_deref()
            .unwrap_or("")
            .contains("Engineer"),
        "signature title should be captured; got {:?}",
        model.signature_title
    );
}

#[test]
fn parses_german_letter_subject_line() {
    let model = parse_cover_letter(
        DE_LETTER,
        None,
        Some("Max Müller"),
        "de",
        "de",
        dummy_style(),
        false,
    );

    assert_eq!(model.letterhead.name, "Max Müller");
    assert!(
        model.date.is_some(),
        "DE letter must have a date; got {:?}",
        model.date
    );
    assert!(
        !model.recipient_lines.is_empty(),
        "recipient block should be present"
    );
    assert!(
        model.subject.is_some(),
        "DE letter must have a subject line"
    );
    let subject = model.subject.as_deref().unwrap();
    assert!(
        subject.to_lowercase().contains("betreff"),
        "subject must contain 'Betreff'; got {subject:?}"
    );
    let sal = model.salutation.as_deref().unwrap_or("");
    assert!(
        sal.to_lowercase().starts_with("sehr geehr"),
        "German salutation not detected; got {sal:?}"
    );
    assert!(
        model
            .signoff
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains("freundlichen"),
        "German signoff not detected; got {:?}",
        model.signoff
    );
}

#[test]
fn de_market_uses_a4_us_market_uses_letter() {
    let m_de = parse_cover_letter("x", None, None, "de", "de", dummy_style(), false);
    assert_eq!(m_de.opts.page_width_mm, A4_W);
    assert_eq!(m_de.opts.page_height_mm, A4_H);

    let m_us = parse_cover_letter("x", None, None, "us", "en", dummy_style(), false);
    assert_eq!(m_us.opts.page_width_mm, LETTER_W);
    assert_eq!(m_us.opts.page_height_mm, LETTER_H);
}

#[test]
fn de_market_opts_carry_subject_label_and_date_position() {
    let m = parse_cover_letter("x", None, None, "de", "de", dummy_style(), false);
    assert!(m.opts.subject_line_used);
    assert_eq!(m.opts.subject_line_label, "Betreff");
    assert_eq!(m.opts.date_position, "top-right");
}

#[test]
fn body_rich_text_preserves_bold() {
    let letter = "\
Alice
alice@example.com

Jan 1, 2025

Hiring Manager
FooCo

Dear Hiring Manager,

I **significantly** improved our pipeline. Results were **outstanding**.

Sincerely,
Alice
";
    let model = parse_cover_letter(
        letter,
        None,
        Some("Alice"),
        "us",
        "en",
        dummy_style(),
        false,
    );
    assert!(!model.body.is_empty(), "body must not be empty");
    // At least one run in the body should be bold
    let has_bold = model.body.iter().any(|para| para.iter().any(|r| r.bold));
    assert!(has_bold, "bold runs should survive in body paragraphs");
}

#[test]
fn strip_md_links_removes_link_syntax() {
    let input =
        "Check out [GitHub](https://github.com/user) and [LinkedIn](https://linkedin.com/in/user).";
    let result = strip_md_links(input);
    assert!(result.contains("GitHub"), "label should remain");
    assert!(result.contains("LinkedIn"), "label should remain");
    assert!(!result.contains("https://"), "URL should be stripped");
}

#[test]
fn unknown_market_falls_back_gracefully() {
    // Should not panic; intl baseline applies
    let model = parse_cover_letter("x", None, None, "zz", "en", dummy_style(), false);
    assert_eq!(model.opts.page_width_mm, A4_W); // intl uses A4
}

/// `ats` reaches `data.opts` verbatim — the whole ATS degradation story for
/// every layout hangs off this one field being threaded, and an ATS toggle
/// that silently fails to reach the renderer looks exactly like one that
/// works.
#[test]
fn ats_flag_reaches_letter_opts() {
    let on = parse_cover_letter("x", None, None, "us", "en", dummy_style(), true);
    let off = parse_cover_letter("x", None, None, "us", "en", dummy_style(), false);
    assert!(on.opts.ats, "ats=true must reach data.opts.ats");
    assert!(!off.opts.ats, "ats=false must reach data.opts.ats");
}

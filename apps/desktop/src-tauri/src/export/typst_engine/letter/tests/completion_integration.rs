//! End-to-end: letter_shape::complete_letter_text output re-parsed by parse_cover_letter.

use super::super::parse_cover_letter;
use super::support::*;

//    straight into this parser ──────────────────────────────────────────

/// The end-to-end fix for the reported bug: a body-only letter with no
/// salutation completed by [`crate::export::letter_shape::complete_letter_text`]
/// then parsed here must flip `body_started` at the injected salutation,
/// so the **bold** paragraph is tokenized into `body` as a rich-text run
/// — NOT space-joined into `recipient_lines` as flattened plain text,
/// which is what happened before Stage 1: with no salutation in the text,
/// `body_started` never flipped, so every paragraph fell into the
/// pre-salutation branch.
#[test]
fn completed_body_only_letter_keeps_bold_as_a_rich_run_in_body() {
    let body = "I reduced deploy time by **40 percent** using Jest.";
    let completed = crate::export::letter_shape::complete_letter_text(body, "us", "Jane Smith");

    let model = parse_cover_letter(
        &completed,
        None,
        Some("Jane Smith"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert!(
        model.recipient_lines.is_empty(),
        "the body paragraph must not have fallen into recipient_lines: {:?}",
        model.recipient_lines
    );
    let bold_run_present = model
        .body
        .iter()
        .flatten()
        .any(|run| run.bold && run.text.contains("40 percent"));
    assert!(
        bold_run_present,
        "the **bold** run must survive as a rich-text run in body: {:?}",
        model.body
    );
}

//    the model open a market letter with its OWN subject line and/or date
//    before the body; `complete_letter_text` must insert the salutation
//    AFTER that furniture, or these lines become unreachable pre-body
//    classification once `parse_cover_letter` flips `body_started` at a
//    salutation sitting ahead of them. ────────────────────────────────

/// The reported repro: a `Betreff:` line the model wrote per
/// `letter_system`'s subject rule must reach `model.subject`, not become
/// an extra body paragraph.
#[test]
fn completed_letter_keeps_a_leading_subject_line_out_of_the_body() {
    let body = "Betreff: Bewerbung als Software Engineer\n\n\
                 Ich bringe sechs Jahre Erfahrung mit.\n\n\
                 In meiner bisherigen Tätigkeit konnte ich viel lernen.";
    let completed = crate::export::letter_shape::complete_letter_text(body, "de", "Max Müller");

    let model = parse_cover_letter(
        &completed,
        None,
        Some("Max Müller"),
        "de",
        "de",
        dummy_style(),
        false,
    );

    assert!(model.subject.is_some(), "got: {:?}", model.subject);
    assert!(
        model
            .subject
            .as_deref()
            .unwrap()
            .to_lowercase()
            .starts_with("betreff"),
        "got: {:?}",
        model.subject
    );
    assert!(model.salutation.is_some());
    assert_eq!(
        model.body.len(),
        2,
        "the subject line must not have become a third body paragraph: {:?}",
        model.body
    );
}

/// Same repro shape with a leading DATE line instead of a subject: the
/// date must reach `model.date`, not the body.
#[test]
fn completed_letter_keeps_a_leading_date_line_out_of_the_body() {
    let body = "Frankfurt, 12. Januar 2025\n\nIch bringe sechs Jahre Erfahrung mit.";
    let completed = crate::export::letter_shape::complete_letter_text(body, "de", "Max Müller");

    let model = parse_cover_letter(
        &completed,
        None,
        Some("Max Müller"),
        "de",
        "de",
        dummy_style(),
        false,
    );

    assert!(model.date.is_some(), "got: {:?}", model.date);
    assert_eq!(
        model.body.len(),
        1,
        "the date line must not have become body prose: {:?}",
        model.body
    );
}

/// Both kinds of furniture together, in either order — `parse_cover_letter`
/// classifies each pre-body line independently, so order must not matter.
#[test]
fn completed_letter_keeps_subject_and_date_furniture_out_of_the_body_either_order() {
    let subject_then_date = "Betreff: Bewerbung als Software Engineer\n\n\
                              12. Januar 2025\n\nIch bringe Erfahrung mit.";
    let completed =
        crate::export::letter_shape::complete_letter_text(subject_then_date, "de", "Max Müller");
    let model = parse_cover_letter(
        &completed,
        None,
        Some("Max Müller"),
        "de",
        "de",
        dummy_style(),
        false,
    );
    assert!(model.subject.is_some(), "got: {:?}", model.subject);
    assert!(model.date.is_some(), "got: {:?}", model.date);
    assert_eq!(model.body.len(), 1, "got: {:?}", model.body);

    let date_then_subject = "12. Januar 2025\n\nBetreff: Bewerbung als Software Engineer\n\n\
                              Ich bringe Erfahrung mit.";
    let completed2 =
        crate::export::letter_shape::complete_letter_text(date_then_subject, "de", "Max Müller");
    let model2 = parse_cover_letter(
        &completed2,
        None,
        Some("Max Müller"),
        "de",
        "de",
        dummy_style(),
        false,
    );
    assert!(model2.subject.is_some(), "got: {:?}", model2.subject);
    assert!(model2.date.is_some(), "got: {:?}", model2.date);
    assert_eq!(model2.body.len(), 1, "got: {:?}", model2.body);
}

/// `us` never has the model write a subject line — a plain body-only
/// letter for that market must be unaffected: the salutation still opens
/// the letter, and no phantom subject/date gets invented.
#[test]
fn completed_letter_for_a_market_without_subject_line_convention_is_unaffected() {
    let body = "I am writing to express my interest in the Software Engineer role.";
    let completed = crate::export::letter_shape::complete_letter_text(body, "us", "Jane Smith");

    let model = parse_cover_letter(
        &completed,
        None,
        Some("Jane Smith"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert_eq!(model.salutation.as_deref(), Some("Dear Hiring Manager,"));
    assert!(model.subject.is_none());
    assert!(model.date.is_none());
    assert_eq!(model.body.len(), 1, "got: {:?}", model.body);
}

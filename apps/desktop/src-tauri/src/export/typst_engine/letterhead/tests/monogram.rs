//! looks_like_date + monogram_initials + letterhead_initials device tests.

use super::super::super::letter::parse_cover_letter;
use super::super::*;
use super::support::*;

#[test]
fn looks_like_date_recognises_common_formats() {
    assert!(looks_like_date("June 2, 2025"));
    assert!(looks_like_date("2. Juni 2025"));
    assert!(looks_like_date("02/06/2025"));
    assert!(looks_like_date("2025-06-02"));
    assert!(!looks_like_date("Dear Hiring Manager,"));
    assert!(!looks_like_date("Acme Corp"));
}

/// The production incident this guards: a body paragraph mentioning a
/// percentage and ending in a full stop satisfies the digit+separator
/// heuristic line-for-line, but it is prose, never a date.
#[test]
fn looks_like_date_rejects_a_long_prose_paragraph_with_digits_and_periods() {
    let prose = "Durch die Einführung von Jest konnte ich die Testabdeckung \
                  von 0 % auf 90 % steigern und die Fehlerquote deutlich senken.";
    assert!(
        !looks_like_date(prose),
        "a prose paragraph must never read as a date"
    );
}

//
// Pure string logic behind `letter_monogram.typ`'s device (and its DOCX
// shaded-run approximation, which calls the SAME function). Every case here
// is one a `.typ` could not be tested on.

#[test]
fn monogram_initials_takes_the_first_and_last_name_tokens() {
    assert_eq!(monogram_initials("Jane Smith"), "JS");
    // First + LAST, not the first two — a `.take(2)` implementation returns
    // "JV" here, which is the wrong monogram for a multi-part surname.
    assert_eq!(monogram_initials("Jane van der Berg"), "JB");
    assert_eq!(monogram_initials("Mary Jane Watson Parker"), "MP");
}

#[test]
fn monogram_initials_uppercases_and_survives_non_ascii_capitals() {
    assert_eq!(monogram_initials("àlvaro èsposito"), "ÀÈ");
    assert_eq!(monogram_initials("Àlvaro Èsposito"), "ÀÈ");
    // `char::to_uppercase` expands ß to "SS"; only the first char is taken so
    // the fixed-size device still holds exactly two glyphs.
    assert_eq!(monogram_initials("ßiggi ßmith").chars().count(), 2);
}

#[test]
fn monogram_initials_handles_mononyms_and_letterless_tokens() {
    assert_eq!(monogram_initials("Prince"), "P");
    assert_eq!(monogram_initials("O'Brien"), "O");
}

/// A pronoun parenthetical is not a name token.
///
/// The fixture is `(they/them)`, NOT `(she/her)`: with "she" the expected
/// `JS` is also what the BROKEN implementation produces, because its `S`
/// comes from "she" — the test passed against the defect. `they` makes the
/// two outcomes distinguishable, `JS` (correct) vs `JT` (searched the token
/// for its first alphanumeric instead of requiring a leading one).
#[test]
fn monogram_initials_drops_pronoun_parentheticals() {
    assert_eq!(monogram_initials("Jane Smith (they/them)"), "JS");
    assert_eq!(monogram_initials("Jane (they/them) Smith"), "JS");
    assert_eq!(monogram_initials("Jane (they/them)"), "J");
    // Trailing em-dash / bullet decoration must not become an initial either.
    assert_eq!(monogram_initials("Jane Smith —"), "JS");
}

/// Titles and qualifications are not names. A monogram for "Dr. Jane Smith"
/// is JS; DS is the doctorate's initial standing in for the given name.
#[test]
fn monogram_initials_drops_titles_and_qualifications() {
    assert_eq!(monogram_initials("Dr. Jane Smith"), "JS");
    assert_eq!(monogram_initials("Prof. Dr. Jane Smith"), "JS");
    // The German honorific the critic named — "DM" was the defect.
    assert_eq!(monogram_initials("Dipl.-Ing. Max Müller"), "MM");
    assert_eq!(monogram_initials("Jane Smith Ph.D."), "JS");
}

/// …but a SINGLE-letter initial is part of the name and keeps counting: the
/// title rule keys on "two or more letters before the period", so "J." is
/// not swept up with "Dr.". Dropping it would make "J. Smith" render "S".
#[test]
fn monogram_initials_keeps_single_letter_initials() {
    assert_eq!(monogram_initials("J. Smith"), "JS");
    assert_eq!(monogram_initials("Jane M. Smith"), "JS");
}

/// A letterhead-less letter parses to an empty name; the device must then
/// render nothing rather than an empty tinted square.
#[test]
fn monogram_initials_is_empty_for_a_nameless_letterhead() {
    assert_eq!(monogram_initials(""), "");
    assert_eq!(monogram_initials("   \t "), "");
    assert_eq!(monogram_initials("--- ***"), "");
}

/// Never more than two glyphs, whatever the name — the `.typ` device is a
/// fixed-size square and a third initial would overflow it.
#[test]
fn monogram_initials_never_exceeds_two_characters() {
    for name in [
        "A B C D E F",
        "Jane Smith",
        "Prince",
        "Maria del Carmen Fernández de la Vega",
        "",
    ] {
        assert!(
            monogram_initials(name).chars().count() <= 2,
            "{name:?} produced more than two initials"
        );
    }
}

/// The parser must publish the initials on the letterhead — the `.typ` reads
/// `data.letterhead.initials`, so a model that omits them silently renders an
/// empty device.
#[test]
fn parsed_letterhead_carries_the_monogram_initials() {
    let model = parse_cover_letter(
        EN_LETTER,
        None,
        Some("Jane Smith"),
        "us",
        "en",
        dummy_style(),
        false,
    );
    assert_eq!(model.letterhead.initials, "JS");
}

/// A letterhead-less letter with no candidate name falls back to the first
/// LINE, which is the salutation — so the device read "DM", from "Dear
/// Hiring Manager,". Three renderer call sites pass an empty
/// `candidate_name`, so this is reachable, not theoretical.
///
/// `meta_name: Some("")` rather than `None` on purpose: that is the shape
/// the renderer actually sends, and `parse_cover_letter` filters it to the
/// same fallback.
#[test]
fn no_device_initials_when_the_name_falls_back_to_a_salutation() {
    let letterhead_less = "Dear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    for meta in [None, Some("")] {
        let model = parse_cover_letter(
            letterhead_less,
            None,
            meta,
            "us",
            "en",
            dummy_style(),
            false,
        );
        assert_eq!(
            model.letterhead.initials, "",
            "meta_name={meta:?}: the monogram device must be empty when the name is really \
             the salutation — it rendered \"DM\" from \"Dear Hiring Manager,\""
        );
    }
}

/// A DATE opening is the fourth kind, and the one BOTH formats missed: the
/// DOCX line filter excluded salutation/sign-off/subject and nothing else,
/// so a letter starting "12 March 2025" put `12` in the device.
#[test]
fn no_device_initials_when_the_letter_opens_with_a_date() {
    for opening in ["12 March 2025", "2. Juni 2025", "02/06/2025", "2025-06-02"] {
        for meta in [None, Some("")] {
            let model = parse_cover_letter(
                &format!("{opening}\n\nDear Hiring Manager,\n\nBody.\n\nSincerely,\n"),
                None,
                meta,
                "us",
                "en",
                dummy_style(),
                false,
            );
            assert_eq!(
                model.letterhead.initials, "",
                "{opening:?} (meta={meta:?}) is a date, not a name — the device must be empty"
            );
        }
    }
}

/// Belt to the date guard's braces: a digit can never BE an initial, so even
/// a numeric opening the date heuristic does not recognise cannot produce
/// one. `is_name_token` requires a leading LETTER, not merely alphanumeric.
#[test]
fn monogram_initials_ignores_numeric_tokens() {
    assert_eq!(monogram_initials("12 March 2025"), "M");
    assert_eq!(monogram_initials("2025"), "");
    assert_eq!(monogram_initials("42 Jane Smith 99"), "JS");
}

/// Same guard for the other two opening kinds the DOCX renderer already
/// refuses to treat as a name.
#[test]
fn no_device_initials_for_a_signoff_or_subject_opening() {
    for opening in [
        "Mit freundlichen Grüßen,",
        "Betreff: Bewerbung als Entwickler",
    ] {
        let model = parse_cover_letter(
            &format!("{opening}\n\nBody text here.\n"),
            None,
            None,
            "de",
            "de",
            dummy_style(),
            false,
        );
        assert_eq!(
            model.letterhead.initials, "",
            "{opening:?} is not a name; the device must stay empty"
        );
    }
}

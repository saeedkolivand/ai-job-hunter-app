//! is_letterhead_name + letterhead-name suppression + resolve_letterhead_candidate tests.

use super::super::super::letter::parse_cover_letter;
use super::super::*;
use super::support::*;

//
// The device guard above only ever hid the monogram square. `letterhead.name`
// itself — and `signature_name`, the identical value used under the
// sign-off — was never guarded, so every one of the six `.typ` layouts
// rendered "12 March 2025" or "Dear Hiring Manager," as the person's name
// whenever no candidate name was supplied.

/// `is_letterhead_name` — the shared predicate behind both guards.
#[test]
fn is_letterhead_name_accepts_real_names_and_refuses_non_name_openings() {
    for real in ["Jane Smith", "Àlvaro Èsposito", "Prince", "J. Smith"] {
        assert!(is_letterhead_name(real), "{real:?} should read as a name");
    }
    for not_a_name in [
        "",
        "   ",
        "Dear Hiring Manager,",
        "Mit freundlichen Grüßen,",
        "Betreff: Bewerbung als Entwickler",
        "12 March 2025",
        "2. Juni 2025",
        "02/06/2025",
        "2025-06-02",
    ] {
        assert!(
            !is_letterhead_name(not_a_name),
            "{not_a_name:?} must not read as a name"
        );
    }
}

/// The shape cap: a long prose line (the fallback's fifth failure mode,
/// past salutation/sign-off/subject/date) is not a name, but a real long
/// name — the longest one in this suite — must still pass.
#[test]
fn is_letterhead_name_rejects_a_long_prose_line_but_keeps_a_real_long_name() {
    let prose: String = (0..40).map(|_| "word").collect::<Vec<_>>().join(" ");
    assert!(
        !is_letterhead_name(&prose),
        "a 40-word line must not read as a name"
    );

    assert!(
        is_letterhead_name("Maria del Carmen Fernández de la Vega"),
        "a real long name (7 tokens, 37 chars) must still read as a name"
    );
}

/// With no candidate name, a date-opening letter must not fabricate a
/// letterhead/signature name from the date — AND the date itself must not
/// be lost. Before this guard, `name_text` fell back to "12 March 2025",
/// which (a) rendered as the name, and (b) matched the header-dedupe skip
/// below verbatim, so the date line was silently swallowed as a duplicate
/// header echo and never reached `model.date` at all.
#[test]
fn letterhead_name_suppressed_for_date_opening_and_date_still_captured() {
    let letter =
        "12 March 2025\n\nDear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    for meta in [None, Some("")] {
        let model = parse_cover_letter(letter, None, meta, "us", "en", dummy_style(), false);

        assert_eq!(
            model.letterhead.name, "",
            "meta={meta:?}: a date opening must not become the letterhead name; got {:?}",
            model.letterhead.name
        );
        assert_eq!(
            model.signature_name, "",
            "meta={meta:?}: the signature block must not fabricate a name from the date"
        );
        assert_eq!(
            model.date.as_deref(),
            Some("12 March 2025"),
            "meta={meta:?}: the date line must still be captured, not dropped as a header echo"
        );
        assert_eq!(
            model.salutation.as_deref(),
            Some("Dear Hiring Manager,"),
            "meta={meta:?}: the salutation must still render normally"
        );
    }
}

/// Same suppression for a salutation-opening letterhead-less letter — the
/// PDF-side counterpart of the DOCX
/// `letterhead_less_letter_keeps_its_salutation_and_body` regression.
#[test]
fn letterhead_name_suppressed_for_salutation_opening_and_salutation_still_captured() {
    let letter = "Dear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    for meta in [None, Some("")] {
        let model = parse_cover_letter(letter, None, meta, "us", "en", dummy_style(), false);

        assert_eq!(
            model.letterhead.name, "",
            "meta={meta:?}: a salutation opening must not become the letterhead name"
        );
        assert_eq!(model.signature_name, "");
        assert_eq!(
            model.salutation.as_deref(),
            Some("Dear Hiring Manager,"),
            "meta={meta:?}: the salutation must still be captured as the salutation"
        );
    }
}

/// Guard the guard: a REAL candidate name must still reach the letterhead
/// and signature untouched — `is_letterhead_name` must not become
/// overzealous and start suppressing legitimate names.
#[test]
fn a_real_candidate_name_is_never_suppressed() {
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
    assert_eq!(model.signature_name, "Jane Smith");
}

//
// CodeRabbit round 1, item 1 (MAJOR, verified before fixing): both DOCX
// line-scanners resolved `candidate_name` via `.unwrap_or(&clean)` with no
// empty-string filter, so `Some("")` — the shape three renderer call sites
// actually send — won over a REAL name on the letter's own first line,
// where the PDF parser (which already filtered) fell through correctly.
// These test the extracted helper directly, the cheapest point to catch a
// regression at — the DOCX integration test
// (`empty_candidate_name_does_not_suppress_a_real_first_line_name`) is the
// one that would have caught the ORIGINAL bug end-to-end.

#[test]
fn resolve_letterhead_candidate_prefers_a_real_meta_name() {
    assert_eq!(
        resolve_letterhead_candidate(Some("Jane Smith"), || "fallback"),
        "Jane Smith"
    );
}

#[test]
fn resolve_letterhead_candidate_falls_through_on_none() {
    assert_eq!(
        resolve_letterhead_candidate(None, || "fallback"),
        "fallback"
    );
}

/// The exact shape the bug was in: `Some("")` (and whitespace-only) must
/// behave identically to `None`, not win as if it were a real name.
#[test]
fn resolve_letterhead_candidate_treats_blank_some_like_none() {
    for blank in [Some(""), Some("   "), Some("\t")] {
        assert_eq!(
            resolve_letterhead_candidate(blank, || "fallback"),
            "fallback",
            "{blank:?} must fall through to the fallback, exactly like None"
        );
    }
}

#[test]
fn resolve_letterhead_candidate_trims_a_real_name() {
    assert_eq!(
        resolve_letterhead_candidate(Some("  Jane Smith  "), || "fallback"),
        "Jane Smith"
    );
}

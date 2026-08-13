//! Letterhead name-guard family — split out of `letter.rs` (verbatim, not
//! trimmed) to stay under the R8 module-size cap.
//!
//! [`is_letterhead_name`] is the single shared predicate behind two guards
//! that must never drift apart:
//! - [`letterhead_initials`] — refuses to derive a monogram DEVICE from
//!   something that isn't a name.
//! - `letter::parse_cover_letter` — refuses to publish the NAME TEXT itself
//!   (`data.letterhead.name` / `signature_name`) when it isn't a name. Every
//!   `.typ` layout reads that one field, so guarding it there too — not just
//!   the device — is what makes all six layouts degrade the same way.
//!
//! `export/docx/mod.rs`'s two line-scanners (DOCX has no shared `LetterModel`
//! to funnel through) call [`is_letterhead_name`] directly too, so PDF and
//! DOCX can never disagree about which openings are not names.

/// Lazy date-pattern regex — matches month names or 4-digit years.
///
/// `pub(super)` (not private): `letter::parse_cover_letter` also calls this
/// directly, for the pre-salutation date/recipient dispatch — not just via
/// [`is_letterhead_name`] below.
pub(super) fn looks_like_date(s: &str) -> bool {
    // Matches lines that contain digits and common date separators, e.g.:
    //   "June 2, 2025" / "2. Juni 2025" / "02/06/2025" / "2025-06-02"
    //   "2 juin 2025" / "le 2 juin 2025"
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    let has_digit = t.chars().any(|c| c.is_ascii_digit());
    if !has_digit {
        return false;
    }
    // Must contain a year-like 4-digit run or a separator ( / . - space)
    // alongside a digit to distinguish from plain phone numbers or IDs.
    let has_year = t.split_whitespace().any(|w| {
        let digits: String = w.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.len() == 4
    });
    let has_sep = t.contains('/') || t.contains('.') || t.contains('-');
    has_year || (has_digit && has_sep)
}

/// Up to **two** uppercase initials for a letterhead monogram device: the
/// initial of the first NAME token and the initial of the last one.
///
/// Derived in Rust rather than in Typst because every interesting case is string
/// handling a `.typ` cannot be tested on: a mononym ("Prince" → `P`), a
/// multi-part surname ("Jane van der Berg" → `JB`, first + LAST, not the first
/// two), and non-ASCII capitals ("Àlvaro Èsposito" → `ÀÈ`, which must survive
/// PDF extraction like every other accented capital in this engine).
///
/// Two kinds of token are **not** names and are dropped:
///
/// 1. Anything that does not START with a LETTER — a pronoun parenthetical,
///    "—", a stray bullet, or a number. The first version searched each token
///    for its first alphanumeric *anywhere*, so "(they/them)" contributed a
///    `T`; "Jane Smith (they/them)" came out `JT`. Alphanumeric was still too
///    loose: an initial is never a digit, so "12 March 2025" offered `12`
///    as a monogram. Letters only.
/// 2. A token that abbreviates a WORD — two or more letters before its period
///    ("Dr.", "Prof.", "Dipl.-Ing.", "Ph.D."). Those are titles and
///    qualifications; "Dr. Jane Smith" is `JS`, and the German
///    "Dipl.-Ing. Max Müller" is `MM`, not `DM`. A SINGLE-letter initial keeps
///    its period and still counts, so "J. Smith" is `JS` rather than `S`.
///
/// Never longer than two characters, so the device is a fixed-size square no
/// matter how long the name is — a third initial would overflow it. Returns an
/// empty string for a nameless letterhead; the template skips the device then.
fn monogram_initials(name: &str) -> String {
    /// Is this token a person's name, as opposed to punctuation, a number or a
    /// title?
    fn is_name_token(tok: &str) -> bool {
        if !tok.starts_with(char::is_alphabetic) {
            return false;
        }
        // Letters before the first period: 1 is an initial ("J."), 2+ is a
        // word abbreviation ("Dr.", "Dipl.-Ing."). No period at all → a name.
        match tok.split_once('.') {
            Some((head, _)) => head.chars().count() < 2,
            None => true,
        }
    }

    let mut initials = name
        .split_whitespace()
        .filter(|tok| is_name_token(tok))
        // Guaranteed `Some` — `is_name_token` required a leading letter.
        .filter_map(|tok| tok.chars().next());

    let Some(first) = initials.next() else {
        return String::new();
    };
    // `to_uppercase` can expand (ß → SS); take one char so the device stays
    // exactly one glyph per initial.
    let up = |c: char| c.to_uppercase().next().unwrap_or(c);

    // `next_back`, not `last`: the iterator is double-ended, and `first` has
    // already been consumed, so this is the last REMAINING token — a mononym
    // therefore yields `None` here rather than re-finding its own initial.
    match initials.next_back() {
        Some(last) => [up(first), up(last)].iter().collect(),
        None => up(first).to_string(),
    }
}

/// Is `s` plausibly a person's NAME, as opposed to one of the other things a
/// "first non-blank line" fallback can pick up by accident: a salutation, a
/// sign-off, a subject/reference line, or a date opening?
///
/// The single shared rule behind two guards that must never drift apart:
/// - [`letterhead_initials`] — refuses to derive a monogram DEVICE from
///   something that isn't a name.
/// - `letter::parse_cover_letter` — refuses to publish the NAME TEXT itself
///   (`data.letterhead.name` / `signature_name`) when it isn't a name. Every
///   `.typ` layout reads that one field, so guarding it here — not just the
///   device — is what makes all six layouts degrade the same way.
///
/// `export/docx/mod.rs`'s two line-scanners (DOCX has no shared `LetterModel`
/// to funnel through) call this directly too, so PDF and DOCX can never
/// disagree about which openings are not names.
///
/// A DATE is the opening the earlier salutation/sign-off/subject-only version
/// of this check missed: a letter whose first line is "12 March 2025" is
/// none of those three, so it passed as "a name" and produced `12` as a
/// monogram (and, before the `parse_cover_letter` guard existed, rendered
/// "12 March 2025" as the person's name in every layout's header).
pub(crate) fn is_letterhead_name(s: &str) -> bool {
    use crate::locale::letter::{is_salutation, is_signoff, is_subject_line};
    let t = s.trim();
    !t.is_empty()
        && !is_salutation(t)
        && !is_signoff(t)
        && !is_subject_line(t)
        && !looks_like_date(t)
}

/// Initials for the letterhead device, or empty when the "name" is not a name.
///
/// The letterhead name falls back to the first non-blank LINE of the letter when
/// no candidate name is supplied — and three renderer call sites pass an empty
/// `candidate_name`, so that fallback is reachable in production. On a
/// letterhead-less letter the first line is the salutation, which made the
/// device read `DM`, from "Dear Hiring Manager,".
///
/// **Both renderers call THIS function** — the DOCX path used to call
/// [`monogram_initials`] directly, which is how the two drifted in the first
/// place. One guard, one place; a fifth opening kind gets added once, in
/// [`is_letterhead_name`].
pub(crate) fn letterhead_initials(name_text: &str) -> String {
    if !is_letterhead_name(name_text) {
        return String::new();
    }
    monogram_initials(name_text)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::letter::{parse_cover_letter, LetterStyle};
    use super::*;

    fn dummy_style() -> LetterStyle {
        LetterStyle {
            c_accent: "#2563EB".to_string(),
            c_body: "#222222".to_string(),
            c_name: "#111111".to_string(),
            c_date: "#555555".to_string(),
            c_rule: "#aaaaaa".to_string(),
            font_name: "Carlito".to_string(),
            font_body: "Carlito".to_string(),
            name_pt: 20.0,
            body_pt: 10.5,
        }
    }

    const EN_LETTER: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street
New York, NY 10001

Dear Hiring Manager,

I am writing to express my interest in the Software Engineer position at Acme Corp. \
I have five years of experience building distributed systems.

During my time at Beta Inc, I led the migration of our payments service, reducing \
latency by 40 percent and cutting costs by 30 percent.

I would welcome the opportunity to discuss how my background aligns with your needs.

Sincerely,

Jane Smith
Software Engineer
";

    #[test]
    fn looks_like_date_recognises_common_formats() {
        assert!(looks_like_date("June 2, 2025"));
        assert!(looks_like_date("2. Juni 2025"));
        assert!(looks_like_date("02/06/2025"));
        assert!(looks_like_date("2025-06-02"));
        assert!(!looks_like_date("Dear Hiring Manager,"));
        assert!(!looks_like_date("Acme Corp"));
    }

    // ── monogram_initials ─────────────────────────────────────────────────────
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
        let letterhead_less =
            "Dear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
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

    // ── letterhead NAME suppression (not just the device) ────────────────────
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
}

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
/// `pub(in crate::export)` (not private): `letter::parse_cover_letter` also
/// calls this directly, for the pre-salutation date/recipient dispatch, not
/// just via [`is_letterhead_name`] below — and
/// `export::letter_shape::complete_letter_text` (sibling `export` module,
/// re-exported through `typst_engine`'s `mod.rs`) needs the SAME heuristic so
/// the completion step and the parser never disagree about what counts as a
/// date line. Scoped to `crate::export` rather than the whole crate: every
/// consumer lives under this module tree, so that is the true minimum.
pub(in crate::export) fn looks_like_date(s: &str) -> bool {
    // Matches lines that contain digits and common date separators, e.g.:
    //   "June 2, 2025" / "2. Juni 2025" / "02/06/2025" / "2025-06-02"
    //   "2 juin 2025" / "le 2 juin 2025"
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    // A date is never a paragraph. Reject a long line BEFORE the digit +
    // separator heuristic below even gets to run: a prose sentence that
    // happens to mention a percentage and end in a full stop ("…von 0 % auf
    // 90 %. Durch die Einführung von Jest…") satisfies "has a digit and a
    // `.`/`/`/`-`" exactly like a real date does — that shape is what let a
    // whole body paragraph get classified as `data.date` in production. The
    // caps are set well above the longest realistic date string in any
    // supported market, including German's optional weekday-prefixed form
    // ("Donnerstag, den 2. Januar 2025" — 5 tokens / 31 chars).
    if t.chars().count() > MAX_DATE_CHARS || t.split_whitespace().count() > MAX_DATE_TOKENS {
        return false;
    }
    let has_digit = t.chars().any(|c| c.is_ascii_digit());
    if !has_digit {
        return false;
    }
    // Must contain a year-like 4-digit run or a separator ( / . - space)
    // alongside a digit to distinguish from plain phone numbers or IDs. The
    // `has_digit` guard above already proved a digit is present by this
    // point, so the rule really is just "a year, or a separator" — the
    // stale `has_digit &&` on the old final expression was always true and
    // said nothing.
    let has_year = t.split_whitespace().any(|w| {
        let digits: String = w.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.len() == 4
    });
    let has_sep = t.contains('/') || t.contains('.') || t.contains('-');
    has_year || has_sep
}

/// Maximum character length a date line may have — see [`looks_like_date`]'s
/// doc comment for why this guard exists.
const MAX_DATE_CHARS: usize = 48;
/// Maximum whitespace-separated tokens a date line may have — same reasoning.
const MAX_DATE_TOKENS: usize = 8;

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
///
/// A SHAPE cap is the fifth guard: none of the four rejections above fire on
/// a plain prose paragraph, so when the "first non-blank line" fallback
/// landed on a 380-character body paragraph (no salutation/sign-off/subject/
/// date phrasing at all — just prose), it passed every check and rendered as
/// the candidate's name, verbatim, in the letterhead AND the signature block.
/// A person's name is short: [`MAX_NAME_CHARS`]/[`MAX_NAME_TOKENS`] are set
/// generously above the longest real name in this file's own test suite
/// ("Maria del Carmen Fernández de la Vega" — 7 tokens / 37 chars, the
/// `a_real_candidate_name_is_never_suppressed` test still passes), while a
/// prose paragraph is reliably an order of magnitude past both.
pub(crate) fn is_letterhead_name(s: &str) -> bool {
    use crate::locale::letter::{is_salutation, is_signoff, is_subject_line};
    let t = s.trim();
    !t.is_empty()
        && !is_salutation(t)
        && !is_signoff(t)
        && !is_subject_line(t)
        && !looks_like_date(t)
        && t.chars().count() <= MAX_NAME_CHARS
        && t.split_whitespace().count() <= MAX_NAME_TOKENS
}

/// Maximum character length a person's name may have — see
/// [`is_letterhead_name`]'s doc comment for why this guard exists.
const MAX_NAME_CHARS: usize = 64;
/// Maximum whitespace-separated tokens a person's name may have — same
/// reasoning.
const MAX_NAME_TOKENS: usize = 8;

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

/// Resolve the candidate name used for the letterhead: prefer `meta_name`,
/// but only when it is non-blank — an empty-string `Some("")` (the shape
/// three renderer call sites actually send when no candidate name is known)
/// must fall through to `fallback` exactly like a real `None`.
///
/// Without this, `Some("").unwrap_or(fallback)` returns `""`, not
/// `fallback` — `Some` is not `None`, so a plain `unwrap_or`/`.or()` chain
/// never reaches the fallback at all. That is precisely the shape CodeRabbit
/// caught: both DOCX line-scanners resolved `candidate_name` this way
/// (`meta.and_then(...).map(...).unwrap_or(&clean)`) with no empty-string
/// filter, so a nameless request (`candidate_name: Some("")`) whose letter
/// legitimately opened with a real name suppressed that name in DOCX while
/// the PDF parser — which already filtered — still rendered it. One shared
/// helper, so the PDF parser (`letter::parse_cover_letter`) and both DOCX
/// line-scanners (`export/docx/mod.rs`) can never drift on this decision
/// again — the same posture as [`is_letterhead_name`] above.
///
/// `fallback` is lazy (`FnOnce`, not a plain `&str`) so a caller whose
/// fallback costs more than a field read — the PDF parser searches
/// `raw_lines` for the first non-blank one — only pays for it when
/// `meta_name` doesn't already win.
pub(crate) fn resolve_letterhead_candidate<'a>(
    meta_name: Option<&'a str>,
    fallback: impl FnOnce() -> &'a str,
) -> &'a str {
    match meta_name {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => fallback(),
    }
}

#[cfg(test)]
mod tests;

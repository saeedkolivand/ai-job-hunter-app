//! Small text helpers shared by the validators: contact-line shape tests,
//! sentence splitting and similarity.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::date_spans;
use crate::validate::EMAIL_RE;

/// A phone number as a résumé HEADER actually writes one — the single shape
/// test behind every "is this line contact details?" question in this module.
///
/// Deliberately stricter than `export::parser::PHONE_RE`
/// (`\+?\d[\d\s\-().]{7,}`), which accepts any seven-character run of digits,
/// spaces, hyphens, dots and parens. That rule is right where it lives: the
/// parser only has to decide which BAND a header line belongs to, and
/// over-matching costs it nothing. It is wrong on both surfaces here, where the
/// answer decides whether a user is accused of something or spared a check —
/// ordinary numeric prose satisfies it constantly ("150 - 200 EUR per hour",
/// "90 000 - 110 000"), which made a salary range a second contact block
/// (`ats.header_in_body`, a Critical) and let a letter paragraph quoting a rate
/// range exempt itself from the fabricated-metric pass.
///
/// Two accepted forms, between them covering the header formats this pipeline's
/// own fixtures use in `en` and `de`:
///
/// 1. an explicit international/area-code marker — a leading `+` or `(`
///    followed by digits (`+49 30 1234567`, `+49 (0)30 1234567`,
///    `(030) 12345678`, `+1 (555) 123-4567`);
/// 2. failing that, an unbroken run of seven or more digits — the local part of
///    a German number written without a marker (`030 1234567`,
///    `0176 12345678`).
///
/// A grouped figure carries at most three digits per group and no marker, so it
/// matches neither. The cost is a MISSED match on a bare US-style number with no
/// parentheses ("555-123-4567", longest run four): accepted, because a header
/// block essentially always carries an email as well, which every caller tests
/// separately, and because both callers would rather miss than over-reach.
static HEADER_PHONE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[+(]\s*\d[\d\s\-./()]{5,}\d|\d{7,}").unwrap());

/// Whether `text` carries a header-shaped phone number **and no date span**. See
/// [`HEADER_PHONE_RE`]; `pub(crate)` so `ats::is_contact_cluster` and
/// [`has_real_contact_match`] cannot drift apart again.
///
/// ## Why the span test is part of the SHAPE, not of a call site
///
/// The `[+(]` arm matches a parenthesized DATE SPAN exactly as readily as an
/// area code: `(2019 - 2021)` is `(`, a digit, eight separator-or-digit
/// characters and a digit. `ats::is_contact_cluster` carried its own guard
/// against precisely that, and the other caller — [`has_real_contact_match`] —
/// did not, so the two answers to one question disagreed. A pre-heading line
/// carrying a span ("Contract work (2019 - 2021): 1 200 000 EUR in payment
/// volume") was read as contact details and struck out of the SOURCE's metric
/// set, and restating its figure came back as a fabrication Critical. One
/// helper, one guard, both callers.
///
/// ## Why the guard is [`date_spans`] and not "carries a year"
///
/// The statement being made is *a date span is not a phone number*, and the
/// first cut of it tested for a bare 1900–2099 run instead — strictly broader
/// than the shape it named, and it wrote off the difference as an accepted
/// missed skip ("+49 30 2019 1234" is a perfectly ordinary German number). That
/// was the wrong cost direction. `factual::metric_lines`' SOURCE side still
/// drops such a line from the truth set inside the contact band (it is a bare
/// phone line by shape), while the letter that repeats the same number keeps it
/// (this test said it was not contact details) — so the two sides dropped one
/// line through two different rules and the candidate's own phone digits came
/// back as a fabricated metric. A drop from the truth set that the claims side
/// does not mirror is an accusation channel, not a missed check.
///
/// [`date_spans`] requires a span SEPARATOR between two years, so it matches the
/// statement exactly: `(2019 - 2021)`, `(Jan 2019 – Mar 2021)` and `2018 to
/// 2021` are refused, and a subscriber number that happens to contain one
/// year-shaped run is contact details again — on BOTH callers, which is the
/// whole point of the guard living here.
pub(crate) fn looks_like_header_phone(text: &str) -> bool {
    HEADER_PHONE_RE.is_match(text) && date_spans(text).is_empty()
}

/// A line that really does carry contact details: a genuine email address, or a
/// header-shaped phone number on a line with no `@` at all.
///
/// Neither half may be the parser's own rule. `is_first_line_contact_shaped`
/// accepts a bare `@`, which any body line mentioning a Slack handle or a
/// `@decorator` satisfies — so a bullet could hide its numbers behind "this is
/// the header" just by carrying one — and its phone half is the loose
/// `PHONE_RE` that [`HEADER_PHONE_RE`] exists to replace.
///
/// `factual::metric_lines` routes its heading-less (cover-letter) band skip
/// through this; `ats::is_contact_cluster` applies the same two halves inline,
/// because it additionally has to reject a date range and to know which of the
/// two matched.
pub(crate) fn has_real_contact_match(text: &str) -> bool {
    if text.contains('@') {
        return EMAIL_RE.is_match(text);
    }
    looks_like_header_phone(text)
}

/// Split prose into sentences on `.`/`!`/`?`.
///
/// Paragraphs (blank-line separated) are unwrapped first: hard-wrapped prose is
/// normal in a generated letter, and splitting on the wrap instead of the
/// sentence would report a uniform ~12-word rhythm for every document and make
/// the burstiness check fire on everything.
///
/// Abbreviations ("e.g.") over-split, which is acceptable for the advisory
/// rhythm checks that consume this.
pub(crate) fn sentences(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n");
    let mut out = Vec::new();
    for paragraph in normalized.split("\n\n") {
        let unwrapped = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
        for fragment in unwrapped.split_inclusive(['.', '!', '?']) {
            let sentence = fragment.trim().trim_matches(['.', '!', '?']).trim();
            if !sentence.is_empty() {
                out.push(sentence.to_string());
            }
        }
    }
    out
}

pub(crate) fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Lowercase `text` with every whitespace run collapsed to one space, so a
/// multi-word phrase still matches across the hard wraps a generated letter is
/// full of ("Studies\nshow" must match "studies show").
pub(crate) fn flattened_lower(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Jaccard similarity of two token sets. `0.0` when both are empty (callers
/// must not treat "nothing in common because there is nothing" as identity).
pub(crate) fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let union = a.union(b).count();
    if union == 0 {
        return 0.0;
    }
    a.intersection(b).count() as f64 / union as f64
}

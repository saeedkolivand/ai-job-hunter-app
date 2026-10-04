//! How far back a source's dates reach: the clock, the open-ended-role test and
//! the career span they yield.

use std::sync::LazyLock;

use chrono::Datelike;
use regex::Regex;

use crate::documents::evidence::{is_open_ended, years_in, PRESENT_MARKERS};

use super::super::super::contains_phrase;

/// The year an open-ended span in the source is measured against — `None` when
/// there is no trustworthy answer.
///
/// This is the ONE non-hermetic input in the file, and `factual/employment.rs`'s
/// neighbouring date check is proud of never reading the clock at all. It has
/// to be read here: "2021 – Present" is what almost every résumé's current role
/// says, and without a today there is no span to compare a tenure against, so a
/// clock-free version of this check would be inert on the majority of real
/// documents.
///
/// **Self-validating rather than trusted.** A clock reading EARLIER than a year
/// the documents themselves name is wrong — a dead CMOS battery, a fresh VM
/// before its first NTP sync — and using it would shrink the allowance and
/// manufacture the exact false Critical this family exists to avoid (a source
/// dated "2016 – Present" with a clock stuck at 1970 makes a truthful "8 years"
/// read as an eight-fold exaggeration). So the clock is checked against the
/// documents and DISCARDED when it fails, taking the span evidence with it: the
/// check then falls back on what the source states in words, or goes quiet.
///
/// **What this costs in determinism, stated plainly.** Two runs a day apart
/// across 31 December can disagree, so `validation_is_deterministic` is true
/// WITHIN a calendar year rather than absolutely. The disagreement is
/// monotone-loosening — a later reading only ever widens the allowance — so a
/// report that passed cannot later fail on the clock alone; only the reverse,
/// and only towards silence.
///
/// A clock that is AHEAD needs no guard — it only ever widens the allowance.
/// The cost of the rule is a document carrying a future year (a typo, a
/// start-date-in-advance entry), which reads as an untrustworthy clock and goes
/// quiet. A missed check, which is this family's chosen direction of error.
pub fn reference_year(source: &str, generated: &str) -> Option<u32> {
    let documented = years_in(source)
        .into_iter()
        .chain(years_in(generated))
        .max()
        .unwrap_or(0);
    let clock = chrono::Utc::now().year().max(0) as u32;
    (clock >= documented).then_some(clock)
}

/// How much text after a span separator is read looking for the span's END.
///
/// Long enough for `Mar 2021` and `bis Dezember 2021`, short enough that the
/// next sentence cannot supply a year and make an open span look closed.
pub const SPAN_TAIL_CHARS: usize = 16;

/// `<year> <span separator> <tail>` — the shape a date column has, read off raw
/// TEXT rather than off a parsed entry.
static SPAN_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(?:19|20)\d{{2}}\s*(?:[-–—]|\bto\b|\bbis\b|\buntil\b|\bhasta\b|\bau\b|\bà\b|\ba\b|\btot\b|\bfino\b|\baté\b)\s*([^\n]{{0,{SPAN_TAIL_CHARS}}})"
    ))
    .unwrap()
});

/// Openers that make a date column open-ended, matched as BARE TOKENS.
///
/// `documents::evidence::is_open_ended` knows these words but requires a year
/// to follow within one optional word, so `Seit März 2016` is open and
/// `Seit 03/2016` is not — and a numeric month is exactly what a German, French
/// or Spanish date column usually carries. Measured: the current role then read
/// as closed at its own start year and a truthful fifteen years became a
/// Critical. Same vocabulary as that function, deliberately, so the two cannot
/// disagree about which words these are; only the adjacency rule differs.
static SPAN_OPENER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(since|seit|from|ab|depuis|desde|dal|dalla|vanaf|sinds)\b").unwrap()
});

/// A [`PRESENT_MARKERS`] word anywhere on a line that ALREADY CARRIES A YEAR —
/// `2021 Present`, `2021 | Heute`, `2015 · Aktuell`, `2016 (ongoing)`.
///
/// `documents::evidence::is_open_ended` used to answer this and deliberately
/// no longer does: it now demands an explicit span separator (`-`, `to`,
/// `bis`, …) between the year and the marker, because that predicate also
/// runs against whole prose sentences, where "a year, and a marker word a few
/// words later" is indistinguishable from ordinary text ("Reduced actual
/// costs by 20% in 2023"). Here it IS distinguishable, and that is why this
/// arm is local to this file: [`source_is_ongoing`] has already established
/// the line carries a year, and every direction of error it can make is the
/// generous one.
///
/// **Why no separator list of its own.** The first attempt at this arm allowed
/// only WHITESPACE between the year and the marker, which left a pipe, a
/// middot, a comma and a parenthesis reading as CLOSED — the one dangerous
/// answer [`source_is_ongoing`] documents. Measured: a truthful
/// `… | 2015 | Present` history collapsed from an eleven-year span to a
/// zero-year one and raised a false `factual.inflated_experience` Critical.
/// Any enumeration of separators fails the same way one spelling further out,
/// and `export::parser::DATE_RE` — this codebase's own definition of a job
/// entry's date range — already allows 30 ARBITRARY characters there. So this
/// asks only for the marker and lets the caller's year requirement carry the
/// structure.
///
/// Same vocabulary as [`is_open_ended`], deliberately, so the two can never
/// disagree about which words these are; only the adjacency rule differs.
fn names_a_present_marker(line: &str) -> bool {
    let lower = line.to_lowercase();
    PRESENT_MARKERS.iter().any(|m| contains_phrase(&lower, m))
}

/// True when the source shows a role that has NOT ended.
///
/// Asked of raw text, and asked STRUCTURALLY: a date column whose separator is
/// followed by something that is not another year is open, whatever word sits
/// there. That is what makes it work for `2015 - Actualidad`, `2019 -
/// Aujourd'hui` and `2020 - Today`, none of which `PRESENT_MARKERS` carries —
/// and for the next spelling nobody has thought of either. The marker list is
/// still consulted, because `seit 2021` is open with no separator at all, and
/// so is [`names_a_present_marker`], because a marker can sit next to a year
/// behind any separator at all — or none.
///
/// Every direction of error here is generous: an unrelated "in 2019 - a big
/// year" reads as an open span and WIDENS the allowance. A false "closed" is
/// the only dangerous answer, and it needs every span in the document to have a
/// year within [`SPAN_TAIL_CHARS`] of its separator — which is what a genuinely
/// closed history looks like.
fn source_is_ongoing(source: &str) -> bool {
    source.lines().any(|line| {
        if years_in(line).is_empty() {
            return false;
        }
        is_open_ended(line)
            || SPAN_OPENER_RE.is_match(line)
            || names_a_present_marker(line)
            || SPAN_TAIL_RE.captures_iter(line).any(|c| {
                c.get(1)
                    .is_none_or(|tail| years_in(tail.as_str()).is_empty())
            })
    })
}

/// The span between the earliest year the source names and its latest end.
///
/// ## Read from the TEXT, never from the parse
///
/// The lenient half of this comparison ([`stated_years`]) reads raw text, so
/// the accusing half must too. Three separate false Criticals were measured
/// when it did not:
///
/// * a history whose second block is headed `EARLIER ROLES` (`career` is in
///   `classify_section`'s lexicon, `roles` is not) lost its 2010-2015 role, and
///   a truthful "15 years" became a Critical;
/// * a Spanish résumé whose current role ends in `Actualidad` — a spelling
///   `PRESENT_MARKERS` does not carry — had its span closed at the role's own
///   START year;
/// * the same shape again wherever `EXPERIENCIA` or any other heading fails to
///   classify, because then there are no entries to read an end from at all.
///
/// Every one of those is the same defect: an under-parse SHRANK the allowance,
/// and a shrunk allowance is an accusation. So neither end is parsed now. The
/// start is the earliest year anywhere in the source, and the end is today when
/// [`source_is_ongoing`] sees a role that has not finished, otherwise the latest
/// year the source names.
///
/// The price, stated rather than hidden: an education entry dated 2012-2016
/// widens the allowance for a career that began in 2019, because this no longer
/// measures "how long did you work" but "can your own document reach back that
/// far at all". That is the strongest claim that can be made without trusting a
/// section classifier in seven languages, and it still catches the class this
/// check was built for — a source whose whole history spans four years against
/// an output claiming eight.
///
/// `None` when the source names no year, or when a role is open and
/// [`reference_year`] found no trustworthy today to close it with.
pub fn career_span_years(source: &str, reference: Option<u32>) -> Option<u32> {
    let years = years_in(source);
    let earliest = years.iter().copied().min()?;
    let latest = if source_is_ongoing(source) {
        reference?
    } else {
        years.iter().copied().max()?
    };
    Some(latest.saturating_sub(earliest))
}

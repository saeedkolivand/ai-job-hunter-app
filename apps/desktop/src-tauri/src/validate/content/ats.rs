//! ATS-parsing hygiene of the CONTENT.
//!
//! Distinct from `validate/mod.rs`, which checks the rendered BYTES. This checks
//! the text: is a keyword stuffed, is a second contact block hiding in the body,
//! does the document have the sections a parser looks for, are its bullets a
//! sensible length and count.
//!
//! None of these are "what a real ATS scores" — no such single thing exists
//! (see the job-match standards). They are the shapes every parser handles
//! badly, and the advice is framed that way.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::{
    function_words, has_curated_function_words, years_in, SectionKind,
};
use crate::documents::keywords::keywords_normalized_list;
use crate::export::parser::is_contact_shaped;
use crate::export::types::{LineKind, ParsedLine};
use crate::validate::EMAIL_RE;

use super::{
    issue, Analysis, ContentIssue, Section, ATS_BULLET_COUNT, ATS_HEADER_IN_BODY,
    ATS_KEYWORD_DENSITY, ATS_LONG_BULLET, ATS_MISSING_SECTION,
};

/// Share of the document one keyword may occupy before it reads as stuffing.
/// Semantic and AI-detection layers penalize repetition, so this is guidance
/// against a real 2026 failure mode, not folklore about beating the bot.
pub const MAX_KEYWORD_DENSITY_RATIO: f64 = 0.04;

/// Absolute repeat ceiling for one keyword, independent of length.
pub const MAX_KEYWORD_OCCURRENCES: usize = 6;

/// Below this many content tokens the density RATIO is meaningless (in a
/// 20-token document any word twice already exceeds 4%). The absolute
/// occurrence ceiling still applies.
pub const MIN_TOKENS_FOR_DENSITY: usize = 50;

/// Character proxy for "longer than two printed lines" at résumé column widths.
pub const MAX_BULLET_CHARS: usize = 200;

/// A role with no bullets says nothing; a role with too many buries its own
/// best line.
pub const MIN_BULLETS_PER_ROLE: usize = 1;
pub const MAX_BULLETS_PER_ROLE: usize = 6;

/// Sections a parser expects to find in a résumé.
const REQUIRED_SECTIONS: &[(SectionKind, &str)] = &[
    (SectionKind::Experience, "Experience"),
    (SectionKind::Education, "Education"),
    (SectionKind::Skills, "Skills"),
];

/// `ats.keyword_density` — one keyword repeated past the stuffing threshold.
///
/// The counted tokens are filtered through [`function_words`] for the target
/// language first. The kernel's own `STOPWORDS` is English-only and its length
/// test counts BYTES, so every German function word wide enough to survive it
/// (`durch`, `eine`, `werden`, `wurde`, `sowie`, and `für` at four bytes)
/// counted as a keyword — and ordinary German prose, which repeats those the
/// way English repeats "the", was accused of keyword stuffing. Same list the
/// evidence extractor keeps function words out of the skills gap with: a word
/// that is not a skill is not a stuffed keyword either. Filtering happens
/// BEFORE `total`, so the density denominator is content words only.
///
/// Which means the check only works for a language whose function words are
/// KNOWN — `en` (via the kernel's `STOPWORDS`) and `de` today. For any other
/// language the filter is a no-op, and both the ratio and the absolute ceiling
/// then count `pour`, `avec`, `para` and `worden` as repeated keywords: ordinary
/// French, Spanish, Italian, Dutch or Portuguese prose is accused of stuffing.
/// So the whole check goes quiet there rather than reporting a number it cannot
/// stand behind — `documents::evidence::has_curated_function_words` is the one
/// place that decides, and adding a list to `function_words` re-enables this
/// automatically.
fn keyword_density_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if !has_curated_function_words(&ctx.lang) {
        return Vec::new();
    }
    let stop = function_words(&ctx.lang);
    let tokens: Vec<String> = keywords_normalized_list(ctx.input.generated)
        .into_iter()
        .filter(|t| !stop.contains(&t.as_str()))
        .collect();
    if tokens.is_empty() {
        return Vec::new();
    }
    let total = tokens.len();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for token in &tokens {
        *counts.entry(token.as_str()).or_default() += 1;
    }
    let mut over: Vec<(&str, usize)> = counts
        .into_iter()
        .filter(|(_, n)| {
            *n > MAX_KEYWORD_OCCURRENCES
                || (total >= MIN_TOKENS_FOR_DENSITY
                    && (*n as f64 / total as f64) > MAX_KEYWORD_DENSITY_RATIO)
        })
        .collect();
    // The HashMap iteration order is not stable; sort so the report is.
    over.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    over.into_iter()
        .map(|(term, n)| {
            issue(
                ATS_KEYWORD_DENSITY,
                None,
                format!(
                    "\"{term}\" appears {n} times in {total} content words. Modern parsers score \
                     meaning, not repetition — say it once well and vary the rest."
                ),
                Some(format!("{term} ×{n}")),
            )
        })
        .collect()
}

/// A phone number as a résumé HEADER actually writes one — the shape test
/// behind this file's Critical, deliberately stricter than
/// `export::parser::PHONE_RE`.
///
/// That regex (`\+?\d[\d\s\-().]{7,}`) accepts any run of seven characters
/// drawn from digits, spaces, hyphens, dots and parens, which ordinary numeric
/// prose satisfies constantly: "150 - 200 EUR per hour" and "90 000 - 110 000"
/// are both "phone numbers" to it. It is the right rule where it lives — the
/// parser only has to decide which BAND a header line belongs to, and
/// over-matching there costs nothing — but here it decided a CRITICAL, so a
/// salary range under any short line was reported as a second contact block.
///
/// Two accepted forms, and between them they cover the header formats the
/// pipeline's own fixtures use in `en` and `de`:
///
/// 1. an explicit international/area-code marker — a leading `+` or `(`
///    followed by digits (`+49 30 1234567`, `+49 (0)30 1234567`,
///    `(030) 12345678`, `+1 (555) 123-4567`);
/// 2. failing that, an unbroken run of seven or more digits — the local part of
///    a German number written without a marker (`030 1234567`,
///    `0176 12345678`).
///
/// A grouped figure has at most three digits per group and no marker, so it
/// matches neither. The cost is a MISSED finding on a bare US-style number with
/// no parentheses ("555-123-4567", longest run four): accepted, because a
/// header block essentially always carries an email too — which
/// [`is_contact_cluster`] tests separately — and this family's rule is that a
/// wrong Critical is worse than a missed one.
static HEADER_PHONE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[+(]\s*\d[\d\s\-./()]{5,}\d|\d{7,}").unwrap());

/// True when a run of lines starting at `i` is a CONTACT BLOCK rather than one
/// line that happens to hold an `@`.
///
/// Two accepted shapes, both mirroring `export::parser`'s own contact rules so
/// this cannot drift from what the renderer treats as a header:
///
/// 1. A one-line block — a real email or a phone PLUS at least two `|`/`·`
///    separators, i.e. the classic "Jane Doe · jane@x.com · +49 …".
/// 2. A two-line block — a real email/phone line at the TOP of a section,
///    directly under a short, punctuation-free line (a name).
///
/// Both narrowings exist because this is a Critical and both false positives
/// were reproduced on truthful documents:
///
/// * "an email" is `validate::EMAIL_RE`, not `contains('@')`. A bullet reading
///   "owned the @payments rotation" carries an `@` and no address.
/// * the name-like neighbour must be the section's FIRST line. A header block
///   opens a document, so a candidate name can only precede it; matching any
///   adjacent short line made a two-line reference, an award, or a bullet
///   followed by a terse note into a "second contact block".
///
/// A single body line mentioning an address is deliberately NOT a match, and
/// neither is a bullet or a job entry — a header block is never either, and a
/// loose phone shape matches a date range (`2018 - 2021` is a digit followed by
/// eight digits, spaces and hyphens), which would otherwise make every
/// pipe-separated employment line a "contact block". The phone half is
/// [`HEADER_PHONE_RE`] rather than the parser's own rule, for the reason stated
/// there.
fn is_contact_cluster(lines: &[&ParsedLine], i: usize) -> bool {
    let line = lines[i];
    if !matches!(
        line.kind,
        LineKind::Contact | LineKind::Text | LineKind::Name
    ) {
        return false;
    }
    let has_email = EMAIL_RE.is_match(&line.text);
    // A phone shape only counts on a line with no stray `@` (that would be an
    // address this check just rejected) and no year: a date span is not a phone
    // number, whatever the regex thinks.
    let has_phone = !line.text.contains('@')
        && HEADER_PHONE_RE.is_match(&line.text)
        && years_in(&line.text).is_empty();
    if !(has_email || has_phone) {
        return false;
    }
    let separators = line.text.matches('|').count()
        + line.text.matches('·').count()
        + line.text.matches('•').count();
    if separators >= 2 && is_contact_shaped(&line.text) {
        return true;
    }
    // A name line: short, unpunctuated, no digits, no separators, and not
    // something the parser already classified as structure (a job entry like
    // "Acme | 2021 - Present" satisfies every shape test but is not a name).
    let name_like = |l: &ParsedLine| {
        let t = l.text.trim();
        matches!(l.kind, LineKind::Name | LineKind::Text)
            && !t.is_empty()
            && t.chars().count() <= 60
            && !t.contains(['.', ',', ';', ':', '|', '·', '•'])
            && !t.chars().any(|c| c.is_ascii_digit())
            && super::word_count(t) <= 5
    };
    // Exactly the second line, under the section's first line.
    i == 1 && name_like(lines[0])
}

/// `ats.header_in_body` — a second contact block inside the document.
///
/// Critical: many parsers, on finding two header blocks, attribute the whole
/// document to one of them, and the candidate never learns why nobody called.
/// (Behaviour differs per vendor — see the job-match standards; there is no
/// single "the ATS" that does this.) Scanned only in sections AFTER the first
/// (section 0 IS the header).
fn header_in_body_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = Vec::new();
    for section in ctx.generated_sections.iter().skip(1) {
        let lines: Vec<&ParsedLine> = section
            .lines
            .iter()
            .filter(|l| !matches!(l.kind, LineKind::Blank))
            .collect();
        for i in 0..lines.len() {
            if is_contact_cluster(&lines, i) {
                issues.push(issue(
                    ATS_HEADER_IN_BODY,
                    section.heading.as_deref(),
                    "A second contact block appears in the body of the document. Many parsers \
                     pick one header and attribute everything to it — keep contact details in \
                     the header only."
                        .to_string(),
                    Some(lines[i].text.clone()),
                ));
                break; // One finding per section is enough to act on.
            }
        }
    }
    issues
}

/// `ats.missing_section` — a standard section a parser looks for is absent.
fn missing_section_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    REQUIRED_SECTIONS
        .iter()
        .filter(|(kind, _)| ctx.section_of_kind(*kind).is_none())
        .map(|(_, name)| {
            issue(
                ATS_MISSING_SECTION,
                None,
                format!(
                    "There is no {name} section. Parsers key off standard headings — add one, \
                     even if it is short."
                ),
                Some((*name).to_string()),
            )
        })
        .collect()
}

/// `ats.long_bullet` — a bullet that runs past two printed lines.
fn long_bullet_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    ctx.generated_sections
        .iter()
        .flat_map(|s| s.bullets().map(move |b| (s, b)))
        .filter(|(_, b)| b.text.chars().count() > MAX_BULLET_CHARS)
        .map(|(section, bullet)| {
            issue(
                ATS_LONG_BULLET,
                section.heading.as_deref(),
                format!(
                    "This bullet is {} characters — over two printed lines. Split it or cut it \
                     back to one result.",
                    bullet.text.chars().count()
                ),
                Some(bullet.text.clone()),
            )
        })
        .collect()
}

/// Bullets grouped per employment entry, in document order.
fn bullets_per_role(section: &Section) -> Vec<(String, usize)> {
    let mut roles: Vec<(String, usize)> = Vec::new();
    for line in &section.lines {
        match line.kind {
            LineKind::JobEntry => roles.push((line.text.clone(), 0)),
            LineKind::Bullet => {
                if let Some(last) = roles.last_mut() {
                    last.1 += 1;
                }
            }
            _ => {}
        }
    }
    roles
}

/// `ats.bullet_count` — a role outside the 1..=6 bullet band.
fn bullet_count_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    ctx.generated_sections
        .iter()
        .filter(|s| s.kind == SectionKind::Experience)
        .flat_map(bullets_per_role)
        .filter(|(_, n)| !(MIN_BULLETS_PER_ROLE..=MAX_BULLETS_PER_ROLE).contains(n))
        .map(|(role, n)| {
            let advice = if n < MIN_BULLETS_PER_ROLE {
                "add at least one result for it".to_string()
            } else {
                format!("keep the {MAX_BULLETS_PER_ROLE} strongest")
            };
            issue(
                ATS_BULLET_COUNT,
                Some("Experience"),
                format!("\"{role}\" has {n} bullets — {advice}."),
                Some(role),
            )
        })
        .collect()
}

pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = header_in_body_issues(ctx);
    issues.extend(keyword_density_issues(ctx));
    issues.extend(missing_section_issues(ctx));
    issues.extend(long_bullet_issues(ctx));
    issues.extend(bullet_count_issues(ctx));
    issues
}

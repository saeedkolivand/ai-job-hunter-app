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

use std::collections::{HashMap, HashSet};

use crate::documents::evidence::{has_curated_function_words, trailing_date_column, SectionKind};
use crate::documents::keywords::keywords_normalized_list_for_lang;
use crate::export::parser::is_contact_shaped;
use crate::export::types::{LineKind, ParsedLine};
use crate::validate::EMAIL_RE;

use super::{
    issue, looks_like_header_phone, Analysis, ContentIssue, Section, ATS_BULLET_COUNT,
    ATS_EMPTY_SECTION, ATS_HEADER_IN_BODY, ATS_KEYWORD_DENSITY, ATS_LONG_BULLET,
    ATS_MISSING_SECTION,
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
///
/// DERIVED, not tuned: the ratio must never be able to accuse a document over
/// three ordinary repeats. `3 / total > 0.04` holds for every `total < 75`, so
/// 75 is exactly the point where three stops firing and four is required.
///
/// It moved from 50 when the keyword kernel became language-aware. Filtering a
/// posting's own function words is correct, and it makes documents ~18% shorter
/// in CONTENT tokens — a real German fixture went 89 -> 73 raw, 82 -> 72 after
/// this check's own filtering, which tipped an honest `backend` x3 from 3.66%
/// to 4.17% and read as keyword stuffing. The threshold was calibrated against
/// a denominator inflated with filler; the filler left, so the floor followed.
/// Measured through the `eval` corpus, not reasoned about.
pub const MIN_TOKENS_FOR_DENSITY: usize = 75;

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
/// The counted tokens come straight from [`keywords_normalized_list_for_lang`],
/// which removes function words in the document's OWN language — so the density
/// denominator is content words only, as it has to be.
///
/// This used to filter a second time through
/// [`crate::documents::evidence::function_words`], because the
/// kernel's `STOPWORDS` was English-only and its length test counts BYTES:
/// every German function word wide enough to survive it (`durch`, `eine`,
/// `werden`, `wurde`, `sowie`, and `für` at four bytes) counted as a keyword,
/// and ordinary German prose was accused of stuffing. The kernel is
/// language-aware now, so that second pass is redundant — and not harmless:
/// filtering twice shrank `total` below what [`MAX_KEYWORD_DENSITY_RATIO`] was
/// calibrated against, and a TRUTHFUL German fixture tipped over the line at
/// `backend` ×3 in ~72 content words (4.17%). Measured, via the `eval` corpus,
/// not reasoned about. Thresholds are unchanged; the denominator is what was
/// wrong.
///
/// [`has_curated_function_words`] still gates the whole check, and still reads
/// `en | de`. It no longer selects a filter — it answers "is this language
/// curated well enough to ACCUSE someone?". The kernel now carries partial
/// lists for fr/es/it/pt/nl too, but partial is the operative word (an
/// uncurated inflected verb form still survives), so this deliberately stays
/// quiet there rather than widening on the back of a list that is curated, not
/// exhaustive.
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
///
/// The same argument closes one more door. `ctx.lang` is the language the
/// document was ASKED for, and every sentence above assumes the document is
/// written in it. [`Analysis::language_mismatch`] is the measurement that it is
/// not — taken against a reliable source control, and already reported as its
/// own Critical — and while it holds, `function_words(&ctx.lang)` filters words
/// that are not on the page: an English target with German output counted
/// `verantwortlich` and `für` as repeated keywords and accused ordinary German
/// prose of stuffing, underneath the one finding the user can act on. An
/// accusation about density in a document we already know we are reading in the
/// wrong language is noise, so this suppresses exactly the way
/// [`Analysis::posting_comparable`] does.
fn keyword_density_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if ctx.language_mismatch || !has_curated_function_words(&ctx.lang) {
        return Vec::new();
    }
    // `ctx.lang` — the language resolved ONCE for the whole document — not a
    // fresh per-call detection. `keywords_normalized_list` would re-detect on
    // `generated` alone, and a short or tech-heavy résumé body is not reliably
    // identifiable in isolation (a German skills line reads as Estonian at
    // confidence 0.23). A disagreement there silently picks a different stopword
    // profile, which moves the denominator and can flip whether this issue is
    // emitted at all. The gate one line up already commits to `ctx.lang`; the
    // tokenizer has to agree with it.
    let tokens: Vec<String> = keywords_normalized_list_for_lang(ctx.input.generated, &ctx.lang);
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
/// [`super::looks_like_header_phone`] rather than the parser's own rule, for the
/// reason stated there — including the year test, which now lives in that
/// helper. The two halves are still applied separately here (rather than
/// through `super::has_real_contact_match`) because this check has to know
/// WHICH of the two matched.
///
/// ## Shape (2) additionally asks WHOSE details these are
///
/// "First line of a section, name-shaped, address underneath" is not a header
/// block; it is the shape of every references list and every named contact
/// person ever written — "REFERENCES / Maria Lang / maria.lang@…". The `i == 1`
/// narrowing was introduced to stop exactly that and lands one index short: the
/// heading goes into `Section::heading` rather than into `lines`, and `Blank` is
/// filtered out, so a referee IS the section's first line. Listing a referee was
/// a Critical.
///
/// What this check claims is in its own message — a **second** contact block,
/// i.e. the candidate's own header appearing twice, which is what makes a parser
/// attribute the document to the wrong person. A referee's details are not that
/// under any reading. So the name over the address must be a name the HEADER
/// BAND already carries ([`header_name_forms`]); someone else's is not a second
/// copy of anything.
///
/// **Why identity and not a `REFERENCES` heading lexicon** (the other repair on
/// the table): a heading list only exempts the sections that say so, and the
/// same block appears under "Ansprechpartner", "Kontakt für Rückfragen",
/// "Empfehlungen" and under no heading the classifier knows at all — while this
/// module has twice been bitten by "that list is English-only" being wrong about
/// a list nobody enumerated. Identity needs no vocabulary, holds in every
/// locale, and states the actual claim. Shape (1) — the one-line
/// `Name · email · phone` block — is untouched: it carries its own separators
/// and is a header block by construction.
///
/// *Cost, paid deliberately:* a genuinely duplicated header whose second copy
/// writes the name differently ("M. Mustermann" under "Max Mustermann"), or
/// omits it, is no longer reported through shape (2). A missed check, which is
/// this family's chosen direction — and a document whose header band carries no
/// name-shaped line at all has no first header for a second one to duplicate.
fn is_contact_cluster(lines: &[&ParsedLine], i: usize, header_names: &HashSet<String>) -> bool {
    let line = lines[i];
    if !matches!(
        line.kind,
        LineKind::Contact | LineKind::Text | LineKind::Name
    ) {
        return false;
    }
    let has_email = EMAIL_RE.is_match(&line.text);
    // A phone shape only counts on a line with no stray `@` — that would be an
    // address this check just rejected. The "and no year" half used to live
    // here as a second clause; it is now inside
    // [`super::looks_like_header_phone`], because the other caller of that
    // helper needed it just as much and did not have it.
    let has_phone = !line.text.contains('@') && looks_like_header_phone(&line.text);
    if !(has_email || has_phone) {
        return false;
    }
    let separators = line.text.matches('|').count()
        + line.text.matches('·').count()
        + line.text.matches('•').count();
    if separators >= 2 && is_contact_shaped(&line.text) {
        return true;
    }
    // Exactly the second line, under the section's first line — and that line
    // must name the CANDIDATE, not a referee. See the doc comment.
    i == 1
        && is_name_like(lines[0])
        && header_names.contains(&super::flattened_lower(&lines[0].text))
}

/// A name line: short, unpunctuated, no digits, no separators, and not something
/// the parser already classified as structure (a job entry like
/// "Acme | 2021 - Present" satisfies every shape test but is not a name).
///
/// Applied to BOTH sides of the identity test in [`is_contact_cluster`] — the
/// candidate's header band and the candidate line in the body — so the two can
/// never disagree about what a name looks like.
fn is_name_like(l: &ParsedLine) -> bool {
    let t = l.text.trim();
    matches!(l.kind, LineKind::Name | LineKind::Text)
        && !t.is_empty()
        && t.chars().count() <= 60
        && !t.contains(['.', ',', ';', ':', '|', '·', '•'])
        && !t.chars().any(|c| c.is_ascii_digit())
        && super::word_count(t) <= 5
}

/// The name-shaped lines of the HEADER BAND (section 0, everything before the
/// first heading), flattened for comparison.
///
/// Section 0 is the header by construction — `split_sections` never gives it a
/// heading — so these are the candidate's own name as their document writes it.
/// Empty when the band carries no name-shaped line, which correctly disables
/// [`is_contact_cluster`]'s second shape: there is no first header for a second
/// one to be a copy of.
fn header_name_forms(ctx: &Analysis) -> HashSet<String> {
    ctx.generated_sections
        .first()
        .map(|band| {
            band.lines
                .iter()
                .filter(|l| is_name_like(l))
                .map(|l| super::flattened_lower(&l.text))
                .collect()
        })
        .unwrap_or_default()
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
    let header_names = header_name_forms(ctx);
    for section in ctx.generated_sections.iter().skip(1) {
        let lines: Vec<&ParsedLine> = section
            .lines
            .iter()
            .filter(|l| !matches!(l.kind, LineKind::Blank))
            .collect();
        for i in 0..lines.len() {
            if is_contact_cluster(&lines, i, &header_names) {
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

/// `ats.empty_section` — a heading survived into the document with nothing
/// under it.
///
/// A Warning, not a Critical, and the severity is the whole design here.
///
/// The defect is real and unambiguous — a heading the DOCUMENT ITSELF
/// introduced and then left blank, model-free to detect. But `repair` acts on
/// Criticals only (`stages::repair::criticals_by_section`) and its remedy is
/// to REGENERATE the offending section. For an empty section that remedy is
/// actively harmful: asked to fill an empty `PROJECTS`, the model invents
/// projects the candidate does not have — a fabrication, in the one part of
/// this pipeline whose entire purpose is preventing them. And where the
/// heading has no `SectionKey` (Certifications/Awards/Publications all
/// classify `Other`), a Critical is not repairable OR clearable: it parks the
/// run at `needsReview` with a finding no loop and no user button can action.
///
/// The correct remedy is REMOVAL, and it already happens structurally, before
/// this check is ever consulted: `model::adapter::push_nonempty_section` drops
/// a zero-content section at the model-building boundary, so the rendered
/// PDF/DOCX cannot carry one. This issue is therefore the REPORT of a defect
/// already handled, not the trigger for fixing it.
///
/// It is what `draft_system`'s old "order the sections EXACTLY as … do not
/// drop" phrasing produced when the model read a fixed ORDER as a fixed
/// MANIFEST and emitted every section on it, real content or not.
///
/// Deliberately conservative: a section with even ONE non-blank line survives
/// untouched, however terse — only a heading with ZERO content beneath it
/// fires, so a legitimately short section (one Publications entry, a
/// one-line Award) is never mistaken for an empty one.
fn empty_section_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    ctx.generated_sections
        .iter()
        .skip(1) // section 0 is the header band (`heading: None`), never named
        .filter(|s| s.heading.is_some())
        .filter(|s| s.lines.iter().all(|l| l.text.trim().is_empty()))
        .map(|s| {
            let name = s.heading.clone().unwrap_or_default();
            issue(
                ATS_EMPTY_SECTION,
                s.heading.as_deref(),
                format!(
                    "The \"{name}\" section has a heading but no content under it — remove the \
                     heading, or fill it in before sending."
                ),
                None,
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
///
/// ## What opens a role is `documents::evidence`'s rule, not a second one
///
/// This used to open a role ONLY on `LineKind::JobEntry` and append every other
/// line to `roles.last_mut()`, which is precisely the misattribution class
/// `documents::evidence::extract_evidence` spent rounds 6–11 removing. Both
/// consumers read the SAME `ParsedLine` stream, so on the ordinary mixed-format
/// résumé — one entry the parser recognises, the next in the extracted-PDF comma
/// form ("Acme Payments, Berlin, 2018 - 2021") — the evidence extractor opened a
/// second role while this one kept appending, and the second employer's bullets
/// were counted against the first. The visible result is a `bullet_count`
/// finding naming the wrong employer, with a number that belongs to two of them.
///
/// So a `Text`/`Contact` line ending in a real date COLUMN opens its own role
/// here too, through the same [`trailing_date_column`] the extractor gates on —
/// a date the line is STRUCTURED by, not one it merely mentions. The role is
/// named by the label in front of the column, which is the same half of the line
/// a recognised `JobEntry` would have given us.
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
            LineKind::Text | LineKind::Contact => {
                if let Some((label, _)) = trailing_date_column(&line.text) {
                    roles.push((label.to_string(), 0));
                }
            }
            _ => {}
        }
    }
    roles
}

/// `ats.bullet_count` — a role outside the 1..=6 bullet band.
///
/// The `section` field is the DOCUMENT's own heading, not the English word
/// "Experience" this used to hardcode. The panel renders that field verbatim as
/// a grouping key, so a German résumé showed one section as two groups —
/// "BERUFSERFAHRUNG" from [`long_bullet_issues`], which reads the heading, and
/// an untranslated "Experience" from here. Same section, same key, one group.
fn bullet_count_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    ctx.generated_sections
        .iter()
        .filter(|s| s.kind == SectionKind::Experience)
        .flat_map(|section| {
            bullets_per_role(section)
                .into_iter()
                .map(move |role| (section, role))
        })
        .filter(|(_, (_, n))| !(MIN_BULLETS_PER_ROLE..=MAX_BULLETS_PER_ROLE).contains(n))
        .map(|(section, (role, n))| {
            let advice = if n < MIN_BULLETS_PER_ROLE {
                "add at least one result for it".to_string()
            } else {
                format!("keep the {MAX_BULLETS_PER_ROLE} strongest")
            };
            issue(
                ATS_BULLET_COUNT,
                section.heading.as_deref(),
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
    issues.extend(empty_section_issues(ctx));
    issues.extend(long_bullet_issues(ctx));
    issues.extend(bullet_count_issues(ctx));
    issues
}

//! Splitting a generated résumé into sections, and splicing one back — the
//! primitive under both the repair loop and the per-section regenerate button.
//!
//! Sections are addressed by LINE RANGE over the original text, not rebuilt
//! from a parsed model. `export::parser::parse_resume` is a line CLASSIFIER —
//! exactly one [`ParsedLine`](crate::export::types::ParsedLine) per
//! `text.lines()` entry — so zipping the two by index gives the section
//! boundaries while every byte of the untouched sections stays untouched.
//! Re-rendering from the parse would silently reformat the parts of the
//! document the repair was not asked to change, which is the one thing a
//! "section-scoped" fix must not do.

use crate::documents::evidence::{classify_section, SectionKind};
use crate::export::parser::parse_resume;
use crate::export::types::{LineKind, ParsedDocument};

use crate::pipeline::resume::types::SectionKey;

/// One section of a generated document, as a line range over the source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSection {
    /// The heading line, or `None` for the leading band before the first
    /// heading (a contact header the model should not have written, or a
    /// document with no headings at all).
    pub heading: Option<String>,
    pub kind: SectionKind,
    /// 0-based index of the first line of the section INCLUDING its heading.
    pub start: usize,
    /// 0-based index one past the last line.
    pub end: usize,
}

impl RawSection {
    /// This section's text, heading line included.
    pub fn text(&self, lines: &[&str]) -> String {
        lines[self.start..self.end].join("\n")
    }
}

/// Split `text` at its section headings.
pub fn split(text: &str) -> Vec<RawSection> {
    split_parsed(text, &parse_resume(text))
}

/// [`split`] over an ALREADY-parsed document.
///
/// The seam exists because `parse_resume` is the expensive half and two callers
/// need both products of it. `split` keeps its signature for everyone who only
/// wants the sections.
pub fn split_parsed(text: &str, parsed: &ParsedDocument) -> Vec<RawSection> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<RawSection> = Vec::new();
    for (index, line) in parsed.lines.iter().enumerate() {
        if matches!(line.kind, LineKind::SectionHeader) {
            if let Some(last) = out.last_mut() {
                last.end = index;
            }
            out.push(RawSection {
                heading: Some(line.text.clone()),
                kind: classify_section(&line.text),
                start: index,
                end: lines.len(),
            });
        } else if out.is_empty() {
            out.push(RawSection {
                heading: None,
                kind: SectionKind::Other,
                start: 0,
                end: lines.len(),
            });
        }
    }
    out
}

/// The section a [`SectionKey`] names, if the document has it.
///
/// `Experience(n)` addresses the n-th EMPLOYMENT ENTRY, which is not a section
/// heading — a résumé has one "Work Experience" heading holding several
/// entries. It therefore resolves to the experience section as a whole: at
/// quality depth the draft is written in one pass, so an entry is not
/// independently regenerable, and rewriting the whole experience section with
/// the failing entry's issues attached is the honest scope. Phase 4's
/// section-wise generator is what makes per-entry addressing real.
pub fn find(sections: &[RawSection], key: SectionKey) -> Option<&RawSection> {
    let wanted = match key {
        SectionKey::Summary => SectionKind::Summary,
        SectionKey::Skills => SectionKind::Skills,
        SectionKey::Projects => SectionKind::Projects,
        SectionKey::Education => SectionKind::Education,
        SectionKey::Experience(_) => SectionKind::Experience,
    };
    sections.iter().find(|section| section.kind == wanted)
}

/// The [`SectionKey`] for a section KIND, or `None` for a kind this grammar has
/// no key for (the leading band, an unrecognised heading).
pub fn key_of(kind: SectionKind) -> Option<SectionKey> {
    match kind {
        SectionKind::Summary => Some(SectionKey::Summary),
        SectionKind::Skills => Some(SectionKey::Skills),
        SectionKind::Projects => Some(SectionKey::Projects),
        SectionKind::Education => Some(SectionKey::Education),
        SectionKind::Experience => Some(SectionKey::Experience(0)),
        SectionKind::Other => None,
    }
}

/// The [`SectionKey`] a validator's `section` label maps to, or `None` when the
/// finding is document-wide (or names a section this grammar has no key for).
///
/// The label is the heading line the validator copied out of the GENERATED
/// document, so it goes back through the same `classify_section` the split used
/// — never a string compare, which would miss "BERUFSERFAHRUNG" against
/// "Work Experience" and regenerate the wrong section.
pub fn key_for_label(label: Option<&str>) -> Option<SectionKey> {
    key_of(classify_section(label?))
}

/// The section whose text contains `span`.
///
/// **Necessary, not a convenience.** The `factual.*` family — the codes the
/// repair loop exists for — scans the whole document and reports
/// `section: None` by design (a fabricated metric is found by comparing number
/// sets, not by walking sections). Grouping only on the validator's own label
/// would therefore leave the commonest Critical unrepairable, which is the
/// silent version of "the repair loop does nothing".
///
/// The span is the issue's `evidence`, i.e. the offending text VERBATIM out of
/// this same document, so a plain substring search over the section's lines is
/// exact rather than heuristic. `None` when the span is empty, appears in no
/// section, or appears in the leading band (which has no key — and which the
/// model should not have written at all).
pub fn containing<'a>(
    sections: &'a [RawSection],
    lines: &[&str],
    span: &str,
) -> Option<&'a RawSection> {
    let span = span.trim();
    if span.is_empty() {
        return None;
    }
    sections
        .iter()
        .find(|section| section.text(lines).contains(span))
}

/// Replace one section's lines with `replacement`, returning the whole document.
///
/// The trailing-newline shape of the original is preserved: a document that
/// ended with a newline still does, and one that did not still does not — an
/// export path that splits on a trailing blank would otherwise see a document
/// change shape because an unrelated section was repaired.
pub fn splice(text: &str, section: &RawSection, replacement: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    out.extend_from_slice(&lines[..section.start]);
    let replacement_lines: Vec<&str> = replacement.lines().collect();
    out.extend_from_slice(&replacement_lines);
    out.extend_from_slice(&lines[section.end..]);
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

/// Whether a regenerated section is usable at all.
///
/// A TRUNCATED section is a FAILED attempt, not a smaller section: splicing one
/// in deletes the rest of the original and the document silently loses content.
/// Four things have to hold — free of any registered fence tag, the
/// replacement must open with a heading line, it must carry at least one line
/// of body under it, and it must contain no SECOND heading. (An over-eager
/// model that answers with the whole résumé used to be waved through on the
/// theory that the splice is section-scoped, so the extra sections would just
/// "land inside this section's range" — but the splice does not re-parse what
/// it inserts, so those sections land in the FINAL document too, doubling
/// EXPERIENCE/PROJECTS/SKILLS/EDUCATION at export. The heading-count check
/// below is what actually makes a multi-section answer unusable — counted by
/// [`real_section_count`], not by raw `SectionHeader` lines, so an ALL-CAPS
/// employer name inside the section being replaced does not itself read as a
/// second section.)
///
/// **The fence-tag check is the same shape gate `humanize`'s
/// `is_usable_rewrite` added** (see that module's doc for the incident): the
/// repair prompt wraps the section it hands the model as `<resume_section>…
/// </resume_section>` and asks for "the replacement section" back — a model
/// that echoes the wrapper instead of just the content would splice
/// `<resume_section>` literally into the document, and a heading/body count
/// alone would not catch it (the wrapper adds a line, it doesn't remove one).
/// Checked against the FULL `crate::prompt_fence` registry, not just this one
/// tag, for the same reason.
pub fn is_usable_replacement(replacement: &str) -> bool {
    if crate::prompt_fence::contains_fence_tag(replacement) {
        return false;
    }
    let mut lines = replacement.lines().map(str::trim).filter(|l| !l.is_empty());
    let Some(first) = lines.next() else {
        return false;
    };
    let parsed = parse_resume(replacement);
    let opens_with_heading = parsed
        .lines
        .iter()
        .find(|line| !line.text.trim().is_empty())
        .is_some_and(|line| matches!(line.kind, LineKind::SectionHeader))
        || classify_section(first) != SectionKind::Other;
    // A SECOND REAL heading means the answer is more than one section — the
    // whole document, or several — and splicing it in doubles every section
    // it names.
    opens_with_heading && lines.next().is_some() && real_section_count(&parsed) <= 1
}

/// How many of `parsed`'s detected `LineKind::SectionHeader` lines classify to
/// an actual section kind, rather than [`SectionKind::Other`].
///
/// `ParsedDocument::section_count` counts every `SectionHeader` LINE, and the
/// ALL-CAPS heading heuristic behind it (`export::parser`) cannot tell a
/// genuine second heading from an ALL-CAPS employer name inside the very
/// section being replaced — "ACME PAYMENTS GMBH" parses as a heading exactly
/// like "EXPERIENCE" does, so `section_count` was 2 on a legitimate
/// single-section reply and [`is_usable_replacement`] rejected it, truncating
/// a repair that had nothing wrong with it. Re-classifying through the SAME
/// [`classify_section`] the split uses turns the heading count into a SECTION
/// count: a company name (or an unrecognised sub-heading like "Languages"
/// under Skills) classifies `Other` and is not counted, while a real second
/// Summary/Skills/Experience/Education/Projects heading is.
fn real_section_count(parsed: &ParsedDocument) -> usize {
    parsed
        .lines
        .iter()
        .filter(|line| matches!(line.kind, LineKind::SectionHeader))
        .filter(|line| classify_section(&line.text) != SectionKind::Other)
        .count()
}

/// Whether a replacement's own leading heading names the SAME section kind it
/// was asked to regenerate.
///
/// `regenerate_one_section` resolves its target by [`SectionKey`] and used to
/// splice back whatever came back without ever re-checking WHAT it got: asked
/// for Summary, handed `"SKILLS\n\nRust · Python · Kafka"`, the shape checks
/// in [`is_usable_replacement`] pass it clean — a well-formed heading with a
/// body — and the splice silently swapped the résumé's Summary for a second
/// Skills section, with nothing naming the loss. Re-classified through the
/// SAME `classify_section` the split used, never a string compare, for the
/// same reason `key_for_label` reads a heading that way (a German heading
/// like "Kenntnisse" must still match `SectionKind::Skills`).
pub fn matches_requested_kind(replacement: &str, expected: SectionKind) -> bool {
    let Some(heading) = replacement.lines().map(str::trim).find(|l| !l.is_empty()) else {
        return false;
    };
    classify_section(heading) == expected
}

/// Whether a regenerated section is acceptable to splice in at all: usable in
/// shape ([`is_usable_replacement`]) AND naming the section kind it was
/// actually asked to regenerate ([`matches_requested_kind`]).
///
/// The ONE gate `regenerate_one_section` runs — and the only one its own
/// tests run, by calling this function rather than rebuilding the same two
/// calls inside a test closure. A hand-rebuilt copy proves the ingredients
/// work; it does not prove the production wiring calls them, which a
/// mutation on `regenerate_one_section`'s own condition would not have caught
/// before this existed.
pub fn accepts(replacement: &str, expected: SectionKind) -> bool {
    is_usable_replacement(replacement) && matches_requested_kind(replacement, expected)
}

/// A compact ANCHOR of the sibling sections already written in this
/// generation run — never the whole document — for
/// [`crate::pipeline::resume::prompts::repair_user`]'s `<document_context>`
/// block.
///
/// **Why an anchor, not the whole document.** A repair round can fan out to
/// `MAX_SECTIONS_PER_ROUND` sections per round, up to twice
/// (`Budget::max_repair_attempts`), so whatever this returns is charged up to
/// 8× per pipeline run. The Summary section (2-4 sentences, the register
/// every other section is meant to share) and one representative Experience
/// bullet (the tense/voice the rest of the document's bullets follow) carry
/// almost the whole signal a section rewrite needs to notice it has drifted
/// into a different language, voice or tense than its siblings — the failure
/// PRs #969/#992's per-section fan-out introduced — for a fraction of what
/// fencing every OTHER section would cost.
///
/// **The section being rewritten is excluded by KIND**, not by name: handing
/// a section its own text as "what to match" would ask it to match the very
/// text it is about to replace.
///
/// Empty when neither anchor survives the exclusion (repairing Summary in a
/// document with no Experience section, or vice versa) — nothing left to
/// compare against, so the caller sends no block at all rather than a
/// misleading partial one (see `prompts::repair_system`'s own `has_context`
/// gate).
pub fn context_anchor(sections: &[RawSection], lines: &[&str], skip: SectionKind) -> String {
    let mut parts: Vec<String> = Vec::new();
    if skip != SectionKind::Summary {
        if let Some(summary) = sections.iter().find(|s| s.kind == SectionKind::Summary) {
            parts.push(summary.text(lines));
        }
    }
    if skip != SectionKind::Experience {
        if let Some(experience) = sections.iter().find(|s| s.kind == SectionKind::Experience) {
            let text = experience.text(lines);
            if let Some(bullet) = representative_bullet(&text) {
                parts.push(bullet.to_string());
            }
        }
    }
    parts.join("\n\n")
}

/// The first bullet-marker line in `text`, or its LAST non-empty line AFTER
/// the heading when no marker is found — `None` when the section carries no
/// body at all.
///
/// Every generated résumé bullet this crate's own fixtures produce opens with
/// `-`/`•`/`*` (`draft_system`'s structure rule leaves the exact marker to the
/// model, so this covers the common ones rather than parsing Markdown). The
/// fallback exists for a source-authored résumé imported verbatim with no
/// marker at all: its LAST non-empty line is still the closest available
/// proxy for a bullet, since the heading and the company/title/dates line
/// always come first in this codebase's own résumé grammar
/// (`export::parser`).
///
/// `text` is a [`RawSection::text`], heading line included, and the heading
/// is always skipped for the fallback: a heading-ONLY Experience section (no
/// body at all) used to have this return the heading itself — "EXPERIENCE",
/// verbatim — as "the voice to imitate" for [`context_anchor`]'s prompt
/// anchor, which is not a bullet by any reading.
fn representative_bullet(text: &str) -> Option<&str> {
    const MARKERS: [&str; 3] = ["- ", "• ", "* "];
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    if let Some(bullet) = lines
        .iter()
        .find(|line| MARKERS.into_iter().any(|marker| line.starts_with(marker)))
    {
        return Some(bullet);
    }
    // Index 0 is always the heading (`RawSection::text` includes it) —
    // `skip(1)` before searching backward so a heading-only section (no body
    // at all) returns `None` instead of handing the heading itself back.
    lines
        .iter()
        .skip(1)
        .rev()
        .find(|line| !line.is_empty())
        .copied()
}

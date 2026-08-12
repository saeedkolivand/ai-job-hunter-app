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

use crate::documents::evidence::{classify_section, split_entry, SectionKind};
use crate::export::parser::parse_resume;
use crate::export::types::LineKind;

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
    let lines: Vec<&str> = text.lines().collect();
    let parsed = parse_resume(text);
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

/// The line range of ONE employment ENTRY inside the experience section.
///
/// What [`find`] cannot give: `Experience(n)` resolves to the whole experience
/// section there, because at quality depth an entry is not independently
/// regenerable. At MAX depth it is — the document was assembled one entry at a
/// time from a seeded roster — so a per-entry regenerate can replace exactly
/// the entry the user clicked instead of re-rolling the other eight.
///
/// An entry begins at a `LineKind::JobEntry` line and runs to the next one, or
/// to the end of the section. `None` when the document has no experience
/// section, or fewer entries than `index` — the caller then falls back to the
/// whole-section path, which is what a document that no longer matches its
/// run's roster deserves.
///
/// The returned range never includes the section HEADING (it starts at the
/// entry line), so splicing it in cannot duplicate or delete one.
///
/// It also never includes the trailing BLANK line(s) below the entry. The blank
/// between two entries is a SEPARATOR that belongs to neither of them, and the
/// replacement a caller splices in (`assemble::chunk`) carries none — so a range
/// that ran to the next entry's first line deleted one blank per regenerate,
/// permanently and invisibly, until the section read as a wall of text. Same at
/// the tail: the last entry's range would otherwise run to the next section's
/// HEADING and eat the blank above it.
///
/// **Positional, and therefore not a WRITE address on its own.** A caller that
/// is about to splice must go through [`named_entry_range`] instead: the index
/// alone says nothing about whether the entry at that position is still the one
/// the caller means. See that function for the three ways they diverge.
pub fn entry_range(document: &str, index: u8) -> Option<RawSection> {
    let split = split(document);
    let section = find(&split, SectionKey::Experience(index))?;
    let parsed = parse_resume(document);
    let starts: Vec<usize> = (section.start..section.end)
        .filter(|line| {
            parsed
                .lines
                .get(*line)
                .is_some_and(|line| matches!(line.kind, LineKind::JobEntry))
        })
        .collect();
    let start = *starts.get(index as usize)?;
    let mut end = starts
        .get(index as usize + 1)
        .copied()
        .unwrap_or(section.end);
    let lines: Vec<&str> = document.lines().collect();
    while end > start + 1
        && lines
            .get(end - 1)
            .is_some_and(|line| line.trim().is_empty())
    {
        end -= 1;
    }
    Some(RawSection {
        heading: None,
        kind: SectionKind::Experience,
        start,
        end,
    })
}

/// The employment entry at `index`, but ONLY when the document's own identity
/// line at that position names `company`.
///
/// **An ordinal is only valid inside the transaction that produced it.**
/// [`entry_range`] counts `LineKind::JobEntry` lines, and the roster index a
/// max-depth regenerate carries was assigned HOURS earlier, by a run whose
/// document may not be this one. Three reachable ways the two diverge, none of
/// them exotic:
///
/// * a section that failed or came back empty during the run — `assemble` skips
///   it, so the document has one fewer entry than the roster and every index
///   past the gap points at the NEXT employer;
/// * a roster entry with no dates — `assemble::identity_line` emits no date
///   column for it, so the line is not a `JobEntry` at all and the same shift
///   happens;
/// * a hand edit between the run and the click, which `ensure_latest_run`
///   deliberately permits.
///
/// Joining on position alone therefore spliced role B's rebuilt entry over role
/// C's lines: C deleted from the saved résumé, B duplicated, in one atomic
/// write the user only learns about from a post-hoc `factual.dropped_role`
/// Critical. A write path re-joining data across a time gap joins on IDENTITY —
/// so the company is read back out of the document with the same
/// [`split_entry`] that `assemble` renders the line for, and a mismatch is a
/// soft `None` the caller degrades on.
///
/// An empty `company` (an unattributed roster entry) can match nothing, so it
/// is a miss too: unattributed beats invented, the same call
/// `split_two_space_label` makes.
pub fn named_entry_range(document: &str, index: u8, company: &str) -> Option<RawSection> {
    let company = company.trim();
    if company.is_empty() {
        return None;
    }
    let entry = entry_range(document, index)?;
    let parsed = parse_resume(document);
    let (found, _title, _dates) = split_entry(parsed.lines.get(entry.start)?);
    found.trim().eq_ignore_ascii_case(company).then_some(entry)
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
/// Two things have to hold — the replacement must open with a heading line, and
/// it must carry at least one line of body under it. (An over-eager model that
/// answers with the whole résumé is caught by the splice being section-scoped:
/// the extra sections land inside this section's range and the validator sees
/// the duplicate.)
pub fn is_usable_replacement(replacement: &str) -> bool {
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
    opens_with_heading && lines.next().is_some()
}

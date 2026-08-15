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

//! Line-level recognition over the re-extracted text: section headings and the
//! candidate name (contact fields live in `contact.rs`). Reuses the parser's
//! heading vocabulary and `classify_section` (en/de/fr/es/it/nl/pt) — no list of its own.

use super::{AtsField, AtsSectionKind};
use crate::documents::evidence::{classify_section, SectionKind};
use crate::export::parser::{is_all_caps_section_heading, is_known_section_name};

use super::super::readback::normalize;

/// Longest line kept, in characters.
pub(super) const MAX_LINE_CHARS: usize = 2_000;
/// Most lines kept.
pub(super) const MAX_LINES: usize = 20_000;
/// A known heading is a vocabulary entry (longest ~26 bytes) or two joined with ` & `;
/// this is a generous ceiling, so a fused-heading boundary beyond it is never examined
/// (which keeps the split linear in the line length).
const MAX_HEADING_BYTES: usize = 128;

/// A line recognised as a section heading.
#[derive(Debug, Clone)]
pub(super) struct Heading {
    pub(super) idx: usize,
    pub(super) text: String,
    pub(super) kind: AtsSectionKind,
    /// `false` for an ALL-CAPS line the vocabulary does not recognise.
    pub(super) standard: bool,
}

/// Trimmed, non-empty lines of the extracted text, in extraction order.
///
/// Bounded: the text comes from an export request or a user-chosen file, so a line is
/// cut at [`MAX_LINE_CHARS`] and only the first [`MAX_LINES`] lines are kept. A real
/// résumé line is far shorter and a résumé far shorter than that.
pub(super) fn split_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| {
            let cut = l
                .char_indices()
                .nth(MAX_LINE_CHARS)
                .map_or(l.len(), |(i, _)| i);
            l[..cut].split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .filter(|l| !l.is_empty())
        .take(MAX_LINES)
        .collect()
}

fn kind_of(heading: &str) -> AtsSectionKind {
    match classify_section(heading) {
        SectionKind::Experience => AtsSectionKind::Experience,
        SectionKind::Education => AtsSectionKind::Education,
        SectionKind::Projects => AtsSectionKind::Projects,
        SectionKind::Skills => AtsSectionKind::Skills,
        SectionKind::Summary => AtsSectionKind::Summary,
        SectionKind::Other => AtsSectionKind::Other,
    }
}

fn heading_text(line: &str) -> &str {
    line.trim_end_matches(':').trim()
}

/// A STANDARD heading: a vocabulary name, or an ALL-CAPS heading the section
/// classifier recognises (`PROFESSIONAL EXPERIENCE`). An upper-case NAME is neither.
fn is_standard_heading(line: &str) -> bool {
    let text = heading_text(line);
    text.split_whitespace().count() <= 6
        && (is_known_section_name(text)
            || (is_all_caps_section_heading(text) && kind_of(text) != AtsSectionKind::Other))
}

/// Index of the first standard heading: the bound of the no-hint name scan.
pub(super) fn first_standard_heading(lines: &[String]) -> Option<usize> {
    lines.iter().position(|l| is_standard_heading(l))
}

/// Every heading-shaped line except `skip` (the name line).
pub(super) fn find_headings(lines: &[String], skip: Option<usize>) -> Vec<Heading> {
    let mut out = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let text = heading_text(line);
        if Some(idx) == skip || text.split_whitespace().count() > 6 {
            continue;
        }
        let known = is_known_section_name(text);
        if !known && !is_all_caps_section_heading(text) {
            continue;
        }
        let kind = kind_of(text);
        out.push(Heading {
            idx,
            text: text.to_string(),
            kind,
            standard: known || kind != AtsSectionKind::Other,
        });
    }
    out
}

/// Split a known ALL-CAPS heading off the text fused onto it
/// (`WORK EXPERIENCEAcme Corp …`, `SKILLS• Rust`).
///
/// This is pdf-extract glyph spacing, not a defect in the PDF: `pdftotext` reads the
/// same files with each heading on its own line (verified on a SwissMinimal render),
/// so a split is reported as a normal heading — the user cannot act on it.
pub(super) fn split_fused_headings(lines: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let cut = line
            .char_indices()
            .skip(4)
            .take_while(|(i, _)| *i <= MAX_HEADING_BYTES)
            .find_map(|(i, c)| {
                let rest = &line[i..];
                let next_lower = rest.chars().nth(1).is_some_and(char::is_lowercase);
                let boundary = (c.is_uppercase() && next_lower) || super::roles::is_bullet(c);
                let head = line[..i].trim();
                (boundary && !head.chars().any(char::is_lowercase) && is_known_section_name(head))
                    .then_some(i)
            });
        match cut {
            Some(i) => {
                out.push(line[..i].trim().to_string());
                out.push(line[i..].trim().to_string());
            }
            None => out.push(line),
        }
    }
    out
}

/// Lowercase particles that sit between the capitalised words of a name
/// (`Jan van der Berg`, `Maria de la Cruz`, `Omar bin Salem`).
const NAME_PARTICLES: &[&str] = &[
    "von", "van", "der", "den", "de", "del", "della", "di", "da", "do", "dos", "du", "la", "le",
    "bin", "ibn", "al", "el", "ter", "ten", "zu", "y", "e",
];

/// Uppercase, or a letter of an uncased script (CJK, Arabic, Hebrew…).
fn is_cap(c: char) -> bool {
    c.is_uppercase() || (c.is_alphabetic() && !c.is_lowercase())
}

fn name_word(w: &str) -> bool {
    w.chars().next().is_some_and(is_cap)
        && w.chars().all(|c| c.is_alphabetic() || "-'’.".contains(c))
}

/// A trailing `, PhD` / `, MBA` / `, M.Sc.`: one or two short tokens.
fn is_credential(tail: &str) -> bool {
    let words: Vec<&str> = tail.split_whitespace().collect();
    (1..=2).contains(&words.len())
        && words.iter().all(|w| {
            let caps = w.chars().filter(|c| c.is_uppercase()).count();
            w.chars().count() <= 10
                && (caps >= 2 || w.contains('.'))
                && w.chars().next().is_some_and(char::is_uppercase)
                && w.chars().all(|c| c.is_alphanumeric() || c == '.')
        })
}

/// The shape of a person's name: 2-5 capitalised words with optional lowercase
/// particles between them, an optional `, CREDENTIAL`, or one uncased-script token.
/// Never a heading (an ALL-CAPS line the section classifier recognises is not a name).
fn name_shaped(s: &str) -> bool {
    let (base, credential) = match s.split_once(',') {
        Some((b, c)) => (b.trim(), Some(c.trim())),
        None => (s.trim(), None),
    };
    if credential.is_some_and(|c| !is_credential(c)) || s.chars().count() > 48 {
        return false;
    }
    if is_known_section_name(base)
        || (is_all_caps_section_heading(base)
            && (kind_of(base) != AtsSectionKind::Other || base.split_whitespace().count() > 4))
    {
        return false;
    }
    let words: Vec<&str> = base.split_whitespace().collect();
    if words.len() == 1 {
        let w = words[0];
        return (2..=8).contains(&w.chars().count())
            && w.chars()
                .all(|c| c.is_alphabetic() && !c.is_uppercase() && !c.is_lowercase());
    }
    let last = words.len().wrapping_sub(1);
    (2..=5).contains(&words.len())
        && words
            .iter()
            .enumerate()
            .all(|(i, w)| name_word(w) || (i != 0 && i != last && NAME_PARTICLES.contains(w)))
}

/// A glyph run fused to the next text (`Jane DoeBerlin, Germany`): the name
/// prefix and the rest, split at the first lower-to-upper boundary.
pub(super) fn split_name_prefix(line: &str) -> Option<(&str, &str)> {
    let mut prev: Option<char> = None;
    for (i, c) in line.char_indices() {
        if prev.is_some_and(char::is_lowercase)
            && c.is_uppercase()
            && line[i..].chars().nth(1).is_some_and(char::is_lowercase)
        {
            let (head, rest) = line.split_at(i);
            return name_shaped(head).then_some((head, rest));
        }
        prev = Some(c);
    }
    None
}

/// The candidate name and the line it sits on.
///
/// With the source name (`hint`, known on export) the extracted text must
/// still contain it; without one the name is the first name-shaped line (or
/// fused name prefix) before the first heading.
pub(super) fn find_name(
    lines: &[String],
    hint: Option<&str>,
    first_heading: Option<usize>,
) -> (AtsField, Option<usize>) {
    let hint = hint.map(str::trim).filter(|h| !normalize(h).is_empty());
    if let Some(h) = hint {
        let needle = normalize(h);
        // Whole words only (`Li` is not in `Lisa`). A name fused to the next glyph run
        // (`Jane DoeBerlin`, `Jane Doejane@example.com`) still counts: through its split
        // prefix, or — for a multi-word name only — as the start of the line.
        let padded = format!(" {needle} ");
        let multi_word = needle.contains(' ');
        let has_name = |l: &String| {
            let norm = normalize(l);
            format!(" {norm} ").contains(&padded)
                || (multi_word && norm.starts_with(&needle))
                || split_name_prefix(l).is_some_and(|(head, _)| normalize(head) == needle)
        };
        return match lines.iter().position(has_name) {
            Some(i) => (AtsField::found(h), Some(i)),
            None => (AtsField::missing(), None),
        };
    }
    let limit = first_heading.unwrap_or(lines.len()).min(8);
    for (i, line) in lines.iter().take(limit).enumerate() {
        if name_shaped(line) {
            return (AtsField::found(line), Some(i));
        }
        if let Some((head, _)) = split_name_prefix(line) {
            return (AtsField::found(head), Some(i));
        }
    }
    (AtsField::missing(), None)
}

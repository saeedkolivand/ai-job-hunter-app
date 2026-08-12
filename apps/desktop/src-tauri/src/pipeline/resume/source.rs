//! Reading the SOURCE résumé — the seeds a max-depth section is generated from.
//!
//! Everything here is a pure function of the candidate's own document, and
//! everything it returns is a fact the model is NOT allowed to author: a
//! project's links, the education lines, which sections the source actually
//! has. The section generator asks the model how to PRESENT these; it never
//! asks for them back.
//!
//! ## Line fidelity
//!
//! `export::parser::parse_resume` emits exactly one
//! [`ParsedLine`] per `text.lines()` entry — the same one-to-one property
//! `stages::sections` relies on to address a section by line range — so this
//! module zips the parse with the ORIGINAL lines and keeps both. The parse
//! answers "what kind of line is this"; the original answers "what did the
//! candidate actually write", which is what a seeded education entry has to
//! carry to still be verbatim.
//!
//! ## One grader, not two
//!
//! Project entry boundaries, link detection and the description-line cap all
//! come from `validate::content` (re-exported there for this reason). The
//! validators grade the OUTPUT of this seeding, so a second definition here
//! would let a truthful document fail a check because the two halves disagreed
//! about where an entry starts.

use crate::documents::evidence::{classify_section, SectionKind};
use crate::export::parser::parse_resume;
use crate::export::types::{LineKind, ParsedLine};
use crate::validate::content::{
    canonical_link, names_a_resource, project_entry_starts, urls_in, MAX_PROJECT_DESCRIPTION_LINES,
};

use super::types_max::ProjectOut;

/// One line of the source, as written AND as classified.
#[derive(Debug, Clone)]
pub struct SourceLine {
    /// The line exactly as the candidate wrote it, trailing whitespace trimmed.
    pub raw: String,
    pub parsed: ParsedLine,
}

/// One section of the source résumé.
#[derive(Debug, Clone)]
pub struct SourceSection {
    /// The heading line as the source wrote it.
    pub heading: String,
    /// The content lines under it, heading excluded.
    pub lines: Vec<SourceLine>,
}

impl SourceSection {
    /// The content lines as text, blank lines dropped.
    pub fn text_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .map(|line| line.raw.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }
}

/// The source's section of `kind`, or `None` when it has none.
///
/// The FIRST match wins: a document with two headings that both classify as
/// Projects has one projects section as far as every other reader in this
/// codebase is concerned (`validate::content::Analysis::section_of_kind` takes
/// the first too), and picking a different one here would seed from a section
/// the validator does not grade.
pub fn section(source: &str, kind: SectionKind) -> Option<SourceSection> {
    let raw: Vec<&str> = source.lines().collect();
    let parsed = parse_resume(source);
    let mut current: Option<SourceSection> = None;
    for (index, line) in parsed.lines.iter().enumerate() {
        if matches!(line.kind, LineKind::SectionHeader) {
            if current.is_some() {
                return current; // the wanted section ended at the next heading
            }
            if classify_section(&line.text) == kind {
                current = Some(SourceSection {
                    heading: line.text.clone(),
                    lines: Vec::new(),
                });
            }
            continue;
        }
        if let Some(section) = current.as_mut() {
            section.lines.push(SourceLine {
                raw: raw.get(index).unwrap_or(&"").trim_end().to_string(),
                parsed: line.clone(),
            });
        }
    }
    current
}

/// The source's education entries, verbatim.
///
/// A degree, an institution and a year are facts; the max-depth education
/// "generation" is a SELECTION over exactly these strings, so this is the
/// allow-list the model's answer is filtered against.
pub fn education_lines(source: &str) -> Vec<String> {
    section(source, SectionKind::Education)
        .map(|section| section.text_lines())
        .unwrap_or_default()
}

/// The source's project entries, seeded into [`ProjectOut`]s.
///
/// Per the owner-locked signature, ONE entry is:
///
/// ```text
/// **Name** · app-link · repo-link      <- title line, links live here
/// Rust · SQLite · Clap                 <- stack line, `·`-separated
/// A short prose description.           <- 1..=MAX_PROJECT_DESCRIPTION_LINES
/// ```
///
/// and the two lower tiers of the degradation ladder are the same shape with
/// the trailing parts absent. What is seeded is `name`, `links` and `stack`; the
/// `description` field carries the SOURCE's own description, which is what the
/// merge step uses to decide whether the model is allowed to write one at all
/// (no source description ⇒ no generated description, ever — absence of data
/// produces the compact form, never filler).
///
/// The stack line is read only when it carries a separator, exactly as
/// `consistency::tier_of` decides the tier: a second line without one is a
/// description that started early, and reading it as a stack would turn the
/// candidate's prose into a `·`-separated technology list.
pub fn seed_projects(source: &str) -> Vec<ProjectOut> {
    let Some(section) = section(source, SectionKind::Projects) else {
        return Vec::new();
    };
    entries(&section)
        .into_iter()
        .filter_map(|entry| seed_one_project(&entry))
        .collect()
}

/// The separator set the locked project signature uses. Same three characters
/// `consistency::tier_of` and `factual::project_entry_name` split on.
const PROJECT_SEPARATORS: [char; 3] = ['·', '|', '•'];

/// Group one section's non-blank lines into entries, using the SHARED
/// entry-opening rule.
fn entries(section: &SourceSection) -> Vec<Vec<&SourceLine>> {
    let mut out: Vec<Vec<&SourceLine>> = Vec::new();
    for line in section
        .lines
        .iter()
        .filter(|l| !matches!(l.parsed.kind, LineKind::Blank) && !l.parsed.text.trim().is_empty())
    {
        if project_entry_starts(&line.parsed) || out.is_empty() {
            out.push(vec![line]);
        } else if let Some(last) = out.last_mut() {
            last.push(line);
        }
    }
    out
}

/// Seed ONE project from its grouped lines. `None` for an entry with no name —
/// unnamable is unmatchable (`factual::project_entry_name` says the same), and
/// an entry with no name cannot be re-rendered as the candidate's own.
fn seed_one_project(entry: &[&SourceLine]) -> Option<ProjectOut> {
    let title = entry.first()?;
    let head = title
        .parsed
        .text
        .split(PROJECT_SEPARATORS)
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if head.is_empty() {
        return None;
    }

    // Links: the title line's, plus any a description line carries. The STACK
    // line is filtered through the shared resource test — a bare `crates.io` on
    // a technology list is the ecosystem, not a link, and `names_a_resource` is
    // the same call the link Critical makes about it.
    let mut links: Vec<String> = Vec::new();
    for (index, line) in entry.iter().enumerate() {
        let found = urls_in(&line.parsed.text);
        for url in found {
            if index == 1 && !names_a_resource(&url) {
                continue;
            }
            if !links
                .iter()
                .any(|kept| canonical_link(kept) == canonical_link(&url))
            {
                links.push(url);
            }
        }
    }

    let stack_line = entry
        .get(1)
        .filter(|line| line.parsed.text.contains(PROJECT_SEPARATORS));
    let stack: Vec<String> = stack_line
        .map(|line| {
            line.parsed
                .text
                .split(PROJECT_SEPARATORS)
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty() && urls_in(item).is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Everything after the title (and the stack line, when there was one) is the
    // description, capped at the same number of lines the structure check
    // accepts — past that the entry has stopped being a project card.
    let first_description = if stack_line.is_some() { 2 } else { 1 };
    let description = entry
        .iter()
        .skip(first_description)
        .take(MAX_PROJECT_DESCRIPTION_LINES)
        .map(|line| line.parsed.text.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<&str>>()
        .join(" ");

    Some(ProjectOut {
        name: head,
        links,
        stack,
        description,
    })
}

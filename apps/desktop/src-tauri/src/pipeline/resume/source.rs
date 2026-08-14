//! Reading the SOURCE résumé — extracting seeds for deterministic normalization.
//!
//! Everything here is a pure function of the candidate's own document, and
//! everything it returns is a fact the model is NOT allowed to author: a
//! project's links, the education lines, which sections the source actually
//! has. The strategy stage asks the model how to PRESENT these; it never
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

use std::sync::LazyLock;

use regex::Regex;

use crate::documents::evidence::{classify_section, SectionKind};
use crate::export::parser::parse_resume;
use crate::export::types::{LineKind, ParsedDocument, ParsedLine};
use crate::validate::content::{
    canonical_link, link_href, names_a_resource, project_entry_starts, urls_in,
    MAX_PROJECT_DESCRIPTION_LINES,
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
    section_from_parsed(source, kind, &parse_resume(source))
}

/// [`section`] over an ALREADY-parsed document — the same seam
/// `stages::sections::split_parsed` offers, for a caller that also needs a
/// [`stages::sections::RawSection`](crate::pipeline::resume::stages::sections::RawSection)
/// over the SAME text and would otherwise pay for a second `parse_resume`
/// (`pipeline::resume::projects::build` is exactly that caller).
pub(crate) fn section_from_parsed(
    source: &str,
    kind: SectionKind,
    parsed: &ParsedDocument,
) -> Option<SourceSection> {
    let raw: Vec<&str> = source.lines().collect();
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
///
/// A link written as a markdown span (`[Website](href)`) is kept VERBATIM in
/// `links` rather than reduced to its bare href — see [`seed_one_project`].
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

/// A markdown link span in résumé prose: `[label](href)`. Captured VERBATIM by
/// [`seed_one_project`] — the export renderer (`model::rich::split_urls`)
/// already turns exactly this shape into a clickable label, so keeping the
/// span intact here is what carries a source's "Website"/"Github" labels
/// through seeding and re-rendering instead of losing them to the bare href.
///
/// The href group is deliberately UNRESTRICTED (not anchored to `https?://`):
/// what makes a captured group an actual link is decided AFTER the match, by
/// running it back through [`urls_in`] — the same grader-facing extractor,
/// which also accepts a scheme-less `www.` host and a handful of others (see
/// `factual::URL_RE`). Anchoring this regex to `https?://` would silently
/// drop the label off `[Website](www.example.com)`.
static MD_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap());

/// Group one section's non-blank lines into entries, using the SHARED
/// entry-opening rule.
///
/// `pub(crate)`: [`crate::pipeline::resume::projects::normalize_projects`]
/// groups the DRAFT's own Projects section through this exact function, so
/// the seeder and the normalizer can never disagree about where an entry
/// starts.
pub(crate) fn entries(section: &SourceSection) -> Vec<Vec<&SourceLine>> {
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
///
/// `pub(crate)`: [`crate::pipeline::resume::projects::normalize_projects`]
/// seeds the DRAFT's own Projects section through this exact function, so the
/// seeder and the normalizer can never disagree about what counts as a link.
pub(crate) fn seed_one_project(entry: &[&SourceLine]) -> Option<ProjectOut> {
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

    // Links: the title line's, plus any a description line carries. A
    // markdown span (`[Website](href)`) is captured FIRST and kept VERBATIM —
    // the label is what the export renderer turns into a clickable name — and
    // every dedup comparison below goes through `link_href` so a labeled and a
    // bare copy of the same URL are never counted as two links. The STACK
    // line is filtered through the shared resource test — a bare `crates.io`
    // on a technology list is the ecosystem, not a link, and
    // `names_a_resource` is the same call the link Critical makes about it.
    let mut links: Vec<String> = Vec::new();
    for (index, line) in entry.iter().enumerate() {
        let text = &line.parsed.text;
        let mut spanned: Vec<String> = Vec::new(); // hrefs already captured as a labeled span
                                                   // Byte ranges of matches actually CONSUMED as a link — the ONLY ones
                                                   // stripped before the bare-URL pass below. A match the single-URL or
                                                   // resource guard SKIPPED (a `[1](note)` footnote, a multi-URL
                                                   // parenthetical) is left in place, or its own bare URL(s) would be
                                                   // silently deleted along with the bracket text around them instead of
                                                   // reaching that pass.
        let mut captured_ranges: Vec<(usize, usize)> = Vec::new();
        for caps in MD_LINK_RE.captures_iter(text) {
            let whole = caps.get(0).unwrap();
            // The bracketed content is only a LINK when its href is one by
            // the SAME rule `urls_in` grades with — a `[1](note)` footnote or
            // a URL-shaped LABEL paired with an unrelated href is not one, and
            // matching a SINGLE candidate keeps a multi-URL parenthetical
            // (rare, but not a link either) out too.
            let candidates = urls_in(&caps[2]);
            let [href] = candidates.as_slice() else {
                continue; // not a link at all — leave it for the bare pass
            };
            if index == 1 && !names_a_resource(href) {
                continue; // a stack-line non-resource — leave it too
            }
            captured_ranges.push((whole.start(), whole.end()));
            spanned.push(canonical_link(href));
            if links
                .iter()
                .any(|kept| canonical_link(link_href(kept)) == canonical_link(href))
            {
                continue;
            }
            // Only WRITE the labeled span when it round-trips: balanced
            // brackets, and re-extracting it through `urls_in` yields exactly
            // this href back. An href containing its own unescaped `)` (a
            // Wikipedia-shaped URL) truncates this regex's own capture, which
            // would otherwise write an unbalanced, unparseable span into the
            // document. The bare (verified) href is always safe.
            let span = whole.as_str().to_string();
            let balanced = span.matches('(').count() == span.matches(')').count();
            let round_trips = urls_in(&span) == [href.clone()];
            links.push(if balanced && round_trips {
                span
            } else {
                href.clone()
            });
        }
        // Bare URLs, over the line with only the CAPTURED spans blanked out
        // — otherwise a URL-shaped LABEL (`[https://other](https://real)`)
        // gets harvested a second time as if it were its own link, while a
        // SKIPPED span's own bare URL(s) still reach this pass.
        let mut stripped = text.clone();
        for (start, end) in captured_ranges.iter().rev() {
            stripped.replace_range(*start..*end, " ");
        }
        for url in urls_in(&stripped) {
            if index == 1 && !names_a_resource(&url) {
                continue;
            }
            if spanned.contains(&canonical_link(&url)) {
                continue; // already captured as part of a labeled span above
            }
            if !links
                .iter()
                .any(|kept| canonical_link(link_href(kept)) == canonical_link(&url))
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

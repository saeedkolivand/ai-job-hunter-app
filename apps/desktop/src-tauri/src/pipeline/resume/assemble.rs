//! Rendering primitives shared by the deterministic Projects normalization
//! (`pipeline::resume::projects`) and the section-splice utilities
//! (`stages::sections`).
//!
//! This module used to render a whole max-depth document from its finished
//! sections. That machinery was removed with the `max` generation depth;
//! [`render_project`] and [`identity_line`] survive because they are also how
//! PR #990's deterministic Projects normalization and the repair loop's
//! section splicing render an entry — unrelated to depth.

use crate::pipeline::resume::types_max::ProjectOut;

/// The `·` the locked project signature separates with. A constant because it
/// is the format, not a preference.
const PROJECT_SEPARATOR: &str = " · ";

/// The two spaces that make an employment entry's date column a DATE COLUMN.
///
/// `export::parser` recognizes an entry by "a 2+ space gap before a date range",
/// and `documents::evidence::split_entry` reads the identity back out of exactly
/// that shape.
///
/// **What one space actually costs, executed rather than assumed:** the line
/// stops being a `LineKind::JobEntry` for every reader — the exporters' bold
/// entry-title treatment, `consistency.date_order`'s date COLUMN, and
/// `split_entry`'s two-space arm — and `extract_evidence` then attaches it to
/// the previous role as a bullet, so a run's own evidence reader sees one
/// unattributed role instead of two. It does NOT raise
/// `factual.dropped_role`: that check substring-searches the whole document for
/// the company NAME, which survives any spacing. The damage is silent, which is
/// why the guard is a parse assertion and not a report assertion.
const DATE_COLUMN_GAP: &str = "  ";

/// ONE employment entry's identity line, in the form
/// `documents::evidence::split_entry` reads back as `(company, title, dates)`.
///
/// `Title, Company  Dates` is that form: the two-space arm takes everything
/// before the gap as the label and the LAST comma segment of the label as the
/// company. Rendering it any other way would be a second entry grammar for the
/// parser to guess at — and the guess it makes wrong is which half is the
/// employer.
///
/// Degrades rather than inventing punctuation it has nothing to put around: an
/// entry with no title is `Company  Dates`, and one with no dates is the bare
/// label (exactly what the source itself would have looked like, since the
/// identity was seeded from it).
/// `pub(crate)` for its readers: `stages::sections::named_entry_range` matches
/// a condensed group by comparing this function's own output against the
/// document line, because that group's "company" is a label and no parser can
/// read it back. Calling the renderer is what makes that comparison exact.
pub(crate) fn identity_line(company: &str, title: &str, dates: &str) -> String {
    let company = company.trim();
    let title = title.trim();
    let dates = dates.trim();
    let label = match (title.is_empty(), company.is_empty()) {
        (false, false) => format!("{title}, {company}"),
        (true, false) => company.to_string(),
        (false, true) => title.to_string(),
        (true, true) => String::new(),
    };
    if dates.is_empty() || label.is_empty() {
        return label;
    }
    format!("{label}{DATE_COLUMN_GAP}{dates}")
}

/// ONE project, at whichever tier of the ladder its SEEDED data supports.
///
/// `pub(crate)`: [`crate::pipeline::resume::projects::normalize_projects`]
/// renders the quality-depth Projects section through this SAME ladder, so
/// the two cannot silently diverge.
pub(crate) fn render_project(project: &ProjectOut) -> String {
    let name = project.name.trim();
    let links = project.links.join(PROJECT_SEPARATOR);
    // Tier 3: no stack and no description. A bullet, so the entry still OPENS
    // an entry for `project_entry_starts` (which reads a bullet or a bold run),
    // and a link line the reader can follow — the honest form for a project the
    // source says nothing else about.
    if project.stack.is_empty() && project.description.trim().is_empty() {
        return if links.is_empty() {
            format!("• {name}")
        } else {
            format!("• {name}{PROJECT_SEPARATOR}{links}")
        };
    }
    // Tiers 1 and 2 open with the bold name, which is what makes the title line
    // an entry start for both graders.
    let mut entry = if links.is_empty() {
        format!("**{name}**")
    } else {
        format!("**{name}**{PROJECT_SEPARATOR}{links}")
    };
    if !project.stack.is_empty() {
        entry.push('\n');
        entry.push_str(&project.stack.join(PROJECT_SEPARATOR));
    }
    let description = project.description.trim();
    if !description.is_empty() {
        entry.push('\n');
        entry.push_str(description);
    }
    entry
}

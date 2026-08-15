//! Rendering primitives shared by the deterministic Projects normalization
//! (`pipeline::resume::projects`) and the section-splice utilities
//! (`stages::sections`).
//!
//! This module used to render a whole max-depth document from its finished
//! sections. That machinery was removed with the `max` generation depth;
//! [`render_project`] survives because it is also how PR #990's
//! deterministic Projects normalization renders an entry — unrelated to
//! depth.

use crate::pipeline::resume::types_max::ProjectOut;

/// The `·` the locked project signature separates with. A constant because it
/// is the format, not a preference.
const PROJECT_SEPARATOR: &str = " · ";

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

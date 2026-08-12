//! `assemble` — ZERO provider calls. The finished sections become the SAME
//! plain-text résumé body the quality-depth draft produces.
//!
//! That sameness is the whole point. Everything downstream of this file —
//! `validate`, `repair`, the persisted report, the editor, the exporters,
//! ADR-0021's header seeding — reads a résumé BODY, and a max-depth run must
//! not hand any of them a second shape to understand. So the layout is code, not
//! model output: the model was asked for bullets, a paragraph, a description and
//! an ordering, and every line break, marker, separator and heading below is
//! decided here.
//!
//! ## No contact header (ADR-0021)
//!
//! The body starts at the first SECTION HEADING. The contact header belongs to
//! the editor at export time, so this renderer has no way to emit one — there is
//! no name, no email and no link anywhere in a [`SectionResult`], which is the
//! same guarantee `SectionKey` gives by having no `Header` variant.
//!
//! ## The projects degradation ladder
//!
//! Three tiers, picked from the SEEDED fields, never filled in:
//!
//! ```text
//! **Ledger CLI** · https://ledger.dev · https://github.com/x/ledger   tier 1
//! Rust · SQLite · Clap                                                (stack + description)
//! A double-entry bookkeeping tool used by two small businesses.
//!
//! **CrossKit** · https://crosskit.dev                                 tier 2
//! TypeScript · Vite                                                   (stack, no description)
//!
//! • Dotfiles · https://github.com/x/dotfiles                          tier 3
//! ```                                                                 (links only)
//!
//! A project with no stack and no description renders as the compact link line
//! — never as an entry with an invented blurb under it. `consistency::tier_of`
//! accepts exactly these three shapes, which is why the tests below assemble a
//! document and then run the real validator over it rather than asserting on
//! strings alone.

use async_trait::async_trait;
use serde_json::json;

use crate::documents::evidence::{classify_section, SectionKind};
use crate::error::AppResult;
use crate::pipeline::resume::types::{SectionKey, SkillGroup};
use crate::pipeline::resume::types_max::{ProjectOut, SectionBody, SectionResult};
use crate::pipeline::resume::QualityCtx;
use crate::pipeline::Stage;

pub struct Assemble;

pub const NAME: &str = "assemble";

/// The `·` the locked project signature separates with, and the `, ` a skills
/// group joins with. Constants because they are the format, not a preference.
const PROJECT_SEPARATOR: &str = " · ";
const SKILL_SEPARATOR: &str = ", ";

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

/// Render the whole document from its finished sections.
///
/// `section_order` is the strategy's own ordering (the model may say Skills
/// before Experience for a career changer); anything it does not name keeps its
/// planned position, so an unrecognized entry can reorder nothing. Consecutive
/// employment entries share ONE heading — a résumé has one "Work Experience"
/// heading holding several entries, and emitting one heading per entry would
/// produce a document with nine of them.
pub fn assemble(sections: &[SectionResult], section_order: &[String]) -> String {
    let ordered = order(sections, section_order);
    let mut out = String::new();
    let mut last_heading: Option<&str> = None;
    for section in ordered {
        if section.is_empty() {
            continue;
        }
        let repeat = last_heading != Some(section.heading.as_str());
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&chunk(section, repeat));
        last_heading = Some(&section.heading);
    }
    out.push('\n');
    out
}

/// ONE section as it appears in the document: its heading (when it opens a new
/// one) and its body.
///
/// The unit the progressive stream appends, so what the user watches build is
/// byte-for-byte what the finished document holds — not a preview of it.
pub fn chunk(section: &SectionResult, with_heading: bool) -> String {
    let body = body(section);
    if with_heading {
        format!("{}\n\n{body}", section.heading.trim())
    } else {
        body
    }
}

/// One section's body, heading excluded.
fn body(section: &SectionResult) -> String {
    match &section.body {
        SectionBody::Summary(text) => text.trim().to_string(),
        SectionBody::Skills(groups) => groups
            .iter()
            .filter(|group| !group.skills.is_empty())
            .map(render_group)
            .collect::<Vec<String>>()
            .join("\n"),
        SectionBody::Entry { plan, bullets } => {
            let mut entry = identity_line(&plan.company, &plan.title, &plan.dates);
            for bullet in bullets {
                entry.push_str("\n- ");
                entry.push_str(bullet.trim());
            }
            entry
        }
        SectionBody::Projects(projects) => projects
            .iter()
            .map(render_project)
            .collect::<Vec<String>>()
            .join("\n\n"),
        SectionBody::Lines(lines) => lines
            .iter()
            .map(|line| line.trim().to_string())
            .collect::<Vec<String>>()
            .join("\n"),
    }
}

fn render_group(group: &SkillGroup) -> String {
    let skills = group.skills.join(SKILL_SEPARATOR);
    if group.label.trim().is_empty() {
        skills
    } else {
        format!("{}: {skills}", group.label.trim())
    }
}

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
fn identity_line(company: &str, title: &str, dates: &str) -> String {
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
fn render_project(project: &ProjectOut) -> String {
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

/// Sort the finished sections into the strategy's order.
///
/// A section the order does not name keeps its PLANNED position rather than
/// being pushed to either end: the planned order is already the conventional
/// one, and a model that names three of five sections has expressed an opinion
/// about three of them.
///
/// **A sort says the opposite of that sentence, whatever key you give it.**
/// Ranking an unnamed kind at `usize::MAX` sorts it to the END, so a
/// `section_order` of just `["experience"]` — an ordinary partial answer, and
/// exactly what a model returns when it only cares about one thing — rendered
/// Experience FIRST and pushed Summary, Skills, Projects and Education behind
/// it. Ranking it at `0` inverts the same bug. Carrying an unnamed section
/// along with the last named one before it in the plan (tried, and rejected by
/// its own test) wedges every unnamed section between two named ones the model
/// asked to put together: `["skills", "summary"]` then rendered Skills,
/// Experience, Projects, Education, Summary.
///
/// So this is not a sort. It is a PERMUTATION OF THE NAMED SECTIONS' OWN SLOTS:
/// the positions the named sections already occupy are the only positions a
/// reorder may write to, and the named sections are dealt back into them in the
/// model's order. A section the order does not name therefore keeps its planned
/// index literally — it cannot move, because nothing writes to its slot.
///
/// `["skills", "summary"]` over the planned `[Summary, Skills, Experience…]`
/// then swaps exactly those two and leaves the rest alone, which is what the
/// model asked for; `["experience"]` over the same plan is a no-op, which is
/// the honest reading of an opinion about one section and nothing else.
///
/// The tie-break inside a rank is the PLANNED index, so two employment entries
/// (same kind, same rank) keep the roster's order — the one ordering the model
/// may not touch — without relying on the sort being stable.
fn order<'a>(sections: &'a [SectionResult], section_order: &[String]) -> Vec<&'a SectionResult> {
    let wanted: Vec<SectionKind> = section_order
        .iter()
        .map(|name| classify_section(name))
        .filter(|kind| !matches!(kind, SectionKind::Other))
        .collect();
    let mut ordered: Vec<&SectionResult> = sections.iter().collect();
    if wanted.is_empty() {
        return ordered;
    }
    let mut named: Vec<(usize, usize)> = sections
        .iter()
        .enumerate()
        .filter_map(|(index, section)| {
            wanted
                .iter()
                .position(|candidate| *candidate == kind_of(section.key))
                .map(|rank| (rank, index))
        })
        .collect();
    // Ascending by construction (`enumerate`), which is what makes "deal the
    // named sections back into their own slots" well defined.
    let slots: Vec<usize> = named.iter().map(|(_, index)| *index).collect();
    named.sort_by_key(|(rank, index)| (*rank, *index));
    for (slot, (_, from)) in slots.into_iter().zip(named) {
        ordered[slot] = &sections[from];
    }
    ordered
}

fn kind_of(key: SectionKey) -> SectionKind {
    match key {
        SectionKey::Summary => SectionKind::Summary,
        SectionKey::Skills => SectionKind::Skills,
        SectionKey::Experience(_) => SectionKind::Experience,
        SectionKey::Projects => SectionKind::Projects,
        SectionKey::Education => SectionKind::Education,
    }
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Assemble {
    fn name(&self) -> &'static str {
        NAME
    }

    /// Pure — the module title's "ZERO provider calls", as something the run
    /// can act on. It is what lets a fan-out stopped by the clock still become
    /// a document instead of eleven paid answers nobody ever sees.
    fn costs_a_provider_call(&self) -> bool {
        false
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let document = assemble(&ctx.sections, &ctx.strategy.section_order);
        // Length and section count only — never the document (ADR-027). The
        // same two numbers the `draft` stage records, so a run's artifact trail
        // reads the same at both depths.
        ctx.ledger.record(
            NAME,
            json!({
                "chars": document.chars().count(),
                "lines": document.lines().count(),
                "sections": ctx.sections.len(),
            }),
        );
        ctx.draft = document;
        Ok(())
    }
}

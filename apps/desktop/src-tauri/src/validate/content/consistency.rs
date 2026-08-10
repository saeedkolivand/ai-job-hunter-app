//! Internal consistency — the document contradicting itself or its source.
//!
//! All Warnings. Each of these has a legitimate explanation often enough
//! (a promotion, a role renamed for clarity, a skill genuinely used but not
//! written up) that a Critical would be wrong; what the user needs is a
//! pointer, not a block.

use std::collections::HashSet;

use crate::documents::evidence::{function_words, identity_tokens, years_in, SectionKind};
use crate::export::types::{LineKind, ParsedLine};

use super::factual::{MAX_SCANNED_ENTRIES, MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS};
use super::{
    issue, Analysis, ContentIssue, Section, CONSISTENCY_DATE_ORDER, CONSISTENCY_PROJECT_STRUCTURE,
    CONSISTENCY_SKILL_NOT_DEMONSTRATED, CONSISTENCY_TITLE_DRIFT,
};

/// Description lines a full project entry may carry after its stack line.
/// Beyond this the entry has stopped being a project card and become prose.
pub const MAX_PROJECT_DESCRIPTION_LINES: usize = 3;

/// `consistency.date_order` — a span whose start year is after its end year.
fn date_order_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = Vec::new();
    for section in &ctx.generated_sections {
        for line in &section.lines {
            let dates = line.right_text.as_deref().unwrap_or(&line.text);
            let years = years_in(dates);
            // Only a genuine two-ended span can be out of order; a single year
            // or a "2021 – Present" span has nothing to compare.
            if let [start, end] = years[..] {
                if start > end {
                    issues.push(issue(
                        CONSISTENCY_DATE_ORDER,
                        section.heading.as_deref(),
                        format!(
                            "This entry runs from {start} to {end} — the end date is before the \
                             start. Swap them."
                        ),
                        Some(dates.trim().to_string()),
                    ));
                }
            }
        }
    }
    issues
}

/// `(company-token set, title)` for every employment entry in `sections`, up to
/// [`MAX_SCANNED_ENTRIES`].
///
/// The cap is here rather than at the call site because BOTH sides of
/// [`title_drift_issues`]' comparison come through this function, and the loop
/// it feeds is O(generated entries × source entries). Same rationale, same
/// constant, as `factual::dropped_role_issues` — imported rather than
/// re-declared so the two scans can never disagree about the bound.
fn titled_entries(sections: &[Section]) -> Vec<(HashSet<String>, String)> {
    sections
        .iter()
        .filter(|s| s.kind == SectionKind::Experience)
        .flat_map(|s| s.lines.iter())
        .filter(|l| matches!(l.kind, LineKind::JobEntry))
        .take(MAX_SCANNED_ENTRIES)
        .map(|l| {
            let label = l.text.as_str();
            // "Senior Engineer | Acme Corp | 2021 – Present" and
            // "Senior Engineer, Acme Corp" both put the title first.
            let (title, company) = label
                .split_once(['|', '·', '•', ','])
                .map(|(t, c)| (t.trim().to_string(), c.to_string()))
                .unwrap_or_else(|| (String::new(), label.to_string()));
            // Only tokens that name a SPECIFIC employer may match two entries to
            // each other, so the identity set is `documents::evidence`'s —
            // the same legal-form and geography exclusions `factual` decides
            // survival on. Sharing a "GmbH" (or a "Berlin") made two unrelated
            // employers the same employer, and the second entry's title was
            // then reported as drift from the first one's on a document that
            // was byte-identical to its source.
            //
            // Digits go on top of that, and only here: the split above keeps
            // the date span inside `company`, so two unrelated employers that
            // merely ended and began in the same year shared "2018".
            let tokens = identity_tokens(&company, MIN_DISTINCTIVE_COMPANY_TOKEN_CHARS)
                .into_iter()
                .filter(|t| !t.chars().any(|c| c.is_ascii_digit()))
                .collect();
            (tokens, title)
        })
        .collect()
}

/// `consistency.title_drift` — the same employer, a different job title.
///
/// Matched on shared company tokens, compared on title tokens. Fires only when
/// the two titles share NO content token at all: "Senior Engineer" → "Staff
/// Engineer" is a promotion the candidate may have had, while "Senior Engineer"
/// → "Product Manager" is the drift worth surfacing.
fn title_drift_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let source = titled_entries(&ctx.source_sections);
    let mut issues = Vec::new();
    for (company, title) in titled_entries(&ctx.generated_sections) {
        if title.trim().is_empty() || company.is_empty() {
            continue;
        }
        let Some((_, source_title)) = source
            .iter()
            .find(|(c, t)| !t.trim().is_empty() && !c.is_disjoint(&company))
        else {
            continue;
        };
        let generated_tokens = ctx.tokens(&title);
        let source_tokens = ctx.tokens(source_title);
        if generated_tokens.is_empty() || source_tokens.is_empty() {
            continue;
        }
        if generated_tokens.is_disjoint(&source_tokens) {
            issues.push(issue(
                CONSISTENCY_TITLE_DRIFT,
                Some("Experience"),
                format!(
                    "Your source résumé calls this role \"{source_title}\" but the generated \
                     document calls it \"{title}\". Use the title your employer actually gave you."
                ),
                Some(format!("{source_title} → {title}")),
            ));
        }
    }
    issues
}

/// How many words a `label:` head may run to before it stops being a category
/// label. "Programming languages:" is three; a sentence that merely contains a
/// colon ("Shipped three services: billing, ledger and search") is more, and its
/// head is content that must keep counting as a claim.
pub const MAX_SKILLS_LABEL_WORDS: usize = 3;

/// A skills line with its leading `Category:` label removed.
///
/// "Languages: Rust, Python" / "Frameworks: React" / "Programmiersprachen: Rust"
/// is the commonest skills layout there is, and the label names a CATEGORY, not
/// a claim — "languages", "frameworks", "databases", "tooling" were all reported
/// back to the user as skills their résumé never demonstrates.
///
/// Only a SHORT head is stripped ([`MAX_SKILLS_LABEL_WORDS`]), and only the
/// first colon is considered: a longer head is prose that happens to carry a
/// colon, and dropping it would silently stop policing everything in front of
/// it.
fn strip_skills_label(line: &str) -> &str {
    match line.split_once(':') {
        Some((head, tail)) if super::word_count(head) <= MAX_SKILLS_LABEL_WORDS => tail,
        _ => line,
    }
}

/// `consistency.skill_not_demonstrated` — a skill listed in the skills section
/// that no experience or project line backs up.
///
/// The ATS argument for listing a skill is real, so this is advice, not a
/// block: it tells the user which claims an interviewer will have nothing to
/// ask about.
fn skill_not_demonstrated_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let Some(skills) = ctx.section_of_kind(SectionKind::Skills) else {
        return Vec::new();
    };
    let demonstrated: HashSet<String> = ctx
        .generated_sections
        .iter()
        .filter(|s| matches!(s.kind, SectionKind::Experience | SectionKind::Projects))
        .flat_map(|s| s.lines.iter())
        .flat_map(|l| ctx.tokens(&l.text))
        .collect();
    if demonstrated.is_empty() {
        return Vec::new(); // Nothing to check against — a skills-only document.
    }

    let claimed: HashSet<String> = skills
        .lines
        .iter()
        .flat_map(|l| ctx.tokens(strip_skills_label(&l.text)))
        .collect();
    // Tokens are STEMMED when the languages align, so map every one back to a
    // readable form before it reaches the user: "kubernet is listed under
    // skills" is a finding nobody can act on. Sorted on the display form so the
    // report reads alphabetically as printed.
    //
    // The same display form is what the function-word filter reads, for the
    // same reason `documents::evidence` filters its skills split on it: the list
    // holds words, not stems. It is the second half of the label fix — a line
    // that spells its category out ("Kenntnisse in Rust und Python") carries the
    // filler inline, where no `label:` rule can reach it.
    let stop = function_words(&ctx.lang);
    let mut missing: Vec<String> = claimed
        .difference(&demonstrated)
        .map(|token| ctx.display(token))
        .filter(|display| !stop.contains(&display.as_str()))
        .collect();
    missing.sort(); // Deterministic: the sets above are unordered.
    missing
        .into_iter()
        .map(|skill| {
            issue(
                CONSISTENCY_SKILL_NOT_DEMONSTRATED,
                Some("Skills"),
                format!(
                    "\"{skill}\" is listed under skills but never appears in your experience or \
                     projects. Add the line that proves it, or drop the claim."
                ),
                Some(skill),
            )
        })
        .collect()
}

/// Which of the three owner-locked project shapes an entry has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectTier {
    /// `**Name** · app-link · repo-link`, a `·`-separated stack line, and 1–3
    /// description lines.
    Full,
    /// Name + links + stack line, no description — the source had no blurb.
    NameLinksStack,
    /// `• Name · Website · Github` on one line — the source had links only.
    Compact,
}

/// Group a projects section's non-blank lines into entries.
///
/// An entry begins at a line that names a project: a bold run (`**Name**`) or a
/// bullet. Everything up to the next such line belongs to it.
///
/// `pub(super)` because `factual::project_links` needs the same grouping to know
/// which line is the stack line — two graders disagreeing about where an entry
/// starts is how a structure warning and a link Critical contradict each other.
pub(super) fn project_entries(section: &Section) -> Vec<Vec<&ParsedLine>> {
    let mut entries: Vec<Vec<&ParsedLine>> = Vec::new();
    for line in section
        .lines
        .iter()
        .filter(|l| !matches!(l.kind, LineKind::Blank) && !l.text.trim().is_empty())
    {
        let starts_entry =
            matches!(line.kind, LineKind::Bullet) || line.segments.iter().any(|s| s.bold);
        if starts_entry || entries.is_empty() {
            entries.push(vec![line]);
        } else if let Some(last) = entries.last_mut() {
            last.push(line);
        }
    }
    entries
}

/// The owner-locked degradation ladder: full → name+links+stack → compact.
/// Anything else is a structure the templates were not designed around.
fn tier_of(entry: &[&ParsedLine]) -> Option<ProjectTier> {
    let title = entry.first()?;
    let separators = ['·', '|', '•'];
    match entry.len() {
        // Compact: everything on the title line, `Name · Website · Github`.
        1 => title
            .text
            .contains(separators)
            .then_some(ProjectTier::Compact),
        // Title + stack line.
        2 => entry[1]
            .text
            .contains(separators)
            .then_some(ProjectTier::NameLinksStack),
        // Title + stack + 1..=MAX description lines.
        n if n <= 2 + MAX_PROJECT_DESCRIPTION_LINES => entry[1]
            .text
            .contains(separators)
            .then_some(ProjectTier::Full),
        _ => None,
    }
}

/// `consistency.project_structure` — a project entry outside the three
/// accepted shapes.
fn project_structure_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    let Some(section) = ctx.section_of_kind(SectionKind::Projects) else {
        return Vec::new();
    };
    project_entries(section)
        .into_iter()
        .filter(|entry| tier_of(entry).is_none())
        .map(|entry| {
            let name = entry.first().map(|l| l.text.clone()).unwrap_or_default();
            issue(
                CONSISTENCY_PROJECT_STRUCTURE,
                Some("Projects"),
                format!(
                    "The project \"{name}\" does not follow one of the three supported layouts \
                     (name + links + stack + up to {MAX_PROJECT_DESCRIPTION_LINES} description \
                     lines, name + links + stack, or the one-line compact form). It may render \
                     unevenly next to the others."
                ),
                Some(name),
            )
        })
        .collect()
}

pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let mut issues = date_order_issues(ctx);
    issues.extend(title_drift_issues(ctx));
    issues.extend(skill_not_demonstrated_issues(ctx));
    issues.extend(project_structure_issues(ctx));
    issues
}

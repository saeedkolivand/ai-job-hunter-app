//! Entry-level grounding of a generated résumé: a claim is checked against the
//! entry it is attached to, not against the résumé as a whole.
//!
//! * a skill/tool the job ad names but the whole source never does
//!   (`factual.unsupported_term`, Critical inside a role);
//! * a skill/tool the source has, but under a different entry
//!   (`factual.conflated_term`, Critical inside a role);
//! * a location on a role the source gives that role no such location for;
//! * a bullet, or a trailing outcome clause, with no overlap with its entry.
//!
//! Every Critical is deterministic and rests on tool-shaped terms only; a
//! merely capitalised word can warn, never park a run. Anything that cannot be
//! identified (an unmatched entry, an empty job ad, a source in another
//! language) is skipped, never guessed at.

use crate::documents::evidence::SectionKind;
use crate::documents::keywords::detected_language;
use crate::export::types::LineKind;
use crate::validate::content::{
    issue, Analysis, ContentIssue, FACTUAL_CONFLATED_ROLE_TERM, FACTUAL_CONFLATED_TERM,
    FACTUAL_UNSUPPORTED_ROLE_TERM, FACTUAL_UNSUPPORTED_TERM,
};
use std::collections::HashSet;

use super::blocks::{all_blocks, counterparts, keys_of, section_blocks, Block, Kind};
use super::entry_checks::{bullet_issues, location_issues, places_in};
use super::terms::{declared_skills, keys, skill_items, stems, terms_in, Term};

/// What the source and the posting contain, resolved once.
pub(super) struct Grounding {
    pub source: HashSet<String>,
    pub job: HashSet<String>,
    pub declared: HashSet<String>,
    /// The places the source's contact block (everything before its first
    /// section) names.
    pub contact_places: HashSet<String>,
    /// The source is not CONFIDENTLY in another language than the output. A
    /// translated term has no source spelling, so when this is false a
    /// job-ad term missing from the source cannot be Critical and the bullet
    /// check does not run. An unreadable language counts as comparable.
    pub comparable: bool,
}

impl Grounding {
    pub fn new(ctx: &Analysis) -> Self {
        Self {
            source: keys(ctx, ctx.input.source_resume),
            job: keys(ctx, ctx.input.job_ad),
            declared: declared_skills(ctx, &ctx.source_sections),
            contact_places: places_in(&contact_text(&ctx.source_sections)),
            comparable: detected_language(ctx.input.source_resume)
                .is_none_or(|detected| detected.get(..2) == Some(ctx.lang.as_str())),
        }
    }
}

/// The source's contact block: every line before its first section heading.
fn contact_text(sections: &[crate::validate::content::Section]) -> String {
    sections
        .iter()
        .filter(|s| s.heading.is_none())
        .flat_map(|s| s.lines.iter())
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}

fn names(terms: &[&Term]) -> String {
    terms
        .iter()
        .map(|t| t.word.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Term findings for one line. `entry` is the kernel keys of the source entry
/// the line belongs to, `None` when it has none to be checked against.
/// `role` raises the Critical tier for tool-shaped terms. `evidence` defaults
/// to the first offending term's own spelling.
pub(super) fn term_issues(
    g: &Grounding,
    terms: &[Term],
    entry: Option<&HashSet<String>>,
    role: bool,
    label: &str,
    section: Option<&str>,
    evidence: Option<&str>,
) -> Vec<ContentIssue> {
    let mut buckets: [Vec<&Term>; 4] = Default::default(); // [unsupported, conflated] x [weak, strong]
    for term in terms {
        if entry.is_some_and(|e| term.supported_by(e)) {
            continue;
        }
        let strong = usize::from(term.strong);
        if !term.supported_by(&g.source) {
            if term.supported_by(&g.job) {
                buckets[strong].push(term);
            }
        } else if entry.is_some() {
            buckets[2 + strong].push(term);
        }
    }
    let mut out = Vec::new();
    for (i, found) in buckets.iter().enumerate() {
        if found.is_empty() {
            continue;
        }
        let (unsupported, strong) = (i < 2, i % 2 == 1);
        // Conflation (a term spelled the same, under the wrong entry) needs no
        // translation; only "absent from the source" does.
        let critical = role && strong && (!unsupported || g.comparable);
        let (code, message) = if unsupported {
            (
                if critical {
                    FACTUAL_UNSUPPORTED_ROLE_TERM
                } else {
                    FACTUAL_UNSUPPORTED_TERM
                },
                format!(
                    "\"{}\" is in the job ad but nowhere in your source résumé, yet this text \
                     claims it. Keep it only if you can speak to it in an interview.",
                    names(found)
                ),
            )
        } else {
            (
                if critical {
                    FACTUAL_CONFLATED_ROLE_TERM
                } else {
                    FACTUAL_CONFLATED_TERM
                },
                format!(
                    "\"{}\" is in your résumé but not under \"{label}\"; it may belong to another \
                     entry. Move it back or drop it.",
                    names(found)
                ),
            )
        };
        out.push(issue(
            code,
            section,
            message,
            Some(evidence.map_or_else(|| found[0].word.clone(), str::to_string)),
        ));
    }
    out
}

fn entry_issues(
    ctx: &Analysis,
    g: &Grounding,
    block: &Block,
    source: &[Block],
    section: Option<&str>,
) -> Vec<ContentIssue> {
    let matches = counterparts(block, source);
    let entry_keys = (!matches.is_empty()).then(|| keys_of(ctx, &matches));
    let entry_stems = (!matches.is_empty()).then(|| {
        stems(
            ctx,
            &matches
                .iter()
                .map(|b| b.text())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    });
    let entry_text = matches
        .iter()
        .map(|b| b.text())
        .collect::<Vec<_>>()
        .join("\n");
    let role = block.kind == Kind::Role;
    let mut out = Vec::new();
    if role && !matches.is_empty() {
        out.extend(location_issues(block, &matches, &g.contact_places, section));
    }
    for line in &block.body {
        if !matches!(line.kind, LineKind::Bullet | LineKind::Text) {
            continue;
        }
        let terms = terms_in(ctx, &line.text, &g.declared);
        out.extend(term_issues(
            g,
            &terms,
            entry_keys.as_ref(),
            role,
            &block.identity,
            section,
            Some(line.raw.trim()),
        ));
        if role && g.comparable {
            if let Some(stems_of_entry) = &entry_stems {
                out.extend(bullet_issues(
                    ctx,
                    line.raw.trim(),
                    &line.text,
                    &entry_text,
                    stems_of_entry,
                    section,
                ));
            }
        }
    }
    out
}

/// Every entry-grounding finding for a generated résumé.
pub(super) fn validate(ctx: &Analysis) -> Vec<ContentIssue> {
    let g = Grounding::new(ctx);
    let source = all_blocks(&ctx.source_sections);
    let mut out = Vec::new();
    for section in &ctx.generated_sections {
        let heading = section.heading.as_deref();
        match section.kind {
            SectionKind::Experience | SectionKind::Projects => {
                for block in section_blocks(section) {
                    out.extend(entry_issues(ctx, &g, &block, &source, heading));
                }
            }
            SectionKind::Skills => {
                for term in skill_items(ctx, section) {
                    out.extend(term_issues(
                        &g,
                        std::slice::from_ref(&term),
                        None,
                        false,
                        "",
                        heading,
                        None,
                    ));
                }
            }
            SectionKind::Summary => {
                for line in &section.lines {
                    let terms = terms_in(ctx, &line.text, &g.declared);
                    out.extend(term_issues(
                        &g,
                        &terms,
                        None,
                        false,
                        "",
                        heading,
                        Some(line.raw.trim()),
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

//! Making the Projects section CODE-OWNED at quality depth, the way
//! `assemble::render_project` already makes it at max.
//!
//! At quality depth the model writes the whole résumé body in one streamed
//! call, projects included — so a draft can rename a project, drop a link, or
//! invent one the source never had, and nothing catches it until the
//! deterministic validator flags it as a Critical several stages later. This
//! module re-renders the DRAFT's own Projects section from the same
//! source-seeded [`ProjectOut`]s the max-depth generator uses, using the same
//! parser the seeder does ([`source::section`]/[`source::entries`]/
//! [`source::seed_one_project`]) so this module and the grader it feeds can
//! never disagree about where an entry starts or what counts as a link.
//!
//! Pure (L2): no `AppHandle`, no store, no event — a caller supplies the
//! document text and the seeds, and gets back the normalized text or `None`
//! for "nothing to do".

use std::collections::BTreeSet;

use crate::documents::evidence::SectionKind;
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::resume::{assemble, source};
use crate::validate::content::canonical_link;

use super::stages::sections;
use super::types_max::ProjectOut;

/// The href inside a link entry, unwrapping a markdown span captured verbatim
/// by [`source::seed_one_project`] (`[label](href)`). A bare URL passes
/// through unchanged. Every canonical comparison this module and the seeder
/// make routes through this first, so a labeled and a bare copy of the same
/// URL are never treated as two different links.
pub(crate) fn link_href(link: &str) -> &str {
    let trimmed = link.trim();
    let Some(rest) = trimmed.strip_prefix('[') else {
        return trimmed;
    };
    let Some(split) = rest.find("](") else {
        return trimmed;
    };
    let href = &rest[split + 2..];
    href.strip_suffix(')').unwrap_or(trimmed)
}

/// Project identity, compared the way `factual::project_entry_name` compares
/// it: lowercase alphanumeric words. Two graders disagreeing about whether two
/// entries are the same project is how a link Critical fires on a truthful
/// document.
///
/// Lifted out of `stages::section_gen` (which re-imports it) so the max-depth
/// rebuild and this module's normalizer share exactly one identity rule.
pub(crate) fn same_project(left: &str, right: &str) -> bool {
    fn key(name: &str) -> String {
        name.split(|c: char| !c.is_alphanumeric())
            .filter(|token| !token.is_empty())
            .map(str::to_lowercase)
            .collect::<Vec<String>>()
            .join(" ")
    }
    let left = key(left);
    !left.is_empty() && left == key(right)
}

/// One paragraph: every internal newline becomes a space, so a description
/// cannot silently add lines to a project entry and push it out of the
/// structure check's accepted shapes.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Whether two link lists name a different SET of resources, comparing
/// through [`link_href`] so a labeled span and a bare copy of one link count
/// as the same entry. `linksRestored` counts an entry whose kept links are not
/// byte-for-byte the set the model's answer carried — a link genuinely
/// restored, not merely re-rendered.
fn link_sets_differ(answered: &[String], seed: &[String]) -> bool {
    let key = |links: &[String]| -> BTreeSet<String> {
        links.iter().map(|l| canonical_link(link_href(l))).collect()
    };
    key(answered) != key(seed)
}

/// Re-seed every project's identity from the SOURCE and keep only the
/// description the model was allowed to write.
///
/// Two rules, both mechanical:
///
/// * `name`, `links` and `stack` come back from the seed unconditionally — the
///   model's copies are discarded even when they match, so a "helpfully"
///   corrected host cannot survive;
/// * a description is kept ONLY for a project whose SOURCE description was
///   non-empty. A project the résumé says nothing about renders as its compact
///   link line, which is the third tier of the owner's degradation ladder;
///   inventing a blurb for it is the failure the ladder exists to prevent.
///
/// Order follows the MODEL's answer (that is the tailoring decision it is
/// allowed to make) and a project it dropped stays dropped — trimming is a
/// normal editorial cut, and `factual.altered_project_link` deliberately does
/// not fire on one.
///
/// Shared by the max-depth section generator (`stages::section_gen::merge`,
/// which re-imports this) and this module's own [`normalize_projects`] — one
/// rebuild rule for "what a kept project entry is allowed to say" is what
/// keeps the two depths' output shape from silently diverging. The THIRD
/// return value (`linksRestored`) is additive: max's caller ignores it, so
/// this change is behavior-preserving there.
pub(crate) fn reseed_projects(
    seeds: &[ProjectOut],
    answered: &[ProjectOut],
) -> (Vec<ProjectOut>, u32, u32) {
    let mut out: Vec<ProjectOut> = Vec::new();
    let mut dropped = 0u32;
    let mut links_restored = 0u32;
    for project in answered {
        let Some(seed) = seeds
            .iter()
            .find(|seed| same_project(&seed.name, &project.name))
        else {
            // A project the source does not have. Not renderable and not
            // repairable: there is no seed to take its links from.
            dropped += 1;
            continue;
        };
        if out.iter().any(|kept| same_project(&kept.name, &seed.name)) {
            continue;
        }
        // A project whose seed carries no links, no stack and no description is
        // below the ladder's bottom rung, not on it: `render_project` emits a
        // bare `• {name}`, `parse_resume` strips the marker, and
        // `consistency::tier_of` then rejects the one-line entry — the document
        // flags ITSELF with `consistency.project_structure`. Dropping it is the
        // same call this function already makes about content the source
        // cannot back.
        if seed.links.is_empty() && seed.stack.is_empty() && seed.description.trim().is_empty() {
            dropped += 1;
            continue;
        }
        let described = !seed.description.trim().is_empty();
        if !described && !project.description.trim().is_empty() {
            dropped += 1; // an invented blurb for a data-less project
        }
        if link_sets_differ(&project.links, &seed.links) {
            links_restored += 1;
        }
        out.push(ProjectOut {
            name: seed.name.clone(),
            links: seed.links.clone(),
            stack: seed.stack.clone(),
            description: if described {
                one_line(&project.description)
            } else {
                String::new()
            },
        });
    }
    (out, dropped, links_restored)
}

/// Resolve each draft entry's identity against the seed it might actually BE,
/// falling back from name to a SHARED LINK.
///
/// [`same_project`] (name) stays the ONLY matching rule [`reseed_projects`]
/// itself knows about — that is what keeps max depth's behavior unchanged.
/// This fallback exists only for the normalizer, which grades a document a
/// model rewrote and must therefore survive a tidied title as long as the URL
/// underneath is still the candidate's own: it renames the draft entry to the
/// seed's name so the existing name-keyed rebuild does the rest, rather than
/// teaching `reseed_projects` a second matching rule to keep in sync with this
/// one.
fn resolve_seed_names(seeds: &[ProjectOut], drafted: Vec<ProjectOut>) -> Vec<ProjectOut> {
    drafted
        .into_iter()
        .map(|mut project| {
            if seeds
                .iter()
                .any(|seed| same_project(&seed.name, &project.name))
            {
                return project; // the name already resolves; nothing to do
            }
            if let Some(seed) = seeds.iter().find(|seed| shares_a_link(seed, &project)) {
                project.name = seed.name.clone();
            }
            project
        })
        .collect()
}

/// Whether `seed` and `project` name the same resource by URL, comparing
/// through [`link_href`] so a labeled span and a bare copy of the same link
/// still agree.
fn shares_a_link(seed: &ProjectOut, project: &ProjectOut) -> bool {
    seed.links.iter().any(|seed_link| {
        project.links.iter().any(|draft_link| {
            canonical_link(link_href(seed_link)) == canonical_link(link_href(draft_link))
        })
    })
}

/// Counts one [`normalize_projects`] pass left behind — content-free, for the
/// draft-stage ledger entry (ADR-027: counts only, never the document text).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ProjectsNormalizeStats {
    pub matched: u32,
    pub dropped: u32,
    pub links_restored: u32,
}

/// Re-render the DRAFT's Projects section from the source-seeded truth,
/// dropping what the model invented and restoring what it altered — the
/// quality-depth mirror of what `assemble::render_project` already guarantees
/// at max.
///
/// `None` is a genuine no-op (empty `seeds`, or the draft has no Projects
/// section) — a caller falls back to the document it already had.
pub(crate) fn normalize_projects(document: &str, seeds: &[ProjectOut]) -> Option<String> {
    build(document, seeds).map(|(text, _)| text)
}

/// [`normalize_projects`], plus the counts its caller records on the ledger.
/// A second function rather than a second computation: `build` runs once.
pub(crate) fn normalize_projects_with_stats(
    document: &str,
    seeds: &[ProjectOut],
) -> Option<(String, ProjectsNormalizeStats)> {
    build(document, seeds)
}

fn build(document: &str, seeds: &[ProjectOut]) -> Option<(String, ProjectsNormalizeStats)> {
    if seeds.is_empty() {
        return None;
    }
    let raw_sections = sections::split(document);
    let raw_section = sections::find(&raw_sections, SectionKey::Projects)?;
    let source_section = source::section(document, SectionKind::Projects)?;
    let drafted: Vec<ProjectOut> = source::entries(&source_section)
        .into_iter()
        .filter_map(|entry| source::seed_one_project(&entry))
        .collect();

    let resolved = resolve_seed_names(seeds, drafted);
    let (kept, dropped, links_restored) = reseed_projects(seeds, &resolved);

    let heading = raw_section.heading.clone().unwrap_or_default();
    let body = kept
        .iter()
        .map(assemble::render_project)
        .collect::<Vec<String>>()
        .join("\n\n");
    let replacement = if body.is_empty() {
        heading
    } else {
        format!("{heading}\n\n{body}")
    };
    let text = sections::splice(document, raw_section, &replacement);
    Some((
        text,
        ProjectsNormalizeStats {
            matched: kept.len() as u32,
            dropped,
            links_restored,
        },
    ))
}

#[cfg(test)]
mod test {
    use super::*;

    fn seed(name: &str, links: &[&str], stack: &[&str], description: &str) -> ProjectOut {
        ProjectOut {
            name: name.to_string(),
            links: links.iter().map(|s| s.to_string()).collect(),
            stack: stack.iter().map(|s| s.to_string()).collect(),
            description: description.to_string(),
        }
    }

    // ── link_href ────────────────────────────────────────────────────────

    #[test]
    fn link_href_unwraps_a_markdown_span_and_passes_a_bare_url_through() {
        assert_eq!(
            link_href("[Website](https://example.com/app)"),
            "https://example.com/app"
        );
        assert_eq!(
            link_href("https://example.com/app"),
            "https://example.com/app"
        );
        // Malformed input (no closing paren) is returned trimmed, not panicked.
        assert_eq!(
            link_href("[Website](https://example.com"),
            "[Website](https://example.com"
        );
    }

    // ── normalize_projects: no-ops ──────────────────────────────────────

    #[test]
    fn empty_seeds_is_a_no_op() {
        let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n";
        assert_eq!(normalize_projects(draft, &[]), None);
    }

    #[test]
    fn a_draft_with_no_projects_section_is_a_no_op() {
        let draft = "PROFESSIONAL SUMMARY\nA payments engineer.\n";
        let seeds = vec![seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "",
        )];
        assert_eq!(normalize_projects(draft, &seeds), None);
    }

    // ── ordering + invention + drop semantics ───────────────────────────

    /// Draft order is preserved, and an entry with no matching seed is
    /// dropped rather than kept.
    ///
    /// Mutation check: iterate `seeds` instead of the parsed draft entries in
    /// `build` and this fails — the seed order ("Alpha" before "Beta" in
    /// `seeds`) would come out ahead of the draft's own "Beta" before
    /// "Alpha".
    #[test]
    fn draft_order_is_preserved_and_an_invented_entry_is_dropped() {
        let seeds = vec![
            seed("Alpha", &["https://github.com/janedoe/alpha"], &[], ""),
            seed("Beta", &["https://github.com/janedoe/beta"], &[], ""),
        ];
        let draft = "PROJECTS\n\n\
             **Beta** · https://github.com/janedoe/beta\n\n\
             **Ghost Project** · https://github.com/janedoe/ghost\n\n\
             **Alpha** · https://github.com/janedoe/alpha\n";
        let normalized = normalize_projects(draft, &seeds).expect("has a projects section");
        let beta_at = normalized.find("Beta").expect("beta kept");
        let alpha_at = normalized.find("Alpha").expect("alpha kept");
        assert!(
            beta_at < alpha_at,
            "draft order (Beta before Alpha) survives"
        );
        assert!(
            !normalized.contains("Ghost"),
            "an entry with no matching seed is dropped, not invented into existence"
        );
    }

    /// An altered link is restored VERBATIM from the seed, not kept as the
    /// model rewrote it.
    #[test]
    fn an_altered_link_is_restored_from_the_seed() {
        let seeds = vec![seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "",
        )];
        let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/some-other-fork/ledger\n";
        let normalized = normalize_projects(draft, &seeds).unwrap();
        assert!(normalized.contains("https://github.com/janedoe/ledger"));
        assert!(!normalized.contains("some-other-fork"));
    }

    /// A link the model DROPPED is restored, because the identity fields come
    /// back from the seed unconditionally.
    #[test]
    fn a_dropped_link_is_restored_from_the_seed() {
        let seeds = vec![seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "",
        )];
        let draft = "PROJECTS\n\n**Ledger CLI**\n";
        let normalized = normalize_projects(draft, &seeds).unwrap();
        assert!(normalized.contains("https://github.com/janedoe/ledger"));
    }

    /// A description invented for a project the source says nothing else
    /// about is dropped, not carried through.
    #[test]
    fn an_invented_description_on_a_dataless_seed_is_dropped() {
        let seeds = vec![seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "", // no source description
        )];
        let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\
             A completely invented blurb about this project.\n";
        let normalized = normalize_projects(draft, &seeds).unwrap();
        assert!(
            !normalized.contains("invented blurb"),
            "a data-less seed must never gain a generated description"
        );
    }

    /// A renamed-but-same-link entry still matches its seed through the
    /// canonical-link fallback, so the seed's identity (its ORIGINAL name)
    /// survives rather than the entry being dropped as invented.
    #[test]
    fn a_renamed_entry_still_matches_its_seed_by_link() {
        let seeds = vec![seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "",
        )];
        let draft =
            "PROJECTS\n\n**Ledger Command Line Tool** · https://github.com/janedoe/ledger\n";
        let normalized = normalize_projects(draft, &seeds).unwrap();
        assert!(
            normalized.contains("Ledger CLI"),
            "matched by link, so the seed's own name is what renders: {normalized}"
        );
    }

    /// The seed's own markdown-labeled span round-trips through seeding and
    /// rendering, and its href agrees with the bare form under
    /// `canonical_link(link_href(..))` — the property that keeps a labeled
    /// and a bare copy of the same link from ever being treated as two.
    #[test]
    fn a_labeled_seed_link_round_trips_and_its_href_is_link_href_stable() {
        let seeds = vec![seed(
            "Ledger CLI",
            &["[Website](https://example.com/ledger)"],
            &[],
            "",
        )];
        let draft = "PROJECTS\n\n**Ledger CLI**\n";
        let normalized = normalize_projects(draft, &seeds).unwrap();
        assert!(
            normalized.contains("[Website](https://example.com/ledger)"),
            "the rendered line must carry the label span verbatim: {normalized}"
        );
        assert_eq!(
            canonical_link(link_href("[Website](https://example.com/ledger)")),
            canonical_link("https://example.com/ledger"),
            "labeled and bare forms of the same URL must compare equal"
        );
    }

    // ── stats ────────────────────────────────────────────────────────────

    #[test]
    fn stats_report_matched_dropped_and_links_restored() {
        let seeds = vec![
            seed("Alpha", &["https://github.com/janedoe/alpha"], &[], ""),
            seed("Beta", &["https://github.com/janedoe/beta"], &[], ""),
        ];
        let draft = "PROJECTS\n\n\
             **Alpha** · https://altered.example.com/alpha\n\n\
             **Ghost** · https://github.com/janedoe/ghost\n\n\
             **Beta** · https://github.com/janedoe/beta\n";
        let (_, stats) = normalize_projects_with_stats(draft, &seeds).unwrap();
        assert_eq!(stats.matched, 2, "Alpha and Beta are kept");
        assert_eq!(stats.dropped, 1, "Ghost has no seed");
        assert_eq!(stats.links_restored, 1, "only Alpha's link was altered");
    }
}

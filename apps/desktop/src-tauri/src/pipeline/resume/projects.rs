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
use crate::export::parser::parse_resume;
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::resume::{assemble, source};
use crate::validate::content::{canonical_link, link_href, project_entry_starts};

use super::stages::sections;
use super::types_max::ProjectOut;

/// Seed [`ProjectOut`]s for normalization, refusing to hand back seeds a
/// PLAIN-TEXT source produces garbage from.
///
/// Every extractor in `extraction::*` (PDF/DOCX/RTF) emits plain prose, no
/// markdown at all — `source::entries` groups a section's lines by
/// [`project_entry_starts`] (bold or bullet) OR "first line of the section",
/// and that second arm exists only so a section with exactly one project
/// still seeds. On a plain-text section it is the ONLY arm that ever fires,
/// so the whole section collapses into ONE mega-entry whose "stack" and
/// "description" are really the next two projects' title lines. Normalizing
/// over that seed would rewrite a CORRECT draft into that garbage and it
/// would still validate clean, because the seed IS what the validator grades
/// against too.
///
/// Two independent guards, because a source can fail either one on its own:
///
/// * **No genuine entry boundary.** `None` (empty seeds, which
///   [`normalize_projects`] already reads as a no-op) when NOT ONE line in
///   the section satisfies [`project_entry_starts`] — the exact precondition
///   `source::entries`' grouping rule depends on, checked here through the
///   SAME predicate rather than re-implemented.
/// * **Cross-contaminated fields**, the shape a still-wrong boundary leaves
///   behind even with a genuine bold/bullet line present: one seed's stack or
///   description holding another seed's own name or link. See
///   [`seeds_are_plausible`].
pub(crate) fn seed_projects_for_normalize(source_resume: &str) -> Vec<ProjectOut> {
    let Some(section) = source::section(source_resume, SectionKind::Projects) else {
        return Vec::new();
    };
    if !section
        .lines
        .iter()
        .any(|line| project_entry_starts(&line.parsed))
    {
        return Vec::new(); // no genuine entry boundary — a plain-text section
    }
    let seeds: Vec<ProjectOut> = source::entries(&section)
        .into_iter()
        .filter_map(|entry| source::seed_one_project(&entry))
        .collect();
    if seeds_are_plausible(&seeds) {
        seeds
    } else {
        Vec::new()
    }
}

/// Whether `seeds` look like a genuine per-project split rather than one
/// entry's fields spilling into another's.
///
/// Checked pairwise: any OTHER seed's name or href showing up inside a seed's
/// own stack/description text is cross-contamination — the shape a
/// still-wrong entry boundary produces even when at least one bold/bullet
/// line was present. Normalizing over it would WRITE that contamination into
/// the document as "restored from the source", which is worse than not
/// normalizing at all. Short names/hrefs (under a few characters) are exempt
/// from the comparison to avoid a coincidental substring match.
fn seeds_are_plausible(seeds: &[ProjectOut]) -> bool {
    seeds.iter().enumerate().all(|(index, seed)| {
        let haystack = format!("{} {}", seed.stack.join(" "), seed.description).to_lowercase();
        seeds
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != index)
            .all(|(_, other)| {
                let name = other.name.trim().to_lowercase();
                let name_leaks = name.chars().count() > 2 && haystack.contains(&name);
                let link_leaks = other.links.iter().any(|link| {
                    let href = link_href(link).trim().to_lowercase();
                    href.chars().count() > 4 && haystack.contains(&href)
                });
                !name_leaks && !link_leaks
            })
    })
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
            // A SECOND answer for a seed already kept — two draft entries
            // (or a rename that collided with an existing one) resolving to
            // the same project. Counted, not silent: an uncounted drop here
            // is exactly how `matched + dropped` stops summing to the
            // draft's own entry count.
            dropped += 1;
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
///
/// **Refuses to rename when MORE than one seed shares the link.** Two seeds
/// legitimately sharing one URL (a monorepo's app and its docs site, say)
/// makes "which one is this draft entry" a guess, and guessing wrong renames
/// the draft entry to the WRONG seed's identity — collapsing two projects
/// into one under `reseed_projects`' own dedup rather than keeping them apart.
/// An unresolved name still gets its normal chance to fail — dropped as
/// invented — rather than silently merged.
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
            let mut sharing = seeds.iter().filter(|seed| shares_a_link(seed, &project));
            if let Some(seed) = sharing.next() {
                if sharing.next().is_none() {
                    project.name = seed.name.clone();
                }
                // else: more than one seed shares this link — ambiguous,
                // refuse to guess which one it is.
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
    // Parsed ONCE — `sections::split_parsed` and `source::section_from_parsed`
    // both need a `ParsedDocument` over this SAME text, and `parse_resume` is
    // the expensive half of each.
    let parsed = parse_resume(document);
    let raw_sections = sections::split_parsed(document, &parsed);
    let raw_section = sections::find(&raw_sections, SectionKey::Projects)?;
    let source_section = source::section_from_parsed(document, SectionKind::Projects, &parsed)?;
    let drafted: Vec<ProjectOut> = source::entries(&source_section)
        .into_iter()
        .filter_map(|entry| source::seed_one_project(&entry))
        .collect();

    let resolved = resolve_seed_names(seeds, drafted);
    let (kept, dropped, links_restored) = reseed_projects(seeds, &resolved);

    // Nothing survived — every draft entry was invented, or the draft's own
    // Projects section had none to begin with. Splicing a heading-only
    // section in would be an undo-less, silent blanking of the section; the
    // honest answer is "nothing to normalize", not "normalize it to empty".
    // `matched == kept.len()`, so this is the same check as "zero matched
    // while something WAS dropped" — a total disagreement between the draft
    // and the seeds is a parse failure to leave invisible, not a rewrite.
    if kept.is_empty() {
        return None;
    }

    let heading = raw_section.heading.clone().unwrap_or_default();
    let body = kept
        .iter()
        .map(assemble::render_project)
        .collect::<Vec<String>>()
        .join("\n\n");
    let mut replacement = format!("{heading}\n\n{body}");
    // The ORIGINAL section's line range (`raw_section.end`) runs up to the
    // NEXT heading, so it includes the blank line(s) that separated the two
    // — which this replacement, built fresh, does not carry. Not the LAST
    // section ⇒ append one back, or the splice would butt the next heading
    // directly against the last rendered line (mirrors the trailing-blank
    // convention `stages::sections::entry_range` documents for entries).
    if raw_section.end < parsed.lines.len() {
        replacement.push_str("\n\n");
    }
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

    // ── C2: all-dropped is a no-op, never a heading-only section ────────

    /// Every draft entry is unrelated to the one seed the source has —
    /// nothing survives `reseed_projects`, and the honest answer is "nothing
    /// to normalize", not a heading with no body under it.
    ///
    /// Mutation check: drop the `kept.is_empty()` guard in `build` and this
    /// fails — the section gets spliced down to just its heading.
    #[test]
    fn all_dropped_is_a_no_op_not_a_heading_only_section() {
        let seeds = vec![seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "",
        )];
        let draft = "PROJECTS\n\n**Totally Unrelated** · https://example.com/x\n";
        assert_eq!(normalize_projects(draft, &seeds), None);
    }

    // ── M1: the blank line to the NEXT section survives ──────────────────

    /// The original section's line range runs up to the next heading, so it
    /// swallows the blank line separating the two — a replacement built fresh
    /// must put one back, or the next heading butts directly against the
    /// last rendered line.
    ///
    /// Mutation check: drop the `raw_section.end < parsed.lines.len()` append
    /// in `build` and this fails (`"ledger.\nEDUCATION"` with no blank line).
    #[test]
    fn the_blank_line_before_the_next_section_survives_normalization() {
        let seeds = vec![seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "",
        )];
        let draft =
            "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\nEDUCATION\n\nMSc.\n";
        let normalized = normalize_projects(draft, &seeds).unwrap();
        assert!(
            normalized.contains("ledger\n\nEDUCATION"),
            "a blank line must separate the normalized section from the next heading: {normalized:?}"
        );
        assert!(!normalized.contains("ledger\nEDUCATION"));
    }

    /// The section IS the last one — appending a trailing blank would add a
    /// spurious blank line at end of document, so nothing is appended.
    #[test]
    fn no_trailing_blank_is_added_when_projects_is_the_last_section() {
        let seeds = vec![seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "",
        )];
        let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n";
        let normalized = normalize_projects(draft, &seeds).unwrap();
        assert!(!normalized.ends_with("\n\n\n"));
    }

    // ── M2: dedup is counted, and an ambiguous link-rename is refused ────

    /// Two draft entries that both resolve to the SAME seed: the second is a
    /// dedup drop, and it must be COUNTED, not silently discarded — an
    /// uncounted drop is exactly how `matched + dropped` stops summing to the
    /// draft's own entry count.
    ///
    /// Mutation check: drop the `dropped += 1` on the dedup `continue` in
    /// `reseed_projects` and `stats.dropped` becomes 0.
    #[test]
    fn a_dedup_collision_is_counted_as_dropped() {
        let seeds = vec![seed(
            "Ledger CLI",
            &["https://github.com/janedoe/ledger"],
            &[],
            "",
        )];
        // Two entries answering to the SAME seed name.
        let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\n\
             **Ledger CLI** · https://github.com/janedoe/ledger\n";
        let (_, stats) = normalize_projects_with_stats(draft, &seeds).unwrap();
        assert_eq!(stats.matched, 1, "only the first survives");
        assert_eq!(stats.dropped, 1, "the second is a counted dedup drop");
    }

    /// Two SEEDS legitimately share one link (a monorepo's app and its docs
    /// site). A draft entry renamed away from EITHER seed's name must not
    /// guess which one it is — an unresolved name still gets its normal
    /// chance to fail (dropped as invented) rather than being silently
    /// merged into whichever seed happened to be first.
    ///
    /// Mutation check: use `.find` instead of refusing on a second match in
    /// `resolve_seed_names` and this fails — the renamed entry survives as
    /// "App" (the first seed) instead of being dropped.
    #[test]
    fn resolve_seed_names_refuses_to_rename_when_two_seeds_share_a_link() {
        let shared = "https://github.com/janedoe/monorepo";
        let seeds = vec![
            seed("App", &[shared], &[], ""),
            seed("Docs", &[shared], &[], ""),
        ];
        let draft = "PROJECTS\n\n**Renamed Thing** · https://github.com/janedoe/monorepo\n";
        let normalized = normalize_projects(draft, &seeds);
        assert!(
            normalized.is_none(),
            "ambiguous rename ⇒ dropped as invented ⇒ nothing kept ⇒ a no-op: {normalized:?}"
        );
    }

    // ── L1/L2/L3: markdown link-span extraction (source.rs) ──────────────

    /// A URL containing its own unescaped `)` (a Wikipedia-shaped link)
    /// truncates the capturing regex's own match — writing that truncated,
    /// unbalanced span into the document would be malformed markdown. The
    /// bare (verified-correct) href is always the safe fallback.
    #[test]
    fn a_paren_containing_href_falls_back_to_the_bare_form_not_a_malformed_span() {
        let source = "PROJECTS\n\n**Rust wiki** · \
            [Rust](https://en.wikipedia.org/wiki/Rust_(programming_language))\n";
        let seeds = crate::pipeline::resume::source::seed_projects(source);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].links.len(), 1);
        let link = &seeds[0].links[0];
        assert!(
            !link.starts_with('['),
            "the round-trip/balance check must fail on a paren-truncated capture and fall \
             back to the bare href rather than writing the unbalanced markdown span \
             verbatim: {link:?}"
        );
    }

    /// A URL-shaped LABEL paired with a different href
    /// (`[https://other](https://real)`) must not ALSO get harvested as its
    /// own bare-URL link — the label text is stripped before the bare-URL
    /// pass runs.
    #[test]
    fn a_url_shaped_label_is_not_double_harvested() {
        let source =
            "PROJECTS\n\n**Site** · [https://other.example.com](https://real.example.com/app)\n";
        let seeds = crate::pipeline::resume::source::seed_projects(source);
        assert_eq!(seeds.len(), 1);
        assert_eq!(
            seeds[0].links.len(),
            1,
            "exactly one link, not the label harvested a second time: {:?}",
            seeds[0].links
        );
    }

    /// A scheme-less `www.` href inside a markdown span is still a link by
    /// `urls_in`'s own rule, so its label must survive too — anchoring the
    /// capture to `https?://` would silently drop it.
    #[test]
    fn a_scheme_less_www_label_survives() {
        let source = "PROJECTS\n\n**Site** · [Website](www.example.com/app)\n";
        let seeds = crate::pipeline::resume::source::seed_projects(source);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].links, vec!["[Website](www.example.com/app)"]);
    }

    // ── C1: seed_projects_for_normalize ───────────────────────────────────

    /// The precondition gate directly: zero lines in the source's Projects
    /// section satisfy `project_entry_starts` (no bold, no bullet — the
    /// shape every `extraction::*` importer emits) ⇒ empty seeds, never the
    /// collapsed mega-entry `source::seed_projects` alone would produce.
    #[test]
    fn seed_projects_for_normalize_refuses_a_plain_text_section() {
        let plain = "PROJECTS\n\n\
            Ledger CLI - https://github.com/janedoe/ledger\n\
            A bookkeeping tool.\n\
            CrossKit - https://github.com/janedoe/crosskit\n";
        // The premise: the naive seeder DOES collapse this (proving the gate
        // is doing real work, not refusing an already-empty result).
        assert_eq!(
            source::seed_projects(plain).len(),
            1,
            "premise: the ungated seeder collapses this into one mega-entry"
        );
        assert!(seed_projects_for_normalize(plain).is_empty());
    }

    /// The happy path survives the gate: a genuinely bold/bullet-structured
    /// source seeds normally.
    #[test]
    fn seed_projects_for_normalize_keeps_a_well_formed_section() {
        let well_formed = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\n\
             **CrossKit** · https://github.com/janedoe/crosskit\n";
        let seeds = seed_projects_for_normalize(well_formed);
        assert_eq!(seeds.len(), 2);
    }

    /// The second, independent guard: cross-contaminated fields (one seed's
    /// description carrying another seed's own name) are refused even though
    /// a genuine bold entry-start line is present.
    #[test]
    fn seeds_are_plausible_catches_cross_contaminated_fields() {
        let clean = vec![
            seed(
                "Ledger CLI",
                &["https://github.com/janedoe/ledger"],
                &[],
                "",
            ),
            seed(
                "CrossKit",
                &["https://github.com/janedoe/crosskit"],
                &[],
                "",
            ),
        ];
        assert!(seeds_are_plausible(&clean));

        let contaminated = vec![
            seed(
                "Ledger CLI",
                &["https://github.com/janedoe/ledger"],
                &[],
                "Then there's the CrossKit project, a design system.",
            ),
            seed(
                "CrossKit",
                &["https://github.com/janedoe/crosskit"],
                &[],
                "",
            ),
        ];
        assert!(!seeds_are_plausible(&contaminated));
    }
}

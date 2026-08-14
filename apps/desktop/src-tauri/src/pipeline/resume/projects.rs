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
//! ## Write authority is narrow, on purpose
//!
//! `source::entries`' grouping rule (bold/bullet starts an entry, everything
//! else glues onto the previous one) is the SAME rule on both the source and
//! the draft side, and it CAN mis-fire on either — a plain-text source
//! collapses several projects into one mega-entry; a plain-text DRAFT does
//! the same. A parse disagreement is indistinguishable, from inside this
//! module, from a model that genuinely invented an entry. So this module
//! never DELETES a draft entry it cannot confidently match to a seed — an
//! unmatched entry is left VERBATIM, in place; the deterministic validators
//! (`factual.altered_project_link`, `consistency.project_structure`) still
//! grade whatever it says. Write authority extends only to entries this
//! module actually matched to a seed, and even then only after the seed list
//! itself passes a plausibility check — see [`seed_projects_for_normalize`].
//!
//! Pure (L2): no `AppHandle`, no store, no event — a caller supplies the
//! document text and the seeds, and gets back an [`ProjectsNormalizeOutcome`]
//! (or a thin `Option<String>` wrapper) describing what happened.

use std::collections::BTreeSet;

use crate::documents::evidence::SectionKind;
use crate::export::parser::parse_resume;
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::resume::{assemble, source};
use crate::validate::content::{canonical_link, link_href, urls_in};

use super::stages::sections;
use super::types_max::ProjectOut;

/// Seed [`ProjectOut`]s for normalization, plus WHY the list came back empty
/// when it did — content-free (ADR-027), for the draft-stage ledger.
///
/// Every extractor in `extraction::*` (PDF/DOCX/RTF) emits plain prose, no
/// markdown at all — `source::entries` groups a section's lines by
/// `project_entry_starts` (bold or bullet) OR "first line of the section",
/// and that second arm exists only so a section with exactly one project
/// still seeds. On a plain-text section it is the only arm that fires, so a
/// multi-project section collapses into fewer, garbled entries.
///
/// Three independent, WHOLE-BAIL guards (a partial filter would still leave
/// the rest of a mis-grouped section to normalize from):
///
/// * **An empty seed.** A seed with no link, no stack AND no description
///   cannot come from a locked-signature entry — every accepted tier carries
///   at least a link (`render_project`'s own bottom rung refuses to emit a
///   bare name; see [`reseed_projects`]'s doc). It is always the symptom of a
///   mis-grouped fragment: a title swallowed by the previous bullet, an
///   achievement bullet counted as its own "entry". The SAME mis-grouping
///   usually corrupts its neighbors too (a title's stack/description
///   mis-attributed to the next project), which is why this bails the WHOLE
///   list rather than filtering the one empty seed out.
/// * **A link inside a description or stack field.** The locked signature
///   puts links ONLY on the title line, and [`source::seed_one_project`]
///   already strips a stack line's own URLs out before it ever reaches
///   `stack` (`names_a_resource` there). So a URL surviving in a seed's
///   `description`/`stack` cannot be a legitimate part of either field — it
///   means this entry's boundary swallowed a FOLLOWING project's title line
///   (a plain-text, multi-project source collapsing into one mega-entry:
///   `Ledger CLI\n<url>\nBeta Sync · <url>\nGo · gRPC` seeds ONE entry named
///   "Ledger CLI" whose merged description carries Beta Sync's own link). A
///   single collapsed mega-entry has no SIBLING to compare against, so
///   [`seeds_are_plausible`] cannot catch this on its own — this guard is
///   independent of seed count. A legitimate description that happens to
///   CITE a URL in prose ("see my write-up at <url>") merely disables
///   normalization for that run (fails safe); the draft is left untouched
///   and validators still grade it.
/// * **Cross-contaminated fields** — see [`seeds_are_plausible`].
///
/// A source whose Projects section actually follows the locked signature
/// (title/stack/description, or a compact `Name · link · link` line — links
/// make a seed non-empty either way, and a stack/description line never
/// legitimately carries a URL) passes all three bails untouched.
pub(crate) fn seed_projects_for_normalize(
    source_resume: &str,
) -> (Vec<ProjectOut>, Option<&'static str>) {
    let seeds: Vec<ProjectOut> = source::section(source_resume, SectionKind::Projects)
        .map(|section| {
            source::entries(&section)
                .into_iter()
                .filter_map(|entry| source::seed_one_project(&entry))
                .collect()
        })
        .unwrap_or_default();
    if seeds.is_empty() {
        return (seeds, Some("no_entry_starts"));
    }
    if seeds.iter().any(|seed| {
        seed.links.is_empty() && seed.stack.is_empty() && seed.description.trim().is_empty()
    }) {
        return (Vec::new(), Some("empty_seed"));
    }
    if seeds.iter().any(|seed| {
        !urls_in(&seed.description).is_empty()
            || seed.stack.iter().any(|item| !urls_in(item).is_empty())
    }) {
        return (Vec::new(), Some("link_in_description"));
    }
    if !seeds_are_plausible(&seeds) {
        return (Vec::new(), Some("implausible_seeds"));
    }
    (seeds, None)
}

/// Whether `seeds` look like a genuine per-project split rather than one
/// entry's fields spilling into another's.
///
/// Compares only STACK entries and LINKS against sibling names/links — never
/// the DESCRIPTION. A truthful description that happens to cross-reference a
/// sibling project by name ("...see also my CrossKit project") is not
/// contamination, and counting it as such switched normalization off for an
/// honest source. Short names/hrefs (a handful of characters or fewer) are
/// exempt from the comparison to avoid a coincidental substring match.
fn seeds_are_plausible(seeds: &[ProjectOut]) -> bool {
    seeds.iter().enumerate().all(|(index, seed)| {
        let haystack = seed
            .stack
            .iter()
            .cloned()
            .chain(seed.links.iter().map(|link| link_href(link).to_string()))
            .collect::<Vec<String>>()
            .join(" ")
            .to_lowercase();
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
/// description the model was allowed to write. **Max-depth only** — see
/// [`build`] for the normalizer's own (verbatim-preserving) matching pass.
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
/// Used by `stages::section_gen::merge` (which re-imports this), which
/// generates a WHOLE section from a JSON answer rather than splicing draft
/// text — there is no "original text" to fall back to there, so an
/// unmatched answer is dropped rather than kept verbatim. The THIRD return
/// value (`linksRestored`) is additive and max's caller ignores it: **output
/// is unchanged there; only this dropped-metric now also counts a dedup
/// collision it silently missed before.**
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
            // A SECOND answer for a seed already kept. Counted, not silent:
            // an uncounted drop here is exactly how `matched + dropped` stops
            // summing to the answer's own entry count.
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

/// Resolve ONE draft entry against `seeds`: NAME match first, then an
/// UNAMBIGUOUS shared-link fallback (a tidied title over the same URL).
/// `None` when neither resolves, OR when more than one seed shares the link —
/// two seeds legitimately sharing one URL (a monorepo's app and its docs
/// site) makes "which one is this" a guess, and [`build`]'s caller must treat
/// an unresolved entry as VERBATIM, never as a guess.
fn resolve_seed_index(seeds: &[ProjectOut], project: &ProjectOut) -> Option<usize> {
    if let Some(index) = seeds
        .iter()
        .position(|seed| same_project(&seed.name, &project.name))
    {
        return Some(index);
    }
    let mut sharing = seeds
        .iter()
        .enumerate()
        .filter(|(_, seed)| shares_a_link(seed, project))
        .map(|(index, _)| index);
    let first = sharing.next()?;
    if sharing.next().is_some() {
        return None; // ambiguous — refuse to guess
    }
    Some(first)
}

/// Counts one [`normalize_projects`]-family call left behind — content-free,
/// for the draft-stage ledger entry (ADR-027: counts only, never the
/// document text).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ProjectsNormalizeStats {
    pub matched: u32,
    pub dropped: u32,
    pub links_restored: u32,
}

/// What one normalization pass over the DRAFT did.
#[derive(Debug)]
pub(crate) enum ProjectsNormalizeOutcome {
    /// The Projects section was re-rendered; here is the new document text
    /// and the counts that describe what changed.
    Applied(String, ProjectsNormalizeStats),
    /// Normalization did not run, and here is WHY — content-free (ADR-027),
    /// so a caller can record it on the draft-stage ledger. A silent skip is
    /// unobservable otherwise.
    Skipped(&'static str),
    /// A genuine no-op with nothing worth reporting: empty `seeds`, the draft
    /// has no Projects section, or nothing the seeds could back was actually
    /// rewritten (every entry was already correct, or every entry stayed
    /// verbatim).
    NoOp,
}

/// [`normalize_projects_outcome`], collapsed to `Option<String>` for a caller
/// that only wants the text (`repair::repair_loop`'s `normalize` closure,
/// the regenerate-section command) and does not report a skip reason.
pub(crate) fn normalize_projects(document: &str, seeds: &[ProjectOut]) -> Option<String> {
    match build(document, seeds) {
        ProjectsNormalizeOutcome::Applied(text, _) => Some(text),
        _ => None,
    }
}

/// [`normalize_projects`], plus the counts a caller records on the ledger.
/// Test-only: production code needs the skip reason too, so it goes through
/// [`normalize_projects_outcome`] directly (`Draft::run`'s
/// `apply_projects_normalization`) rather than this thinner wrapper.
#[cfg(test)]
pub(crate) fn normalize_projects_with_stats(
    document: &str,
    seeds: &[ProjectOut],
) -> Option<(String, ProjectsNormalizeStats)> {
    match build(document, seeds) {
        ProjectsNormalizeOutcome::Applied(text, stats) => Some((text, stats)),
        _ => None,
    }
}

/// Re-render the DRAFT's Projects section from the source-seeded truth,
/// dropping what the model invented and restoring what it altered — the
/// quality-depth mirror of what `assemble::render_project` already guarantees
/// at max. See the module doc for why write authority is narrow.
pub(crate) fn normalize_projects_outcome(
    document: &str,
    seeds: &[ProjectOut],
) -> ProjectsNormalizeOutcome {
    build(document, seeds)
}

fn build(document: &str, seeds: &[ProjectOut]) -> ProjectsNormalizeOutcome {
    if seeds.is_empty() {
        return ProjectsNormalizeOutcome::NoOp;
    }
    // Parsed ONCE — `sections::split_parsed` and `source::section_from_parsed`
    // both need a `ParsedDocument` over this SAME text, and `parse_resume` is
    // the expensive half of each.
    let parsed = parse_resume(document);
    let raw_sections = sections::split_parsed(document, &parsed);
    let Some(raw_section) = sections::find(&raw_sections, SectionKey::Projects) else {
        return ProjectsNormalizeOutcome::NoOp;
    };
    let Some(source_section) =
        source::section_from_parsed(document, SectionKind::Projects, &parsed)
    else {
        return ProjectsNormalizeOutcome::NoOp;
    };

    // Every draft entry, PARSED but not yet judged — matching happens next,
    // deciding NOTHING about deletion here.
    let entries: Vec<(String, Option<ProjectOut>)> = source::entries(&source_section)
        .into_iter()
        .map(|entry| {
            let raw_text = entry
                .iter()
                .map(|line| line.raw.as_str())
                .collect::<Vec<&str>>()
                .join("\n");
            (raw_text, source::seed_one_project(&entry))
        })
        .collect();

    enum Resolution {
        /// No seed resolves (unmatched or ambiguous) — kept as its own raw
        /// text, never deleted.
        Verbatim,
        /// A SECOND entry resolving to a seed already used by an earlier one.
        Dedup,
        /// Resolves to `seeds[_]`, rebuilt from it.
        Matched(usize),
    }

    // First pass: resolve every entry, tracking which seed indices got used.
    let mut used_indices: Vec<usize> = Vec::new();
    let mut resolutions: Vec<Resolution> = Vec::with_capacity(entries.len());
    for (_, project) in &entries {
        let Some(project) = project else {
            resolutions.push(Resolution::Verbatim); // unnamed — cannot be judged
            continue;
        };
        match resolve_seed_index(seeds, project) {
            Some(index) if used_indices.contains(&index) => {
                if same_project(&seeds[index].name, &project.name) {
                    // An ordinary dedup: the SAME name, listed twice.
                    resolutions.push(Resolution::Dedup);
                } else {
                    // A DIFFERENT, non-identically-named entry resolved to
                    // this seed only through the shared-link fallback — the
                    // seed's own link list is being claimed by two entries
                    // the draft itself never called the same project. That is
                    // the signature of ONE seed having swallowed MULTIPLE
                    // projects' links (a plain-text SOURCE collapsing several
                    // projects into a single `entries()` group — the shape
                    // the empty-seed and plausibility gates above cannot
                    // catch when the resulting single seed's merged links and
                    // description both happen to be non-empty). Not an
                    // ordinary "listed twice" dedup: bail the whole pass
                    // rather than attach a stranger's link to whichever
                    // entry got there first.
                    return ProjectsNormalizeOutcome::Skipped("seed_claims_multiple_entries");
                }
            }
            Some(index) => {
                used_indices.push(index);
                resolutions.push(Resolution::Matched(index));
            }
            None => resolutions.push(Resolution::Verbatim),
        }
    }

    // The draft-side parse-disagreement bail: a seed this pass never matched
    // to ANY entry, whose own link is nonetheless present somewhere in the
    // draft's Projects section text, means the draft's OWN entry grouping
    // missed a boundary (two projects merged into one draft entry — the same
    // mis-grouping this module's source-side gate exists to catch, just on
    // the other document). A model that legitimately trimmed a project for
    // relevance leaves no trace of its link at all, so this never fires on an
    // honest omission. The whole Projects section is then suspect, not just
    // the merged pair, so this bails everything rather than guessing which
    // entries are still trustworthy.
    let draft_text = source_section
        .lines
        .iter()
        .map(|line| line.raw.as_str())
        .collect::<Vec<&str>>()
        .join(" ");
    let draft_urls: BTreeSet<String> = urls_in(&draft_text)
        .into_iter()
        .map(|url| canonical_link(&url))
        .collect();
    let draft_disagrees = seeds.iter().enumerate().any(|(index, seed)| {
        !used_indices.contains(&index)
            && seed
                .links
                .iter()
                .any(|link| draft_urls.contains(&canonical_link(link_href(link))))
    });
    if draft_disagrees {
        return ProjectsNormalizeOutcome::Skipped("draft_parse_disagreement");
    }

    // Second pass: build the replacement body. An UNMATCHED entry's own text
    // survives unchanged — write authority extends only to entries actually
    // matched to a seed.
    let mut pieces: Vec<String> = Vec::new();
    let mut matched = 0u32;
    let mut dropped = 0u32;
    let mut links_restored = 0u32;
    for ((raw_text, project), resolution) in entries.iter().zip(resolutions.iter()) {
        match resolution {
            Resolution::Verbatim => pieces.push(raw_text.clone()),
            Resolution::Dedup => dropped += 1,
            Resolution::Matched(index) => {
                let seed = &seeds[*index];
                let project = project
                    .as_ref()
                    .expect("a Matched resolution only follows a parsed project");
                let described = !seed.description.trim().is_empty();
                if !described && !project.description.trim().is_empty() {
                    dropped += 1; // an invented blurb for a data-less project
                }
                if link_sets_differ(&project.links, &seed.links) {
                    links_restored += 1;
                }
                let rebuilt = ProjectOut {
                    name: seed.name.clone(),
                    links: seed.links.clone(),
                    stack: seed.stack.clone(),
                    description: if described {
                        one_line(&project.description)
                    } else {
                        String::new()
                    },
                };
                pieces.push(assemble::render_project(&rebuilt));
                matched += 1;
            }
        }
    }

    if matched == 0 {
        // Nothing this pass actually restored — either every entry was
        // already correct, or every entry stayed verbatim because none of
        // them backed a seed. Re-splicing verbatim text back over itself
        // would be a needless byte-shuffle at best; leave the document
        // untouched.
        return ProjectsNormalizeOutcome::NoOp;
    }

    let heading = raw_section.heading.clone().unwrap_or_default();
    let body = pieces.join("\n\n");
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
    ProjectsNormalizeOutcome::Applied(
        text,
        ProjectsNormalizeStats {
            matched,
            dropped,
            links_restored,
        },
    )
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

    // ── ordering + the never-delete guarantee ───────────────────────────

    /// Draft order is preserved, and an entry with no matching seed survives
    /// VERBATIM — never deleted, never invented over.
    ///
    /// Mutation check: iterate `seeds` instead of the parsed draft entries in
    /// `build` and this fails — the seed order ("Alpha" before "Beta" in
    /// `seeds`) would come out ahead of the draft's own "Beta" before
    /// "Alpha".
    #[test]
    fn draft_order_is_preserved_and_an_unmatched_entry_survives_verbatim() {
        let seeds = vec![
            seed("Alpha", &["https://github.com/janedoe/alpha"], &[], ""),
            seed("Beta", &["https://github.com/janedoe/beta"], &[], ""),
        ];
        let draft = "PROJECTS\n\n\
             **Beta** · https://github.com/janedoe/beta\n\n\
             **Ghost Project** · A project with no source link at all\n\n\
             **Alpha** · https://github.com/janedoe/alpha\n";
        let normalized = normalize_projects(draft, &seeds).expect("has a projects section");
        let beta_at = normalized.find("Beta").expect("beta kept");
        let ghost_at = normalized.find("Ghost").expect("ghost kept VERBATIM");
        let alpha_at = normalized.find("Alpha").expect("alpha kept");
        assert!(
            beta_at < ghost_at && ghost_at < alpha_at,
            "draft order (Beta, Ghost, Alpha) survives: {normalized}"
        );
        assert!(
            normalized.contains("A project with no source link at all"),
            "an entry with no matching seed is preserved VERBATIM, never deleted: {normalized}"
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
    /// survives rather than the entry being treated as unmatched.
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

    /// A well-formed source normalizes fully: link restored, label preserved,
    /// draft order kept, and a legitimate description survives untouched —
    /// the positive case proving the feature still fires, not just the
    /// negative/no-op guards around it.
    #[test]
    fn a_well_formed_source_normalizes_fully() {
        let seeds = vec![
            seed(
                "Ledger CLI",
                &["[Website](https://ledger.example.dev)"],
                &["Rust", "SQLite"],
                "A double-entry bookkeeping tool for small businesses.",
            ),
            seed(
                "CrossKit",
                &["https://github.com/janedoe/crosskit"],
                &[],
                "",
            ),
        ];
        let draft = "PROJECTS\n\n\
             **CrossKit** · https://an-altered-fork.example.com/crosskit\n\n\
             **Ledger CLI** · https://ledger.example.dev\n\
             A double-entry bookkeeping tool for small businesses.\n";
        let (normalized, stats) = normalize_projects_with_stats(draft, &seeds).unwrap();
        let crosskit_at = normalized.find("CrossKit").unwrap();
        let ledger_at = normalized.find("Ledger CLI").unwrap();
        assert!(
            crosskit_at < ledger_at,
            "draft order survives: {normalized}"
        );
        assert!(normalized.contains("https://github.com/janedoe/crosskit"));
        assert!(!normalized.contains("an-altered-fork"));
        assert!(normalized.contains("[Website](https://ledger.example.dev)"));
        assert!(normalized.contains("A double-entry bookkeeping tool for small businesses."));
        assert_eq!(stats.matched, 2);
        assert_eq!(stats.links_restored, 1, "only CrossKit's link was altered");
    }

    // ── C2 / all-verbatim: never a heading-only section ─────────────────

    /// Every draft entry is unrelated to the one seed the source has — none
    /// of them matches, so all stay verbatim and NOTHING was actually
    /// restored: a genuine no-op, never a section spliced down to just its
    /// heading.
    #[test]
    fn nothing_matched_is_a_no_op_not_a_heading_only_section() {
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
    /// site). A draft entry that cannot resolve to either by name must not
    /// guess — refused as ambiguous, so it stays VERBATIM. There is only one
    /// entry in this draft and it is not matched, so `matched == 0`, and the
    /// draft-side disagreement check also fires (the shared link IS present,
    /// unattached, in the draft text) — the whole pass is a no-op either way.
    #[test]
    fn an_ambiguous_link_rename_is_refused_and_the_entry_stays_verbatim() {
        let shared = "https://github.com/janedoe/monorepo";
        let seeds = vec![
            seed("App", &[shared], &[], ""),
            seed("Docs", &[shared], &[], ""),
        ];
        let draft = "PROJECTS\n\n**Renamed Thing** · https://github.com/janedoe/monorepo\n";
        assert_eq!(normalize_projects(draft, &seeds), None);
    }

    // ── L1/L2/L3: markdown link-span extraction (source.rs) ──────────────

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

    #[test]
    fn a_scheme_less_www_label_survives() {
        let source = "PROJECTS\n\n**Site** · [Website](www.example.com/app)\n";
        let seeds = crate::pipeline::resume::source::seed_projects(source);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].links, vec!["[Website](www.example.com/app)"]);
    }

    /// N3: a SKIPPED span (its parens hold no single recognizable URL) must
    /// not delete the bare URL that is genuinely present elsewhere on the
    /// SAME line — only spans actually captured as links are stripped before
    /// the bare-URL pass runs.
    #[test]
    fn a_skipped_span_does_not_delete_a_sibling_bare_url_on_the_same_line() {
        let source =
            "PROJECTS\n\n**Site** · [1](not a url at all) · https://github.com/janedoe/site\n";
        let seeds = crate::pipeline::resume::source::seed_projects(source);
        assert_eq!(seeds.len(), 1);
        assert!(
            seeds[0]
                .links
                .iter()
                .any(|l| l.contains("github.com/janedoe/site")),
            "the bare URL after a skipped, non-link bracket must still be harvested: {:?}",
            seeds[0].links
        );
    }

    // ── C1: the three reproduced corruption shapes are now no-ops ───────

    /// **C1-a: plain titles + bulleted achievements.** Bullets satisfy
    /// `project_entry_starts`, so a naive "any entry-start line" gate would
    /// have passed this — but the swallowed plain titles never become their
    /// own seed, so the ACHIEVEMENT BULLETS end up as fully-empty seeds
    /// (no link, no stack, no description). The empty-seed whole-bail catches
    /// it: no normalization at all, validators still grade the draft as-is.
    #[test]
    fn c1a_plain_titles_with_bulleted_achievements_disables_normalization() {
        let source = "PROJECTS\n\n\
            Ledger CLI\n\
            - Built a payment reconciliation tool\n\
            - Used Rust and SQLite\n\
            CrossKit\n\
            - An award-winning design system\n";
        let (seeds, reason) = seed_projects_for_normalize(source);
        assert!(seeds.is_empty(), "must disable normalization: {seeds:?}");
        assert_eq!(reason, Some("empty_seed"));

        // And end to end: a correct draft is left byte-for-byte untouched.
        let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\n\
             **CrossKit** · https://github.com/janedoe/crosskit\n";
        assert_eq!(normalize_projects(draft, &seeds), None);
    }

    /// **C1-b: bold titles + bullet achievements** (the max-depth-style
    /// shape). Each bold title opens its OWN entry here (unlike C1-a, it is
    /// not swallowed by the next bullet) — but that title-only entry then has
    /// no link, no stack and no description of its own (its achievements are
    /// bullets, which each open THEIR OWN entry instead of joining the
    /// title's), so it is a fully-empty seed too. The SAME empty-seed
    /// whole-bail that catches C1-a catches this shape as well.
    #[test]
    fn c1b_bold_titles_with_bullet_achievements_disables_normalization() {
        let source = "PROJECTS\n\n\
            **Ledger CLI**\n\
            - Built a payment reconciliation tool\n\
            - Used Rust and SQLite\n\
            **CrossKit**\n\
            - An award-winning design system\n";
        let (seeds, reason) = seed_projects_for_normalize(source);
        assert!(seeds.is_empty(), "must disable normalization: {seeds:?}");
        assert_eq!(reason, Some("empty_seed"));

        let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\n\
             **CrossKit** · https://github.com/janedoe/crosskit\n";
        assert_eq!(normalize_projects(draft, &seeds), None);
    }

    /// **C1-c: the DRAFT side has no entry boundary.** A plain-text draft
    /// merges two projects into one draft "entry" — the second project's
    /// link is present in the draft's Projects text but never attached to
    /// its own recognized entry. The draft-side parse-disagreement bail
    /// catches it: the unmatched seed's link is found unattached in the
    /// draft text, so the WHOLE pass is skipped — the draft is returned
    /// untouched, nothing merged or deleted.
    #[test]
    fn c1c_a_plain_text_draft_merging_two_projects_disables_normalization() {
        let seeds = vec![
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
        // Plain text: no bold, no bullet — both projects glue into ONE
        // `entries()` group, so only "Ledger CLI" (the entry's own title)
        // can ever resolve; CrossKit's link is present but unattached.
        let draft = "PROJECTS\n\n\
            Ledger CLI\n\
            https://github.com/janedoe/ledger\n\
            CrossKit\n\
            https://github.com/janedoe/crosskit\n";
        let outcome = normalize_projects_outcome(draft, &seeds);
        assert!(
            matches!(
                outcome,
                ProjectsNormalizeOutcome::Skipped("draft_parse_disagreement")
            ),
            "an unattached-but-present seed link must disable the whole pass: {outcome:?}"
        );
        assert_eq!(normalize_projects(draft, &seeds), None);
    }

    // ── the mega-seed guard: a link surviving in description/stack ──────

    /// **The residual mega-seed shape.** A 2-project plain-text source with
    /// no bold/bullet anywhere collapses into ONE seed (no sibling to
    /// compare against, so `seeds_are_plausible` is vacuous; the seed has a
    /// name, a link AND a description, so the empty-seed bail does not fire
    /// either) — but that single seed's swallowed SECOND title line
    /// ("Beta Sync · <url>") leaks its own URL into the merged
    /// `description`. A URL can never legitimately live in `description` or
    /// `stack` (the locked signature puts links on the title line only, and
    /// `seed_one_project` already strips a stack line's own URLs before they
    /// ever reach `stack`), so its presence there is unambiguous evidence the
    /// entry boundary swallowed a following project.
    #[test]
    fn a_link_surviving_in_the_description_disables_normalization() {
        let source = "PROJECTS\n\n\
            Ledger CLI\n\
            https://github.com/janedoe/ledger\n\
            Beta Sync · https://github.com/janedoe/beta\n\
            Go · gRPC\n";
        let (seeds, reason) = seed_projects_for_normalize(source);
        assert!(seeds.is_empty(), "must disable normalization: {seeds:?}");
        assert_eq!(reason, Some("link_in_description"));

        // End to end: a correct one-project draft is left byte-for-byte
        // untouched — no writing beta's URL onto the Ledger CLI entry.
        let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n";
        assert_eq!(normalize_projects(draft, &seeds), None);
    }

    /// **The negative case.** A single, honestly-formatted project (link ON
    /// the title line — the locked signature — so it never leaks into the
    /// description) still normalizes fully: the `link_in_description` guard
    /// must not disable the feature for the ordinary, correct shape.
    #[test]
    fn an_honest_single_project_source_still_normalizes() {
        let source = "PROJECTS\n\n\
            Ledger CLI · https://github.com/janedoe/ledger\n\
            A bookkeeping tool for freelancers.\n";
        let (seeds, reason) = seed_projects_for_normalize(source);
        assert_eq!(
            reason, None,
            "an honest source must not be disabled: {seeds:?}"
        );
        assert!(!seeds.is_empty());

        let draft = "PROJECTS\n\n**Ledger CLI** · https://github.com/janedoe/ledger\n\
            A bookkeeping tool for freelancers.\n";
        let outcome = normalize_projects_outcome(draft, &seeds);
        match outcome {
            ProjectsNormalizeOutcome::Applied(_, stats) => {
                assert_eq!(
                    stats.matched, 1,
                    "the feature still fires on an honest source"
                );
            }
            other => panic!("expected the feature to fire: {other:?}"),
        }
    }

    // ── N1: seeds_are_plausible ignores a truthful cross-reference ──────

    #[test]
    fn seeds_are_plausible_ignores_a_truthful_description_naming_a_sibling() {
        let plausible = vec![
            seed(
                "Ledger CLI",
                &["https://github.com/janedoe/ledger"],
                &[],
                "A bookkeeping tool — see also my CrossKit project for the design system side.",
            ),
            seed(
                "CrossKit",
                &["https://github.com/janedoe/crosskit"],
                &[],
                "",
            ),
        ];
        assert!(
            seeds_are_plausible(&plausible),
            "a truthful description naming a sibling by NAME must not switch normalization off"
        );
    }

    #[test]
    fn seeds_are_plausible_still_catches_a_link_leaking_into_a_stack_line() {
        let contaminated = vec![
            seed(
                "Ledger CLI",
                &["https://github.com/janedoe/ledger"],
                &["Rust", "https://github.com/janedoe/crosskit"],
                "",
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

    // ── stats ────────────────────────────────────────────────────────────

    #[test]
    fn stats_report_matched_dropped_and_links_restored() {
        let seeds = vec![
            seed("Alpha", &["https://github.com/janedoe/alpha"], &[], ""),
            seed("Beta", &["https://github.com/janedoe/beta"], &[], ""),
        ];
        let draft = "PROJECTS\n\n\
             **Alpha** · https://altered.example.com/alpha\n\n\
             **Ghost** · https://example.com/ghost-with-no-seed\n\n\
             **Beta** · https://github.com/janedoe/beta\n";
        let (_, stats) = normalize_projects_with_stats(draft, &seeds).unwrap();
        assert_eq!(stats.matched, 2, "Alpha and Beta are kept");
        assert_eq!(stats.links_restored, 1, "only Alpha's link was altered");
    }
}

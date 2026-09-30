//! Making the Projects section CODE-OWNED — `project_render::render_project`
//! is the same renderer this module's normalizer produces text through.
//!
//! The model writes the whole résumé body in one streamed call, projects
//! included — so a draft can rename a project, drop a link, or invent one the
//! source never had, and nothing catches it until the deterministic validator
//! flags it as a Critical several stages later. This module re-renders the
//! DRAFT's own Projects section from the same source-seeded [`ProjectOut`]s,
//! using the same parser the seeder does ([`source::section`]/
//! [`source::entries`]/[`source::seed_one_project`]) so this module and the
//! grader it feeds can never disagree about where an entry starts or what
//! counts as a link.
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
use crate::pipeline::resume::{project_render, source};
use crate::validate::content::{canonical_link, link_href, urls_in};

use super::project_seed::ProjectOut;
use super::stages::sections;

/// Seed [`ProjectOut`]s for normalization, plus WHY the list came back empty
/// when it did — content-free (ADR-027), for the draft-stage ledger.
///
/// Every extractor in `extraction::*` (PDF/DOCX/RTF) emits plain prose, no
/// markdown at all. `project_entry_starts` therefore reads a THIRD signal
/// besides bold and bullet: a short, non-sentence line directly above a
/// technology stack is a title (`export::parser::is_project_title_shaped`).
/// Before that, the only arm a plain-text section could fire was "first line
/// of the section", so a multi-project section collapsed into ONE garbled
/// mega-entry whose description swallowed every following project's title —
/// which the `link_in_description` bail below then correctly refused to
/// normalize from. A plain-prose section now groups correctly, so these bails
/// fire on genuinely broken input rather than on every imported résumé.
///
/// They are still load-bearing: a section with no stack lines at all has no
/// shape signal either, and still collapses.
///
/// Three independent, WHOLE-BAIL guards (a partial filter would still leave
/// the rest of a mis-grouped section to normalize from):
///
/// * **An empty seed.** A seed with no link, no stack AND no description
///   cannot come from a locked-signature entry — every accepted tier carries
///   at least a link (`render_project`'s own bottom rung refuses to emit a
///   bare name). It is always the symptom of a mis-grouped fragment: a title
///   swallowed by the previous bullet, an achievement bullet counted as its
///   own "entry". The SAME mis-grouping
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
/// quality-depth mirror of what `project_render::render_project` already
/// guarantees at max. See the module doc for why write authority is narrow.
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

    enum Resolution<'a> {
        /// No seed resolves (unmatched or ambiguous) — kept as its own raw
        /// text, never deleted.
        Verbatim,
        /// A SECOND entry resolving to a seed already used by an earlier one.
        Dedup,
        /// Resolves to `seeds[_]`, rebuilt from it. Carries the ALREADY-PARSED
        /// draft entry it was resolved from — structurally, not by re-reading
        /// `entries` at build time — so the second pass below can never reach
        /// a `None` here; there is nothing to `.expect()` past.
        Matched(usize, &'a ProjectOut),
    }

    // First pass: resolve every entry, tracking which seed indices got used.
    let mut used_indices: Vec<usize> = Vec::new();
    let mut resolutions: Vec<Resolution<'_>> = Vec::with_capacity(entries.len());
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
                resolutions.push(Resolution::Matched(index, project));
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
    for ((raw_text, _), resolution) in entries.iter().zip(resolutions.iter()) {
        match resolution {
            Resolution::Verbatim => pieces.push(raw_text.clone()),
            Resolution::Dedup => dropped += 1,
            Resolution::Matched(index, project) => {
                let seed = &seeds[*index];
                let project = *project;
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
                pieces.push(project_render::render_project(&rebuilt));
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
    // convention that separates entries: the blank belongs to neither side).
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
mod tests;

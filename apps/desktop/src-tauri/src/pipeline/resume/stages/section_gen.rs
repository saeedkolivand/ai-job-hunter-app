//! `sections` — the max-depth generator: ONE structured call per résumé
//! section, in a fixed order, sequentially.
//!
//! ## Why one call per section beats one call per document
//!
//! Not for quality alone: it is what makes the core rule mechanical at the
//! grain it actually breaks. A whole-body draft is asked for a document and
//! trusted to keep every employer, every link and every date; a section call is
//! asked for BULLETS and handed the identity separately, so renaming an
//! employer is not something the model has a field for. Each call also carries
//! only the slice of the source that section is grounded in, which is the
//! decomposition the 2026 measurements reward and the memory constraint the
//! plan sets.
//!
//! ## The order is fixed and seeded, never model-chosen
//!
//! Summary → Skills → one entry per [`CompanyPlan`] (the condensed
//! "earlier roles" group last) → Projects → Education. The roster comes from
//! `stages::strategy`, which seeded it from the parsed source, and Projects and
//! Education appear only when the SOURCE has them — a section the candidate's
//! own résumé does not have is a section a model would have to invent.
//!
//! ## Every answer is filtered before it is kept
//!
//! * citations that the grounded evidence map does not back are DROPPED, never
//!   repaired ([`ground_citations`]) — the same philosophy as the verbatim-quote
//!   filter in `stages::match_evidence`;
//! * a skill the source résumé does not state is dropped;
//! * an education entry that is not a character-for-character copy of a source
//!   line is dropped, and the SEEDED line is what gets rendered;
//! * a project's name, links and stack are re-seeded from the source whatever
//!   the model returned, and a description is kept only for a project whose
//!   source already had one.
//!
//! Every drop is counted and never named (ADR-027): the counts land in the
//! stage artifact, the content does not.

use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use crate::documents::evidence::{extract_evidence, EvidenceRole, SectionKind};
use crate::error::{AppError, AppResult};
use crate::pipeline::budget::StoppedReason;
use crate::pipeline::cache::KvCache;
use crate::pipeline::resume::cache::StageCacheKey;
use crate::pipeline::resume::prompt_blocks::resume_conventions;
use crate::pipeline::resume::section_prompts::{section_system, section_user};
use crate::pipeline::resume::types::{
    CompanyPlan, EvidenceItem, EvidenceMap, EvidenceStatus, JobAnalysis, ResumeStrategy,
    SectionKey, SkillGroup,
};
use crate::pipeline::resume::types_max::{
    example_for, schema_for, EducationOut, ExperienceOut, ProjectOut, ProjectsOut, SectionBody,
    SectionResult, SectionSeed, SectionSlot, SkillsOut, SummaryOut,
};
use crate::pipeline::resume::{
    cache, guard_deadline, source, QualityCtx, RunDeadline, RunLedger, SectionProgress,
};
use crate::pipeline::{Completer, Stage};

pub struct Sections;

pub const NAME: &str = "sections";

/// Bullets ONE employment entry may carry.
///
/// Six is what a recruiter reads before skimming, and the cap matters more than
/// the number: the fan-out is per section, so a model that answers with forty
/// bullets for each of nine roles turns one run into a document nothing can
/// render. The prompt asks for three to six; this enforces the ceiling.
pub const MAX_BULLETS_PER_ENTRY: usize = 6;

/// Char cap on the summary paragraph — four sentences of ordinary prose, with
/// room. Past this the model has written a cover letter.
pub const MAX_SUMMARY_CHARS: usize = 900;

/// Skills groups and skills-per-group a section may render. Both bound a model
/// that answers by listing every noun in the résumé.
pub const MAX_SKILL_GROUPS: usize = 8;
pub const MAX_SKILLS_PER_GROUP: usize = 20;

/// Citations one section may carry. The evidence map holds at most 40 items and
/// a section evidencing a dozen of them has stopped being a citation trail.
pub const MAX_CITATIONS: usize = 12;

/// Bullet markers a model prefixes its bullets with even when asked for plain
/// strings. Stripped on the way in because `assemble` renders the marker
/// itself, and `- - Migrated 40 services` is what happens otherwise.
const BULLET_MARKERS: [char; 7] = ['-', '–', '—', '*', '•', '·', '▪'];

/// One parsed section answer, discriminated by the seed it was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionAnswer {
    Summary(SummaryOut),
    Skills(SkillsOut),
    Experience(ExperienceOut),
    Projects(ProjectsOut),
    Education(EducationOut),
}

/// What one whole fan-out did — everything the ledger and the stage artifact
/// report, so the loop itself stays pure.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SectionsStats {
    pub planned: usize,
    /// Sections that produced usable content.
    pub kept: u32,
    /// Provider round-trips actually made.
    pub calls: u32,
    /// Sections served from the stage cache.
    pub cached: u32,
    /// Sections whose call errored — a failed SECTION, never a failed run.
    pub failed: u32,
    /// Sections whose answer parsed but carried nothing to render.
    pub empty: u32,
    /// The day's provider ceiling refused a call mid-fan-out.
    pub budgeted: bool,
    /// The run's wall clock ran out mid-fan-out.
    pub timed_out: bool,
    pub dropped_citations: u32,
    pub dropped_content: u32,
}

// ── Planning ─────────────────────────────────────────────────────────────────

/// The sections one max-depth run will generate, in order, fully seeded.
///
/// Pure, so "the source decides which sections exist" and "a role is never
/// dropped" are tests rather than claims. `max_sections` is the budget's
/// ceiling: the list is truncated from the TAIL, which drops Education before
/// Projects and Projects before an employment entry — the same priority a human
/// trimming to one page uses, and the only order in which the cut cannot cost a
/// `factual.dropped_role` Critical.
pub fn plan_sections(
    source_resume: &str,
    strategy: &ResumeStrategy,
    lang: &str,
    max_sections: usize,
) -> Vec<SectionSlot> {
    let conventions = resume_conventions(lang);
    let mut slots: Vec<SectionSlot> = Vec::new();

    slots.push(SectionSlot {
        key: SectionKey::Summary,
        heading: conventions.summary.to_string(),
        seed: SectionSeed::Summary,
    });

    let groups = grounded_groups(&strategy.skills_groups, source_resume);
    if !groups.is_empty() {
        slots.push(SectionSlot {
            key: SectionKey::Skills,
            heading: conventions.skills.to_string(),
            seed: SectionSeed::Skills(groups),
        });
    }

    for (index, plan) in strategy.per_company.iter().enumerate() {
        // `u8` is the wire grammar's own bound (`experience:<u8>`); the roster
        // is capped at nine entries, so the conversion cannot fail in practice
        // and a roster that somehow grew past 255 loses its tail rather than
        // emitting a key the generated guard would reject.
        let Ok(index) = u8::try_from(index) else {
            break;
        };
        slots.push(SectionSlot {
            key: SectionKey::Experience(index),
            heading: conventions.experience.to_string(),
            seed: SectionSeed::Experience(Box::new(plan.clone())),
        });
    }

    let projects = source::seed_projects(source_resume);
    if !projects.is_empty() {
        slots.push(SectionSlot {
            key: SectionKey::Projects,
            // The one heading with no entry in `resume_conventions`, so it is
            // taken from the source itself. A run that also translates the
            // document therefore keeps the candidate's own Projects wording —
            // stated rather than hidden, and better than inventing a heading in
            // a language the conventions table does not cover.
            heading: source::section(source_resume, SectionKind::Projects)
                .map(|section| section.heading)
                .unwrap_or_else(|| "Projects".to_string()),
            seed: SectionSeed::Projects(projects),
        });
    }

    let education = source::education_lines(source_resume);
    if !education.is_empty() {
        slots.push(SectionSlot {
            key: SectionKey::Education,
            heading: conventions.education.to_string(),
            seed: SectionSeed::Education(education),
        });
    }

    slots.truncate(max_sections);
    slots
}

/// The strategy's skills groups, with every skill the SOURCE does not state
/// removed.
///
/// The strategy prompt says a group may only carry skills the résumé
/// demonstrates, and `stages::strategy` does not enforce it — only `emphasis`
/// is filtered there. A skills SECTION is where an unsupported term does the
/// most damage (it reads as a claim, and 2026 ATS "candidate graph" checks
/// compare claimed against demonstrated), so the seed is filtered before the
/// model ever sees it.
fn grounded_groups(groups: &[SkillGroup], source_resume: &str) -> Vec<SkillGroup> {
    let haystack = normalized(source_resume);
    groups
        .iter()
        .take(MAX_SKILL_GROUPS)
        .map(|group| SkillGroup {
            label: group.label.clone(),
            skills: group
                .skills
                .iter()
                .filter(|skill| states(&haystack, skill))
                .take(MAX_SKILLS_PER_GROUP)
                .cloned()
                .collect(),
        })
        .filter(|group| !group.skills.is_empty())
        .collect()
}

/// The source slice ONE section is grounded in — the only text of the
/// candidate's own document that reaches its prompt.
///
/// Summary and Skills legitimately draw on the whole résumé (a summary is about
/// all of it), so they fall back to it when the source has no section of their
/// own; every other section gets exactly its own lines.
pub fn source_slice(slot: &SectionSlot, source_resume: &str, roles: &[EvidenceRole]) -> String {
    match &slot.seed {
        SectionSeed::Summary => section_or_whole(source_resume, SectionKind::Summary),
        SectionSeed::Skills(_) => section_or_whole(source_resume, SectionKind::Skills),
        SectionSeed::Experience(plan) => role_slice(plan, slot.key, roles),
        SectionSeed::Projects(_) => source::section(source_resume, SectionKind::Projects)
            .map(|section| section.text_lines().join("\n"))
            .unwrap_or_default(),
        SectionSeed::Education(lines) => lines.join("\n"),
    }
}

fn section_or_whole(source_resume: &str, kind: SectionKind) -> String {
    source::section(source_resume, kind)
        .map(|section| section.text_lines().join("\n"))
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| source_resume.to_string())
}

/// The source lines behind ONE roster entry.
///
/// The CONDENSED group stands for every role past the per-company cap, so its
/// slice is all of them — identity line included, because the group's whole job
/// is to show that the history continues.
fn role_slice(plan: &CompanyPlan, key: SectionKey, roles: &[EvidenceRole]) -> String {
    let index = match key {
        SectionKey::Experience(index) => index as usize,
        _ => 0,
    };
    let selected: Vec<&EvidenceRole> = if plan.condensed {
        roles.iter().skip(index).collect()
    } else {
        roles.get(index).into_iter().collect()
    };
    selected
        .iter()
        .map(|role| {
            let mut block = format!("{} — {} ({})", role.company, role.title, role.dates);
            for bullet in &role.bullets {
                block.push_str("\n- ");
                block.push_str(&bullet.text);
            }
            block
        })
        .collect::<Vec<String>>()
        .join("\n\n")
}

// ── Filtering ────────────────────────────────────────────────────────────────

/// Whitespace-collapsed lowercase, so "Node.js" and "node.js" compare equal and
/// a line break inside the source cannot hide a term.
fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .to_lowercase()
}

/// Whether the (already normalized) source states `term`.
fn states(haystack: &str, term: &str) -> bool {
    let term = normalized(term);
    !term.is_empty() && haystack.contains(&term)
}

/// The citations the grounded evidence map actually backs, plus how many were
/// dropped.
///
/// A citation matches an item the map does NOT list as `missing`, by its
/// requirement text or by its verbatim source quote. Everything else goes,
/// silently and without a re-ask — the same decision `stages::match_evidence`
/// takes about a paraphrased quote, for the same reason: a citation the source
/// cannot back is worth less than no citation, and asking the model again
/// invites it to produce a better-looking one.
pub fn ground_citations(citations: &[String], evidence: &EvidenceMap) -> (Vec<String>, u32) {
    let grounded: Vec<&EvidenceItem> = evidence
        .items
        .iter()
        .filter(|item| item.status != EvidenceStatus::Missing)
        .collect();
    let mut kept: Vec<String> = Vec::new();
    let mut dropped = 0u32;
    for citation in citations {
        let needle = citation.trim();
        if needle.is_empty() {
            continue;
        }
        let backed = grounded.iter().any(|item| {
            item.requirement.trim().eq_ignore_ascii_case(needle)
                || (!item.source_quote.trim().is_empty()
                    && item.source_quote.trim().eq_ignore_ascii_case(needle))
        });
        if !backed {
            dropped += 1;
            continue;
        }
        if kept.len() < MAX_CITATIONS && !kept.iter().any(|k| k.eq_ignore_ascii_case(needle)) {
            kept.push(needle.to_string());
        }
    }
    (kept, dropped)
}

/// Fold one section's answer onto its seed: re-seed everything the model may
/// not author, filter everything it may, and count what went.
///
/// Pure, and the single place the "what the model is allowed to author" table in
/// [`types_max`](crate::pipeline::resume::types_max) is enforced.
pub fn merge(
    slot: &SectionSlot,
    answer: SectionAnswer,
    source_resume: &str,
    evidence: &EvidenceMap,
) -> SectionResult {
    let (body, citations, dropped_content) = match (&slot.seed, answer) {
        (_, SectionAnswer::Summary(out)) => {
            let text = clamp_chars(
                strip_heading(&out.summary, &slot.heading),
                MAX_SUMMARY_CHARS,
            );
            (SectionBody::Summary(text), out.used_evidence, 0)
        }
        (SectionSeed::Skills(_), SectionAnswer::Skills(out)) => {
            let before: usize = out.groups.iter().map(|g| g.skills.len()).sum();
            let groups = grounded_groups(&out.groups, source_resume);
            let after: usize = groups.iter().map(|g| g.skills.len()).sum();
            (
                SectionBody::Skills(groups),
                out.used_evidence,
                before.saturating_sub(after) as u32,
            )
        }
        (_, SectionAnswer::Experience(out)) => {
            let bullets: Vec<String> = out
                .bullets
                .iter()
                .map(|bullet| strip_marker(bullet))
                .filter(|bullet| !bullet.is_empty())
                .take(MAX_BULLETS_PER_ENTRY)
                .collect();
            (SectionBody::Bullets(bullets), out.used_evidence, 0)
        }
        (SectionSeed::Projects(seeds), SectionAnswer::Projects(out)) => {
            let (projects, dropped) = reseed_projects(seeds, &out.projects);
            (SectionBody::Projects(projects), out.used_evidence, dropped)
        }
        (SectionSeed::Education(lines), SectionAnswer::Education(out)) => {
            let (entries, dropped) = keep_source_lines(lines, &out.entries);
            (SectionBody::Lines(entries), out.used_evidence, dropped)
        }
        // A shape that does not match its seed cannot be merged onto it. It
        // reaches here only if a future caller parses the wrong type for a
        // slot, so the honest result is an empty section — which the loop
        // records as `empty` and `assemble` skips — rather than a section
        // rendered from someone else's answer.
        (seed, _) => (empty_body(seed), Vec::new(), 0),
    };
    let (used_evidence, dropped_citations) = ground_citations(&citations, evidence);
    SectionResult {
        key: slot.key,
        heading: slot.heading.clone(),
        body,
        used_evidence,
        dropped_citations,
        dropped_content,
        from_cache: false,
    }
}

fn empty_body(seed: &SectionSeed) -> SectionBody {
    match seed {
        SectionSeed::Summary => SectionBody::Summary(String::new()),
        SectionSeed::Skills(_) => SectionBody::Skills(Vec::new()),
        SectionSeed::Experience(_) => SectionBody::Bullets(Vec::new()),
        SectionSeed::Projects(_) => SectionBody::Projects(Vec::new()),
        SectionSeed::Education(_) => SectionBody::Lines(Vec::new()),
    }
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
fn reseed_projects(seeds: &[ProjectOut], answered: &[ProjectOut]) -> (Vec<ProjectOut>, u32) {
    let mut out: Vec<ProjectOut> = Vec::new();
    let mut dropped = 0u32;
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
        let described = !seed.description.trim().is_empty();
        if !described && !project.description.trim().is_empty() {
            dropped += 1; // an invented blurb for a data-less project
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
    (out, dropped)
}

/// Project identity, compared the way `factual::project_entry_name` compares
/// it: lowercase alphanumeric words. Two graders disagreeing about whether two
/// entries are the same project is how a link Critical fires on a truthful
/// document.
fn same_project(left: &str, right: &str) -> bool {
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

/// Keep only the answers that are a character-for-character copy of a seeded
/// source line, and render the SEEDED line rather than the model's copy.
///
/// Comparison is whitespace-collapsed and case-insensitive so a re-indented
/// copy still matches; what gets rendered is the seed, so the document carries
/// the candidate's own spelling either way.
fn keep_source_lines(seeds: &[String], answered: &[String]) -> (Vec<String>, u32) {
    let mut out: Vec<String> = Vec::new();
    let mut dropped = 0u32;
    for entry in answered {
        let needle = normalized(entry);
        match seeds.iter().find(|seed| normalized(seed) == needle) {
            Some(seed) if !out.contains(seed) => out.push(seed.clone()),
            Some(_) => {}
            None => dropped += 1,
        }
    }
    (out, dropped)
}

/// Drop a heading line the model wrote anyway, then trim. Matched against the
/// heading THIS section will be rendered with, so an ordinary first sentence is
/// never mistaken for one.
fn strip_heading(text: &str, heading: &str) -> String {
    let mut lines = text.lines().peekable();
    if lines.peek().is_some_and(|first| {
        first
            .trim()
            .trim_end_matches(':')
            .eq_ignore_ascii_case(heading.trim())
    }) {
        lines.next();
    }
    lines.collect::<Vec<&str>>().join("\n").trim().to_string()
}

/// One paragraph: every internal newline becomes a space, so a description
/// cannot silently add lines to a project entry and push it out of the
/// structure check's accepted shapes.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn strip_marker(bullet: &str) -> String {
    bullet
        .trim()
        .trim_start_matches(BULLET_MARKERS)
        .trim()
        .to_string()
}

/// Clamp on CHARACTER boundaries — `text` is model output and arbitrary UTF-8,
/// and a byte slice through a multibyte char aborts a release build.
fn clamp_chars(text: String, max: usize) -> String {
    if text.chars().count() <= max {
        return text;
    }
    text.chars()
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

// ── The fan-out ──────────────────────────────────────────────────────────────

/// Generate every planned section, with the PROVIDER CALL injected.
///
/// Same seam shape, and the same reason, as `stages::repair::repair_loop`: a
/// `Completer` is bound to an `AppHandle`, this crate has no Tauri harness, and
/// the decisions worth proving — the deadline checked BETWEEN calls, one
/// section's error not costing the run the other eleven, the day's ceiling
/// stopping the fan-out rather than failing it — are exactly the ones a reader
/// cannot verify by reading.
///
/// No error here fails the run. A section that errors is a failed SECTION
/// (`assemble` renders the document without it, and a missing employment entry
/// becomes an honest `factual.dropped_role` Critical rather than a silently
/// short résumé); the day's ceiling is [`StoppedReason::Budgeted`], which keeps
/// what was already produced.
pub async fn generate_sections<F, Fut>(
    slots: &[SectionSlot],
    deadline: RunDeadline,
    progress: Option<&dyn SectionProgress>,
    mut ask: F,
) -> (Vec<SectionResult>, SectionsStats)
where
    F: FnMut(usize, SectionSlot) -> Fut,
    Fut: Future<Output = AppResult<SectionResult>>,
{
    let mut stats = SectionsStats {
        planned: slots.len(),
        ..SectionsStats::default()
    };
    let mut out: Vec<SectionResult> = Vec::new();

    for (index, slot) in slots.iter().enumerate() {
        // BETWEEN calls, not only before the stage: twelve calls at up to
        // `OLLAMA_COMPLETION` each is an hour a boundary-granular check would
        // not interrupt — the lesson the repair loop's per-section check
        // already carries.
        if deadline.passed() {
            stats.timed_out = true;
            break;
        }
        if let Some(progress) = progress {
            progress.section_started(slot.key, index, slots.len());
        }
        let outcome = ask(index, slot.clone()).await;
        let produced = match outcome {
            Ok(result) => {
                if result.from_cache {
                    stats.cached += 1;
                } else {
                    stats.calls += 1;
                }
                stats.dropped_citations += result.dropped_citations;
                stats.dropped_content += result.dropped_content;
                if result.is_empty() {
                    stats.empty += 1;
                    false
                } else {
                    stats.kept += 1;
                    out.push(result);
                    true
                }
            }
            // The day's ceiling refused this call, and would refuse every later
            // one the same way. Keep what the run already has — that is what
            // `Budgeted` means.
            Err(AppError::RateLimited(_)) => {
                stats.budgeted = true;
                if let Some(progress) = progress {
                    progress.section_finished(slot.key, index, slots.len(), false);
                }
                break;
            }
            Err(_) => {
                stats.failed += 1;
                false
            }
        };
        if let Some(progress) = progress {
            progress.section_finished(slot.key, index, slots.len(), produced);
        }
    }

    (out, stats)
}

/// `KvCache` namespace for ONE section, under the pipeline's own
/// `resume_stage:` prefix: `section:summary`, `section:experience:3`. Per
/// SECTION rather than per stage, because the whole point of the max fan-out is
/// that the sections are independent — a re-run that changed one company's
/// angle must miss that entry's cache and hit the other eleven.
fn cache_stage(slot: &SectionSlot) -> String {
    format!("section:{}", slot.key.to_wire())
}

/// Read one section's cached ANSWER. The stored value is the model's own typed
/// answer, not the merged result: re-running [`merge`] on a hit means a cached
/// section is filtered by today's rules rather than by whatever they were when
/// it was written.
fn cached_answer(
    cache: Option<&KvCache>,
    slot: &SectionSlot,
    key: &StageCacheKey,
) -> Option<SectionAnswer> {
    let stage = cache_stage(slot);
    match &slot.seed {
        SectionSeed::Summary => cache::get(cache, &stage, key).map(SectionAnswer::Summary),
        SectionSeed::Skills(_) => cache::get(cache, &stage, key).map(SectionAnswer::Skills),
        SectionSeed::Experience(_) => cache::get(cache, &stage, key).map(SectionAnswer::Experience),
        SectionSeed::Projects(_) => cache::get(cache, &stage, key).map(SectionAnswer::Projects),
        SectionSeed::Education(_) => cache::get(cache, &stage, key).map(SectionAnswer::Education),
    }
}

fn store_answer(
    cache: Option<&KvCache>,
    slot: &SectionSlot,
    key: &StageCacheKey,
    answer: &SectionAnswer,
) {
    let json = match answer {
        SectionAnswer::Summary(out) => serde_json::to_string(out),
        SectionAnswer::Skills(out) => serde_json::to_string(out),
        SectionAnswer::Experience(out) => serde_json::to_string(out),
        SectionAnswer::Projects(out) => serde_json::to_string(out),
        SectionAnswer::Education(out) => serde_json::to_string(out),
    };
    if let Ok(json) = json {
        cache::put(cache, &cache_stage(slot), key, &json);
    }
}

/// ONE section's structured call.
///
/// The `guard` is rebuilt per section from the run's own ledger and clock, for
/// the reason `Completer::complete_json` requires one at all: the re-ask it may
/// decide on is a second full provider call, and a run already out of time must
/// not pay for it.
#[allow(clippy::too_many_arguments)]
async fn ask_section(
    completer: &Completer,
    ledger: &RunLedger,
    deadline: RunDeadline,
    slot: &SectionSlot,
    lang: &str,
    source_slice: &str,
    analysis: &JobAnalysis,
    strategy: &ResumeStrategy,
    evidence: &EvidenceMap,
) -> AppResult<SectionAnswer> {
    let guard = || guard_deadline(ledger, deadline);
    let system = section_system(&slot.seed, lang);
    let user = section_user(slot, source_slice, analysis, strategy, evidence);
    let hint = example_for(&slot.seed);
    let schema = schema_for(&slot.seed);
    let schema = Some(&schema);
    Ok(match &slot.seed {
        SectionSeed::Summary => SectionAnswer::Summary(
            completer
                .complete_json(guard, &system, &user, hint, schema)
                .await?,
        ),
        SectionSeed::Skills(_) => SectionAnswer::Skills(
            completer
                .complete_json(guard, &system, &user, hint, schema)
                .await?,
        ),
        SectionSeed::Experience(_) => SectionAnswer::Experience(
            completer
                .complete_json(guard, &system, &user, hint, schema)
                .await?,
        ),
        SectionSeed::Projects(_) => SectionAnswer::Projects(
            completer
                .complete_json(guard, &system, &user, hint, schema)
                .await?,
        ),
        SectionSeed::Education(_) => SectionAnswer::Education(
            completer
                .complete_json(guard, &system, &user, hint, schema)
                .await?,
        ),
    })
}

#[async_trait]
impl<'a> Stage<QualityCtx<'a>> for Sections {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, ctx: &mut QualityCtx<'a>) -> AppResult<()> {
        let slots = plan_sections(
            ctx.input.source_resume,
            &ctx.strategy,
            ctx.input.target_language,
            ctx.budget.max_sections,
        );
        // The same reader `stages::strategy` seeded the roster with, so "which
        // lines belong to role 3" has one answer in this run.
        let roles = extract_evidence(ctx.input.source_resume, ctx.input.job_ad).roles;

        // Copied/borrowed out of `ctx` so the per-section closure below borrows
        // neither it nor itself; the shared borrows all end with the await.
        let input = ctx.input;
        let completer = ctx.completer;
        let cache_handle = ctx.cache;
        let deadline = ctx.deadline;
        let progress = ctx.progress;
        let base_key = ctx.cache_key.clone();
        let ledger = Arc::clone(&ctx.ledger);
        let analysis = &ctx.analysis;
        let strategy = &ctx.strategy;
        let evidence = &ctx.evidence;
        let roles = &roles;

        let (sections, stats) = generate_sections(&slots, deadline, progress, |_, slot| {
            let ledger = Arc::clone(&ledger);
            let mut key = base_key.clone();
            async move {
                // Each section's key extends the run's chain with its own wire
                // key, so the twelve entries are independent rows under one
                // upstream identity (ADR-017).
                key.extend(&slot.key.to_wire());
                if let Some(cached) = cached_answer(cache_handle, &slot, &key) {
                    let mut result = merge(&slot, cached, input.source_resume, evidence);
                    result.from_cache = true;
                    return Ok(result);
                }
                let slice = source_slice(&slot, input.source_resume, roles);
                let scoped = scoped_evidence(&slot, evidence);
                let answer = ask_section(
                    completer,
                    &ledger,
                    deadline,
                    &slot,
                    input.target_language,
                    &slice,
                    analysis,
                    strategy,
                    &scoped,
                )
                .await?;
                store_answer(cache_handle, &slot, &key, &answer);
                Ok(merge(&slot, answer, input.source_resume, evidence))
            }
        })
        .await;

        if stats.timed_out {
            ctx.ledger.stop(StoppedReason::RunTimeout);
        }
        if stats.budgeted {
            ctx.ledger.stop(StoppedReason::Budgeted);
        }
        for _ in 0..stats.calls {
            ctx.ledger.count_call(false);
        }
        for _ in 0..stats.cached {
            ctx.ledger.count_call(true);
        }
        // Counts only (ADR-027) — never a section's text, a dropped skill, or a
        // citation.
        ctx.ledger.record(
            NAME,
            json!({
                "planned": stats.planned,
                "kept": stats.kept,
                "cached": stats.cached,
                "failed": stats.failed,
                "empty": stats.empty,
                "droppedCitations": stats.dropped_citations,
                "droppedContent": stats.dropped_content,
                "budgeted": stats.budgeted,
                "timedOut": stats.timed_out,
            }),
        );
        ctx.sections = sections;
        Ok(())
    }
}

/// The evidence a section may cite.
///
/// Scoped for an employment entry — the items quoting THAT employer, plus the
/// ones its plan asks it to evidence — because a twelve-call fan-out that
/// re-sends all forty items each time spends most of its context on requirements
/// the section is not about. Falls back to the whole map when the scope is
/// empty, so a role with no attributed evidence can still cite what the résumé
/// supports.
pub fn scoped_evidence(slot: &SectionSlot, evidence: &EvidenceMap) -> EvidenceMap {
    let SectionSeed::Experience(plan) = &slot.seed else {
        return evidence.clone();
    };
    let items: Vec<EvidenceItem> = evidence
        .items
        .iter()
        .filter(|item| {
            item.status != EvidenceStatus::Missing
                && (item
                    .source_company
                    .trim()
                    .eq_ignore_ascii_case(plan.company.trim())
                    || plan
                        .emphasis
                        .iter()
                        .any(|term| term.trim().eq_ignore_ascii_case(item.requirement.trim())))
        })
        .cloned()
        .collect();
    if items.is_empty() {
        return evidence.clone();
    }
    EvidenceMap { items }
}

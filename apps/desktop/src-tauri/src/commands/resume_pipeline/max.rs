//! What differs between a quality run and a MAX run, in one place.
//!
//! The two depths share `execute` — the same resolution, the same run row, the
//! same persistence, the same terminal-state derivation — because everything
//! about a staged run except the STAGE LIST and its ceilings is identical, and
//! two copies of that shell is how the copies drift. What is genuinely
//! depth-dependent is three small choices, and they live here so the command
//! reads as one flow with three lookups rather than as two flows.
//!
//! The other half of this module is the max-depth per-section REGENERATE path,
//! which is a different shape from the quality one (it rebuilds an entry from
//! the run's persisted artifacts instead of asking a model to rewrite a section
//! of text) and is genuinely its own body.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::commands::ai_provider::timeouts;
use crate::documents::evidence::extract_evidence;
use crate::error::AppResult;
use crate::pipeline::budget::Budget;
use crate::pipeline::resume::stages::{section_gen, sections, SectionOutcome};
use crate::pipeline::resume::types::{
    EvidenceMap, GenerationDepth, JobAnalysis, ResumeStrategy, SectionKey,
};
use crate::pipeline::resume::types_max::{SectionSeed, SectionSlot};
use crate::pipeline::resume::{
    assemble, max_pipeline, quality_pipeline, run_deadline, QualityCtx, RunDeadline, RunLedger,
};
use crate::pipeline::runs::PipelineRunStore;
use crate::pipeline::{Completer, Pipeline};

use super::hooks::DETAIL_KEY;

/// Whether the STAGED pipeline runs this depth at all.
///
/// `fast` is the untouched single-shot TS path, and "the fast path stays
/// byte-for-byte" is a shipped guarantee — routing it through these stages
/// would silently change what the user asked for. A predicate rather than an
/// inline `match` in `execute` so that guarantee is a test.
pub(super) fn is_staged(depth: GenerationDepth) -> bool {
    matches!(depth, GenerationDepth::Quality | GenerationDepth::Max)
}

/// The stage list one depth runs.
pub(super) fn pipeline_for<'a>(depth: GenerationDepth) -> Pipeline<QualityCtx<'a>> {
    match depth {
        GenerationDepth::Max => max_pipeline(),
        // `fast` never reaches here — `execute` rejects it through
        // [`is_staged`] before resolving anything — and quality is the only
        // other staged depth.
        _ => quality_pipeline(),
    }
}

/// The compile-time ceilings one depth runs under. Never renderer-supplied:
/// the wire request has no budget field to bind (pinned by
/// `the_run_request_carries_no_budget_field`).
pub(super) fn budget_for(depth: GenerationDepth) -> Budget {
    match depth {
        GenerationDepth::Max => Budget::RESUME_MAX,
        _ => Budget::RESUME_QUALITY,
    }
}

/// The wall clock one depth's run gets, at this reasoning effort.
///
/// Both depths take the LARGER of their budget floor and their effort-scaled
/// allowance ([`run_deadline`]) — the two are pinned to agree at the bottom
/// tier, so in practice this is the scaled value above `minimal`/`low`.
pub(super) fn deadline_for(depth: GenerationDepth, effort: Option<&str>) -> Duration {
    let scaled = match depth {
        GenerationDepth::Max => timeouts::max_run_deadline(effort),
        _ => timeouts::quality_run_deadline(effort),
    };
    run_deadline(budget_for(depth), scaled)
}

// ── Per-entry regenerate ─────────────────────────────────────────────────────

/// The two artifacts a max-depth per-entry regenerate needs back: the plan the
/// entry was written from, and the evidence its citations are checked against.
#[derive(Debug, Clone)]
pub(super) struct RunArtifacts {
    pub strategy: ResumeStrategy,
    pub evidence: EvidenceMap,
    pub analysis: JobAnalysis,
}

/// Recover a finished max run's artifacts from its persisted event trail.
///
/// **Every miss here is SOFT.** A run that predates the detail, an artifact the
/// store's 16 KiB clamp truncated (the truncation marker makes it unparseable
/// BY DESIGN), a hand-edited database, a quality-depth run — all of them return
/// `None`, and the caller degrades to the repair-style whole-section rewrite
/// that has always been there. A click must never error because an optimization
/// was unavailable.
///
/// The LAST matching event wins: a stage that ran twice (it cannot today) would
/// have its newest artifact read, which is the one the document was built from.
pub(super) fn artifacts_for(store: &PipelineRunStore, run_id: &str) -> Option<RunArtifacts> {
    let events = store.events_for_run(run_id);
    let detail = |stage: &str| -> Option<Value> {
        events
            .iter()
            .rev()
            .filter(|event| event.stage == stage)
            .find_map(|event| {
                serde_json::from_str::<Value>(&event.artifact_json)
                    .ok()?
                    .get(DETAIL_KEY)
                    .cloned()
            })
    };
    let strategy: ResumeStrategy = serde_json::from_value(detail("strategy")?).ok()?;
    let evidence: EvidenceMap = serde_json::from_value(detail("match_evidence")?).ok()?;
    // The analysis is not persisted in full (its counts-only artifact is all the
    // trail needs), and the section prompt reads it only for tailoring context —
    // so an empty one degrades the ADVICE rather than the grounding.
    let analysis = detail("analyze_job")
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    if strategy.per_company.is_empty() {
        return None; // a plan with no roles cannot rebuild one
    }
    Some(RunArtifacts {
        strategy,
        evidence,
        analysis,
    })
}

/// Rebuild ONE employment entry of a finished max-depth document.
///
/// The quality path asks a model to rewrite the SECTION as text and splices the
/// answer back. This one does something different, and better, because the
/// document was assembled rather than drafted: it regenerates the entry through
/// the SAME [`section_gen::generate_one`] the run used — seeded identity, scoped
/// evidence, the same filters — renders it through the SAME `assemble`, and
/// splices only that entry's line range. The other eight roles are not re-rolled
/// and their bullets cannot change.
///
/// [`SectionOutcome::Missing`] when the document has no such entry (a document
/// edited since the run, or a roster that no longer matches it) — the caller
/// then falls back to the whole-section path rather than failing the click.
#[allow(clippy::too_many_arguments)]
pub(super) async fn regenerate_entry(
    completer: &Completer,
    artifacts: &RunArtifacts,
    source_resume: &str,
    job_ad: &str,
    target_language: &str,
    document: &str,
    index: u8,
    note: Option<&str>,
) -> AppResult<SectionOutcome> {
    let Some(plan) = artifacts.strategy.per_company.get(index as usize) else {
        return Ok(SectionOutcome::Missing);
    };
    let Some(entry) = sections::entry_range(document, index) else {
        return Ok(SectionOutcome::Missing);
    };
    let slot = SectionSlot {
        key: SectionKey::Experience(index),
        // The heading is not re-rendered — the splice replaces the ENTRY's line
        // range, which starts below it.
        heading: String::new(),
        seed: SectionSeed::Experience(Box::new(plan.clone())),
    };
    let roles = extract_evidence(source_resume, job_ad).roles;
    // A click is not a run: it has no ledger to stop and no run clock to share.
    // The bound it gets is one step's worth of wall time, which is what
    // `guard_deadline` needs to refuse the re-ask of a call that already hung.
    let ledger = Arc::new(RunLedger::new());
    let deadline = RunDeadline::starting_now(Budget::RESUME_MAX.step_timeout);
    let result = section_gen::generate_one(
        completer,
        &ledger,
        deadline,
        &slot,
        target_language,
        source_resume,
        &roles,
        &artifacts.analysis,
        &artifacts.strategy,
        &artifacts.evidence,
        note,
    )
    .await?;
    if result.is_empty() {
        // The same verdict the run's own fan-out reaches for an empty answer,
        // and the same one the quality path reaches for a truncated section: a
        // failed attempt, never a spliced-in gap.
        return Ok(SectionOutcome::Unusable);
    }
    let rendered = assemble::chunk(&result, false);
    Ok(SectionOutcome::Replaced(sections::splice(
        document, &entry, &rendered,
    )))
}

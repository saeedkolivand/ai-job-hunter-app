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

use std::time::Duration;

use crate::commands::ai_provider::timeouts;
use crate::pipeline::budget::Budget;
use crate::pipeline::resume::types::GenerationDepth;
use crate::pipeline::resume::{max_pipeline, quality_pipeline, run_deadline, QualityCtx};
use crate::pipeline::Pipeline;

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

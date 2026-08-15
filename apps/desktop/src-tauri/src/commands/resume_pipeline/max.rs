//! The staged pipeline's own budget/deadline lookups.
//!
//! This module used to pick between the quality and max depths — the stage
//! list, the budget, and the deadline all varied by depth. Since the `max`
//! depth was removed, every run takes the same pipeline; what survives is the
//! quality-only budget/deadline pair, kept as functions (rather than inlined
//! at the call site in `mod.rs`) because [`paying_stages`] is still real
//! decision logic worth naming and testing on its own.

use std::time::Duration;

use crate::commands::ai_provider::timeouts;
use crate::pipeline::budget::Budget;
use crate::pipeline::resume::{quality_pipeline, run_deadline, QualityCtx};
use crate::pipeline::Pipeline;

/// The stages that can actually SPEND a provider call — the only ones a
/// per-stage model override can apply to.
///
/// Read off the pipeline that runs (`free_stage_names`), not off a transcribed
/// list: a stage that stops paying is then a test failure rather than a
/// resolution nobody needed. The store also refuses to STORE an override on a
/// free stage (`ai_config::stage_overrides::is_overridable_stage`); this is
/// the second belt, and the one that matters for a hand-edited database —
/// resolving an override propagates its error, so a bad row on a stage that
/// never calls a provider would otherwise abort the whole run before it
/// started.
pub(super) fn paying_stages() -> Vec<&'static str> {
    let pipeline: Pipeline<QualityCtx<'_>> = quality_pipeline();
    let free = pipeline.free_stage_names();
    pipeline
        .stage_names()
        .into_iter()
        .filter(|stage| !free.contains(stage))
        .collect()
}

/// The compile-time ceiling a run runs under. Never renderer-supplied: the
/// wire request has no budget field to bind (pinned by
/// `the_run_request_carries_no_budget_field`).
pub(super) fn budget() -> Budget {
    Budget::RESUME_QUALITY
}

/// The wall clock a run gets, at this reasoning effort.
///
/// Takes the LARGER of the budget floor and the effort-scaled allowance
/// ([`run_deadline`]).
pub(super) fn deadline_for(effort: Option<&str>) -> Duration {
    run_deadline(budget(), timeouts::quality_run_deadline(effort))
}

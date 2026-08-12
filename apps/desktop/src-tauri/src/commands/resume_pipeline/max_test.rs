//! Tests for the depth SWITCH — what a max run does differently from a quality
//! run, and what it deliberately does the same.
//!
//! A focused module rather than more of the (already large) sibling `test`
//! file: everything here is about one decision, and every guard was
//! mutation-checked by applying the change its doc names.

use super::max::{budget_for, deadline_for, is_staged, pipeline_for};
use crate::pipeline::budget::Budget;
use crate::pipeline::resume::types::GenerationDepth;
use crate::pipeline::resume::{MAX_STAGES, QUALITY_STAGES};

/// **The fast path stays byte-for-byte.** `fast` is the untouched single-shot
/// TS path; admitting it here would silently run seven stages for a user who
/// asked for one call, and the depth toggle's own copy promises otherwise.
///
/// Mutation check: add `GenerationDepth::Fast` to `is_staged`'s match and this
/// fails.
#[test]
fn the_staged_pipeline_admits_quality_and_max_and_refuses_fast() {
    assert!(is_staged(GenerationDepth::Quality));
    assert!(is_staged(GenerationDepth::Max));
    assert!(!is_staged(GenerationDepth::Fast));
}

/// Each depth runs its OWN stage list, and the constant the renderer's timeline
/// keys on is compared against the pipeline that actually runs — not against a
/// literal, which would only pin the literal.
///
/// Mutation check: swap `max_pipeline()` for `quality_pipeline()` in
/// `pipeline_for`, or reorder a stage in either pipeline, and this fails.
#[test]
fn each_depth_runs_its_own_pinned_stage_list() {
    assert_eq!(
        pipeline_for(GenerationDepth::Max).stage_names(),
        MAX_STAGES,
        "MAX_STAGES must describe the pipeline that runs"
    );
    assert_eq!(
        pipeline_for(GenerationDepth::Quality).stage_names(),
        QUALITY_STAGES
    );

    // The first three stages are the SAME stages at both depths — that is what
    // lets a max run of an already-analyzed posting hit the quality run's cache
    // entries instead of paying for the analysis again.
    assert_eq!(MAX_STAGES[..3], QUALITY_STAGES[..3]);
    // …and where they diverge, max replaces the one streamed draft with the
    // section fan-out plus a pure renderer.
    assert!(MAX_STAGES.contains(&"sections") && MAX_STAGES.contains(&"assemble"));
    assert!(!MAX_STAGES.contains(&"draft"));
}

/// The ceilings are picked from the DEPTH, never from the request — and the two
/// budgets are genuinely different objects, so a max run cannot silently inherit
/// the quality section count or deadline.
///
/// Mutation check: return `Budget::RESUME_QUALITY` for `Max` in `budget_for`
/// and both the budget assertion and the deadline one fail.
#[test]
fn each_depth_runs_under_its_own_backend_owned_budget() {
    assert_eq!(budget_for(GenerationDepth::Max), Budget::RESUME_MAX);
    assert_eq!(budget_for(GenerationDepth::Quality), Budget::RESUME_QUALITY);
    assert_ne!(Budget::RESUME_MAX, Budget::RESUME_QUALITY);
    assert!(
        Budget::RESUME_MAX.max_steps > Budget::RESUME_QUALITY.max_steps,
        "max depth takes one step per section on top of the framing stages"
    );
}

/// A max run gets MORE wall clock than a quality run at every tier — it makes
/// twelve calls where quality makes one — and the deadline scales with effort
/// at both depths.
///
/// Mutation check: point `deadline_for(Max, …)` at `quality_run_deadline` and
/// the "strictly longer" assertion fails.
#[test]
fn the_max_deadline_is_longer_than_the_quality_one_and_scales_with_effort() {
    for effort in [None, Some("medium"), Some("high"), Some("max")] {
        assert!(
            deadline_for(GenerationDepth::Max, effort)
                > deadline_for(GenerationDepth::Quality, effort),
            "the max deadline must exceed the quality one at {effort:?}"
        );
    }
    assert!(
        deadline_for(GenerationDepth::Max, Some("max")) > deadline_for(GenerationDepth::Max, None),
        "a higher reasoning effort must buy more wall clock"
    );
    // The floor is the budget's own constant, so the two cannot drift apart.
    assert_eq!(
        deadline_for(GenerationDepth::Max, None),
        Budget::RESUME_MAX.run_timeout
    );
}

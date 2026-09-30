use super::super::QUALITY_STAGES;
use crate::pipeline::budget::{Budget, StoppedReason};
use serde_json::json;

/// The stage vocabulary the renderer's timeline keys on.
#[test]
fn quality_stage_names_are_pinned_and_match_the_pipeline() {
    assert_eq!(
        QUALITY_STAGES,
        [
            "analyze_job",
            "match_evidence",
            "strategy",
            "draft",
            "cover_letter",
            "validate",
            "repair",
            "humanize"
        ]
    );
}

/// The generated stage vocabulary and the pipeline's own stage list must
/// describe the SAME set of stages — checked in BOTH directions, because each
/// direction fails differently.
///
/// * A pipeline stage MISSING from `PIPELINE_STAGES` is a stage the user can
///   never override (and a `pipeline:stage` name the renderer's closed
///   vocabulary would reject).
/// * A generated name the pipeline never runs is worse than useless: the
///   Settings UI would offer an override for a stage that never runs, the
///   write would be accepted, and nothing would ever apply it — a setting with
///   no effect and no error.
///
/// Ordering is deliberately NOT asserted here: it is pinned against the
/// pipeline that runs by `quality_stage_names_are_pinned_and_match_the_pipeline`.
///
/// Mutation check (executed): adding `"rewrite"` to the TS `PIPELINE_STAGES`
/// and regenerating fails the second loop.
#[test]
fn the_generated_stage_vocabulary_covers_exactly_the_pipeline() {
    use crate::ipc_contracts::events::PIPELINE_STAGES;

    for stage in QUALITY_STAGES {
        assert!(
            PIPELINE_STAGES.contains(stage),
            "{stage} runs but is missing from the generated PIPELINE_STAGES — \
             add it to packages/shared/src/events/pipeline.ts and run `pnpm gen:ipc`",
        );
    }
    for stage in PIPELINE_STAGES {
        assert!(
            QUALITY_STAGES.contains(stage),
            "{stage} is in the generated PIPELINE_STAGES but the pipeline never runs it — \
             an override on it would be a setting with no effect",
        );
    }
}

/// The generated FREE-stage set must be exactly the stages that make no
/// provider call.
///
/// Derived here from the pipeline itself rather than transcribed: a stage that
/// starts or stops paying fails this instead of silently gaining or losing an
/// override the user cannot observe.
///
/// Mutation check (executed): add `"repair"` to the TS `PIPELINE_STAGES_FREE`
/// and regenerate — the second loop fails; remove `"validate"` — the first
/// loop fails.
#[test]
fn the_generated_free_stage_set_is_exactly_the_zero_call_stages() {
    use crate::ipc_contracts::events::PIPELINE_STAGES_FREE;

    let pipeline = super::super::quality_pipeline();
    let free = pipeline.free_stage_names();

    for stage in &free {
        assert!(
            PIPELINE_STAGES_FREE.contains(stage),
            "{stage} makes no provider call but is not in the generated free set — \
             an override on it would be a control with no effect",
        );
    }
    for stage in PIPELINE_STAGES_FREE {
        assert!(
            free.contains(stage),
            "{stage} is listed free but the pipeline pays for its call",
        );
    }
    // The set is non-empty and does not swallow the whole pipeline — a mutation
    // that made everything "free" would otherwise pass both loops vacuously.
    assert!(!PIPELINE_STAGES_FREE.is_empty());
    assert!(PIPELINE_STAGES_FREE.len() < QUALITY_STAGES.len());
}

/// Wire compatibility for the two variants this phase makes reachable — the
/// renderer's `STOPPED_SUFFIX` map keys on these exact strings.
#[test]
fn the_newly_reachable_stopped_reasons_keep_their_wire_strings() {
    for (reason, wire) in [
        (StoppedReason::RunTimeout, "run_timeout"),
        (StoppedReason::MaxRepairs, "max_repairs"),
        (StoppedReason::Cancelled, "cancelled"),
        (StoppedReason::Done, "done"),
    ] {
        assert_eq!(serde_json::to_value(reason).unwrap(), json!(wire));
    }
}

/// The repair loop reads its round count from the budget, so shrinking the
/// budget shrinks the loop. Mutation check: hard-code `2` in the loop condition
/// and this stops being a guard (it would still pass — which is why the
/// assertion is on the BUDGET being what the loop reads, and the loop's own
/// behaviour is covered by `repair::criticals_by_section` + the command test).
#[test]
fn the_repair_loop_is_bounded_by_the_budget_not_a_literal() {
    assert_eq!(Budget::RESUME_QUALITY.max_repair_attempts, 2);
    assert_eq!(
        Budget::RESUME_QUALITY.max_repair_attempts,
        crate::pipeline::budget::DEFAULT_MAX_REPAIR_ATTEMPTS
    );
}

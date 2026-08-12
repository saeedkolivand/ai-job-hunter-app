//! Tests for the depth SWITCH — what a max run does differently from a quality
//! run, and what it deliberately does the same.
//!
//! A focused module rather than more of the (already large) sibling `test`
//! file: everything here is about one decision, and every guard was
//! mutation-checked by applying the change its doc names.

use serde_json::json;

use super::hooks::{with_detail, DETAIL_KEY};
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
    // A const block, because both operands are compile-time constants and
    // clippy is right that a runtime `assert!` on two of them proves nothing at
    // test time that it would not prove at build time.
    const _: () = assert!(
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

// ── The persisted artifact detail ────────────────────────────────────────────

/// The DB row carries BOTH halves: the counts the runs panel and the event
/// trail already read, and — at max depth only — the full artifact a later
/// per-entry regenerate needs. Nesting rather than replacing is what keeps a
/// reader that knows nothing about the detail seeing exactly what it saw before.
///
/// Mutation check: return the detail alone from `with_detail` and the counts
/// assertion fails; return the artifact alone and the detail one does.
#[test]
fn the_full_artifact_rides_inside_the_counts_not_instead_of_them() {
    let counts = json!({ "cached": false, "companies": 3 });
    let detail = json!({ "perCompany": [{ "company": "Acme Payments" }] });

    let row = with_detail(Some(counts.clone()), Some(detail.clone())).expect("a row artifact");
    assert_eq!(row.get("companies"), counts.get("companies"));
    assert_eq!(row.get(DETAIL_KEY), Some(&detail));
}

/// A stage with NO detail — every stage at quality depth, and most of them at
/// max — produces the row it always produced. The detail is additive or it is
/// a behaviour change to a shipped trail.
///
/// Mutation check: make `with_detail` return `Some(artifact)` unconditionally
/// (dropping the `detail?`) and the `None` case gains an empty object where
/// the row previously had one shape.
#[test]
fn a_stage_without_a_detail_writes_exactly_the_artifact_it_always_did() {
    let counts = json!({ "issues": 2, "criticals": 0 });
    assert_eq!(with_detail(Some(counts.clone()), None), None);
    assert_eq!(with_detail(None, None), None);
    // …and with a detail but no counts, the row still carries the detail rather
    // than dropping it on the floor.
    let detail = json!({ "items": [] });
    let row = with_detail(None, Some(detail.clone())).expect("a row artifact");
    assert_eq!(row.get(DETAIL_KEY), Some(&detail));
}

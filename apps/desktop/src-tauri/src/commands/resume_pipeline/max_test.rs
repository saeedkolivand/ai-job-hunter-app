//! Tests for the depth SWITCH — what a max run does differently from a quality
//! run, and what it deliberately does the same.
//!
//! A focused module rather than more of the (already large) sibling `test`
//! file: everything here is about one decision, and every guard was
//! mutation-checked by applying the change its doc names.

use serde_json::json;

use super::hooks::{with_detail, DETAIL_KEY};
use super::max::{budget_for, deadline_for, is_staged, pipeline_for, role_matches_plan};
use crate::documents::evidence::EvidenceRole;
use crate::pipeline::budget::Budget;
use crate::pipeline::resume::types::{CompanyPlan, GenerationDepth};
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

/// **A run stopped by its own clock keeps what it already paid for.** That
/// claim is what sized `Budget::RESUME_MAX`'s deadline at the reachable 7 200 s
/// instead of the 12 000 s worst case, and it was false: `sections` breaks out
/// of its fan-out on the deadline and returns its finished sections `Ok`, and
/// the boundary check then refused `assemble` — so a run with eleven paid
/// answers in hand persisted nothing and reported `failed`.
///
/// The two stages between the fan-out and a saved document make no provider
/// call, which is exactly why the boundary may let them through. Read off the
/// pipelines that actually run, so marking a paid stage free (or a free stage
/// paid) fails here rather than in production.
///
/// Mutation check: delete `Assemble::costs_a_provider_call` and the max list
/// loses `assemble`; add the override to `Sections`, `Repair` or `Judge` and
/// the "everything else costs money" assertion fails.
#[test]
fn only_the_zero_call_stages_may_run_after_the_deadline() {
    assert_eq!(
        pipeline_for(GenerationDepth::Max).free_stage_names(),
        vec!["assemble", "validate"],
        "the render + the check are the two stages that turn paid answers into a saved document"
    );
    assert_eq!(
        pipeline_for(GenerationDepth::Quality).free_stage_names(),
        vec!["validate"]
    );
    // Every OTHER stage of both pipelines costs a call, so none of them can be
    // reached past the deadline.
    for depth in [GenerationDepth::Max, GenerationDepth::Quality] {
        let pipeline = pipeline_for(depth);
        let free = pipeline.free_stage_names();
        let paid: Vec<&str> = pipeline
            .stage_names()
            .into_iter()
            .filter(|stage| !free.contains(stage))
            .collect();
        assert!(
            paid.contains(&"repair") && paid.contains(&"strategy"),
            "{depth:?}: a stage that fans out to a provider must never be free: {paid:?}"
        );
        assert!(
            !free.contains(&"sections") && !free.contains(&"llm_judge") && !free.contains(&"draft"),
            "{depth:?}: {free:?}"
        );
    }
}

// ── The per-entry regenerate's identity join ─────────────────────────────────

/// A roster entry, with its title and dates SPELLABLE.
///
/// They used to be hard-coded, which is precisely why the company-only join
/// survived a review: two stints at one employer — a promotion, the commonest
/// roster shape after one — could not be written down with these helpers, so
/// the case that breaks the join could not be tested.
fn plan(company: &str, title: &str, dates: &str, condensed: bool) -> CompanyPlan {
    CompanyPlan {
        company: company.to_string(),
        title: title.to_string(),
        dates: dates.to_string(),
        condensed,
        ..CompanyPlan::default()
    }
}

fn role(company: &str, title: &str, dates: &str) -> EvidenceRole {
    EvidenceRole {
        company: company.to_string(),
        title: title.to_string(),
        dates: dates.to_string(),
        bullets: Vec::new(),
    }
}

/// The two stints at ONE employer that the old fixtures could not express.
fn junior() -> (CompanyPlan, EvidenceRole) {
    (
        plan("Acme Payments", "Backend Engineer", "2018 - 2021", false),
        role("Acme Payments", "Backend Engineer", "2018 - 2021"),
    )
}

fn senior() -> (CompanyPlan, EvidenceRole) {
    (
        plan("Acme Payments", "Staff Engineer", "2021 - Present", false),
        role("Acme Payments", "Staff Engineer", "2021 - Present"),
    )
}

/// The SOURCE half of the identity join. `section_gen::source_slice` reads the
/// facts for an employment entry out of `roles[index]`, so a source résumé
/// re-imported or re-ordered between the run and the click would hand the
/// prompt ANOTHER employer's bullets under this plan's seeded identity line —
/// and the seeded-identity rule cannot catch that, because the identity is
/// precisely the part that stays right.
///
/// **Compared on the whole TRIPLE.** Company alone left the defect alive for
/// two stints at one employer: both rows answer to "Acme Payments", so a
/// shifted source still "matched" and the junior stint's bullets were used to
/// rebuild the senior one.
///
/// Mutation check: compare `company` only and the promotion case below fails;
/// make this return `true` unconditionally and the shifted-source case does.
/// (`title` and `dates` are not separately pinned HERE — the promotion fixture
/// differs in both, and the document-side twin
/// `a_regenerate_tells_two_stints_at_one_employer_apart` is where each field
/// carries its own case.)
#[test]
fn a_regenerate_refuses_a_source_role_that_is_no_longer_the_plans_employer() {
    let (junior_plan, junior_role) = junior();
    let (senior_plan, senior_role) = senior();
    let roles = [
        role("Globex Logistics", "Backend Engineer", "2015 - 2018"),
        junior_role.clone(),
    ];

    assert!(role_matches_plan(&junior_plan, 1, &roles));
    // Case-insensitively, because the roster and the reader both derive their
    // string from the same line but not necessarily from the same run.
    assert!(role_matches_plan(
        &plan("acme payments", "backend engineer", "2018 - 2021", false),
        1,
        &roles
    ));

    // A role prepended to the source since the run: index 1 is now Globex, and
    // rebuilding Acme from Globex's bullets is a fabrication with a truthful
    // identity line on top of it.
    let shifted = [
        role("Initech", "Platform Engineer", "2012 - 2015"),
        role("Globex Logistics", "Backend Engineer", "2015 - 2018"),
        junior_role.clone(),
    ];
    assert!(!role_matches_plan(&junior_plan, 1, &shifted));

    // **The promotion.** Both stints are at Acme Payments, so a company-only
    // join cannot tell them apart — and the source order having shifted by one
    // then rebuilds the SENIOR stint out of the JUNIOR stint's bullets, with a
    // seeded identity line that stays correct the whole time.
    let promotion = [senior_role.clone(), junior_role.clone()];
    assert!(
        role_matches_plan(&senior_plan, 0, &promotion)
            && role_matches_plan(&junior_plan, 1, &promotion),
        "the premise: in order, both stints resolve"
    );
    assert!(
        !role_matches_plan(&senior_plan, 1, &promotion),
        "an employer name is not a primary key — the title and dates say which stint"
    );
    assert!(!role_matches_plan(&junior_plan, 0, &promotion));

    // Past the end of a shortened source.
    assert!(!role_matches_plan(&junior_plan, 5, &roles));
    // Unattributed: nothing to join on, so it degrades rather than guessing.
    assert!(!role_matches_plan(&plan("", "", "", false), 0, &roles));

    // The condensed group is exempt BY CONSTRUCTION: it stands for every role
    // past the cap, its slice is `roles[index..]`, and "Earlier roles" is a
    // label rather than an employer — so it can never match one.
    assert!(role_matches_plan(
        &plan("Earlier roles", "Initech, Globex", "2005 - 2015", true),
        1,
        &roles
    ));
    assert!(
        !role_matches_plan(
            &plan("Earlier roles", "Initech, Globex", "2005 - 2015", false),
            1,
            &roles
        ),
        "the exemption must come from the condensed FLAG, not from the label"
    );
}

// ── The persisted artifact detail ────────────────────────────────────────────

/// **The detail is server-side, and the wire proves it.** `record_detail`'s doc
/// says the full artifact rides in the persisted row "and nowhere else"; the
/// run-detail response handed the trail to the renderer verbatim, so every
/// `get` of a max run shipped up to 2 × 16 KiB of employment history and
/// verbatim résumé quotes to a reader with no consumer for either.
///
/// Mutation check: return the parsed value unchanged from `wire_artifact` and
/// the first assertion fails; return `Value::String(raw)` for the unparseable
/// case and the truncated-artifact assertion does.
#[test]
fn the_wire_carries_a_stage_artifacts_counts_but_never_its_detail() {
    let row = with_detail(
        Some(json!({ "cached": false, "companies": 3 })),
        Some(json!({ "perCompany": [{ "company": "Acme Payments" }] })),
    )
    .expect("a row artifact")
    .to_string();

    let wire = super::wire_artifact(&row);
    assert_eq!(wire.get("companies"), Some(&json!(3)));
    assert_eq!(
        wire.get(DETAIL_KEY),
        None,
        "the detail is what a per-entry regenerate reads on the SERVER"
    );

    // A clamped artifact is unparseable BY DESIGN, and it is the only artifact
    // large enough to be clamped — so the old "ship the raw string" arm was the
    // same leak with a truncation marker on the end.
    let clamped = crate::pipeline::runs::clamp_artifact(&row.repeat(2_000));
    let wire = super::wire_artifact(&clamped);
    assert_eq!(wire, json!({ "truncated": true }));
    assert!(
        !wire.to_string().contains("Acme Payments"),
        "a truncated artifact must not carry its content to the renderer either"
    );
}

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
/// max — produces the row it always produced: its COUNTS, unchanged and
/// untouched. The detail is additive or it is a behaviour change to a shipped
/// trail.
///
/// This is the assertion that used to read `== None`, which pinned a defect
/// rather than a behaviour: `with_detail` opened with `let detail = detail?;`,
/// so every detail-less stage (all six at quality depth, five of eight at max)
/// had its counts DELETED on the way to `pipeline_run_events.artifact_json` —
/// the row became `"{}"` and the `pipeline:stage` event's
/// `issueCount`/`criticalCount` became null for every run at both depths.
///
/// Mutation check: restore `let detail = detail?;` as the first line of
/// `with_detail` and the counts assertion fails.
#[test]
fn a_stage_without_a_detail_writes_exactly_the_artifact_it_always_did() {
    let counts = json!({ "issues": 2, "criticals": 0 });
    assert_eq!(
        with_detail(Some(counts.clone()), None),
        Some(counts),
        "a detail-less stage keeps its counts — they are what the runs panel reads"
    );
    assert_eq!(with_detail(None, None), None);
    // …and with a detail but no counts, the row still carries the detail rather
    // than dropping it on the floor.
    let detail = json!({ "items": [] });
    let row = with_detail(None, Some(detail.clone())).expect("a row artifact");
    assert_eq!(row.get(DETAIL_KEY), Some(&detail));
}

// ── The completeness floor on an overwrite ───────────────────────────────────

/// **A stopped run may overwrite the saved document, but not with a résumé that
/// has no work history.**
///
/// `persist_document` is an OVERWRITE — one `ai_generations` row per posting, no
/// versioning — and since a deadline-stopped fan-out now keeps its sections
/// (F3), a partial document can reach it. That trade is deliberate for a run
/// that lost TAIL sections: the missing employer is a visible
/// `factual.dropped_role`, the run lands `needsReview`, and the user can see the
/// document is short. It is NOT acceptable for a document with no employment
/// section at all — that is not a shorter résumé, and replacing a good previous
/// one with it is a loss nothing in the panel can describe.
///
/// Mutation check: return `true` unconditionally from `is_persistable` and the
/// stopped-and-empty case fails; drop the `stopped_early` guard and the
/// COMPLETED run with no experience section stops being persistable.
#[test]
fn a_stopped_run_may_only_overwrite_the_saved_resume_with_a_document_that_has_work_history() {
    use crate::pipeline::budget::StoppedReason;

    const WITH_WORK: &str = "Professional Summary\n\nA payments engineer.\n\n\
                             Work Experience\n\nStaff Engineer, Acme  2021 - Present\n\
                             - Owned the settlement service\n";
    const NO_WORK: &str = "Professional Summary\n\nA payments engineer.\n\nSkills\n\nGo, Rust\n";
    const EMPTY_SECTION: &str = "Professional Summary\n\nA payments engineer.\n\nWork Experience\n";

    // A run that REACHED THE END is never refused: a source résumé with no
    // employment section is a real input, and the report is where that is said.
    for stopped in [None, Some(StoppedReason::Done)] {
        assert!(super::is_persistable(NO_WORK, stopped));
        assert!(super::is_persistable(WITH_WORK, stopped));
    }

    // A run stopped EARLY keeps the trade only when it has work history.
    for stopped in [
        StoppedReason::RunTimeout,
        StoppedReason::Budgeted,
        StoppedReason::Cancelled,
    ] {
        assert!(
            super::is_persistable(WITH_WORK, Some(stopped)),
            "{stopped:?}: a short résumé beats discarding what the run paid for"
        );
        assert!(
            !super::is_persistable(NO_WORK, Some(stopped)),
            "{stopped:?}: a résumé with no work history must not replace a good one"
        );
        assert!(
            !super::is_persistable(EMPTY_SECTION, Some(stopped)),
            "{stopped:?}: a heading with nothing under it is not work history"
        );
    }
}

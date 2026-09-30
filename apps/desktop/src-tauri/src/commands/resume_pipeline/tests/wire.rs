use super::super::max;
use crate::ipc_contracts::events::PIPELINE_STAGE_PHASES;
use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;
use crate::pipeline::budget::Budget;
use crate::pipeline::resume::types::SectionKey;
use serde_json::json;
use std::time::Duration;

/// **Budgets are never renderer-supplied.** The same lock as
/// `agent_run_request_carries_only_identity_no_routing`, applied to the other
/// unbounded-spend knob: `maxSteps`/`maxTokens`/`runTimeout` bound how much ONE
/// run may spend on a paid API, and the anti-abuse limiter caps how OFTEN a run
/// starts, not that. The request struct has nowhere to bind them, so serde
/// silently drops a compromised renderer's attempt.
///
/// Mutation check: add a `max_steps` field to the generated struct (via the Zod
/// schema) and the `is_object` assertion below fails.
#[test]
fn run_request_carries_only_identity_no_budget_and_no_routing() {
    let req: ResumePipelineRunRequest = serde_json::from_value(json!({
        "resumeId": "res-1",
        "jobId": "job-9",
        // A compromised renderer's attempted spend + egress escalation.
        "maxSteps": 9_999,
        "maxTokens": 100_000_000,
        "runTimeout": 86_400,
        "provider": "openai-compatible",
        "model": "evil",
        "baseUrl": "http://attacker.example",
    }))
    .expect("deserializes from the identity-only wire shape, ignoring the extra keys");
    assert_eq!(req.resume_id, "res-1");
    assert_eq!(req.job_id, "job-9");
    // Default, not a renderer-chosen escalation.
    assert_eq!(req.target_language, "en");
    // PR-2: an existing caller that omits `includeCoverLetter` (every caller
    // this build ships) gets the no-op default — the `cover_letter` stage
    // finishes instantly at zero cost, byte-identical to a build that never
    // had the stage at all.
    assert!(!req.include_cover_letter);
    // …and the ONE default on this request that is `true`: a caller that omits
    // `includeResume` still gets a résumé. The safe direction — every caller
    // that predates the cover-letter-only run keeps generating one.
    assert!(req.include_resume);

    // WIRED, not hardcoded. Without this second deserialize a codegen bug that
    // emitted a constant `true` would pass the assertion above, and the whole
    // cover-letter-only run would silently generate a résumé again — the exact
    // defect this field exists to close.
    let cover_only: ResumePipelineRunRequest = serde_json::from_value(json!({
        "resumeId": "res-1",
        "jobId": "job-9",
        "includeResume": false,
        "includeCoverLetter": true,
    }))
    .expect("deserializes with the résumé turned off");
    assert!(!cover_only.include_resume);
    assert!(cover_only.include_cover_letter);

    // Same treatment for `researchCompany`: an existing caller that omits it
    // gets the no-op default — no research call, no `<company_research>`
    // block, byte-identical to a build that never had the toggle at all.
    assert!(!req.research_company);

    // Re-serializing must not resurrect any of them: the round-trip is exactly
    // the field set the backend owns.
    let round_tripped = serde_json::to_value(&req).expect("serializable");
    let object = round_tripped.as_object().expect("object");
    for forbidden in [
        "maxSteps",
        "maxTokens",
        "runTimeout",
        "provider",
        "model",
        "baseUrl",
    ] {
        assert!(
            !object.contains_key(forbidden),
            "{forbidden} must not exist on the run request"
        );
    }
}

/// The emitter's `phase` literals must be exactly the generated vocabulary the
/// renderer's `PipelineStagePhase` is derived from. Mutation check: emit
/// `"finished"` instead of `"finish"` and this fails.
#[test]
fn the_emitted_phase_vocabulary_matches_the_generated_contract() {
    let emitted = super::super::hooks::emitted_phases();
    assert_eq!(
        emitted.to_vec(),
        PIPELINE_STAGE_PHASES.to_vec(),
        "the emitter's phases and the frozen contract's must be the same closed set"
    );
}

/// **`"header"` is rejected at the command boundary**, and not by a hand-written
/// branch: the parse runs the generated grammar, which has no header token, so
/// the contact header the editor owns at export time (ADR-0021) is unreachable
/// from this command by construction.
///
/// The command itself needs an `AppHandle`, which this crate has no harness for,
/// so the assertion is on the exact parse the command performs FIRST — before
/// it touches any state. Mutation check: accept an unknown key by defaulting to
/// `Summary` and this fails.
#[test]
fn regenerate_section_rejects_header_before_touching_any_state() {
    for rejected in ["header", "Header", "HEADER", "contact", "name", ""] {
        assert!(
            SectionKey::from_wire(rejected).is_none(),
            "{rejected:?} must be rejected at the boundary"
        );
    }
    assert!(SectionKey::from_wire("summary").is_some());
}

/// The budget floor and the run's own kind string — both load-bearing: the
/// floor is what `run_deadline` falls back to, and `kind` is half the store's
/// retention partition, so changing either silently re-partitions someone's
/// history.
///
/// **`RUN_DEPTH` is pinned here too.** It is the ONE literal `execute` writes
/// into every new `RunRow.depth` and hands `persist_document` — there is no
/// depth CHOICE left to make, so this constant is the whole guard against a
/// stray edit quietly reviving `"fast"`/`"max"` (or a typo) on every run this
/// build creates. `RunRow.depth` itself stays a plain `String` column (never a
/// closed Rust enum) precisely so an existing row recorded at `"fast"`/`"max"`
/// before this depth's removal still reads back without erroring.
///
/// Mutation check (executed): change `RUN_DEPTH` to `"fast"` and this fails.
#[test]
fn the_run_kind_the_budget_floor_and_the_run_depth_are_pinned() {
    assert_eq!(super::super::RUN_KIND, "resume");
    assert_eq!(super::super::RUN_DEPTH, "quality");
    assert_eq!(
        Budget::RESUME_QUALITY.run_timeout,
        Duration::from_secs(4_800)
    );
}

/// `max::paying_stages` is the belt that keeps a stored override on a stage
/// which makes no AI call from ever being RESOLVED — and resolving propagates
/// its error, so one bad row on an inert stage would abort a run before it
/// started. The store refuses to write such a row (first belt); this is what
/// holds when the row got there another way, e.g. a hand-edited database or a
/// stage that stopped paying after the row was written.
///
/// Mutation check (executed): neuter the filter to `|_| true` and both loops
/// fail here.
#[test]
fn paying_stages_never_includes_a_stage_that_makes_no_ai_call() {
    use crate::ipc_contracts::events::PIPELINE_STAGES_FREE;

    // Pinned as a LITERAL list, in order. Deriving the expectation
    // (`stage_names()` minus `free_stage_names()`) would restate the
    // implementation and pass whatever it did; this fails if a paying stage is
    // ever silently DROPPED, which is the costly direction — an override the
    // user sets, Settings shows, and nothing ever resolves.
    let expected = [
        "analyze_job",
        "strategy",
        "draft",
        "cover_letter",
        "repair",
        "humanize",
    ];
    let paying = max::paying_stages();
    assert_eq!(
        paying, expected,
        "the pipeline lost or gained a paying stage"
    );
    for free in PIPELINE_STAGES_FREE {
        assert!(
            !paying.contains(free),
            "would resolve routing for the inert stage {free:?}"
        );
    }
}

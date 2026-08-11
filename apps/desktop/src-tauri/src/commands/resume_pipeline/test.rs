//! Tests for the shell layer's own decisions: the wire contract's shape, the
//! report wrapper the renderer reads, and the two run-stopping seams.
//!
//! Each guard names the mutation that makes it fail; each was applied and
//! reverted, not assumed.

use std::time::Duration;

use serde_json::json;

use super::hooks::apply_stop;
use super::report;
use crate::ipc_contracts::events::PIPELINE_STAGE_PHASES;
use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;
use crate::pipeline::budget::{Budget, StoppedReason};
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::resume::{RunLedger, QUALITY_STAGES};
use crate::pipeline::runs::{PipelineRunStore, RunEventRow, RunRow};
use crate::validate::content::{validate_content, ContentInput, ContentReport, DocKind};

// ── Wire-contract locks ─────────────────────────────────────────────────────

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
    // Defaults, not renderer-chosen escalations.
    assert_eq!(req.depth, "quality");
    assert_eq!(req.target_language, "en");

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
    let emitted = super::hooks::emitted_phases();
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

// ── The quality-report wrapper ──────────────────────────────────────────────

/// The staleness anchor has to be byte-identical to the renderer's `hashText`,
/// or every report the pipeline writes reads as stale the moment it is
/// reopened — a green badge turning into "this report is out of date" on text
/// nobody edited.
///
/// The expected values were produced by RUNNING the renderer's own
/// `hashText` (`apps/desktop/src/renderer/lib/generate/quality-report.ts`) —
/// not derived by hand from the algorithm, which is how two "implementations of
/// djb2" end up disagreeing about `ToInt32`.
///
/// Mutation check: hash over bytes instead of UTF-16 units and the em-dash case
/// fails; use `i64` instead of a wrapping `i32` and the 64-character case does.
#[test]
fn the_source_text_hash_matches_the_renderer_algorithm() {
    assert_eq!(report::hash_text(""), 5_381);
    assert_eq!(report::hash_text("a"), 177_604);
    assert_eq!(report::hash_text("abc"), 193_409_669);
    // A multi-byte character: ONE UTF-16 unit, THREE UTF-8 bytes. A byte-wise
    // hash gets a different answer here and nowhere else, which is exactly the
    // kind of drift that only shows up on a real résumé (an em dash, a curly
    // quote, an accented name).
    assert_eq!(report::hash_text("—"), 169_393);
    // Long enough to wrap 32 bits many times over, and past 2^31 — so a
    // non-wrapping or signed-final result differs.
    assert_eq!(report::hash_text(&"x".repeat(64)), 3_300_627_717);
}

fn report_for(generated: &str, source: &str) -> ContentReport {
    validate_content(&ContentInput {
        generated,
        source_resume: source,
        job_ad: "We need a payments engineer.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    })
}

const CLEAN_SOURCE: &str = "Jane Doe\n\nPROFESSIONAL SUMMARY\nA payments engineer.\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";
const FABRICATING_DRAFT: &str = "PROFESSIONAL SUMMARY\nA payments engineer who cut costs by 47% across 12 teams.\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n";

/// The wrapper is the renderer's v2 shape, with the pipeline's two documented
/// additions — the DEPTH as `pipeline`, and the fabrications INSIDE the
/// document's own slot (beside `sourceTextHash`, for the same merge reason).
///
/// Mutation check: hoist `fabrications` to the top level and the
/// slot-membership assertion fails — which is the bug it prevents: the store's
/// merge overlays whole top-level keys, so a letter-only save would orphan the
/// résumé's review list.
#[test]
fn the_wrapper_is_v2_shaped_and_keeps_fabrications_inside_the_slot() {
    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build(
        "quality",
        1_700_000_000,
        Some((&report, FABRICATING_DRAFT)),
        None,
    );
    let parsed: serde_json::Value = serde_json::from_str(&wrapper).expect("valid JSON");

    assert_eq!(parsed["schemaVersion"], json!(2));
    assert_eq!(parsed["pipeline"], json!("quality"));
    assert_eq!(parsed["generatedAt"], json!(1_700_000_000u64));
    assert_eq!(
        parsed["resume"]["sourceTextHash"],
        json!(report::hash_text(FABRICATING_DRAFT))
    );
    // A document this run did not validate contributes NO key — an empty one
    // would overlay (and erase) the stored letter's slot.
    assert!(parsed.get("coverLetter").is_none());
    let flagged = parsed["resume"]["fabrications"]
        .as_array()
        .expect("the fabricated metric must be listed for review");
    assert!(!flagged.is_empty());
    assert!(flagged[0]["issueKey"]
        .as_str()
        .is_some_and(|k| k.contains('#')));
    assert!(flagged[0]["evidence"]
        .as_str()
        .is_some_and(|e| !e.is_empty()));
    assert!(
        flagged[0].get("decision").is_none(),
        "undecided until the user says"
    );
}

/// A clean report carries NO `fabrications` key at all, rather than an empty
/// array — "is anything undecided?" must be one test, not one plus a length
/// check the renderer has to remember.
#[test]
fn a_clean_report_carries_no_review_list() {
    let report = report_for(CLEAN_SOURCE, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, CLEAN_SOURCE)), None);
    let parsed: serde_json::Value = serde_json::from_str(&wrapper).expect("valid JSON");
    assert!(parsed["resume"].get("fabrications").is_none());
    assert!(!report::has_unresolved(&wrapper));
}

/// **The run stays `needsReview` until every flagged bullet is decided.**
/// Nothing is removed silently, so an undecided finding must keep the run out
/// of "clean".
///
/// Mutation check: make `has_unresolved` return `false` unconditionally and the
/// first assertion fails; make `record_decision` a no-op and the last one does.
#[test]
fn a_run_stays_in_review_until_every_finding_is_decided() {
    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, FABRICATING_DRAFT)), None);
    assert!(
        report::has_unresolved(&wrapper),
        "a fresh finding is undecided"
    );

    let parsed: serde_json::Value = serde_json::from_str(&wrapper).unwrap();
    let keys: Vec<String> = parsed["resume"]["fabrications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["issueKey"].as_str().unwrap().to_string())
        .collect();
    assert!(!keys.is_empty());

    let mut current = wrapper;
    for (index, key) in keys.iter().enumerate() {
        // Still unresolved while ANY finding is undecided.
        if index > 0 {
            assert!(report::has_unresolved(&current));
        }
        current = report::record_decision(&current, key, "keep").expect("a known key resolves");
    }
    assert!(
        !report::has_unresolved(&current),
        "deciding every finding must clear the review state"
    );
    // The verdict is RECORDED, not applied — nothing was deleted from any text.
    let decided: serde_json::Value = serde_json::from_str(&current).unwrap();
    assert_eq!(
        decided["resume"]["fabrications"][0]["decision"],
        json!("keep")
    );
}

/// **A Critical the review cannot clear keeps the run in review.**
///
/// `factual.dropped_role` names an ABSENCE, so it is deliberately not in the
/// Remove/Keep panel — and a run that flipped to `completed` because every
/// *reviewable* finding was decided would be presenting a résumé that silently
/// lost an employer as clean. That is the worst outcome this whole review
/// mechanism exists to prevent, so it gets its own guard.
///
/// Mutation check: make `still_needs_review` delegate to `has_unresolved` alone
/// and the dropped-role case fails.
#[test]
fn an_unreviewable_critical_keeps_a_run_in_review_after_every_bullet_is_decided() {
    // A generated document that drops an employer AND fabricates a figure: the
    // first is unreviewable, the second is a Remove/Keep entry.
    let source = "Jane Doe\n\nPROFESSIONAL SUMMARY\nA payments engineer.\n\nWORK EXPERIENCE\n\nSenior Engineer | Acme Payments | 2021 - Present\n- Built the ledger service\n\nEngineer | Beta Systems | 2019 - 2021\n- Shipped the API\n";
    let generated = "PROFESSIONAL SUMMARY\nA payments engineer who cut costs by 47%.\n\nWORK EXPERIENCE\n\nSenior Engineer | Acme Payments | 2021 - Present\n- Built the ledger service\n";

    let report = report_for(generated, source);
    let codes: Vec<&str> = report.issues.iter().map(|i| i.code).collect();
    assert!(
        codes.contains(&crate::validate::content::FACTUAL_DROPPED_ROLE),
        "fixture must drop a role; got {codes:?}"
    );
    assert!(codes.contains(&crate::validate::content::FACTUAL_UNSOURCED_METRIC));

    let wrapper = report::build("quality", 1, Some((&report, generated)), None);
    let parsed: serde_json::Value = serde_json::from_str(&wrapper).unwrap();
    let keys: Vec<String> = parsed["resume"]["fabrications"]
        .as_array()
        .expect("the metric is reviewable")
        .iter()
        .map(|entry| entry["issueKey"].as_str().unwrap().to_string())
        .collect();
    // The dropped role is NOT in the panel — it has no span to decide about.
    assert!(
        !keys
            .iter()
            .any(|key| key.starts_with(crate::validate::content::FACTUAL_DROPPED_ROLE)),
        "a dropped role has no Remove/Keep answer and must not be listed; got {keys:?}"
    );

    let mut current = wrapper;
    for key in &keys {
        current = report::record_decision(&current, key, "remove").expect("known key");
    }
    assert!(
        !report::has_unresolved(&current),
        "every reviewable finding is decided"
    );
    assert!(
        report::still_needs_review(&current),
        "…but the dropped role still blocks: the run must not read as clean"
    );
}

/// An unknown key, a report that no longer carries the finding, and an
/// unparseable blob are all no-ops — never an error, and never a decision
/// silently applied to the wrong finding.
#[test]
fn an_unmatched_decision_is_a_no_op_rather_than_a_mis_applied_one() {
    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, FABRICATING_DRAFT)), None);
    assert!(report::record_decision(&wrapper, "factual.unsourced_metric#999", "remove").is_none());
    assert!(report::record_decision("not json", "anything", "keep").is_none());
    assert!(report::record_decision("{}", "anything", "keep").is_none());
}

// ── The two run-stopping seams ──────────────────────────────────────────────

/// `StageHooks::before` is the cancellation seam: a cancelled run stops at the
/// NEXT stage boundary, before paying for another provider call, and records
/// `Cancelled` so the command marks the job cancelled rather than failed.
///
/// Asserted on [`apply_stop`], which IS that decision — the emit half of
/// `before` needs an `AppHandle` this crate has no harness for, and a decision
/// only provable by reading the code is not a guard (the same seam shape as
/// `Completer::from_config`).
///
/// Mutation check: drop the `cancelled` branch and both assertions fail; drop
/// the `ledger.stop(...)` and the reason assertion does.
#[test]
fn a_cancelled_run_stops_at_the_next_stage_boundary() {
    let ledger = RunLedger::new();
    let outcome = apply_stop(
        &ledger,
        true,
        Duration::from_secs(1),
        Duration::from_secs(600),
    );
    assert!(outcome.is_err(), "a cancelled run must not enter the stage");
    assert_eq!(ledger.stopped(), Some(StoppedReason::Cancelled));
}

/// The same seam is where the RUN DEADLINE becomes real — the only place
/// `StoppedReason::RunTimeout` is reachable. Mutation check: remove the
/// deadline branch and both assertions fail.
#[test]
fn a_run_past_its_deadline_stops_with_run_timeout() {
    let ledger = RunLedger::new();
    let outcome = apply_stop(
        &ledger,
        false,
        Duration::from_secs(2_701),
        Duration::from_secs(2_700),
    );
    assert!(outcome.is_err());
    assert_eq!(ledger.stopped(), Some(StoppedReason::RunTimeout));
    // …and a run still inside its deadline proceeds — otherwise the test above
    // would pass against a hook that stops every run.
    let ok = RunLedger::new();
    assert!(apply_stop(
        &ok,
        false,
        Duration::from_secs(1),
        Duration::from_secs(2_700)
    )
    .is_ok());
    assert_eq!(ok.stopped(), None);
}

/// Cancellation wins over the deadline when both hold: the user asked for a
/// cancel, and "it timed out" is a worse answer to the same event (and maps to
/// a different terminal job state).
#[test]
fn cancellation_outranks_the_deadline() {
    let ledger = RunLedger::new();
    let _ = apply_stop(&ledger, true, Duration::from_secs(9_999), Duration::ZERO);
    assert_eq!(ledger.stopped(), Some(StoppedReason::Cancelled));
}

/// The budget floor and the run's own kind string — both load-bearing: the
/// floor is what `run_deadline` falls back to, and `kind` is half the store's
/// retention partition, so changing either silently re-partitions someone's
/// history.
#[test]
fn the_run_kind_and_the_budget_floor_are_pinned() {
    assert_eq!(super::RUN_KIND, "resume");
    assert_eq!(
        Budget::RESUME_QUALITY.run_timeout,
        Duration::from_secs(45 * 60)
    );
}

// ── Run-store round-trip, with the shapes this emitter actually produces ────

/// A whole run through the store, written the way `execute` + `RunHooks` write
/// one: the `running` row, six stages' `start`/`finish` pairs carrying real
/// ledger artifacts, then the terminal row.
///
/// Worth a test rather than trusting the store's own suite because THIS is
/// where the two sides meet: the store's `phase` CHECK is a schema constraint,
/// and an emitter that wrote `"finished"` would fail every insert at runtime
/// with nothing but a `warn!` to show for it. It also pins that the emitter's
/// `kind` is what `listForJob` filters on.
///
/// Mutation check: emit a phase outside the CHECK's vocabulary and the event
/// count drops to zero; change `RUN_KIND` on one side only and the filtered
/// list is empty.
#[test]
fn a_run_round_trips_through_the_store_with_real_stage_events() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = PipelineRunStore::open(dir.path()).expect("store opens");

    let ledger = RunLedger::new();
    ledger.record("analyze_job", json!({ "cached": false, "mustHave": 4 }));
    ledger.record("validate", json!({ "issues": 3, "criticals": 1 }));
    ledger.count_call(false);
    ledger.count_call(true);
    ledger.note_repair(1, false);

    let mut row = RunRow {
        id: "run-1".to_string(),
        job_url: "https://boards.example/jobs/42".to_string(),
        kind: super::RUN_KIND.to_string(),
        depth: "quality".to_string(),
        status: "running".to_string(),
        started_at: 1_700_000_000_000,
        finished_at: None,
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    };
    store
        .upsert_run(&row)
        .expect("the running row is written first");

    let mut seq = 0u32;
    for (index, stage) in QUALITY_STAGES.iter().enumerate() {
        for phase in ["start", "finish"] {
            store
                .append_event(&RunEventRow {
                    run_id: row.id.clone(),
                    seq,
                    ts: 1_700_000_000_000 + u64::from(seq),
                    stage: (*stage).to_string(),
                    phase: phase.to_string(),
                    artifact_json: ledger
                        .artifact(stage)
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "{}".to_string()),
                })
                .unwrap_or_else(|e| panic!("stage {index} phase {phase} must persist: {e}"));
            seq += 1;
        }
    }

    row.status = "needsReview".to_string();
    row.finished_at = Some(1_700_000_100_000);
    row.stopped_reason = Some("max_repairs".to_string());
    row.metrics_json = ledger.metrics().to_string();
    store
        .upsert_run(&row)
        .expect("the terminal row replaces it");

    let read = store.run("run-1").expect("the run is readable");
    assert_eq!(read.status, "needsReview");
    assert_eq!(read.stopped_reason.as_deref(), Some("max_repairs"));
    let metrics: serde_json::Value = serde_json::from_str(&read.metrics_json).expect("metrics");
    assert_eq!(metrics["calls"], json!(1));
    assert_eq!(metrics["cached"], json!(1));
    assert_eq!(metrics["repairRounds"], json!(1));

    let events = store.events_for_run("run-1");
    assert_eq!(
        events.len(),
        QUALITY_STAGES.len() * 2,
        "every stage's start/finish pair must survive the schema's phase CHECK"
    );
    assert!(
        events.windows(2).all(|pair| pair[0].seq < pair[1].seq),
        "seq order"
    );
    let validate = events
        .iter()
        .find(|event| event.stage == "validate" && event.phase == "finish")
        .expect("the validate stage's finish event");
    let artifact: serde_json::Value =
        serde_json::from_str(&validate.artifact_json).expect("the artifact is JSON");
    assert_eq!(artifact["criticals"], json!(1));

    // `listForJob` filters on this flow's kind; a run of another kind against
    // the same posting must not appear in the résumé runs list.
    store
        .upsert_run(&RunRow {
            id: "run-agent".to_string(),
            kind: "agent".to_string(),
            ..row.clone()
        })
        .expect("an agent run shares the tables");
    let resume_runs: Vec<_> = store
        .runs_for_job(&row.job_url)
        .into_iter()
        .filter(|candidate| candidate.kind == super::RUN_KIND)
        .collect();
    assert_eq!(resume_runs.len(), 1);
    assert_eq!(resume_runs[0].id, "run-1");
}

use super::super::types::JobAnalysis;
use super::super::{effective_letter_text, resolved_top_requirements, RunLedger};
use crate::pipeline::budget::StoppedReason;
use crate::validate::content::{validate_content, ContentInput, DocKind};
use serde_json::json;

/// The root-cause fix: the run's requirement list comes from `analyze_job`'s
/// own extraction — must-haves first, then nice-to-haves, deduped
/// case-insensitively — even when the request sent an empty list (today's
/// renderer always does).
///
/// Mutation check: return `fallback` unconditionally and this fails.
#[test]
fn resolved_top_requirements_prefers_the_analysis_over_an_empty_request_list() {
    let analysis = JobAnalysis {
        must_have: vec![
            "Kubernetes".to_string(),
            "kubernetes".to_string(), // case-insensitive duplicate
            "payments domain".to_string(),
        ],
        nice_to_have: vec!["GraphQL".to_string()],
        ..JobAnalysis::default()
    };
    let resolved = resolved_top_requirements(&analysis, &[]);
    assert_eq!(
        resolved,
        vec![
            "Kubernetes".to_string(),
            "payments domain".to_string(),
            "GraphQL".to_string(),
        ]
    );
}

/// A run whose analysis produced nothing (not yet run, or genuinely empty)
/// falls back to the request's own list rather than losing it.
#[test]
fn resolved_top_requirements_falls_back_when_the_analysis_produced_nothing() {
    let fallback = vec!["fallback requirement".to_string()];
    let resolved = resolved_top_requirements(&JobAnalysis::default(), &fallback);
    assert_eq!(resolved, fallback);
}

/// FIRST writer wins: a run cancelled at stage 2 must not be relabelled by a
/// later stage's own stop. Mutation check: drop the `is_none()` guard in
/// `RunLedger::stop` and this fails.
#[test]
fn the_ledger_keeps_the_earliest_stop_reason() {
    let ledger = RunLedger::new();
    assert_eq!(ledger.stopped(), None);
    ledger.stop(StoppedReason::Cancelled);
    ledger.stop(StoppedReason::MaxRepairs);
    assert_eq!(ledger.stopped(), Some(StoppedReason::Cancelled));
}

/// Cached stages must not be counted as provider calls — the metric is what a
/// user reads as "what did this run cost me".
#[test]
fn the_ledger_separates_live_calls_from_cache_hits() {
    let ledger = RunLedger::new();
    ledger.count_call(false);
    ledger.count_call(false);
    ledger.count_call(true);
    ledger.note_repair(1, true);
    let metrics = ledger.metrics();
    assert_eq!(metrics["calls"], json!(2));
    assert_eq!(metrics["cached"], json!(1));
    assert_eq!(metrics["repairRounds"], json!(1));
    assert_eq!(metrics["reverted"], json!(true));
}

/// Stage artifacts are content-free (ADR-027): the hook copies them straight
/// onto the wire and into the event trail, so a stage that recorded a quote
/// would leak résumé text into a channel that claims to carry none.
///
/// **Recurses.** The top-level walk accepted `value.is_object()` wholesale, so
/// the one artifact that actually nests — `validate`'s `codes` histogram — was
/// waved through unexamined, and a stage that hid a quote one level down (the
/// obvious place to put a "which text failed" map) passed. Every LEAF must be a
/// number, a boolean or null; object KEYS are exempt because the only ones here
/// are the fixed `CONTENT_ISSUE_CODES` vocabulary.
///
/// Mutation check: record `json!({ "codes": { "factual.unsourced_metric":
/// "cut costs by 47%" } })` and this fails; it passed before the recursion.
#[test]
fn recorded_stage_artifacts_are_content_free() {
    fn assert_leaves_are_content_free(path: &str, value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, nested) in map {
                    assert_leaves_are_content_free(&format!("{path}.{key}"), nested);
                }
            }
            serde_json::Value::Array(items) => {
                for (index, nested) in items.iter().enumerate() {
                    assert_leaves_are_content_free(&format!("{path}[{index}]"), nested);
                }
            }
            other => assert!(
                other.is_number() || other.is_boolean() || other.is_null(),
                "artifact field {path} must be a count/flag, not text; got {other}"
            ),
        }
    }

    // A REAL validate artifact, nested histogram included — a hand-written flat
    // object would only pin the shape the test itself invented.
    let report = validate_content(&ContentInput {
        generated: "PROFESSIONAL SUMMARY\nA payments engineer who cut costs by 47%.\n",
        source_resume: "Jane Doe\n\nPROFESSIONAL SUMMARY\nA payments engineer.\n",
        job_ad: "We need a payments engineer.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::Resume,
    });
    let histogram = super::super::stages::code_histogram(&report);
    assert!(
        histogram.as_object().is_some_and(|codes| !codes.is_empty()),
        "the fixture must produce a NESTED histogram, or the recursion is untested"
    );

    let ledger = RunLedger::new();
    ledger.record(
        "validate",
        json!({ "issues": report.issues.len(), "criticals": 1, "codes": histogram }),
    );
    ledger.record(
        "repair",
        json!({ "rounds": 1, "reverted": false, "timedOut": false, "criticalsRemaining": 0 }),
    );
    for stage in ["validate", "repair"] {
        let artifact = ledger.artifact(stage).expect("recorded");
        assert_leaves_are_content_free(stage, &artifact);
    }
}

/// **The one rule that keeps `validate`/`repair`/`persist`/the terminal-status
/// checks from disagreeing about "the letter".** A free function so this is a
/// test on two `&str`s rather than a claim about a `QualityCtx` this crate
/// cannot build without a live `Completer` — see `effective_letter_text`'s own
/// doc.
///
/// Mutation check: swap the branches (prefer the request text) and the first
/// assertion fails.
#[test]
fn effective_letter_text_prefers_the_stage_letter_and_falls_back_to_the_request_text() {
    assert_eq!(
        effective_letter_text(
            "Dear hiring manager, I am writing to apply...",
            "legacy text"
        ),
        "Dear hiring manager, I am writing to apply..."
    );
    assert_eq!(effective_letter_text("", "legacy text"), "legacy text");
    assert_eq!(
        effective_letter_text("   ", "legacy text"),
        "legacy text",
        "whitespace-only counts as empty — the stage never writes a whitespace-only letter, but a \
         trimmed-empty guard is exactly what makes the fallback trustworthy"
    );
    assert_eq!(effective_letter_text("", ""), "");
}

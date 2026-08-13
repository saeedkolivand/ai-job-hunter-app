//! Tests for the shell layer's own decisions: the wire contract's shape, the
//! report wrapper the renderer reads, and the two run-stopping seams.
//!
//! Each guard names the mutation that makes it fail; each was applied and
//! reverted, not assumed.

use std::time::Duration;

use serde_json::json;

use super::hooks::{apply_stop, with_detail};
use super::max;
use super::notify::run_notification;
use super::report;
use crate::ai_generations::{AiGenerationRecord, AiGenerationStore};
use crate::ipc_contracts::events::PIPELINE_STAGE_PHASES;
use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;
use crate::pipeline::budget::{Budget, StoppedReason};
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::resume::{RunLedger, QUALITY_STAGES};
use crate::pipeline::runs::{PipelineRunStore, RunEventRow, RunRow};
use crate::pipeline::StageInfo;
use crate::validate::content::{
    validate_content, ContentInput, ContentIssue, ContentMetrics, ContentReport, DocKind,
};

/// A [`StageInfo`] whose only interesting field is whether the stage costs a
/// provider call — the shape `Pipeline::run_hooked` builds and hands to
/// `before`. Written as a helper rather than a bare bool at each call site
/// because `apply_stop` deliberately takes the whole struct (see its doc).
fn stage_info(costs_a_call: bool) -> StageInfo {
    StageInfo {
        pipeline: "resume_max",
        stage: if costs_a_call { "repair" } else { "validate" },
        index: 0,
        total: 1,
        costs_a_call,
    }
}

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

/// One synthetic reviewable finding, so the anchor cases below can state the
/// exact span they are about instead of hoping the validator emits it.
fn fabrication_report(evidence: &str) -> ContentReport {
    ContentReport {
        ok: false,
        issues: vec![ContentIssue {
            severity: crate::validate::Severity::Critical,
            code: crate::validate::content::FACTUAL_UNSOURCED_METRIC,
            section: None,
            message: "the draft states a number the source résumé does not".to_string(),
            evidence: Some(evidence.to_string()),
        }],
        metrics: ContentMetrics::default(),
    }
}

fn only_fabrication(wrapper: &str) -> serde_json::Value {
    let parsed: serde_json::Value = serde_json::from_str(wrapper).expect("valid JSON");
    let entries = parsed["resume"]["fabrications"]
        .as_array()
        .expect("one reviewable finding")
        .clone();
    assert_eq!(entries.len(), 1, "fixture declares exactly one finding");
    entries.into_iter().next().expect("the entry")
}

/// **Every entry carries the whole LINE its evidence sits on** — the anchor a
/// "Remove" is applied against.
///
/// `evidence` is NOT that anchor and never was: the validator's span is
/// routinely a bare token (`"47%"`, one keyword), so a renderer deleting "every
/// line containing the evidence" deletes whatever else quotes it — which is how
/// a Remove came to erase the contact header. The line is located HERE, at
/// report-build time, because this is the last layer holding the exact text the
/// report was produced over.
///
/// This is also the SERIALIZED-SHAPE pin the renderer's contract rests on:
/// dropping the field (mutation: delete the `entry.insert("line", …)` branch)
/// fails the `line` lookup below, applied and reverted.
#[test]
fn every_fabrication_entry_anchors_on_the_document_line_it_was_found_on() {
    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, FABRICATING_DRAFT)), None);
    let parsed: serde_json::Value = serde_json::from_str(&wrapper).expect("valid JSON");
    let flagged = parsed["resume"]["fabrications"]
        .as_array()
        .expect("the fabricated metric is listed for review");
    assert!(!flagged.is_empty());

    for entry in flagged {
        let evidence = entry["evidence"].as_str().expect("a span");
        let line = entry["line"]
            .as_str()
            .unwrap_or_else(|| panic!("{evidence:?} was quoted from the draft, so it HAS a line"));
        assert!(
            line.contains(evidence.trim()),
            "the anchor must contain the span it was located from: {line:?} vs {evidence:?}"
        );
        // Trimmed, and a REAL line of the document — the renderer matches a
        // whole trimmed line, so a fragment or an untrimmed copy anchors on
        // nothing.
        assert_eq!(line, line.trim());
        assert!(
            FABRICATING_DRAFT.lines().any(|l| l.trim() == line),
            "{line:?} is not a line of the validated document"
        );
    }
    assert!(
        flagged.iter().any(|entry| entry["line"]
            == json!("A payments engineer who cut costs by 47% across 12 teams.")),
        "the fabricated metric's own bullet is the anchor: {flagged:?}"
    );
}

/// **A span occurring on several lines carries NO anchor at all.**
///
/// Two of the reviewable codes routinely emit non-unique spans — a
/// `factual.unsupported_date` is a bare year, a `factual.unsourced_metric` a
/// bare figure — and an anchor picked by document order names whichever line
/// happens to come FIRST: a year sitting in an education line would anchor the
/// verdict for a flagged job entry, and the Remove would delete the education
/// line. Determinism is not correctness; refusing is, and the renderer's
/// `removeEvidenceLines` refuses safely on a missing anchor.
///
/// Mutation check: anchor on the first occurrence (the old behaviour) and the
/// `is_none` assertions fail with the education line as the anchor.
#[test]
fn a_span_on_several_lines_carries_no_anchor() {
    // The review's own scenario: the flagged year also sits in an education
    // line that comes FIRST in document order.
    let text = "EDUCATION\nB.Sc. Computer Science, 2019\n\nWORK EXPERIENCE\nEngineer | Acme | 2019 - Present\n";
    let report = fabrication_report("2019");
    let wrapper = report::build("quality", 1, Some((&report, text)), None);
    let entry = only_fabrication(&wrapper);
    assert!(
        entry.get("line").is_none(),
        "a non-unique span has no honest anchor: {entry}"
    );
    // Still decidable — the entry survives, only the automatic apply is off.
    let key = entry["issueKey"].as_str().expect("a key").to_string();
    assert!(report::record_decision(&wrapper, &key, "remove").is_some());

    // Re-issued from the same report + text: byte-identical — uniqueness is a
    // property of the text, so the refusal is as deterministic as the anchor.
    assert_eq!(
        report::build("quality", 1, Some((&report, text)), None),
        wrapper
    );

    // A bare-figure metric, same shape: "250" in the flagged bullet AND in a
    // second line. Neither line may be guessed at.
    let text = "PROFESSIONAL SUMMARY\nCut costs by 250 hours a month.\n\nWORK EXPERIENCE\n- Saved 250 hours again.\n";
    let wrapper = report::build("quality", 1, Some((&fabrication_report("250"), text)), None);
    assert!(only_fabrication(&wrapper).get("line").is_none());

    // …and the SAME span twice on ONE line still anchors: the line is unique,
    // which is the property the removal needs.
    let text = "PROFESSIONAL SUMMARY\nCut 250 costs by 250 hours.\n";
    let wrapper = report::build("quality", 1, Some((&fabrication_report("250"), text)), None);
    assert_eq!(
        only_fabrication(&wrapper)["line"],
        json!("Cut 250 costs by 250 hours.")
    );
}

/// **No honest anchor → NO key**, rather than a guessed one.
///
/// Two cases, and both are real: a span the document no longer contains (the
/// entry was re-issued over text the user already edited), and a line so long it
/// is not a bullet anyone reviews. The entry itself survives in both — it still
/// has to be decidable, or the run is stranded at `needsReview` forever — and
/// the renderer is told "cannot apply automatically" by the field's absence.
///
/// Mutation check: fall back to `text` (or to the evidence) instead of `None`
/// and the `is_none` assertions fail.
#[test]
fn an_entry_with_no_locatable_line_omits_the_anchor_but_stays_decidable() {
    // (1) The span is not in the document at all.
    let wrapper = report::build(
        "quality",
        1,
        Some((
            &fabrication_report("47%"),
            "PROFESSIONAL SUMMARY\nA payments engineer.\n",
        )),
        None,
    );
    let entry = only_fabrication(&wrapper);
    assert!(
        entry.get("line").is_none(),
        "an unlocatable span must carry no anchor: {entry}"
    );
    let key = entry["issueKey"].as_str().expect("a key");
    assert!(
        report::record_decision(&wrapper, key, "remove").is_some(),
        "the finding is still decidable — otherwise the run never leaves review"
    );

    // (2) The containing line is past the cap.
    let long_line = format!(
        "{} 47% {}",
        "x".repeat(report::MAX_LINE_CHARS),
        "y".repeat(50)
    );
    let text = format!("PROFESSIONAL SUMMARY\n{long_line}\n");
    let over_cap = report::build(
        "quality",
        1,
        Some((&fabrication_report("47%"), &text)),
        None,
    );
    assert!(
        only_fabrication(&over_cap).get("line").is_none(),
        "a line past MAX_LINE_CHARS is a paste artifact, not a reviewable bullet"
    );

    // …and one character under the cap still anchors, so the guard is a cap and
    // not an off-switch.
    let at_cap = "z".repeat(report::MAX_LINE_CHARS - 4) + " 47%";
    let text = format!("PROFESSIONAL SUMMARY\n{at_cap}\n");
    let within = report::build(
        "quality",
        1,
        Some((&fabrication_report("47%"), &text)),
        None,
    );
    assert_eq!(only_fabrication(&within)["line"], json!(at_cap));
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
    assert!(!report::has_unresolved(&wrapper, CLEAN_SOURCE, ""));
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
        report::has_unresolved(&wrapper, FABRICATING_DRAFT, ""),
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
            assert!(report::has_unresolved(&current, FABRICATING_DRAFT, ""));
        }
        current = report::record_decision(&current, key, "keep").expect("a known key resolves");
    }
    assert!(
        !report::has_unresolved(&current, FABRICATING_DRAFT, ""),
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
        // "keep" rather than "remove": a Keep settles on the verdict alone,
        // which isolates what this test is about — the dropped role blocking
        // AFTER every reviewable finding is genuinely settled. (An unapplied
        // Remove would now block on its own; that rule has its own test.)
        current = report::record_decision(&current, key, "keep").expect("known key");
    }
    assert!(
        !report::has_unresolved(&current, generated, ""),
        "every reviewable finding is decided"
    );
    assert!(
        report::still_needs_review(&current, generated, ""),
        "…but the dropped role still blocks: the run must not read as clean"
    );
}

/// **A Remove is intent, the document is fact, and the run finishes only when
/// they agree** — the renderer's `isFabricationResolved` rule, applied on the
/// Rust side too, because the run row is what drives the panel's headline.
///
/// Without the document half of the rule, `resolveFabrication` flipped a run
/// to `completed` on the last recorded Remove while the review panel — which
/// checks the document — still showed the same entry as "removal pending": the
/// two sides of one run disagreeing about whether it was finished.
///
/// Mutation check: count only decision-absence in `unresolved_count` (the old
/// rule) and the first pair of assertions fails.
#[test]
fn a_recorded_but_unapplied_remove_keeps_the_run_unfinished() {
    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, FABRICATING_DRAFT)), None);
    let keys = fabrication_keys(&wrapper);
    let mut current = wrapper;
    for key in &keys {
        current = report::record_decision(&current, key, "remove").expect("known key");
    }

    // Every entry carries a verdict — and against the UNEDITED text the run is
    // still unfinished, because every flagged span is still in the document.
    assert_eq!(
        report::unresolved_count(&current, FABRICATING_DRAFT, ""),
        keys.len(),
        "an unapplied Remove must keep counting"
    );
    assert!(report::still_needs_review(&current, FABRICATING_DRAFT, ""));

    // The span leaving the document is what settles it.
    let edited = FABRICATING_DRAFT.replace(
        "A payments engineer who cut costs by 47% across 12 teams.\n",
        "",
    );
    assert_eq!(report::unresolved_count(&current, &edited, ""), 0);
    assert!(!report::still_needs_review(&current, &edited, ""));
}

/// **Only the INVENTED-link arm of `factual.altered_project_link` is
/// reviewable.** The validator's other arm — a SOURCE link missing or altered
/// in the output — names an ABSENCE, exactly like `factual.dropped_role`: its
/// evidence is the source URL, which by definition is not in the generated
/// document, so a Remove/Keep row would ask a question with no correct answer
/// (and the panel would render it as "you may have edited this away", which is
/// not what happened). It stays a Critical the review cannot clear, which is
/// what keeps the run at `needsReview`.
///
/// Mutation check: drop the presence gate in `fabrications` and
/// `only_fabrication` fails on two entries; gate EVERY code on presence
/// instead and `an_entry_with_no_locatable_line_omits_the_anchor_but_stays_decidable`
/// fails (an orphaned metric entry must stay decidable).
#[test]
fn an_absent_source_link_blocks_the_run_without_entering_the_review_panel() {
    let generated = "PROJECTS\n**Tool** · https://example.test/invented\n";
    let report = ContentReport {
        ok: false,
        issues: vec![
            // Arm 1: the source URL the output lost — absent from `generated`.
            ContentIssue {
                severity: crate::validate::Severity::Critical,
                code: crate::validate::content::FACTUAL_ALTERED_PROJECT_LINK,
                section: Some("Projects".to_string()),
                message: "the project link from your source résumé is missing or altered"
                    .to_string(),
                evidence: Some("https://example.test/original".to_string()),
            },
            // Arm 2: a link the model invented — present in `generated`.
            ContentIssue {
                severity: crate::validate::Severity::Critical,
                code: crate::validate::content::FACTUAL_ALTERED_PROJECT_LINK,
                section: Some("Projects".to_string()),
                message: "links to a URL that is not in your source résumé".to_string(),
                evidence: Some("https://example.test/invented".to_string()),
            },
        ],
        metrics: ContentMetrics::default(),
    };
    let wrapper = report::build("quality", 1, Some((&report, generated)), None);
    let entry = only_fabrication(&wrapper);
    assert_eq!(
        entry["evidence"],
        json!("https://example.test/invented"),
        "only the invented link is a decidable span: {entry}"
    );

    // Deciding the reviewable entry does NOT finish the run: the absence arm
    // still blocks, as the Critical the review cannot clear.
    let key = entry["issueKey"].as_str().expect("a key").to_string();
    let decided = report::record_decision(&wrapper, &key, "keep").expect("a known key");
    assert!(!report::has_unresolved(&decided, generated, ""));
    assert!(
        report::still_needs_review(&decided, generated, ""),
        "the absent source link must keep the run in review"
    );
}

/// **A wrapper write moves a run row only between the two review-terminal
/// states** — `recomputed_status`, the decision both wrapper writers share.
///
/// The regenerate direction is the one that was missing: a regenerated section
/// can introduce a fresh fabrication on a `completed` run, and a row left at
/// `completed` makes the panel read "done" while suppressing the review block
/// entirely. The resolve direction (needsReview → completed) already existed;
/// one helper keeps the two from diverging.
///
/// Mutation check: guard on `STATUS_NEEDS_REVIEW` alone (the old resolve-only
/// shape) and the completed→needsReview case fails; drop the terminal-state
/// guard and the `failed` cases do.
#[test]
fn a_wrapper_write_moves_a_run_row_only_between_the_review_terminal_states() {
    use super::recomputed_status;
    // A fresh finding un-cleans a completed run…
    assert_eq!(
        recomputed_status(super::STATUS_COMPLETED, true),
        Some(super::STATUS_NEEDS_REVIEW)
    );
    // …and the last agreeing verdict finishes a needsReview one.
    assert_eq!(
        recomputed_status(super::STATUS_NEEDS_REVIEW, false),
        Some(super::STATUS_COMPLETED)
    );
    // Already right: nothing to write.
    assert_eq!(recomputed_status(super::STATUS_COMPLETED, false), None);
    assert_eq!(recomputed_status(super::STATUS_NEEDS_REVIEW, true), None);
    // How the run ENDED is not the report's to rewrite — and `running` is not
    // terminal.
    assert_eq!(recomputed_status(super::STATUS_FAILED, true), None);
    assert_eq!(recomputed_status(super::STATUS_FAILED, false), None);
    assert_eq!(recomputed_status(super::STATUS_CANCELLED, false), None);
    assert_eq!(recomputed_status("running", true), None);

    // The call sites, grep-shaped (the commands need a Tauri harness this
    // crate does not have): BOTH wrapper writers recompute the row.
    assert!(
        include_str!("mod.rs")
            .matches("recomputed_status(&row.status")
            .count()
            >= 2,
        "regenerate_section and resolve_fabrication must both recompute the run row"
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
        &stage_info(true),
    );
    assert!(outcome.is_err(), "a cancelled run must not enter the stage");
    assert_eq!(ledger.stopped(), Some(StoppedReason::Cancelled));

    // A cancel stops a FREE stage too: the deadline's exemption is about not
    // throwing away paid work, and a user who pressed Cancel asked for the run
    // to stop rendering as well as to stop spending.
    let ledger = RunLedger::new();
    assert!(apply_stop(
        &ledger,
        true,
        Duration::from_secs(1),
        Duration::from_secs(600),
        &stage_info(false)
    )
    .is_err());
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
        &stage_info(true),
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
        Duration::from_secs(2_700),
        &stage_info(true)
    )
    .is_ok());
    assert_eq!(ok.stopped(), None);
}

/// **An expired deadline stops the next PAID stage, not the next stage.**
///
/// The boundary check's whole justification is that a stage boundary is where
/// stopping is free — nothing is in flight, and the next provider call has not
/// been paid for. Applied to a stage that makes NO call, that reasoning inverts
/// into its opposite: a max run whose clock ran out mid-fan-out had `assemble`
/// (pure) and `validate` (deterministic) refused, so eleven paid section
/// answers became an empty draft, no report, nothing persisted and
/// `status=failed`.
///
/// The run is still STOPPED — the reason is recorded on this path too, which is
/// what makes `terminal_state` resolve it to `needsReview`/`completed` +
/// `run_timeout` rather than to a clean finish.
///
/// Mutation check: return the error unconditionally (drop the `costs_a_call`
/// guard) and the free-stage assertion fails; skip `ledger.stop` on the free
/// path and the reason assertion does.
#[test]
fn a_zero_call_stage_still_runs_after_the_deadline_and_the_run_still_says_so() {
    let ledger = RunLedger::new();
    let outcome = apply_stop(
        &ledger,
        false,
        Duration::from_secs(2_701),
        Duration::from_secs(2_700),
        &stage_info(false),
    );
    assert!(
        outcome.is_ok(),
        "a stage that costs nothing must be allowed to turn paid work into a document"
    );
    assert_eq!(
        ledger.stopped(),
        Some(StoppedReason::RunTimeout),
        "the run is still deadline-stopped — running the free stages does not hide that"
    );
}

/// Cancellation wins over the deadline when both hold: the user asked for a
/// cancel, and "it timed out" is a worse answer to the same event (and maps to
/// a different terminal job state).
#[test]
fn cancellation_outranks_the_deadline() {
    let ledger = RunLedger::new();
    let _ = apply_stop(
        &ledger,
        true,
        Duration::from_secs(9_999),
        Duration::ZERO,
        &stage_info(true),
    );
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
        Duration::from_secs(75 * 60)
    );
}

// ── The terminal state ──────────────────────────────────────────────────────

/// **A cancelled run reports `cancelled`, never `failed` + `"done"`.**
///
/// The live path this reproduces: the user cancels during the DRAFT stage.
/// `chat_stream` aborts and returns `AppError::Message("Job cancelled")` — not
/// `AppError::Cancelled` — the draft stage propagates it, and because `repair`
/// never starts, no later `before()` runs `apply_stop`, so the ledger records
/// NOTHING. The old derivation then read `stopped == None` as
/// `StoppedReason::Done` and `cancelled == false`, producing
/// `status=failed stoppedReason="done"` for a run the user cancelled — and the
/// renderer maps `done` to a success suffix.
///
/// Mutation check: derive `cancelled` from the ledger alone (drop the
/// `token_cancelled` argument's effect) and the status assertion fails; restore
/// `stopped.unwrap_or(Done)` and the reason assertion does.
#[test]
fn a_cancel_the_ledger_never_saw_still_reports_cancelled() {
    // Exactly the ledger a cancelled draft leaves behind: empty.
    let ledger = RunLedger::new();
    assert_eq!(
        ledger.stopped(),
        None,
        "the premise: the ledger saw nothing"
    );

    let (status, reason) = super::hooks::terminal_state(&ledger, false, true, false, false);
    assert_eq!(status, "cancelled");
    assert_eq!(reason.as_deref(), Some("cancelled"));
    assert_eq!(
        ledger.stopped(),
        Some(StoppedReason::Cancelled),
        "…and the ledger is corrected, so the metrics agree with the row"
    );
}

/// A run that FAILED for any other reason carries no stopped reason at all
/// rather than the `done` that used to be the fallback. `Done` means "the last
/// stage completed"; a failure is the one thing it must never label.
///
/// Mutation check: restore `stopped.unwrap_or(StoppedReason::Done)` and the
/// `None` assertion fails.
#[test]
fn a_failed_run_never_reports_done() {
    let ledger = RunLedger::new();
    let (status, reason) = super::hooks::terminal_state(&ledger, false, false, false, false);
    assert_eq!(status, "failed");
    assert_eq!(
        reason, None,
        "a run that failed with no recorded stop reason reports none — never \"done\""
    );
}

/// The rest of the terminal-state table, so the fixes above cannot have moved
/// the ordinary outcomes: a clean run is `completed` + `done`, a run with
/// undecided findings is `needsReview`, and a recorded reason always wins over
/// the derived one.
#[test]
fn the_terminal_state_table_holds_for_the_ordinary_outcomes() {
    let clean = RunLedger::new();
    assert_eq!(
        super::hooks::terminal_state(&clean, true, false, false, true),
        ("completed", Some("done".to_string()))
    );

    let review = RunLedger::new();
    assert_eq!(
        super::hooks::terminal_state(&review, true, false, true, true),
        ("needsReview", Some("done".to_string()))
    );

    // A stage that stopped the run keeps its own reason, and `needsReview` is
    // still not a failure.
    let repaired = RunLedger::new();
    repaired.stop(StoppedReason::MaxRepairs);
    assert_eq!(
        super::hooks::terminal_state(&repaired, true, false, true, true),
        ("needsReview", Some("max_repairs".to_string()))
    );

    // The deadline with NOTHING saved: the stage errored out before a document
    // existed, so the run failed — but it says WHY.
    let timed_out = RunLedger::new();
    timed_out.stop(StoppedReason::RunTimeout);
    assert_eq!(
        super::hooks::terminal_state(&timed_out, false, false, false, false),
        ("failed", Some("run_timeout".to_string()))
    );

    // A run already stopped for a reason of its own, cancelled afterwards: the
    // two halves come from different places on purpose. The REASON is
    // first-writer-wins (the budget ceiling is what stopped it, and it did come
    // first); the STATUS is the token's, because the user acted — and `execute`
    // reads the status to choose `job_cancel` over `job_fail`.
    let budgeted = RunLedger::new();
    budgeted.stop(StoppedReason::Budgeted);
    assert_eq!(
        super::hooks::terminal_state(&budgeted, false, true, false, true),
        ("cancelled", Some("budgeted".to_string()))
    );
}

/// **The deadline's two enforcement points must reach the same verdict about
/// the same run.**
///
/// Expiring INSIDE the repair loop breaks the loop, keeps the accumulated
/// document and returns `Ok`, so the run lands `needsReview` + `run_timeout`.
/// Expiring at the stage BOUNDARY one instant later goes through `apply_stop`,
/// which returns `Err` — and that came out `failed`, even though
/// `persist_document` had already written the same real, reviewable résumé to
/// `ai_generations`. Same document, same reason, opposite verdict, and `failed`
/// is the one that tells the user their run produced nothing.
///
/// Mutation check: drop the `persisted` term from `terminal_state` (or the
/// `RunTimeout` test in it) and the first two assertions fail; derive
/// `cancelled` from the ledger's reason alone (drop the `!ok && token_cancelled`
/// term) and the cancel assertion fails.
#[test]
fn a_deadline_that_still_saved_a_document_is_not_a_failure() {
    // Undecided findings in the saved report — the ordinary shape of a run the
    // repair loop ran out of time on.
    let review = RunLedger::new();
    review.stop(StoppedReason::RunTimeout);
    assert_eq!(
        super::hooks::terminal_state(&review, false, false, true, true),
        ("needsReview", Some("run_timeout".to_string())),
        "the boundary check must land where the in-loop check does"
    );

    // Nothing left to decide: the document is usable as it stands.
    let clean = RunLedger::new();
    clean.stop(StoppedReason::RunTimeout);
    assert_eq!(
        super::hooks::terminal_state(&clean, false, false, false, true),
        ("completed", Some("run_timeout".to_string()))
    );

    // A run whose CANCEL TOKEN fired: the user acted, so the STATUS is
    // `cancelled` — the outcome the comment on this row always argued for and
    // the one `execute` needs to emit `job_cancel` rather than `job_fail`. The
    // REASON stays first-writer-wins (`run_timeout` is what stopped it, and it
    // happened first), which is the same split the `budgeted` row in the table
    // above pins. Reading the status off the reason alone reported `failed`
    // here, i.e. "your run produced nothing", for a run the user themselves
    // ended.
    let cancelled = RunLedger::new();
    cancelled.stop(StoppedReason::RunTimeout);
    assert_eq!(
        super::hooks::terminal_state(&cancelled, false, true, true, true),
        ("cancelled", Some("run_timeout".to_string()))
    );

    // Only `RunTimeout` qualifies. A provider error mid-run leaves a document
    // whose report describes a different document, so it stays a failure even
    // though something was saved.
    let errored = RunLedger::new();
    errored.stop(StoppedReason::MaxRepairs);
    assert_eq!(
        super::hooks::terminal_state(&errored, false, false, true, true),
        ("failed", Some("max_repairs".to_string()))
    );
}

// ── Admission + server-side clamps ──────────────────────────────────────────

/// **Every provider-calling command in this module goes through the limiter.**
///
/// `resume_pipeline_regenerate_section` did not: it went straight to
/// `charge_daily` + `complete`, and `PROVIDER_DAILY_MAX` is a per-DAY TOTAL,
/// not a rate — so a renderer loop on that button burns the whole ceiling in
/// seconds and every other AI feature in the app is dead until UTC midnight.
///
/// The command needs an `AppHandle` this crate has no harness for, so the
/// assertion is on the exact admission call it makes, against a real `Limiter`:
/// the bucket's concurrency cap refuses the next caller with the retriable
/// error the command's `?` propagates, and the guard releases on drop.
///
/// **This pins the MECHANISM, not the number.** Raising
/// `AGENT_RUN_CONCURRENCY_MAX` does NOT fail it — the loop below is derived
/// from that constant, deliberately, because the value belongs to `limits` and
/// is pinned there. Mutation check for what this DOES guard: make `acquire`
/// return `Ok` on a full gate and the refusal assertion fails; make the guard
/// leak its permit and the re-open assertion does. The call SITE is pinned by
/// the source lock below — deleting the `acquire` call fails that one (verified).
#[test]
fn the_regenerate_section_bucket_refuses_a_caller_past_its_concurrency_cap() {
    let limiter = std::sync::Arc::new(crate::limits::Limiter::default());
    let held: Vec<_> = (0..crate::limits::AGENT_RUN_CONCURRENCY_MAX)
        .map(|index| {
            limiter
                .acquire(
                    "agent_run",
                    crate::limits::AGENT_RUN_RATE_MAX,
                    crate::limits::AGENT_RUN_CONCURRENCY_MAX,
                )
                .unwrap_or_else(|e| panic!("slot {index} must be admitted: {e}"))
        })
        .collect();

    let refused = limiter.acquire(
        "agent_run",
        crate::limits::AGENT_RUN_RATE_MAX,
        crate::limits::AGENT_RUN_CONCURRENCY_MAX,
    );
    assert!(
        matches!(refused, Err(crate::error::AppError::RateLimited(_))),
        "the bucket must refuse, and with the retriable variant the command propagates"
    );
    drop(held);
    assert!(
        limiter
            .acquire(
                "agent_run",
                crate::limits::AGENT_RUN_RATE_MAX,
                crate::limits::AGENT_RUN_CONCURRENCY_MAX,
            )
            .is_ok(),
        "the guard is RAII — releasing it must re-open the slot"
    );
}

/// The source-level half of the lock above: the two provider-calling commands
/// in this module must ADMIT before they spend. Grep-shaped for the same reason
/// `job_analysis_never_reaches_match_scoring` is — the command bodies need a
/// Tauri harness, and "it calls the limiter" is otherwise provable only by
/// reading the code.
///
/// Mutation check: delete either `acquire` call and this fails.
#[test]
fn every_provider_calling_command_admits_before_it_spends() {
    let source = include_str!("mod.rs");
    assert!(
        source.contains(".acquire_queued("),
        "resume_pipeline_run must park on the concurrency cap"
    );
    assert!(
        source.contains(".acquire("),
        "resume_pipeline_regenerate_section must be admitted (and REFUSED, not parked — it is a \
         click, not a run)"
    );
}

/// **The wire schema's caps are Zod, and Zod does not run on this transport.**
/// A direct IPC caller can send an unbounded `targetLanguage`, a 10 000-entry
/// `topRequirements`, or a 5 MB cover letter, all of which reach a prompt or a
/// stored row. The command mirrors the caps server-side, HOISTED from
/// `commands::resume` rather than re-declared.
///
/// Mutation check: drop any one clamp and its assertion fails. The multi-byte
/// straddle is deliberate — a naive byte truncate splits it and produces
/// invalid UTF-8.
#[test]
fn oversized_run_request_free_text_is_clamped_server_side() {
    use crate::commands::resume::{
        TARGET_LANGUAGE_CAP, TOP_REQUIREMENTS_CAP, TOP_REQUIREMENT_BYTES_CAP,
    };

    let huge = "a".repeat(TARGET_LANGUAGE_CAP - 1) + "\u{1F600}" + &"b".repeat(5_000);
    let req: ResumePipelineRunRequest = serde_json::from_value(json!({
        "resumeId": "res-1",
        "jobId": "job-9",
        "jobUrl": format!("https://boards.example/{}", "u".repeat(9_000)),
        "targetLanguage": huge,
        "topRequirements": (0..500).map(|i| format!("{i} {}", "r".repeat(2_000)))
            .collect::<Vec<_>>(),
        "coverLetterText": "c".repeat(1_000_000),
    }))
    .expect("the hostile shape still deserializes — nothing rejects it on the wire");

    let clamped = super::clamp_request(&req);
    assert!(clamped.target_language.len() <= TARGET_LANGUAGE_CAP);
    assert!(
        !clamped.target_language.contains('\u{1F600}'),
        "must cut before the multi-byte char, not through it"
    );
    assert_eq!(clamped.top_requirements.len(), TOP_REQUIREMENTS_CAP);
    assert!(clamped
        .top_requirements
        .iter()
        .all(|r| r.len() <= TOP_REQUIREMENT_BYTES_CAP));
    assert!(clamped.job_url.len() <= 2_048);
    assert!(
        clamped.cover_letter.len() <= crate::applications::MAX_JOB_DESCRIPTION_BYTES,
        "the letter is validated AND stored — an unbounded one reaches both"
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
/// The artifact goes through `with_detail`, which is what `RunHooks::after`
/// does — so BOTH halves of that wrapper are pinned here at the row level: a
/// detail-less stage's counts reach the row (`validate`'s `criticals`), and a
/// stage that left a detail carries it nested beside them (`strategy`).
///
/// Mutation check: emit a phase outside the CHECK's vocabulary and the event
/// count drops to zero; change `RUN_KIND` on one side only and the filtered
/// list is empty; make `with_detail` drop the artifact when there is no detail
/// and the `criticals` assertion fails.
#[test]
fn a_run_round_trips_through_the_store_with_real_stage_events() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = PipelineRunStore::open(dir.path()).expect("store opens");

    let ledger = RunLedger::new();
    ledger.record("analyze_job", json!({ "cached": false, "mustHave": 4 }));
    ledger.record("strategy", json!({ "cached": false, "companies": 3 }));
    ledger.record_detail("strategy", json!({ "perCompany": [{ "company": "Acme" }] }));
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
                    // Through `with_detail`, exactly as `RunHooks::after`
                    // builds it — not `ledger.artifact` directly, which is what
                    // let the wrapper delete every detail-less stage's counts
                    // while this end-to-end test still read them.
                    artifact_json: with_detail(ledger.artifact(stage), ledger.detail(stage))
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

    // The other half of the wrapper: a stage that left a FULL artifact carries
    // it nested under the counts, not instead of them.
    let strategy = events
        .iter()
        .find(|event| event.stage == "strategy" && event.phase == "finish")
        .expect("the strategy stage's finish event");
    let artifact: serde_json::Value =
        serde_json::from_str(&strategy.artifact_json).expect("the artifact is JSON");
    assert_eq!(artifact["companies"], json!(3));
    assert_eq!(
        artifact[super::hooks::DETAIL_KEY]["perCompany"][0]["company"],
        json!("Acme")
    );

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

/// **A write command refuses an OLDER run of the same posting.**
///
/// Every run of a posting merges into ONE `ai_generations` aggregate, so
/// `find_for_job` can only ever return the newest run's document while
/// `listForJob` legitimately advertises three runs. Without this guard,
/// `regenerateSection(oldRunId)` rewrote the NEWEST document and returned a
/// detail whose `resumeText` was never that run's — a silent cross-run edit,
/// which is worse than a refusal.
///
/// Mutation check: return `Ok(())` unconditionally from `ensure_latest_run` and
/// the older-run assertion fails; drop the empty-`job_url` exemption and the
/// unlinked case fails.
#[test]
fn a_write_against_an_older_run_of_the_same_posting_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = PipelineRunStore::open(dir.path()).expect("store opens");

    let base = RunRow {
        id: String::new(),
        job_url: "https://boards.example/jobs/42".to_string(),
        kind: super::RUN_KIND.to_string(),
        depth: "quality".to_string(),
        status: "completed".to_string(),
        started_at: 0,
        finished_at: None,
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    };
    let older = RunRow {
        id: "run-older".to_string(),
        started_at: 1_700_000_000_000,
        ..base.clone()
    };
    let newer = RunRow {
        id: "run-newer".to_string(),
        started_at: 1_700_000_500_000,
        ..base.clone()
    };
    store.upsert_run(&older).expect("older run persists");
    store.upsert_run(&newer).expect("newer run persists");

    assert!(
        super::ensure_latest_run(&store, &newer).is_ok(),
        "the newest run owns the document"
    );
    let refused = super::ensure_latest_run(&store, &older);
    assert!(
        matches!(refused, Err(crate::error::AppError::Validation(_))),
        "an older run must be refused rather than silently editing the newest document"
    );

    // A run of ANOTHER kind against the same posting is not competition — the
    // aggregate is partitioned by `(job_url, kind)`.
    store
        .upsert_run(&RunRow {
            id: "run-agent".to_string(),
            kind: "agent".to_string(),
            started_at: 1_700_000_900_000,
            ..base.clone()
        })
        .expect("an agent run shares the tables");
    assert!(
        super::ensure_latest_run(&store, &newer).is_ok(),
        "a newer run of a different kind must not lock the résumé flow out"
    );

    // An UNLINKED run has no aggregate at all, so this guard must stand aside
    // and let the "no saved résumé" error be the one the user sees.
    let unlinked = RunRow {
        id: "run-unlinked".to_string(),
        job_url: String::new(),
        started_at: 1_700_000_100_000,
        ..base
    };
    store.upsert_run(&unlinked).expect("unlinked run persists");
    assert!(super::ensure_latest_run(&store, &unlinked).is_ok());
}

// ── The run/document divergence ─────────────────────────────────────────────

const DIVERGENCE_JOB_URL: &str = "https://boards.example/jobs/77";

/// Every `issueKey` in a wrapper's résumé review list, in report order.
fn fabrication_keys(wrapper: &str) -> Vec<String> {
    let parsed: serde_json::Value = serde_json::from_str(wrapper).expect("valid JSON");
    parsed["resume"]["fabrications"]
        .as_array()
        .expect("the fixture must flag something")
        .iter()
        .map(|entry| entry["issueKey"].as_str().expect("issueKey").to_string())
        .collect()
}

/// One aggregate row for `DIVERGENCE_JOB_URL`, written the way the pipeline
/// writes one. Returns the row id the merge is keyed to.
fn seed_aggregate(store: &AiGenerationStore, text: &str, wrapper: &str) -> String {
    store
        .save_application(AiGenerationRecord {
            id: "gen-divergence".to_string(),
            created_at: 1_700_000_000_000,
            job_url: DIVERGENCE_JOB_URL.to_string(),
            resume_text: text.to_string(),
            quality_report: wrapper.to_string(),
            ..super::empty_record()
        })
        .expect("the aggregate is written")
}

/// **An applied "Remove" moves the document and must NOT cost the user their
/// review.**
///
/// The live path: `FabricationReview` records the verdict and then deletes the
/// flagged line through the editor's own change handler, which saves via
/// `AiGenerationStore::update_texts` — text only, `quality_report` untouched. So
/// `resume_text` moves away from the text the report was computed over while the
/// report, its review list and every recorded verdict stay exactly as stored.
/// The divergence is deliberate (see the module doc); what this pins is that it
/// costs nothing but a stale hash.
///
/// The order is the live one: `FabricationReview` APPLIES the removal (a text
/// edit) and only then records the verdict, so the decision is always stamped
/// against a document that has already moved.
///
/// Mutation check: have `update_texts` also write `quality_report` and the
/// byte-identical + still-pending + verdict-lands assertions all fail; make
/// `unresolved_count` count decided entries too and the final `0` does.
#[test]
fn an_edit_that_moves_the_document_leaves_every_verdict_landable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).expect("store opens");

    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, FABRICATING_DRAFT)), None);
    let keys = fabrication_keys(&wrapper);
    let id = seed_aggregate(&store, FABRICATING_DRAFT, &wrapper);

    // The user APPLIES the removal first: the flagged line leaves the document
    // through the ordinary editor save, which is text-only by design.
    let edited = FABRICATING_DRAFT.replace(
        "A payments engineer who cut costs by 47% across 12 teams.\n",
        "",
    );
    assert_ne!(edited, FABRICATING_DRAFT, "the fixture line must be gone");
    store
        .update_texts(&id, Some(edited.clone()), None)
        .expect("the edit is persisted");

    let after = store
        .find_for_job(DIVERGENCE_JOB_URL)
        .expect("the aggregate is still there");
    assert_eq!(after.resume_text, edited, "the document moved");
    assert_eq!(
        after.quality_report, wrapper,
        "a text edit must not touch the report column — byte for byte"
    );
    assert_eq!(
        report::unresolved_count(&after.quality_report, &after.resume_text, ""),
        keys.len(),
        "…so the review is still pending: an edit neither resolves nor wipes it"
    );

    // The report now describes an EARLIER version of the document. That is the
    // renderer's "checked before your edits" state, not corruption.
    let parsed: serde_json::Value = serde_json::from_str(&after.quality_report).unwrap();
    assert_eq!(
        parsed["resume"]["sourceTextHash"],
        json!(report::hash_text(FABRICATING_DRAFT))
    );
    assert_ne!(
        parsed["resume"]["sourceTextHash"],
        json!(report::hash_text(&edited)),
        "the staleness anchor is what tells the panel to say so"
    );

    // …and THEN the verdict is recorded, against a document whose text no
    // longer contains the evidence. It still lands: `record_decision` stamps by
    // issueKey and reads no text. If it did not, every applied Remove would
    // strand its run in `needsReview` forever.
    let decided = report::record_decision(&after.quality_report, &keys[0], "remove")
        .expect("an orphaned finding must stay decidable");
    store
        .update_quality_report(&id, decided)
        .expect("the verdict is persisted");
    let settled = store
        .find_for_job(DIVERGENCE_JOB_URL)
        .expect("row")
        .quality_report;
    // Against the EDITED text: the span is gone, so the Remove is fact as well
    // as intent — the pair agrees, and the run may finish.
    assert_eq!(report::unresolved_count(&settled, &edited, ""), 0);
    assert!(!report::still_needs_review(&settled, &edited, ""));
}

/// **A save replaces a report SLOT whole — carrying the review list forward is
/// the writer's job, not the store's.**
///
/// `merge_quality_report` merges per TOP-LEVEL key, which protects the OTHER
/// document's slot and nothing inside this one. So a writer that recomputes the
/// résumé slot and drops `fabrications` wipes the review list and every verdict
/// in it, after which `resolveFabrication` no-ops forever (the renderer's own
/// `mergeRecheckedReport` overlays the previous slot's extra keys for exactly
/// this reason — 3aad7d52). This test exists so that obligation is visible on
/// the Rust side and a change to it has to be deliberate.
///
/// Mutation check: make `merge_quality_report` keep `existing` (the "protect the
/// stored report" change someone will eventually reach for) and both wiped-list
/// assertions fail — which is the honest signal that the contract documented in
/// the module doc and in `resumePipeline.ts` moved.
#[test]
fn a_save_replaces_a_slot_whole_so_the_writer_owns_the_review_list() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).expect("store opens");

    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, FABRICATING_DRAFT)), None);
    let keys = fabrication_keys(&wrapper);
    let decided = report::record_decision(&wrapper, &keys[0], "keep").expect("a known key");
    seed_aggregate(&store, FABRICATING_DRAFT, &decided);

    // A re-check that rebuilds the slot from scratch: same report, no review
    // list. The stored one does NOT survive underneath it.
    let mut stripped: serde_json::Value = serde_json::from_str(&decided).unwrap();
    stripped["resume"]
        .as_object_mut()
        .expect("the résumé slot")
        .remove("fabrications");
    seed_aggregate(&store, FABRICATING_DRAFT, &stripped.to_string());
    let wiped = store
        .find_for_job(DIVERGENCE_JOB_URL)
        .expect("row")
        .quality_report;
    assert_eq!(report::unresolved_count(&wiped, FABRICATING_DRAFT, ""), 0);
    assert!(
        report::record_decision(&wiped, &keys[0], "remove").is_none(),
        "with the list gone there is nothing left to stamp — the silent-no-op state"
    );

    // A writer that CARRIES the list forward keeps every verdict, which is what
    // the renderer's re-check does.
    seed_aggregate(&store, FABRICATING_DRAFT, &decided);
    let carried = store
        .find_for_job(DIVERGENCE_JOB_URL)
        .expect("row")
        .quality_report;
    let parsed: serde_json::Value = serde_json::from_str(&carried).unwrap();
    assert_eq!(
        parsed["resume"]["fabrications"][0]["decision"],
        json!("keep")
    );
    assert!(report::record_decision(&carried, &keys[0], "remove").is_some());
}

// ── The terminal notification (ADR-016) ─────────────────────────────────────

/// **A finished run announces what it actually ended in.**
///
/// The four terminal states get four different cards, and a non-terminal status
/// gets none — a run is announced once, when it is over. `needsReview` must not
/// read as a failure (the document is usable) and `failed` must not read as
/// ready.
///
/// Asserted on the pure builder: `push_and_notify` needs an `AppHandle` this
/// crate has no harness for, so the SIDE EFFECT is one line at one call site
/// while the whole decision is here (the same seam shape as `apply_stop` and
/// `truncation_notification`).
///
/// Mutation check: return a card for `running` and the first assertion fails;
/// collapse `needsReview` onto the `completed` title and the "not a failure /
/// not clean" pair does; drop the singular arm from `review_note` and the
/// "1 flagged claim" assertion does.
#[test]
fn the_terminal_notification_reports_the_state_the_run_ended_in() {
    // Not terminal, and not a status this module knows: nothing to announce.
    assert!(run_notification("running", 3, "Senior Engineer", "Acme").is_none());
    assert!(run_notification("queued", 0, "Senior Engineer", "Acme").is_none());

    let completed = run_notification(super::STATUS_COMPLETED, 0, "Senior Engineer", "Acme")
        .expect("a terminal run is announced");
    assert_eq!(completed.kind, "resume.pipeline_run");
    assert_eq!(completed.body, "Senior Engineer · Acme");
    assert!(completed
        .route
        .as_ref()
        .is_some_and(|route| route.to == "/jobs"));

    let review = run_notification(super::STATUS_NEEDS_REVIEW, 2, "Senior Engineer", "Acme")
        .expect("needsReview is terminal");
    assert!(
        review.body.contains("2 flagged claims"),
        "the user is told how much is left: {}",
        review.body
    );
    assert!(
        review.title.contains("ready"),
        "needsReview is not a failure — the document exists"
    );
    assert_ne!(review.title, completed.title, "…and it is not clean either");

    // One finding is not "1 flagged claims", and ZERO is the unreviewable
    // Critical (`factual.dropped_role`), not "0 flagged claims".
    let one = run_notification(super::STATUS_NEEDS_REVIEW, 1, "", "").expect("terminal");
    assert!(one.body.contains("1 flagged claim needs"), "{}", one.body);
    assert!(
        one.body.starts_with("Untitled posting"),
        "a posting with no title or company still names itself: {}",
        one.body
    );
    let unreviewable = run_notification(super::STATUS_NEEDS_REVIEW, 0, "Engineer", "").unwrap();
    assert!(!unreviewable.body.contains('0'), "{}", unreviewable.body);
    assert!(unreviewable.body.contains("cannot clear"));
    assert!(unreviewable.body.starts_with("Engineer — "));

    let failed = run_notification(super::STATUS_FAILED, 0, "Senior Engineer", "Acme").unwrap();
    let cancelled =
        run_notification(super::STATUS_CANCELLED, 0, "Senior Engineer", "Acme").unwrap();
    let titles = [
        completed.title,
        review.title,
        failed.title,
        cancelled.title.clone(),
    ];
    let mut unique = titles.to_vec();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 4, "four outcomes, four cards: {titles:?}");
    // The cancel card exists because a cancel lands at the next stage boundary,
    // which can be minutes after the click — this is "it has stopped now".
    assert!(cancelled.body.contains("has stopped"));

    // The call SITE, grep-shaped for the same reason
    // `every_provider_calling_command_admits_before_it_spends` is: `execute`
    // needs a Tauri harness this crate does not have, so "a finished run
    // actually announces itself" is otherwise provable only by reading the code
    // — and the decision above is inert without the call.
    assert!(
        include_str!("mod.rs").contains("notify::notify_terminal("),
        "execute must push the terminal notification"
    );
}

/// **A crafted posting title cannot displace what the card is FOR.**
///
/// The label is scraped, attacker-influenceable text and it comes first in every
/// body; the store clamps the whole body to `MAX_BODY_CHARS` and this string can
/// leave the app as an OS banner. Unbudgeted, a 600-character title eats the
/// entire clamp and the user is shown pure attacker text with no clause saying
/// what happened — so the label is capped and the fixed half always survives.
///
/// Asserted against the store's OWN constant, not a second copy of 500: the
/// relation being pinned is "label + clause fits the clamp", and a test carrying
/// its own number stops testing the clamp the moment the store's changes.
///
/// Mutation check: remove the `LABEL_CAP` clamp from `posting_label` and every
/// `survives` assertion below fails.
#[test]
fn a_crafted_posting_title_cannot_displace_the_notification_clause() {
    const CLAMP: usize = crate::notifications::MAX_BODY_CHARS;
    let hostile = "T".repeat(600);
    let cases = [
        (
            super::STATUS_NEEDS_REVIEW,
            3usize,
            "Keep or Remove decision",
        ),
        (super::STATUS_NEEDS_REVIEW, 0, "cannot clear"),
        (super::STATUS_FAILED, 0, "before it produced a résumé"),
        (super::STATUS_CANCELLED, 0, "has stopped"),
    ];
    for (status, unresolved, clause) in cases {
        let body = run_notification(status, unresolved, &hostile, &hostile)
            .expect("a terminal status is announced")
            .body;
        assert!(
            body.chars().count() <= CLAMP,
            "{status}: the body must fit the store's clamp before it is cut ({} chars)",
            body.chars().count()
        );
        // The clamp the store will actually apply, applied here: the clause has
        // to survive it, not merely exist somewhere past it.
        let survives: String = body.chars().take(CLAMP).collect();
        assert!(
            survives.contains(clause),
            "{status}: {clause:?} was displaced by the scraped title — {survives}"
        );
        assert!(
            survives.starts_with("TTT"),
            "{status}: the posting still names itself"
        );
    }

    // An ordinary title is untouched — the cap is a backstop, not a formatter.
    let ordinary = run_notification(super::STATUS_COMPLETED, 0, "Senior Engineer", "Acme")
        .expect("terminal")
        .body;
    assert_eq!(ordinary, "Senior Engineer · Acme");
}

/// The number in the notification is the number the review panel will show — one
/// definition of "undecided", read from the persisted wrapper.
///
/// Mutation check: count every fabrication instead of the undecided ones and the
/// post-verdict assertion fails; make `unresolved_count` panic-free-but-wrong on
/// a bad blob (return 1) and the last assertion does.
#[test]
fn the_review_notification_counts_the_findings_the_panel_lists() {
    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, FABRICATING_DRAFT)), None);
    let keys = fabrication_keys(&wrapper);
    assert_eq!(
        report::unresolved_count(&wrapper, FABRICATING_DRAFT, ""),
        keys.len()
    );
    assert!(report::has_unresolved(&wrapper, FABRICATING_DRAFT, ""));

    let body = run_notification(
        super::STATUS_NEEDS_REVIEW,
        report::unresolved_count(&wrapper, FABRICATING_DRAFT, ""),
        "Engineer",
        "Acme",
    )
    .expect("terminal")
    .body;
    assert!(body.contains(&keys.len().to_string()), "{body}");

    let mut current = wrapper;
    for key in &keys {
        current = report::record_decision(&current, key, "keep").expect("a known key");
    }
    assert_eq!(report::unresolved_count(&current, FABRICATING_DRAFT, ""), 0);
    assert!(!report::has_unresolved(&current, FABRICATING_DRAFT, ""));

    // No report at all (a fast-path row, or a run that failed before validate)
    // counts nothing rather than inventing review work.
    assert_eq!(report::unresolved_count("", "", ""), 0);
    assert_eq!(report::unresolved_count("not json", "", ""), 0);
    assert_eq!(report::unresolved_count("{}", "", ""), 0);
}

// ── Per-stage routing is only resolved for stages that pay ──────────────────

/// `max::paying_stages` is the belt that keeps a stored override on a stage
/// which makes no AI call from ever being RESOLVED — and resolving propagates
/// its error, so one bad row on an inert stage would abort a run before it
/// started. The store refuses to write such a row (first belt); this is what
/// holds when the row got there another way, e.g. a hand-edited database or a
/// stage that stopped paying after the row was written.
///
/// Asserted for BOTH depths, because "free" is per-depth: the quality pipeline
/// and the max pipeline do not run the same stages.
///
/// Mutation check (executed): neuter the filter to `|_| true` and both depths
/// yield their free stages, failing here.
#[test]
fn paying_stages_never_includes_a_stage_that_makes_no_ai_call() {
    use crate::ipc_contracts::events::PIPELINE_STAGES_FREE;
    use crate::pipeline::resume::types::GenerationDepth;

    // Pinned as LITERAL lists, in order. Deriving the expectation
    // (`stage_names()` minus `free_stage_names()`) would restate the
    // implementation and pass whatever it did; these fail if a paying stage is
    // ever silently DROPPED, which is the costly direction — an override the
    // user sets, Settings shows, and nothing ever resolves.
    for (depth, expected) in [
        (
            GenerationDepth::Quality,
            &[
                "analyze_job",
                "match_evidence",
                "strategy",
                "draft",
                "repair",
            ][..],
        ),
        (
            GenerationDepth::Max,
            &[
                "analyze_job",
                "match_evidence",
                "strategy",
                "sections",
                "repair",
                "llm_judge",
            ][..],
        ),
    ] {
        let paying = max::paying_stages(depth);
        assert_eq!(paying, expected, "{depth:?} lost or gained a paying stage");
        for free in PIPELINE_STAGES_FREE {
            assert!(
                !paying.contains(free),
                "{depth:?} would resolve routing for the inert stage {free:?}"
            );
        }
    }
}

use super::super::report;
use super::support::{report_for, CLEAN_SOURCE, FABRICATING_DRAFT};
use crate::validate::content::{ContentInput, DocKind};
use serde_json::json;

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

/// **A cover-letter-only run's wrapper carries NO `resume` key at all.**
///
/// The exact mirror of the test below, in the direction the cover-only run
/// needs. `AiGenerationStore`'s merge overlays whole TOP-LEVEL keys, so an
/// empty-but-PRESENT `resume` slot would erase the posting's stored one —
/// its report, and every Keep/Remove verdict the user had already recorded
/// against an earlier run's résumé. Omitting the key is what leaves that slot
/// untouched, and it is why `persist_document` passes `ctx.report.as_ref()`
/// rather than an unconditional `Some`.
///
/// Mutation check: change `persist_document` to hand `report::build` a
/// `Some((&empty_report, ""))` for the résumé and the final assertion fails.
#[test]
fn a_letter_only_wrapper_omits_the_resume_key_so_the_merge_cannot_erase_it() {
    const LETTER: &str = "Dear hiring team,

I have run settlement systems for four years.
";

    let letter_report = crate::validate::content::validate_content(&ContentInput {
        generated: LETTER,
        source_resume: CLEAN_SOURCE,
        job_ad: "We need a payments engineer.",
        top_requirements: &[],
        target_language: "en",
        doc_kind: DocKind::CoverLetter,
    });
    let wrapper = report::build(
        "quality",
        1_700_000_000,
        None,
        Some((&letter_report, LETTER)),
    );
    let parsed: serde_json::Value = serde_json::from_str(&wrapper).expect("valid JSON");

    // The letter this run DID write is described, against absolutes.
    assert_eq!(parsed["schemaVersion"], json!(2));
    assert_eq!(parsed["pipeline"], json!("quality"));
    assert_eq!(parsed["generatedAt"], json!(1_700_000_000u64));
    assert!(parsed["coverLetter"]["report"].is_object());

    // …and the résumé slot is ABSENT, not empty. `is_null()` would also hold
    // for an explicit JSON null, which the merge would still overlay.
    assert!(
        parsed.get("resume").is_none(),
        "a run that wrote no résumé must contribute no `resume` key — an empty          one overwrites the posting's stored slot, verdicts included"
    );

    // The two readers that decide a run's terminal state must both survive the
    // missing key rather than treating it as a clean résumé or panicking.
    assert!(!report::still_needs_review(&wrapper, "", LETTER));
    assert_eq!(report::unresolved_count(&wrapper, "", LETTER), 0);
}

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

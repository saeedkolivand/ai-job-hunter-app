use super::super::report;
use super::support::{fabrication_keys, report_for, CLEAN_SOURCE, FABRICATING_DRAFT};
use crate::ai_generations::{AiGenerationRecord, AiGenerationStore};
use serde_json::json;

const DIVERGENCE_JOB_URL: &str = "https://boards.example/jobs/77";

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
            ..super::super::empty_record()
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

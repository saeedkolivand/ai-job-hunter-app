use super::super::report;
use super::support::{
    fabrication_keys, fabrication_report, only_fabrication, report_for, CLEAN_SOURCE,
    FABRICATING_DRAFT,
};
use crate::validate::content::{ContentIssue, ContentMetrics, ContentReport};
use serde_json::json;

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

/// **Resolution reads the anchored `line`, not a bare-`evidence` substring
/// search across the whole document.**
///
/// `evidence` is routinely a bare token (a certification acronym here) that
/// can legitimately recur on a DIFFERENT line the user never touched — a
/// project bullet naming the same acronym, say. `entry_resolved`'s job is "is
/// the flagged BULLET gone", not "does this substring exist anywhere" —
/// checking `evidence` alone answers the wrong question and strands a
/// genuinely-applied removal in `needsReview` forever.
///
/// Mutation check: revert `entry_resolved` to search `evidence` alone (drop
/// the `line` branch) and the final assertion fails — the run stays stuck at
/// 1 unresolved entry even though the flagged bullet is gone.
#[test]
fn resolution_reads_the_anchored_line_not_a_recurring_bare_token() {
    let text = "PROFESSIONAL SUMMARY\nCertified Kubernetes Administrator (CKA).\n\n\
                WORK EXPERIENCE\n- Built the ledger service\n";
    let wrapper = report::build("quality", 1, Some((&fabrication_report("CKA"), text)), None);
    let entry = only_fabrication(&wrapper);
    assert_eq!(
        entry["line"],
        json!("Certified Kubernetes Administrator (CKA)."),
        "the bare acronym is unique in this document, so it anchors"
    );
    let key = entry["issueKey"].as_str().expect("a key").to_string();
    let decided = report::record_decision(&wrapper, &key, "remove").expect("a known key");

    // The user removes exactly the flagged bullet — the anchored one — and
    // separately writes an unrelated project note that also names "CKA".
    let edited = "PROFESSIONAL SUMMARY\nA payments engineer.\n\nWORK EXPERIENCE\n\
                  - Built the ledger service\n\nPROJECTS\n\
                  - Built a CKA exam prep tool for study groups.\n";
    assert!(
        edited.contains("CKA"),
        "the unrelated project bullet still names the bare token"
    );
    assert!(!edited.contains("Certified Kubernetes Administrator (CKA)."));
    assert_eq!(
        report::unresolved_count(&decided, edited, ""),
        0,
        "the flagged bullet is gone; a DIFFERENT line naming the same bare token must not \
         strand the run"
    );
    assert!(!report::still_needs_review(&decided, edited, ""));
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
    use super::super::recomputed_status;
    // A fresh finding un-cleans a completed run…
    assert_eq!(
        recomputed_status(super::super::STATUS_COMPLETED, true),
        Some(super::super::STATUS_NEEDS_REVIEW)
    );
    // …and the last agreeing verdict finishes a needsReview one.
    assert_eq!(
        recomputed_status(super::super::STATUS_NEEDS_REVIEW, false),
        Some(super::super::STATUS_COMPLETED)
    );
    // Already right: nothing to write.
    assert_eq!(
        recomputed_status(super::super::STATUS_COMPLETED, false),
        None
    );
    assert_eq!(
        recomputed_status(super::super::STATUS_NEEDS_REVIEW, true),
        None
    );
    // How the run ENDED is not the report's to rewrite — and `running` is not
    // terminal.
    assert_eq!(recomputed_status(super::super::STATUS_FAILED, true), None);
    assert_eq!(recomputed_status(super::super::STATUS_FAILED, false), None);
    assert_eq!(
        recomputed_status(super::super::STATUS_CANCELLED, false),
        None
    );
    assert_eq!(recomputed_status("running", true), None);

    // The call sites, grep-shaped (the commands need a Tauri harness this
    // crate does not have): BOTH wrapper writers recompute the row.
    assert!(
        include_str!("../regenerate.rs")
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

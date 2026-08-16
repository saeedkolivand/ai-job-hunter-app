//! Tests for `max.rs`'s own decisions (the budget/deadline lookups and the
//! paying-stage filter) plus the wire-safety and document-completeness checks
//! that used to sit beside the depth switch this module implemented.
//!
//! A focused module rather than folding these into the (already large)
//! sibling `test` file: every guard here was mutation-checked, and the
//! comment on each names the change that makes it fail.

use serde_json::json;

use super::hooks::DETAIL_KEY;
use super::max::{budget, deadline_for, paying_stages};
use crate::pipeline::budget::Budget;
use crate::pipeline::resume::QUALITY_STAGES;

/// The pipeline this module resolves budget/deadline for is the one that
/// actually runs.
///
/// Mutation check: swap `quality_pipeline()` for a different stage list in
/// `paying_stages` and this fails.
#[test]
fn paying_stages_matches_the_pipelines_own_paid_stages() {
    assert_eq!(
        paying_stages(),
        vec![
            "analyze_job",
            "match_evidence",
            "strategy",
            "draft",
            "cover_letter",
            "repair",
            "humanize",
        ]
    );
    assert!(
        !paying_stages().contains(&"validate"),
        "validate makes no provider call and must never be resolved"
    );
}

/// The ceiling is picked from the backend-owned constant, never renderer
/// input — and it agrees with the pipeline's own free/paid split.
///
/// Mutation check: return anything other than `Budget::RESUME_QUALITY` from
/// `budget()` and this fails (it was `Budget::AGENT_PREP` before that
/// agent-flow budget was deleted, PR-5 step 2).
#[test]
fn the_run_uses_the_backend_owned_quality_budget() {
    assert_eq!(budget(), Budget::RESUME_QUALITY);
    assert_eq!(
        crate::pipeline::resume::quality_pipeline().free_stage_names(),
        vec!["validate"],
        "the check is the one stage that turns paid answers into a saved document for free"
    );
    for stage in paying_stages() {
        assert!(
            crate::pipeline::resume::quality_pipeline()
                .stage_names()
                .contains(&stage),
            "{stage} must be a real stage of the pipeline that runs"
        );
    }
}

/// The wall clock scales with effort and floors at the budget's own constant.
///
/// Mutation check: point `deadline_for` at a flat constant instead of
/// `run_deadline(budget(), …)` and the floor assertion fails.
#[test]
fn the_deadline_scales_with_effort_and_floors_at_the_budget() {
    assert_eq!(deadline_for(None), Budget::RESUME_QUALITY.run_timeout);
    assert!(
        deadline_for(Some("max")) > deadline_for(None),
        "a higher reasoning effort must buy more wall clock"
    );
}

// ── The persisted artifact detail's WIRE safety ─────────────────────────────

/// **A nested detail never reaches the renderer, whatever wrote it.** The
/// max-depth per-entry regenerate that used to persist one is gone, but an
/// EXISTING `pipeline_run_events` row from before this deletion can still
/// carry the key, and there is no migration touching old rows.
///
/// Mutation check: return the parsed value unchanged from `wire_artifact` and
/// the first assertion fails; return `Value::String(raw)` for the unparseable
/// case and the truncated-artifact assertion does.
#[test]
fn the_wire_strips_a_detail_key_an_old_row_might_still_carry() {
    let mut artifact = json!({ "cached": false, "companies": 3 });
    artifact.as_object_mut().expect("an object").insert(
        DETAIL_KEY.to_string(),
        json!({ "perCompany": [{ "company": "Acme Payments" }] }),
    );
    let row = artifact.to_string();

    let wire = super::wire_artifact(&row);
    assert_eq!(wire.get("companies"), Some(&json!(3)));
    assert_eq!(
        wire.get(DETAIL_KEY),
        None,
        "a nested detail must never reach the renderer"
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

// ── The completeness floor on an overwrite ───────────────────────────────────

/// **A run may overwrite the saved document, but not with a résumé that lost a
/// work history the SOURCE has.**
///
/// `persist_document` is an OVERWRITE — one `ai_generations` row per posting, no
/// versioning — and since a deadline-stopped run keeps a real draft (F3), a
/// partial document can reach it. That trade is deliberate for a run that lost
/// TAIL sections: the missing employer is a visible `factual.dropped_role`, the
/// run lands `needsReview`, and the user can see the document is short. It is
/// NOT acceptable for a document that lost ALL of a real work history.
///
/// **The discriminator is the SOURCE, and two earlier ones were not.** Keying
/// on the recorded stop REASON refused a completed run over a source that has
/// no employment section at all (a new graduate, an academic CV) the moment its
/// repair round hit the daily cap — `stages::repair` records `Budgeted` for a
/// stop it recovered from and returns `Ok(())`. Keying on the run's OUTCOME
/// then missed the case the gate exists for: the removed `max` depth's
/// per-section fan-out treated a daily-cap refusal as `Budgeted`, broke, and
/// returned `Ok(())` too, and nothing downstream turned that into an `Err` —
/// so a max run that hit the cap right after Summary had `outcome == Ok` and a
/// summary-only document overwrote the saved résumé. The arm that asserted
/// THAT was persistable pinned the defect, the same way round 1's `== None`
/// did one module over.
///
/// Mutation check: drop the source-side term (refuse whenever the draft has no
/// work history) and the new-graduate case fails; drop the draft-side term
/// (always persist) and the truncated-fan-out case does.
#[test]
fn a_document_that_lost_the_sources_whole_work_history_may_not_overwrite_it() {
    const SOURCE_WITH_WORK: &str = "Professional Summary\n\nA payments engineer.\n\n\
                                    Work Experience\n\nStaff Engineer, Acme  2021 - Present\n\
                                    - Owned the settlement service\n";
    const SOURCE_NO_WORK: &str = "Professional Summary\n\nA recent graduate.\n\n\
                                  Education\n\nMSc Computer Science, TU Berlin  2022 - 2024\n";
    const WITH_WORK: &str = "Professional Summary\n\nA payments engineer.\n\n\
                             Work Experience\n\nStaff Engineer, Acme  2021 - Present\n\
                             - Owned the settlement service\n";
    const NO_WORK: &str = "Professional Summary\n\nA payments engineer.\n\nSkills\n\nGo, Rust\n";
    const EMPTY_SECTION: &str = "Professional Summary\n\nA payments engineer.\n\nWork Experience\n";

    // (a) The hazard: a run refused by the daily cap right after Summary.
    //     The run returns Ok — nothing converts `Budgeted` into an error — so
    //     only the documents can tell that everything was lost.
    assert!(
        !super::is_persistable(SOURCE_WITH_WORK, NO_WORK),
        "a document that dropped the source's entire work history must not overwrite it"
    );
    assert!(
        !super::is_persistable(SOURCE_WITH_WORK, EMPTY_SECTION),
        "a heading with nothing under it is not work history"
    );

    // (b) The new graduate / academic CV: the source has no employment section
    //     either, so there is nothing to have lost and the save proceeds. This
    //     is the case a stop-reason gate silently refused — `completed` status,
    //     unchanged document, no explanation anywhere.
    assert!(
        super::is_persistable(SOURCE_NO_WORK, NO_WORK),
        "a source with no work history is a real input, not a truncated run"
    );
    assert!(super::is_persistable(SOURCE_NO_WORK, EMPTY_SECTION));

    // A document that KEPT its work history is always fine, however short.
    assert!(super::is_persistable(SOURCE_WITH_WORK, WITH_WORK));
    assert!(super::is_persistable(SOURCE_NO_WORK, WITH_WORK));

    // Both sides read through the SAME seam, so an undated entry — which is not
    // a `LineKind::JobEntry` — counts as work history on both, and a source
    // full of them cannot make every run unsaveable.
    const UNDATED: &str =
        "Work Experience\n\nStaff Engineer, Acme\n- Owned the settlement service\n";
    assert!(super::is_persistable(UNDATED, UNDATED));
    assert!(!super::is_persistable(UNDATED, NO_WORK));
}

/// **A REFUSED save is not a successful run.**
///
/// `is_persistable` rejects a document that lost the source's whole work
/// history, and `terminal_state` would otherwise read the pipeline's `Ok` and
/// report `completed`: a green run, an unchanged saved document, and nothing
/// anywhere saying why. The three verdicts are distinguished because two of
/// them mean opposite things to the run — `Nothing` is the unlinked or
/// empty-draft case, which is benign and reported by its own path.
///
/// Mutation check: collapse `Refused` into `Nothing` and the refused case stops
/// failing the run; return `Refused` for an empty draft and an ordinary failed
/// run gains a second, contradictory explanation.
#[test]
fn a_refused_save_is_distinguishable_from_having_nothing_to_save() {
    use super::SaveVerdict;

    const SOURCE_WITH_WORK: &str = "Work Experience\n\nStaff Engineer, Acme  2021 - Present\n\
                                    - Owned the settlement service\n";
    const NO_WORK: &str = "Professional Summary\n\nA payments engineer.\n";
    const URL: &str = "https://boards.example/jobs/1";

    assert_eq!(
        super::save_verdict(SOURCE_WITH_WORK, SOURCE_WITH_WORK, "", URL),
        SaveVerdict::Save
    );
    assert!(
        matches!(
            super::save_verdict(SOURCE_WITH_WORK, NO_WORK, "", URL),
            SaveVerdict::Refused(_)
        ),
        "a document that lost the source's work history is REFUSED, not skipped"
    );

    // Benign non-saves stay benign: an unlinked run is session-only by design,
    // and an empty draft is a run that already failed on its own terms.
    assert_eq!(
        super::save_verdict(SOURCE_WITH_WORK, NO_WORK, "", ""),
        SaveVerdict::Nothing
    );
    assert_eq!(
        super::save_verdict(SOURCE_WITH_WORK, "   ", "", URL),
        SaveVerdict::Nothing
    );
    // …and a source with no work history of its own is never refused.
    assert_eq!(
        super::save_verdict(NO_WORK, NO_WORK, "", URL),
        SaveVerdict::Save
    );
}

/// **A leaked prompt-fence tag refuses the save too — from EITHER document.**
///
/// `draft` and `cover_letter` are the sole producers of these documents (no
/// upstream stage to fall back to the way `humanize`/`repair` can), so an
/// echoed `<generated_resume>`/`<candidate_resume>` wrapper from either one
/// must be caught here or it reaches the saved aggregate and the exported PDF
/// unfiltered — the defect `humanize.rs`/`sections.rs`'s own
/// `contains_fence_tag` gates never covered because neither stage sits
/// downstream of `draft`/`cover_letter`.
///
/// Mutation check: check only `draft` (never `letter`) and the letter-only
/// case stops refusing; drop the `contains_fence_tag` calls entirely and
/// every case here stops refusing while
/// `a_refused_save_is_distinguishable_from_having_nothing_to_save` above
/// still passes (proving THAT test alone cannot catch this regression).
#[test]
fn a_leaked_fence_tag_in_either_document_refuses_the_save() {
    use super::SaveVerdict;

    const SOURCE: &str = "Work Experience\n\nStaff Engineer, Acme  2021 - Present\n\
                          - Owned the settlement service\n";
    const CLEAN_DRAFT: &str = "Work Experience\n\nStaff Engineer, Acme  2021 - Present\n\
                               - Owned the settlement service\n";
    const URL: &str = "https://boards.example/jobs/1";
    // A real leak looks like the WHOLE document swallowed inside the wrapper
    // the pipeline uses to fence untrusted input — see `prompt_fence.rs` — so
    // the work history is still there, tags aside; this isolates the
    // assertion to the fence-tag check rather than tripping the (separate,
    // already-covered) work-history one.
    const LEAKED_DRAFT: &str = "<generated_resume>\nWork Experience\n\n\
                                Staff Engineer, Acme  2021 - Present\n\
                                - Owned the settlement service\n</generated_resume>";
    const LEAKED_LETTER: &str = "<candidate_resume>\nDear hiring team,\n</candidate_resume>";

    // A clean draft + no letter still saves.
    assert_eq!(
        super::save_verdict(SOURCE, CLEAN_DRAFT, "", URL),
        SaveVerdict::Save
    );

    // The draft itself leaked.
    assert!(matches!(
        super::save_verdict(SOURCE, LEAKED_DRAFT, "", URL),
        SaveVerdict::Refused(_)
    ));

    // The draft is clean but the LETTER leaked — must still refuse; this is
    // exactly the gap a draft-only check would miss.
    assert!(matches!(
        super::save_verdict(SOURCE, CLEAN_DRAFT, LEAKED_LETTER, URL),
        SaveVerdict::Refused(_)
    ));

    // The refusal must be ACTIONABLE, not a bare tag — and distinguishable
    // from the work-history refusal so the two defects don't share a
    // (potentially misleading) message.
    let SaveVerdict::Refused(reason) = super::save_verdict(SOURCE, LEAKED_DRAFT, "", URL) else {
        panic!("expected Refused");
    };
    assert!(
        !reason.is_empty(),
        "the user must get an actionable message, not a silent empty result"
    );
    let SaveVerdict::Refused(work_history_reason) = super::save_verdict(
        SOURCE,
        "Professional Summary\n\nA payments engineer.\n",
        "",
        URL,
    ) else {
        panic!("expected Refused");
    };
    assert_ne!(
        reason, work_history_reason,
        "a leaked fence tag and a lost work history are different defects"
    );
}

/// Every pinned stage vocabulary agrees: `QUALITY_STAGES` describes exactly
/// the pipeline `paying_stages`/`budget`/`deadline_for` are resolved for.
///
/// Mutation check: reorder a stage in `quality_pipeline()` and this fails.
#[test]
fn the_quality_pipeline_matches_its_pinned_stage_list() {
    assert_eq!(
        crate::pipeline::resume::quality_pipeline().stage_names(),
        QUALITY_STAGES
    );
}

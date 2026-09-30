use super::support::persist_document_source;

/// **Resurrection regression — the lookup must be READ-ONLY.**
/// `persist_document` briefly called `ApplicationStore::upsert_for_origin`
/// to establish the FK (see the no-longer-true history in the old version of
/// this doc, still visible in git blame): that call CREATES an Application
/// on a `job_url` miss. A staged run takes minutes; if the user deletes the
/// Application it was launched from while the run is still working, landing
/// the run then upserting on that url silently RESURRECTS the delete — no
/// race required, just the plain sequence run-in-flight → delete → run
/// lands. This is the identical hazard
/// `ApplicationStore::link_orphaned_generations` already closed at ITS call
/// site (own doc); `find_by_job_url` is the same read-only primitive, reused
/// here rather than re-invented.
///
/// Pins that `persist_document` looks the Application up READ-ONLY
/// (`find_by_job_url`, never `upsert_for_origin`) and that a FOUND id still
/// lands on the persisted record — so the fix narrowed create-on-miss, it
/// did not disable linking altogether.
///
/// Mutation check, both applied and reverted: change `find_by_job_url(` back
/// to `upsert_for_origin(` and the second assertion fails; delete the
/// `application_id,` field from the `AiGenerationRecord` literal (falls back
/// to `..empty_record()`'s `None`) and the third fails.
///
/// **This is a SOURCE pin, not a behavior test** — see
/// [`persist_document_lookup_never_resurrects_a_deleted_application`] and
/// [`persist_document_lookup_links_to_an_existing_application`] below for
/// the store-level guards that actually drive the chain this test can only
/// assert the shape of.
#[test]
fn persist_document_looks_up_the_application_read_only_never_creates_one() {
    let body = persist_document_source();
    assert!(
        body.contains("find_by_job_url("),
        "persist_document must look the Application up READ-ONLY — the same \
         primitive link_orphaned_generations uses for the identical hazard"
    );
    assert!(
        !body.contains("upsert_for_origin("),
        "persist_document must never create-on-miss — that is exactly the \
         resurrection hazard e695f4cf closed at the sibling call site"
    );
    assert!(
        body.contains("application_id,"),
        "a FOUND id must still land on the AiGenerationRecord literal — otherwise \
         the fix disabled linking entirely instead of narrowing it to read-only"
    );
}

/// **False-positive regression — the save gate must ignore a fence tag in
/// the user's own PASTED reference letter.**
///
/// `persist_document` used to gate `save_verdict` on `ctx.letter_text()`,
/// which falls back to `input.cover_letter` (the renderer's `coverLetterText`
/// field, i.e. text the USER supplied) whenever the `cover_letter` stage
/// produced nothing — the ordinary shape of `include_cover_letter = false`.
/// `cover_letter_text` on the persisted record is `ctx.letter.clone()`
/// always, never that fallback (see the field's own doc comment above), so
/// the gate was inspecting text that was never going to be written to the
/// aggregate. A registered fence tag occurring INCIDENTALLY inside a user's
/// own pasted letter — `<question>` is a realistic one, since it is an
/// ordinary word a real cover letter can contain — refused the ENTIRE save,
/// résumé included, over a defect that could not possibly reach the saved
/// document.
///
/// Two things are pinned together: the CALL SITE (a source pin — the same
/// technique `persist_document_looks_up_the_application_read_only_never_creates_one`
/// above uses, for the same `AppHandle`-harness reason) and the CONSEQUENCE
/// (`save_verdict`'s own verdict for the two arguments that call could pass),
/// so a regression that reintroduces the fallback fails here even if the
/// call site is rewritten in some shape this source pin doesn't literally
/// match.
///
/// Mutation check: revert `persist_document`'s call to
/// pass `letter_text` (i.e. `ctx.letter_text()` bound to a local, the pre-fix
/// shape) in place of `&ctx.letter` and the
/// source-pin assertions below fail immediately.
#[test]
fn persist_document_save_gate_ignores_a_fence_tag_in_the_users_own_pasted_letter() {
    use super::super::SaveVerdict;

    // Whitespace-collapsed before matching: the call is rustfmt-owned and has
    // already been re-wrapped once (the `include_resume` argument pushed it over
    // the line width). The PROPERTY pinned here is which letter expression sits
    // in the third argument slot, not how the call happens to be laid out — a
    // source pin that also asserts the formatting fails on every unrelated
    // re-wrap and teaches nothing.
    let body = persist_document_source()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        body.contains("save_verdict( ctx.input.source_resume, &ctx.draft, &ctx.letter,"),
        "persist_document must gate save_verdict on &ctx.letter — the text \
         cover_letter_text actually persists — never letter_text()'s fallback \
         to the user's own pasted reference letter"
    );
    assert!(
        !body.contains("&ctx.draft, letter_text,"),
        "the pre-fix call shape (gated on the letter_text() fallback) must be gone"
    );

    // The consequence: when the stage produced no letter of its own
    // (`ctx.letter` empty — exactly what `include_cover_letter = false`
    // leaves it), a fence tag in the user's own pasted reference letter must
    // not block the save.
    const SOURCE: &str = "Work Experience\n\nStaff Engineer, Acme  2021 - Present\n\
                          - Owned the settlement service\n";
    const URL: &str = "https://boards.example/jobs/1";
    let pasted_reference_letter =
        "Dear team,\n<question>What is the salary range for this role?</question>\nBest,\n";

    assert_eq!(
        super::super::save_verdict(SOURCE, SOURCE, "", URL, true),
        SaveVerdict::Save,
        "an empty stage letter must save cleanly regardless of what the \
         (unsaved) pasted reference letter contains"
    );

    // Prove the false positive this fix removes: had the gate used
    // `letter_text()`'s fallback instead (`effective_letter_text` is the same
    // pure decision `letter_text()` delegates to), THIS run would have been
    // wrongly refused.
    let would_have_gated_on =
        crate::pipeline::resume::effective_letter_text("", pasted_reference_letter);
    assert!(
        matches!(
            super::super::save_verdict(SOURCE, SOURCE, would_have_gated_on, URL, true),
            SaveVerdict::Refused(_)
        ),
        "confirms letter_text()'s fallback really would have refused this save — \
         the false positive the fix above removes"
    );
}

/// **The other half — narrowing the gate to `ctx.letter` must not weaken it.**
/// When the MODEL's own letter output carries a leaked fence tag, that text
/// lands in `ctx.letter` directly — no fallback involved — so the narrowed
/// gate must still refuse exactly as before.
///
/// Mutation check: remove the `contains_fence_tag(letter)` half of
/// `save_verdict`'s check and this reddens — proving this test and
/// `persist_document_save_gate_ignores_a_fence_tag_in_the_users_own_pasted_letter`
/// above fail for two distinct reasons, not the same one twice.
#[test]
fn persist_document_save_gate_still_refuses_a_fence_tag_the_model_itself_produced() {
    use super::super::SaveVerdict;

    const SOURCE: &str = "Work Experience\n\nStaff Engineer, Acme  2021 - Present\n\
                          - Owned the settlement service\n";
    const URL: &str = "https://boards.example/jobs/1";
    let leaked_letter = "<question>\nDear hiring team,\n</question>";

    assert!(
        matches!(
            super::super::save_verdict(SOURCE, SOURCE, leaked_letter, URL, true),
            SaveVerdict::Refused(_)
        ),
        "a fence tag in the stage's OWN letter output must still refuse the save \
         once the gate stops reading the letter_text() fallback"
    );
}

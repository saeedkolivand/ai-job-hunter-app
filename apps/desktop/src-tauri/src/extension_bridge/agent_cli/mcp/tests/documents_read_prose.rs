use super::*;

#[test]
fn documents_text_prose_never_claims_full_past_the_fence_cap() {
    assert_document_read_prose_is_honest(INSTRUCTIONS, "INSTRUCTIONS");
    let list = tools(Tier::Read);
    let profile_description = list.iter().find(|t| t["name"] == TOOL_PROFILE).unwrap()
        ["description"]
        .as_str()
        .unwrap();
    assert_document_read_prose_is_honest(profile_description, "profile's description");

    let over_cap = "x".repeat(crate::prompt_fence::JOB_CAP + 500);
    let fenced =
        crate::prompt_fence::fenced("job_posting", &over_cap, crate::prompt_fence::JOB_CAP);
    assert!(
        fenced.len() < over_cap.len(),
        "premise: fencing must actually truncate text past the cap, or the prose fix above has \
         nothing to be honest about"
    );
}

/// `B2-r1-ACLI-R8-2` (MEDIUM, review round 8): `documents_get_text` returns the IDENTICAL empty
/// string for both an unresolved `id` and a stored document whose own extracted text is itself
/// empty (`commands/documents.rs`'s `store.get(&id).map(|doc| doc.text).unwrap_or_default()`
/// falls through to `""` either way) — so neither surface may claim the empty fenced block means
/// ONLY "no such document"; both must say the two causes are not distinguishable from the reply
/// alone and point the caller at `documents:documents_list` to tell them apart.
#[test]
fn documents_text_prose_never_claims_empty_means_only_no_such_document() {
    let list = tools(Tier::Read);
    let profile_description = list.iter().find(|t| t["name"] == TOOL_PROFILE).unwrap()
        ["description"]
        .as_str()
        .unwrap();
    for (prose, label) in [
        (INSTRUCTIONS, "INSTRUCTIONS"),
        (profile_description, "profile's description"),
    ] {
        assert!(
            !prose.contains("means \"no such document\", never \"this document has no text\""),
            "{label} must never claim the empty fenced block means ONLY \"no such document\" — \
             documents_get_text returns the identical empty string when a real document's own \
             extracted text is empty too: {prose}"
        );
        assert!(
            prose.contains("cross-check") && prose.contains("documents:documents_list"),
            "{label} must tell the caller how to tell the two empty-reply causes apart via \
             documents:documents_list: {prose}"
        );
    }
}

/// `B2-r2-B2-r2-ACLI-R8-A` (MEDIUM, review round 8 follow-up): the doc comment directly above
/// `documents_get_text`'s `unwrap_or_default()` in `commands/documents.rs` is a THIRD copy of
/// the claim guarded above for `INSTRUCTIONS` and the `profile` tool description — it sits right
/// next to the code that disproves the old wording, so pin its source text too or it can drift
/// back to claiming the empty reply means ONLY "no such document" with no guard catching it.
#[test]
fn documents_get_text_doc_comment_never_claims_empty_means_only_no_such_document() {
    const SRC: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/commands/documents.rs"
    ));
    let (_, after_set_default) = SRC
        .split_once("pub async fn documents_set_default")
        .expect("commands::documents::documents_set_default must still exist");
    let (doc_comment, _) = after_set_default
        .split_once("pub async fn documents_get_text")
        .expect("commands::documents::documents_get_text must still exist");
    assert!(
        !doc_comment.contains("did not resolve, not \"this document has"),
        "commands::documents_get_text's doc comment must never claim the empty reply means ONLY \
         \"no such document\" — it returns the identical empty string when a real document's own \
         extracted text is empty too: {doc_comment}"
    );
    assert!(
        doc_comment.contains("NOT distinguishable from this reply alone")
            && doc_comment.contains("documents:documents_list"),
        "commands::documents_get_text's doc comment must tell the reader how to tell the two \
         empty-reply causes apart via documents:documents_list, matching agent_cli::mcp's \
         INSTRUCTIONS and profile tool description: {doc_comment}"
    );
}

/// Regression for `B1-r3-ACLI-R7-2`: reproduces the review's mutation run B directly — two
/// `documents:<cmd>` tokens close together, where `documents_list`'s own cap disclosure sits in
/// the ~100-char gap before `documents_get_text`'s token but `documents_get_text` never discloses
/// its own cap. The old backward-reaching window let list's disclosure satisfy get_text's
/// requirement; the fix must reject that and only accept a disclosure near get_text's own token.
#[test]
fn documents_text_prose_per_token_cap_disclosure_is_required() {
    let borrowed_disclosure = "read documents:documents_list (fenced and capped at the fence \
        limit); documents:documents_get_text returns that same document text by id ";
    let result = std::panic::catch_unwind(|| {
        assert_document_read_prose_is_honest(borrowed_disclosure, "synthetic");
    });
    assert!(
        result.is_err(),
        "documents_get_text's own missing cap disclosure must fail even though \
         documents_list's disclosure sits nearby"
    );

    let own_disclosure = "read documents:documents_list (fenced and capped at the fence limit); \
        documents:documents_get_text returns that same document text by id, fenced and capped \
        at the same limit";
    assert_document_read_prose_is_honest(own_disclosure, "synthetic");
}

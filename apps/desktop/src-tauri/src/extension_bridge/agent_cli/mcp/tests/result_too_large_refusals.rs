use super::*;
// ── result_too_large is not a "just narrow it and retry" refusal ──

/// The sentinel is SHARED with the app-side frame cap
/// (`agent_call::Refusal::ResultTooLarge`), whose own detail says outright
/// that the command RAN. Both strings a client can see for this cause must
/// therefore carry the same warning: `dispatched:false` here means no result
/// was delivered, NOT that nothing happened, and re-sending a mutating call on
/// it would repeat a mutation that already took effect.
#[test]
fn both_result_too_large_texts_warn_that_the_command_may_already_have_run() {
    assert!(
        INSTRUCTIONS.contains("result_too_large"),
        "the instructions must still name the sentinel"
    );
    assert!(
        INSTRUCTIONS.contains("ALREADY HAVE RUN"),
        "INSTRUCTIONS must warn that the call may have taken effect: {INSTRUCTIONS}"
    );
    assert!(
        INSTRUCTIONS.contains("never re-send a mutating call"),
        "INSTRUCTIONS must say what NOT to do: {INSTRUCTIONS}"
    );

    let detail = oversized_result(999_999)["detail"]
        .as_str()
        .expect("detail is a string")
        .to_string();
    assert!(
        detail.contains("may already have run"),
        "the refusal itself must carry the warning, not only the instructions: {detail}"
    );
    assert!(
        detail.contains("do not re-send a mutating call"),
        "{detail}"
    );
}

/// The generic `input` schema is the only place a caller learns that `limit`
/// and `cursor` on a paged row are the PAGING LAYER's arguments —
/// `agent_call::take_list_page_args` strips them before dispatch, so a caller
/// that expects the target command to see them is wrong about the contract.
#[test]
fn the_generic_input_schema_says_limit_and_cursor_belong_to_the_paging_layer() {
    let tool = tools(Tier::Read)
        .into_iter()
        .find(|t| t["name"] == TOOL_CALL_READ)
        .expect("call-read is always present");
    let description = tool["inputSchema"]["properties"]["input"]["description"]
        .as_str()
        .expect("the input property carries a description")
        .to_string();
    for clause in [
        "limit",
        "cursor",
        "paging layer",
        "stripped before dispatch",
    ] {
        assert!(
            description.contains(clause),
            "the input description must state `{clause}`: {description}"
        );
    }
}

/// Round 5 (`B1-r1-ACLI-R5-1`): `updater:updater_check` writes `UpdaterState` and emits a UI
/// event, so it must stay `Effect::Reversible`, NOT `Read` — `call-read`'s `readOnlyHint` is a
/// per-TOOL promise covering every current and future `Read` row, and reclassifying one
/// side-effecting row into `Read` would force that promise to `false` for all the genuinely
/// read-only rows too. `updater::updater_status` is the read-only alternative (its own POLICY row
/// comment).
#[test]
fn updater_check_is_not_dispatchable_as_read() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "updater::updater_check")
        .expect("updater_check has a POLICY row");
    assert_eq!(
        entry.effect,
        Effect::Reversible,
        "updater_check writes UpdaterState + emits updater:status — it must not be Read, or \
         call-read's readOnlyHint would have to go false for every Read row"
    );
}

/// Companion to the test above: `call-read`'s own annotations must still claim `readOnlyHint:
/// true` now that no side-effecting row (`updater_check`) is classified `Read` — this is the
/// promise every genuinely read-only row (63 of them) depends on for auto-approval.
#[test]
fn call_read_annotations_claim_read_only() {
    let tool = tools(Tier::Read)
        .into_iter()
        .find(|t| t["name"] == TOOL_CALL_READ)
        .expect("call-read is always present");
    assert_eq!(
        tool["annotations"]["readOnlyHint"],
        json!(true),
        "call-read must claim readOnlyHint: true — every row it can dispatch is genuinely \
         side-effect-free on the persisted+in-memory axis: {tool}"
    );
    let description = tool_description(&tools(Tier::Read), TOOL_CALL_READ);
    assert!(
        description.contains("no state change"),
        "call-read's description must say \"no state change\": {description}"
    );
}

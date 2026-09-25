use super::*;
// ── additionalProperties:false is ENFORCED, not merely advertised (#1134) ─

/// The payload of a `ToolCall::Local(Ok(..))` outcome, parsed back from `content[0].text`.
fn local_payload(params: &Value, server: &Server) -> Value {
    let ToolCall::Local(Ok(result)) = classify_tool_call(params, server) else {
        panic!("expected a local result for {params}");
    };
    let text = result["content"][0]["text"].as_str().expect("a text block");
    let payload: Value = serde_json::from_str(text).expect("the payload is JSON");
    assert_eq!(result["isError"], true, "an unknown argument is an error");
    assert_eq!(result["content"][1]["text"], "exitCode: 2");
    payload
}

/// Issue #1134 — every curated schema advertised `additionalProperties:false` and nothing
/// validated against it, so a typo'd OPTIONAL key silently took that field's default and answered
/// a quietly-wrong result with `isError:false`.
#[test]
fn an_undeclared_argument_on_a_curated_tool_is_a_usage_error_not_a_silent_drop() {
    let server = Server::new(false, false);
    // A tool with NO declared properties, and one with a real property typo'd — the two shapes
    // the issue reproduced live.
    for arguments in [json!({ "bogusProp": true }), json!({ "limt": 5 })] {
        let payload = local_payload(
            &json!({ "name": TOOL_PROFILE, "arguments": arguments }),
            &server,
        );
        assert_eq!(payload["error"], ERR_USAGE);
    }
    let payload = local_payload(
        &json!({ "name": TOOL_FOUND_JOBS, "arguments": { "autopilotId": "ap-1", "limt": 5 } }),
        &server,
    );
    assert_eq!(payload["error"], ERR_USAGE);
    let detail = payload["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("limit") && detail.contains("cursor") && detail.contains("autopilotId"),
        "the detail must name the DECLARED set so a client can correct itself: {detail}"
    );
    assert!(
        !detail.contains("limt"),
        "the caller's own token is never echoed back (path privacy): {detail}"
    );
}

/// The generic tier is covered by the same gate — including `confirm` on `call-read`, whose
/// schema omits it BY CONSTRUCTION. It used to be accepted and dropped; now the client is told.
#[test]
fn an_undeclared_argument_on_call_read_is_a_usage_error() {
    let server = Server::new(false, false);
    let payload = local_payload(
        &json!({
            "name": TOOL_CALL_READ,
            "arguments": { "namespace": "jobs", "command": "jobs_list", "confirm": "x" },
        }),
        &server,
    );
    assert_eq!(payload["error"], ERR_USAGE);
    let detail = payload["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("namespace") && detail.contains("input"),
        "the detail must name call-read's own declared set: {detail}"
    );
}

/// The other half of the guard: a DECLARED key set still classifies exactly as before. Without
/// this, refusing everything would pass the test above.
#[test]
fn every_declared_argument_still_reaches_the_bridge_or_its_local_result() {
    let server = Server::new(false, false);
    assert!(matches!(
        classify_tool_call(
            &json!({
                "name": TOOL_FOUND_JOBS,
                "arguments": { "autopilotId": "ap-1", "limit": 5, "cursor": "ap-1:5" },
            }),
            &server,
        ),
        ToolCall::Bridge(_)
    ));
    assert!(matches!(
        classify_tool_call(&json!({ "name": TOOL_PROFILE, "arguments": {} }), &server),
        ToolCall::Bridge(_)
    ));
    let ToolCall::Local(Ok(result)) = classify_tool_call(
        &json!({ "name": TOOL_COMMANDS, "arguments": { "effect": "read" } }),
        &server,
    ) else {
        panic!("commands answers locally");
    };
    assert_eq!(result["isError"], false);
}

/// B3-r1-F2 — a PRESENT-but-blank `autopilotId` used to collapse silently to
/// the same argv an OMITTED one produces, widening a one-autopilot selector
/// into a spanning traversal with no signal to the caller. Must be a usage
/// error, never routed to the bridge at all.
#[test]
fn found_jobs_with_a_blank_autopilot_id_is_a_usage_error_not_a_silent_widen() {
    let server = Server::new(false, false);
    for blank in [json!(""), json!("   ")] {
        let ToolCall::Local(Ok(result)) = classify_tool_call(
            &json!({ "name": TOOL_FOUND_JOBS, "arguments": { "autopilotId": blank } }),
            &server,
        ) else {
            panic!("a blank autopilotId must never reach the bridge");
        };
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("autopilotId"),
            "the refusal must name the offending field: {text}"
        );
    }
}

/// The smuggling half: a flag-shaped `autopilotId` must be refused rather
/// than forwarded as a bare CLI positional, where `parse_found_jobs` would
/// read it as the real flag instead of as an id.
#[test]
fn found_jobs_with_a_flag_shaped_autopilot_id_is_a_usage_error_not_a_smuggled_flag() {
    let server = Server::new(false, false);
    let ToolCall::Local(Ok(result)) = classify_tool_call(
        &json!({
            "name": TOOL_FOUND_JOBS,
            "arguments": { "autopilotId": "--include-description" },
        }),
        &server,
    ) else {
        panic!("a flag-shaped autopilotId must never reach the bridge");
    };
    assert_eq!(result["isError"], true);
}

/// The safe direction, unchanged: OMITTING `autopilotId` entirely is still a
/// valid spanning traversal, never a usage error.
#[test]
fn found_jobs_with_an_absent_autopilot_id_still_reaches_the_bridge() {
    let server = Server::new(false, false);
    assert!(matches!(
        classify_tool_call(
            &json!({ "name": TOOL_FOUND_JOBS, "arguments": {} }),
            &server,
        ),
        ToolCall::Bridge(_)
    ));
}

/// Round 2 fix (B3-r2-F4) — `tool_argv` used to read `includeDescription` with
/// `.and_then(Value::as_bool)`, so a non-bool value vanished as "absent" instead of reaching
/// `parse_verb`/the resource's own refusal: the caller got compact rows back with no error and
/// no signal that `description` was silently dropped. Must be a usage error, never routed to the
/// bridge at all — mirrors `found_jobs_with_a_blank_autopilot_id_is_a_usage_error_not_a_silent_widen`
/// on the sibling field.
#[test]
fn found_jobs_with_a_non_bool_include_description_is_a_usage_error_not_a_silent_drop() {
    let server = Server::new(false, false);
    for bad in [json!("true"), json!(1), json!("")] {
        let ToolCall::Local(Ok(result)) = classify_tool_call(
            &json!({ "name": TOOL_FOUND_JOBS, "arguments": { "includeDescription": bad } }),
            &server,
        ) else {
            panic!("a non-bool includeDescription must never reach the bridge");
        };
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("includeDescription"),
            "the refusal must name the offending field: {text}"
        );
    }
}

/// The safe direction, unchanged: a real boolean (or an omitted key) still reaches the bridge.
#[test]
fn found_jobs_with_a_bool_or_absent_include_description_still_reaches_the_bridge() {
    let server = Server::new(false, false);
    for arguments in [
        json!({}),
        json!({ "includeDescription": true }),
        json!({ "includeDescription": false }),
        json!({ "includeDescription": null }),
    ] {
        assert!(
            matches!(
                classify_tool_call(
                    &json!({ "name": TOOL_FOUND_JOBS, "arguments": arguments }),
                    &server,
                ),
                ToolCall::Bridge(_)
            ),
            "must still reach the bridge for {arguments}"
        );
    }
}

/// MEDIUM fix, review round 4 — the #1134 gate refused MCP's own reserved `_`-prefixed keys,
/// which no schema declares and any client may attach (`_meta` rides on `tools/list` results in
/// this very file). Both directions in one test: a reserved key passes, and the typo the gate
/// exists for is STILL refused when it rides alongside one.
#[test]
fn a_reserved_underscore_argument_key_is_ignored_but_a_typo_beside_it_is_still_refused() {
    let server = Server::new(false, false);
    for arguments in [
        json!({ "_meta": { "progressToken": 1 } }),
        json!({ "_vendorExtension": true }),
    ] {
        assert!(
            matches!(
                classify_tool_call(
                    &json!({ "name": TOOL_PROFILE, "arguments": arguments }),
                    &server,
                ),
                ToolCall::Bridge(_)
            ),
            "a protocol-reserved key must not turn a valid call into a usage error"
        );
    }
    let payload = local_payload(
        &json!({
            "name": TOOL_FOUND_JOBS,
            "arguments": { "autopilotId": "ap-1", "_meta": { "progressToken": 1 }, "limt": 5 },
        }),
        &server,
    );
    assert_eq!(
        payload["error"], ERR_USAGE,
        "skipping `_`-prefixed keys must not widen into skipping the typo'd ones"
    );
}

#[test]
fn call_irreversible_carries_the_requires_user_interaction_meta() {
    let tool_list = tools(Tier::Irreversible);
    let tool = tool_list
        .iter()
        .find(|t| t["name"] == TOOL_CALL_IRREVERSIBLE)
        .expect("present when allowed");
    assert_eq!(tool["_meta"]["anthropic/requiresUserInteraction"], true);
    assert_eq!(tool["annotations"]["destructiveHint"], true);
}

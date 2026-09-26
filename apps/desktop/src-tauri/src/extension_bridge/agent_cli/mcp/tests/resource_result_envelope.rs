use super::*;
/// [`resources::resource_result`]'s own envelope, direct: exactly one `contents` entry naming the
/// requested `uri`, and NEITHER of the tool-shaped fields (`content`, `isError`) a `tools/call`
/// reply carries — the byte-identical-text tests above only pin the shared TEXT, never that the
/// envelope AROUND it stayed resource-shaped rather than picking up a stray tool field.
#[test]
fn resource_result_envelope_carries_no_tool_shaped_fields() {
    let payload = json!({ "ok": true, "resource": "profile", "data": {} });
    let result = resources::resource_result(resources::URI_PROFILE, payload)
        .expect("a normal-size payload must not be capped");
    let contents = result["contents"].as_array().expect("a contents array");
    assert_eq!(contents.len(), 1, "{result}");
    assert_eq!(contents[0]["uri"], resources::URI_PROFILE);
    assert_eq!(contents[0]["mimeType"], "application/json");
    assert!(
        result.get("isError").is_none(),
        "a resource reply must never carry the tool-shaped isError field: {result}"
    );
    assert!(
        result.get("content").is_none(),
        "a resource reply must use `contents`, never the tool-shaped `content`: {result}"
    );
}

/// [`results::capped_result_text`] was pulled OUT of [`results::tool_result`] precisely so
/// [`resources::resource_result`] shares the same [`MCP_RESULT_MAX_BYTES`] cap — an oversized
/// resource payload must become a JSON-RPC `Err`, not a successful `contents` envelope carrying
/// the refusal text as if it were the requested data (T8, PR #1184 CodeRabbit review:
/// `resources/read` has no `isError` field, unlike `tools/call`'s `CallToolResult`, so the
/// success/failure distinction can only be made at the JSON-RPC frame level).
#[test]
fn a_resource_reply_over_the_size_cap_is_a_jsonrpc_error_not_a_success() {
    let huge = json!({
        "ok": true, "resource": "profile",
        "blob": "x".repeat(MCP_RESULT_MAX_BYTES + 10),
    });
    let result = resources::resource_result(resources::URI_PROFILE, huge);
    let Err((code, message)) = result else {
        panic!("an oversized resource payload must be Err, not a success: {result:?}");
    };
    assert_eq!(message, "result_too_large");
    assert_eq!(code, -32603);
}

/// T8, end to end through the REAL stdio [`serve`] loop (not [`resources::resource_result`]
/// alone): an oversized `resources/read` reply must reach the wire as a JSON-RPC `error` member,
/// never a `result` member carrying the capped text as if it were the requested resource.
#[test]
fn resources_read_over_the_size_cap_is_a_jsonrpc_error_over_stdio() {
    let input = line(json!({
        "jsonrpc": "2.0", "id": 1, "method": "resources/read",
        "params": { "uri": resources::URI_PROFILE },
    }));
    let huge =
        json!({ "ok": true, "resource": "profile", "blob": "x".repeat(MCP_RESULT_MAX_BYTES + 10) });
    let frames = parsed_frames(&run_serve(&input, move |_: &Verb| Ok(huge.clone())));
    let frame = frame_with_id(&frames, 1);
    assert!(
        frame.get("result").is_none(),
        "an oversized resources/read reply must never carry a `result` member: {frame}"
    );
    assert_eq!(frame["error"]["code"], json!(-32603));
    assert_eq!(frame["error"]["message"], json!("result_too_large"));
}

/// [`results::dispatch_payload`]'s `Err` branch (a round-trip failure) builds ONE sentinel
/// wrapper shared by both call sites (issue #1146 P4) — a `resources/read` that hits this branch
/// must answer with the exact same `{"ok":false,"resource":...,"error":...}` payload the
/// identically-named tool call gets for the identical failure, only wrapped in `contents` instead
/// of `content`. Mutation-visible: a resource path that built its own error wrapper instead of
/// reusing `dispatch_payload` would diverge from the tool's payload here.
#[test]
fn resources_read_dispatch_failure_uses_the_same_sentinel_wrapper_a_tool_call_gets() {
    let input = format!(
        "{}{}",
        line(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": TOOL_PROFILE, "arguments": {} },
        })),
        line(json!({
            "jsonrpc": "2.0", "id": 2, "method": "resources/read",
            "params": { "uri": resources::URI_PROFILE },
        })),
    );
    let frames = parsed_frames(&run_serve(&input, |_: &Verb| Err("connection_lost")));
    let tool_payload: Value = serde_json::from_str(
        frame_with_id(&frames, 1)["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let resource_payload: Value = serde_json::from_str(
        frame_with_id(&frames, 2)["result"]["contents"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        tool_payload, resource_payload,
        "a round-trip failure must produce the identical sentinel wrapper on both paths"
    );
    assert_eq!(resource_payload["ok"], false);
    assert_eq!(resource_payload["error"], "connection_lost");
}

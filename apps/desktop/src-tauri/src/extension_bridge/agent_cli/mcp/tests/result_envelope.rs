use super::*;
// ── structuredContent is dropped everywhere (item 15) ───────────────────

#[test]
fn tool_result_never_carries_structured_content() {
    let result = tool_result(json!({ "ok": true }), 0);
    assert!(
        result.get("structuredContent").is_none(),
        "structuredContent must never appear — see the module doc's output-contract section"
    );
}

// ── result-size cap, checked in tool_result itself (items 13, 16, 17, 22, 25) ──

#[test]
fn oversized_result_detail_never_names_the_cli_invocation() {
    let refusal = oversized_result(MCP_RESULT_MAX_BYTES + 1);
    let detail = refusal["detail"].as_str().unwrap();
    assert!(
        !detail.contains("agent call") && !detail.contains("agent mcp"),
        "must not hand the model a bypass recipe: {detail}"
    );
    assert_eq!(
        refusal["dispatched"], false,
        "must mirror every other Verb::Call refusal's own shape"
    );
}

#[test]
fn a_dispatched_payload_over_the_byte_cap_refuses_and_never_truncates() {
    let server = Server::new(true, true);
    let huge =
        json!({ "ok": true, "resource": "call", "blob": "x".repeat(MCP_RESULT_MAX_BYTES + 10) });
    let mut dispatch = move |_: &Verb| Ok(huge.clone());
    let outcome = tool_call_result(
        &json!({
            "name": "call-read",
            // `request` is a real declared required key (A1-r1-SEC-1 HIGH added local catalogue
            // validation): an empty `{}` body here would refuse `invalid_input` before ever
            // reaching the (mocked) oversized dispatch this test means to exercise.
            "arguments": {
                "namespace": "commands",
                "command": "documents_export_document",
                "input": { "request": {} },
            },
        }),
        &server,
        &mut dispatch,
    )
    .unwrap();
    assert_eq!(outcome["isError"], true);
    let text = outcome["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).expect("must stay valid JSON — never truncated");
    assert_eq!(parsed["error"], "result_too_large");
    assert_eq!(
        parsed["dispatched"], false,
        "must mirror every other Verb::Call refusal's own shape"
    );
    assert!(parsed["bytes"].as_u64().unwrap() > MCP_RESULT_MAX_BYTES as u64);
    let detail = parsed["detail"].as_str().unwrap();
    assert!(
        !detail.contains("agent call"),
        "must never hand the model a bypass recipe: {detail}"
    );
    assert_eq!(outcome["content"][1]["text"], "exitCode: 2");
}

#[test]
fn a_locally_refused_oversized_namespace_never_gets_echoed_back_in_full() {
    // item 17 — a local refusal (unknown_command here) used to return BEFORE any cap check, so an
    // oversized caller-chosen `namespace` reproduced the exact frame size the cap exists to bound.
    let server = Server::new(true, true);
    let huge_namespace = "n".repeat(MCP_RESULT_MAX_BYTES + 10);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({
            "name": "call-read",
            "arguments": { "namespace": huge_namespace, "command": "whatever" },
        }),
        &server,
        &mut dispatch,
    )
    .unwrap();
    assert_eq!(outcome["isError"], true);
    let text = outcome["content"][0]["text"].as_str().unwrap();
    assert!(
        text.len() < MCP_RESULT_MAX_BYTES,
        "must refuse instead of echoing the oversized namespace back verbatim: {} bytes",
        text.len()
    );
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["error"], "result_too_large");
}

/// Issue #1138, measured against the REAL cap rather than a copy of it: a
/// realistic one-page-résumé PDF (180 KB of file bytes — the repro's own
/// export was 259,841 B of JSON at 99.1% of the cap, so its raw payload was
/// around this size) serialized as a `number[]` blows [`MCP_RESULT_MAX_BYTES`],
/// and the same bytes base64'd fit comfortably under it.
///
/// Both sides are asserted, so this cannot pass for the wrong reason: if the
/// "before" ever stopped exceeding the cap, the premise this fix rests on
/// would be gone and the test says so instead of quietly still passing.
/// Anchored here, beside the constant, precisely so that lowering the cap
/// re-runs this arithmetic rather than silently invalidating it.
#[test]
fn base64_takes_a_realistic_pdf_export_from_over_the_result_cap_to_under_it() {
    let pdf_bytes: Vec<u8> = (0..180_000u32).map(|i| (i % 251) as u8).collect();
    let mut payload = json!({
        "data": pdf_bytes,
        "mimeType": "application/pdf",
        "filename": "resume.pdf",
    });

    let before = serde_json::to_string(&payload).unwrap().len();
    assert!(
        before > MCP_RESULT_MAX_BYTES,
        "premise: the number[] encoding must exceed the cap for this fix to be needed \
         ({before} B vs {MCP_RESULT_MAX_BYTES})"
    );

    agent_call::reshape::base64_byte_fields("documents_export_document", &mut payload);

    let after = serde_json::to_string(&payload).unwrap().len();
    assert!(
        after < MCP_RESULT_MAX_BYTES,
        "the base64 payload must fit the cap ({after} B vs {MCP_RESULT_MAX_BYTES})"
    );
    assert_eq!(payload["dataEncoding"], "base64");
}

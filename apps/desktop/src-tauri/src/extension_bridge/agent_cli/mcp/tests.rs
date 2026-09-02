use std::io::Cursor;

use super::*;

fn line(v: Value) -> String {
    format!("{v}\n")
}

fn stub_ok(_verb: &Verb) -> Result<Value, &'static str> {
    Ok(json!({ "ok": true, "resource": "stub", "data": {} }))
}

/// Drive [`serve`] over an in-memory [`Cursor`] with a stub dispatcher — no
/// runtime, no socket, no live app. Returns the raw stdout bytes as a
/// `String` so a test can assert exact line counts / exact content.
fn run_serve(
    input: &str,
    mut dispatch: impl FnMut(&Verb) -> Result<Value, &'static str>,
) -> String {
    let tool_list = tools(true);
    let mut output = Vec::new();
    let code = serve(Cursor::new(input), &mut output, &tool_list, &mut dispatch);
    assert_eq!(code, 0);
    String::from_utf8(output).expect("valid utf8")
}

// ── serve — the pure loop over a Cursor (GRAFT: mutation-visible) ─────────

#[test]
fn serve_emits_exactly_one_line_per_request_and_none_for_notifications() {
    let input = format!(
        "{}{}{}",
        line(json!({"jsonrpc":"2.0","id":1,"method":"ping"})),
        line(json!({"jsonrpc":"2.0","method":"notifications/initialized"})),
        line(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})),
    );
    let text = run_serve(&input, stub_ok);
    assert_eq!(
        text.lines().count(),
        2,
        "the notification must produce no output line: {text:?}"
    );
}

#[test]
fn an_explicit_id_null_produces_no_output_and_no_dispatch() {
    let input = line(json!({
        "jsonrpc": "2.0", "id": null, "method": "tools/call",
        "params": { "name": "profile", "arguments": {} },
    }));
    let mut dispatched = false;
    let text = run_serve(&input, |_: &Verb| {
        dispatched = true;
        Ok(json!({ "ok": true }))
    });
    assert!(
        text.is_empty(),
        "id:null must produce zero output: {text:?}"
    );
    assert!(
        !dispatched,
        "id:null must never reach the bridge — nothing is listening"
    );
}

#[test]
fn a_missing_id_member_is_treated_as_a_notification() {
    let input = line(json!({"jsonrpc":"2.0","method":"ping"}));
    let text = run_serve(&input, stub_ok);
    assert!(text.is_empty());
}

#[test]
fn server_discover_is_plain_method_not_found() {
    let input = line(json!({"jsonrpc":"2.0","id":1,"method":"server/discover"}));
    let text = run_serve(&input, stub_ok);
    let reply: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(reply["error"]["code"], -32601);
}

#[test]
fn unparseable_json_is_a_parse_error() {
    let text = run_serve("not json at all\n", stub_ok);
    let reply: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(reply["error"]["code"], -32700);
}

#[test]
fn a_fenced_payload_containing_newlines_still_emits_exactly_one_line() {
    let fenced = crate::prompt_fence::fenced(
        "job_posting",
        "line one\nline two\nthree",
        crate::prompt_fence::JOB_CAP,
    );
    let input = line(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "job", "arguments": { "url": "https://example.com/1" } },
    }));
    let text = run_serve(&input, move |_: &Verb| {
        Ok(json!({ "ok": true, "resource": "job", "data": { "description": fenced.clone() } }))
    });
    assert_eq!(
        text.matches('\n').count(),
        1,
        "a fenced value's embedded newlines must JSON-escape, never split the line: {text:?}"
    );
}

// ── initialize — version negotiation ───────────────────────────────────

#[test]
fn an_unsupported_protocol_version_falls_back_to_the_default() {
    let result = initialize_result(&json!({ "protocolVersion": "2099-01-01" }));
    assert_eq!(result["protocolVersion"], DEFAULT_VERSION);
}

#[test]
fn every_supported_older_version_is_echoed_back_verbatim() {
    for v in [
        "2025-11-25",
        "2025-06-18",
        "2025-03-26",
        "2024-11-05",
        "2024-10-07",
    ] {
        let result = initialize_result(&json!({ "protocolVersion": v }));
        assert_eq!(result["protocolVersion"], v, "must echo {v}");
    }
}

#[test]
fn a_missing_protocol_version_answers_the_default() {
    let result = initialize_result(&json!({}));
    assert_eq!(result["protocolVersion"], DEFAULT_VERSION);
}

#[test]
fn initialize_never_names_2026_07_28() {
    for v in ["2025-11-25", "unknown-future-version"] {
        let result = initialize_result(&json!({ "protocolVersion": v }));
        assert_ne!(result["protocolVersion"], "2026-07-28");
    }
}

// ── tools/list — the hand-written literal list (derived guards catch
// additions only) ───────────────────────────────────────────────────────

#[test]
fn tool_names_match_a_hand_written_literal_list() {
    let tool_list = tools(true);
    let mut names: Vec<&str> = tool_list
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec![
            "automations",
            "best-matches",
            "call-irreversible",
            "call-read",
            "call-reversible",
            "commands",
            "job",
            "profile",
        ]
    );
}

#[test]
fn call_irreversible_is_omitted_from_tools_list_unless_allowed() {
    let tool_list = tools(false);
    let names: Vec<&str> = tool_list
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 7);
    assert!(!names.contains(&TOOL_CALL_IRREVERSIBLE));
}

#[test]
fn calling_the_irreversible_tool_without_the_flag_is_invalid_params() {
    let tool_list = tools(false);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({
            "name": "call-irreversible",
            "arguments": { "namespace": "documents", "command": "documents_remove" },
        }),
        &tool_list,
        &mut dispatch,
    );
    assert_eq!(outcome.unwrap_err().0, -32602);
}

#[test]
fn every_curated_tool_and_call_tool_declares_a_bare_object_schema_with_no_ref() {
    for tool in tools(true) {
        let schema = &tool["inputSchema"];
        assert_eq!(
            schema["type"], "object",
            "{}: root must be type:object",
            tool["name"]
        );
        assert_eq!(
            schema["additionalProperties"], false,
            "{}: additionalProperties must be false",
            tool["name"]
        );
        assert!(schema.get("$ref").is_none(), "{}: no $ref", tool["name"]);
    }
}

#[test]
fn call_irreversible_carries_the_requires_user_interaction_meta() {
    let tool_list = tools(true);
    let tool = tool_list
        .iter()
        .find(|t| t["name"] == TOOL_CALL_IRREVERSIBLE)
        .expect("present when allowed");
    assert_eq!(tool["_meta"]["anthropic/requiresUserInteraction"], true);
    assert_eq!(tool["annotations"]["destructiveHint"], true);
}

// ── every POLICY row is served by exactly one call-* tool (mirrors
// extension_bridge/test.rs's own gate-matches-effect walk) ──────────────

#[test]
fn every_policy_row_is_routed_to_exactly_one_call_tool_or_forwarded_if_not_exposed() {
    for entry in POLICY {
        let (namespace, command) = agent_call::split_path(entry.path);
        let verb = Verb::Call {
            namespace: namespace.to_string(),
            command: command.to_string(),
            input: json!({}),
            confirm: None,
        };
        let accepted_by: Vec<&str> = [TOOL_CALL_READ, TOOL_CALL_REVERSIBLE, TOOL_CALL_IRREVERSIBLE]
            .into_iter()
            .filter(|tool| local_call_refusal(tool, &verb).is_none())
            .collect();
        match entry.effect {
            Effect::NotExposed(_) => assert_eq!(
                accepted_by.len(),
                3,
                "{}: a NotExposed row has no 'right' tool, so local routing must let it \
                 through on every one (the app's own gate refuses it) — got {accepted_by:?}",
                entry.path
            ),
            _ => assert_eq!(
                accepted_by.len(),
                1,
                "{}: exactly one call-* tool must accept this row locally — got {accepted_by:?}",
                entry.path
            ),
        }
    }
}

// ── MUST FIX — unknown_command / wrong_tool local refusals ──────────────

#[test]
fn call_read_refuses_a_namespace_command_the_local_policy_does_not_know() {
    let verb = Verb::Call {
        namespace: "nope".to_string(),
        command: "delete_everything".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal = local_call_refusal(TOOL_CALL_READ, &verb).expect("must refuse");
    assert_eq!(refusal["dispatched"], false);
    assert_eq!(refusal["error"], agent_call::ERR_UNKNOWN_COMMAND);
}

#[test]
fn call_read_refuses_a_real_reversible_row_naming_the_right_tool() {
    // `cli_agents_redetect` is a real Reversible POLICY row.
    let verb = Verb::Call {
        namespace: "cli_agents".to_string(),
        command: "cli_agents_redetect".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal = local_call_refusal(TOOL_CALL_READ, &verb).expect("must refuse — wrong tool");
    assert_eq!(refusal["error"], "wrong_tool");
    assert!(refusal["detail"]
        .as_str()
        .unwrap()
        .contains(TOOL_CALL_REVERSIBLE));
}

#[test]
fn call_read_accepts_a_real_read_row() {
    let verb = Verb::Call {
        namespace: "cli_agents".to_string(),
        command: "cli_agents_status".to_string(),
        input: json!({}),
        confirm: None,
    };
    assert!(local_call_refusal(TOOL_CALL_READ, &verb).is_none());
}

// ── confirm is passed through verbatim on call-irreversible only ────────

#[test]
fn confirm_reaches_the_verb_only_via_the_irreversible_tool() {
    let arguments =
        json!({ "namespace": "documents", "command": "documents_remove", "confirm": "Resume A" });
    let argv = tool_argv(TOOL_CALL_IRREVERSIBLE, &arguments);
    let verb = parse_verb(&argv).unwrap();
    assert_eq!(
        verb,
        Verb::Call {
            namespace: "documents".to_string(),
            command: "documents_remove".to_string(),
            input: json!({}),
            confirm: Some("Resume A".to_string()),
        }
    );
}

#[test]
fn a_confirm_argument_sent_to_call_read_is_silently_ignored() {
    let arguments =
        json!({ "namespace": "jobs", "command": "jobs_list", "confirm": "should never forward" });
    let argv = tool_argv(TOOL_CALL_READ, &arguments);
    let verb = parse_verb(&argv).unwrap();
    assert_eq!(
        verb,
        Verb::Call {
            namespace: "jobs".to_string(),
            command: "jobs_list".to_string(),
            input: json!({}),
            confirm: None,
        }
    );
}

#[test]
fn confirmation_required_result_carries_the_cli_payload_verbatim_plus_one_note() {
    let payload = json!({
        "dispatched": false, "namespace": "ai", "command": "ai_set_provider_key",
        "error": agent_call::ERR_CONFIRMATION_REQUIRED,
        "detail": "read `agent call ai:ai_has_provider_key` and pass its own `has` field as --confirm",
    });
    let result = tool_result(payload.clone(), 4);
    assert_eq!(result["isError"], true);
    let blocks = result["content"].as_array().unwrap();
    assert_eq!(
        blocks[0]["text"],
        payload.to_string(),
        "content[0] must be the CLI payload byte-for-byte"
    );
    assert_eq!(blocks[1]["text"], "exitCode: 4");
    assert!(
        blocks.len() >= 3,
        "a confirmation_required result must carry a third, mapping block"
    );
    assert!(blocks[2]["text"].as_str().unwrap().contains("call-read"));
}

// ── commands (local, no bridge) ──────────────────────────────────────────

#[test]
fn commands_filters_by_effect_and_never_touches_the_bridge() {
    let all = commands_value(&json!({}));
    let read_only = commands_value(&json!({ "effect": "read" }));
    let all_rows = all["commands"].as_array().unwrap().len();
    let read_rows = read_only["commands"].as_array().unwrap().len();
    assert!(read_rows > 0 && read_rows < all_rows);
    for row in read_only["commands"].as_array().unwrap() {
        assert_eq!(row["effect"], "read");
        assert_eq!(row["tool"], TOOL_CALL_READ);
    }
}

#[test]
fn commands_names_the_proof_source_for_an_irreversible_row() {
    let out = commands_value(&json!({ "effect": "irreversible" }));
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "ai_set_provider_key")
        .expect("ai_set_provider_key is a real Irreversible row");
    assert_eq!(row["proofFrom"], "ai:ai_has_provider_key");
    assert_eq!(row["proofInput"], "provider");
}

// ── source hygiene guard (mutation-visible: adding any of these turns
// this test red immediately) ────────────────────────────────────────────

#[test]
fn mcp_source_never_prints_or_pretty_prints() {
    const SOURCE: &str = include_str!("../mcp.rs");
    for banned in ["println!(", "print!(", "to_string_pretty("] {
        assert!(
            !SOURCE.contains(banned),
            "mcp.rs must never call {banned} — see emit()'s own doc"
        );
    }
}

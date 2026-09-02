use std::io::Cursor;

use super::*;

fn line(v: Value) -> String {
    format!("{v}\n")
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn stub_ok(_verb: &Verb) -> Result<Value, &'static str> {
    Ok(json!({ "ok": true, "resource": "stub", "data": {} }))
}

/// Drive [`serve`] over an in-memory [`Cursor`] with a stub dispatcher — no runtime, no socket, no
/// live app. Always the most permissive launch mode (both flags) unless a test needs otherwise —
/// returns the raw stdout bytes as a `String` so a test can assert exact line counts / content.
fn run_serve(
    input: &str,
    mut dispatch: impl FnMut(&Verb) -> Result<Value, &'static str>,
) -> String {
    let server = Server::new(true, true);
    let mut output = Vec::new();
    let code = serve(Cursor::new(input), &mut output, &server, &mut dispatch);
    assert_eq!(code, 0);
    String::from_utf8(output).expect("valid utf8")
}

fn names(list: &[Value]) -> Vec<&str> {
    let mut n: Vec<&str> = list.iter().map(|t| t["name"].as_str().unwrap()).collect();
    n.sort_unstable();
    n
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
fn serve_dispatches_and_replies_single_flight_never_reordering_a_ping_between_two_calls() {
    // CodeRabbit, PR #1092 (item c) — pins the module doc's own "Single-flight" section: `serve`
    // reads one line, dispatches it SYNCHRONOUSLY, and writes its reply before reading the next
    // line, so a `ping` sandwiched between two `tools/call` frames can never jump the queue, and
    // the two calls themselves must dispatch in input order (never concurrently/reordered).
    let dispatch_order = std::cell::RefCell::new(Vec::new());
    let input = format!(
        "{}{}{}",
        line(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "job", "arguments": { "url": "https://example.com/first" } },
        })),
        line(json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" })),
        line(json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "job", "arguments": { "url": "https://example.com/second" } },
        })),
    );
    let text = run_serve(&input, |verb: &Verb| {
        if let Verb::Job { url } = verb {
            dispatch_order.borrow_mut().push(url.clone());
        }
        Ok(json!({ "ok": true, "resource": "job", "data": {} }))
    });

    let reply_ids: Vec<i64> = text
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).expect("each line is one JSON-RPC reply")["id"]
                .as_i64()
                .expect("every reply here carries a numeric id")
        })
        .collect();
    assert_eq!(
        reply_ids,
        vec![1, 2, 3],
        "replies must come back strictly in input order — the sandwiched ping's reply (id 2) \
         only after the first call's (id 1) and before the second's (id 3): {reply_ids:?}"
    );
    assert_eq!(
        *dispatch_order.borrow(),
        vec!["https://example.com/first", "https://example.com/second"],
        "the two tools/call frames must dispatch in input order with nothing interleaved — the \
         sandwiched ping never dispatches at all"
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
    let result = initialize_result(&json!({ "protocolVersion": "2099-01-01" }), INSTRUCTIONS);
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
        let result = initialize_result(&json!({ "protocolVersion": v }), INSTRUCTIONS);
        assert_eq!(result["protocolVersion"], v, "must echo {v}");
    }
}

#[test]
fn a_missing_protocol_version_answers_the_default() {
    let result = initialize_result(&json!({}), INSTRUCTIONS);
    assert_eq!(result["protocolVersion"], DEFAULT_VERSION);
}

#[test]
fn initialize_never_names_2026_07_28() {
    for v in ["2025-11-25", "unknown-future-version"] {
        let result = initialize_result(&json!({ "protocolVersion": v }), INSTRUCTIONS);
        assert_ne!(result["protocolVersion"], "2026-07-28");
    }
}

// ── INSTRUCTIONS / build_instructions (items 7, 14, 18, 24, 27) ─────────

#[test]
fn instructions_name_both_missing_pointer_and_app_closed_and_map_cli_phrasing_onto_tools() {
    assert!(INSTRUCTIONS.contains("app_not_located"));
    assert!(INSTRUCTIONS.contains("app_not_running"));
    assert!(INSTRUCTIONS.contains("call-read"));
    assert!(
        INSTRUCTIONS.contains("--confirm"),
        "must map CLI --confirm phrasing onto this tool's own confirm argument"
    );
}

#[test]
fn instructions_name_connection_lost_alongside_rate_limited_in_the_no_retry_sentence() {
    // item 18 — a payload too large for the bridge frame surfaces as connection_lost, which
    // reads as transient; naming only rate_limited invited a retry loop.
    assert!(INSTRUCTIONS.contains("connection_lost"));
    assert!(INSTRUCTIONS.contains("rate_limited"));
}

/// Every `"error":` STRING LITERAL mcp.rs's own source writes directly — never `agent_call`'s
/// `pub(super)` sentinels (`ERR_UNKNOWN_COMMAND`/`ERR_NOT_EXPOSED`/`ERR_CONFIRMATION_REQUIRED`),
/// referenced by path there and never respelled here. A test-only fixture (item 24): nothing in
/// production reads it, only the two tests below.
const MCP_SENTINELS: &[&str] = &["wrong_tool", "result_too_large"];

#[test]
fn instructions_name_every_mcp_only_sentinel() {
    // item 24 — wrong_tool/result_too_large are MCP-only outcomes named nowhere else.
    for sentinel in MCP_SENTINELS {
        assert!(
            INSTRUCTIONS.contains(sentinel),
            "INSTRUCTIONS must name MCP-only sentinel `{sentinel}`"
        );
    }
}

/// Find the next `"error"` key in `source` at or after `from` whose value is a string literal,
/// tolerating ANY amount of whitespace (including a newline, i.e. rustfmt splitting key and value
/// across lines) between `"error"`, `:`, and the opening quote — CodeRabbit, PR #1092: the prior
/// scanner matched only the exact spelling `"error": "` (one space), so `"error":"x"` or a
/// line-split write would silently produce NO match while the "found is non-empty" sanity check
/// stayed green on whatever it DID happen to catch elsewhere in the file. Returns the literal's
/// value and the index just past its closing quote, so the caller can resume scanning from there;
/// a `"error"` occurrence whose value isn't a string (e.g. `"error": some_const`) is skipped, not
/// treated as a scan failure.
fn next_error_literal(source: &str, from: usize) -> Option<(&str, usize)> {
    let bytes = source.as_bytes();
    let mut search_from = from;
    loop {
        let key_pos = source[search_from..].find("\"error\"")?;
        let after_key = search_from + key_pos + "\"error\"".len();
        let mut i = after_key;
        while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if bytes.get(i) != Some(&b':') {
            search_from = after_key;
            continue;
        }
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if bytes.get(i) != Some(&b'"') {
            search_from = after_key;
            continue;
        }
        let value_start = i + 1;
        let value_end = value_start + source[value_start..].find('"')?;
        return Some((&source[value_start..value_end], value_end + 1));
    }
}

#[test]
fn every_error_literal_in_mcp_source_is_named_in_mcp_sentinels() {
    // item 24 — a drift guard: every `"error": "..."` string literal this file's own source
    // writes must be a member of MCP_SENTINELS (never `agent_call`'s shared sentinels, which are
    // referenced by path, not respelled here).
    const SOURCE: &str = include_str!("../mcp.rs");
    let mut idx = 0;
    let mut found = Vec::new();
    while let Some((value, next)) = next_error_literal(SOURCE, idx) {
        found.push(value);
        idx = next;
    }
    assert!(
        !found.is_empty(),
        "sanity: the scanner must find at least one literal"
    );
    for f in &found {
        assert!(
            MCP_SENTINELS.contains(f),
            "mcp.rs writes \"error\": \"{f}\" but MCP_SENTINELS doesn't name it"
        );
    }
}

#[test]
fn build_instructions_appends_one_sentence_per_enabled_tier_and_never_duplicates_the_base() {
    let none = build_instructions(Tier::Read);
    let reversible = build_instructions(Tier::Reversible);
    let irreversible = build_instructions(Tier::Irreversible);
    assert_eq!(none, INSTRUCTIONS, "no flags must not append anything");
    assert!(reversible.starts_with(INSTRUCTIONS));
    assert!(reversible.contains("reversible write tier is enabled"));
    assert!(
        !reversible.contains("irreversible tier is enabled"),
        "the irreversible notice must not appear at the reversible tier: {reversible}"
    );
    assert!(irreversible.contains("reversible write tier is enabled"));
    assert!(irreversible.contains("irreversible tier is enabled"));
    assert_eq!(
        irreversible.matches("loopback bridge").count(),
        1,
        "must append, never duplicate, the base INSTRUCTIONS text"
    );
}

#[test]
fn instructions_notices_are_worded_by_tier_not_by_the_literal_flag_typed() {
    // item 27 — launched with ONLY --allow-irreversible; Tier::Irreversible implies the
    // reversible tier too, so BOTH notices append, but neither may claim a flag never typed.
    let text = build_instructions(Tier::from_flags(false, true));
    assert!(
        !text.contains("--allow-reversible") && !text.contains("--allow-irreversible"),
        "notices must be worded by TIER, not by the literal flag: {text}"
    );
    assert!(text.contains("reversible write tier is enabled"));
    assert!(text.contains("irreversible tier is enabled"));
}

// ── curated_tool description join (item 8) ──────────────────────────────

#[test]
fn curated_tool_joins_base_and_extra_as_two_sentences_not_a_run_on() {
    let tool = tools(Tier::Read)
        .into_iter()
        .find(|t| t["name"] == TOOL_BEST_MATCHES)
        .unwrap();
    let description = tool["description"].as_str().unwrap().to_string();
    assert!(
        description.contains(". "),
        "base and extra must be joined as two sentences: {description}"
    );
    assert!(
        !description.contains(") title/company"),
        "must never join with a bare space (the live run-on this fix closed): {description}"
    );
}

#[test]
fn job_carries_the_same_untrusted_fields_notice_as_best_matches() {
    // pre-PR gate — `job` returns the identical title/company/location fields best-matches
    // does (both now fenced), so both tools' descriptions must carry the identical notice.
    let list = tools(Tier::Read);
    let notice = list
        .iter()
        .find(|t| t["name"] == TOOL_BEST_MATCHES)
        .unwrap()["description"]
        .as_str()
        .unwrap()
        .rsplit_once(". ")
        .unwrap()
        .1
        .to_string();
    let job_description = list.iter().find(|t| t["name"] == TOOL_JOB).unwrap()["description"]
        .as_str()
        .unwrap();
    assert!(
        job_description.contains(&notice),
        "job's description must carry the same untrusted-fields notice as best-matches: \
         {job_description}"
    );
}

// ── Tier (item 21) ───────────────────────────────────────────────────────

#[test]
fn tier_irreversible_always_implies_reversible() {
    assert!(Tier::Irreversible.allows_reversible());
    assert!(Tier::Reversible.allows_reversible());
    assert!(!Tier::Read.allows_reversible());
    assert!(Tier::Irreversible.allows_irreversible());
    assert!(!Tier::Reversible.allows_irreversible());
    assert!(!Tier::Read.allows_irreversible());
}

#[test]
fn from_flags_resolves_every_combination_including_irreversible_alone() {
    assert_eq!(Tier::from_flags(false, false), Tier::Read);
    assert_eq!(Tier::from_flags(true, false), Tier::Reversible);
    assert_eq!(Tier::from_flags(false, true), Tier::Irreversible);
    assert_eq!(Tier::from_flags(true, true), Tier::Irreversible);
}

#[test]
fn server_new_false_true_still_resolves_the_full_irreversible_tier() {
    // The exact gap the review named: nothing previously constructed Server::new(false, true).
    let server = Server::new(false, true);
    assert_eq!(server.tier, Tier::Irreversible);
    assert_eq!(
        names(&server.tools),
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

// ── tools/list — the hand-written literal list, all three launch modes
// (item 11 — mutation-checked by deleting the reversible gate) ─────────

#[test]
fn tool_names_by_launch_mode_match_hand_written_literal_lists() {
    assert_eq!(
        names(&tools(Tier::Read)),
        vec![
            "automations",
            "best-matches",
            "call-read",
            "commands",
            "job",
            "profile"
        ],
        "default server (no flags) must be read tier + commands only"
    );
    assert_eq!(
        names(&tools(Tier::Reversible)),
        vec![
            "automations",
            "best-matches",
            "call-read",
            "call-reversible",
            "commands",
            "job",
            "profile",
        ],
        "--allow-reversible must add exactly call-reversible"
    );
    assert_eq!(
        names(&tools(Tier::Irreversible)),
        vec![
            "automations",
            "best-matches",
            "call-irreversible",
            "call-read",
            "call-reversible",
            "commands",
            "job",
            "profile",
        ],
        "--allow-irreversible must add call-irreversible on top of call-reversible"
    );
}

#[test]
fn calling_the_reversible_tool_without_the_flag_is_invalid_params() {
    let server = Server::new(false, false);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({
            "name": "call-reversible",
            "arguments": { "namespace": "cli_agents", "command": "cli_agents_redetect" },
        }),
        &server,
        &mut dispatch,
    );
    assert_eq!(outcome.unwrap_err().0, -32602);
}

#[test]
fn calling_the_irreversible_tool_without_the_flag_is_invalid_params() {
    let server = Server::new(false, false);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({
            "name": "call-irreversible",
            "arguments": { "namespace": "documents", "command": "documents_remove" },
        }),
        &server,
        &mut dispatch,
    );
    assert_eq!(outcome.unwrap_err().0, -32602);
}

#[test]
fn every_curated_tool_and_call_tool_declares_a_bare_object_schema_with_no_ref() {
    for tool in tools(Tier::Irreversible) {
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
    let tool_list = tools(Tier::Irreversible);
    let tool = tool_list
        .iter()
        .find(|t| t["name"] == TOOL_CALL_IRREVERSIBLE)
        .expect("present when allowed");
    assert_eq!(tool["_meta"]["anthropic/requiresUserInteraction"], true);
    assert_eq!(tool["annotations"]["destructiveHint"], true);
}

// ── mcp --help / launch-arg parsing (items 10, 11, 23, 28) ──────────────

#[test]
fn parse_launch_args_accepts_any_subset_of_the_two_flags_in_any_order() {
    assert_eq!(parse_launch_args(&[]).unwrap(), LaunchArgs::default());
    assert_eq!(
        parse_launch_args(&args(&["--allow-reversible"])).unwrap(),
        LaunchArgs {
            help: false,
            allow_reversible: true,
            allow_irreversible: false,
        }
    );
    assert_eq!(
        parse_launch_args(&args(&["--allow-irreversible", "--allow-reversible"])).unwrap(),
        LaunchArgs {
            help: false,
            allow_reversible: true,
            allow_irreversible: true,
        },
        "order must not matter"
    );
}

#[test]
fn parse_launch_args_accepts_help_anywhere_and_rejects_anything_else() {
    assert!(parse_launch_args(&args(&["--help"])).unwrap().help);
    assert!(
        parse_launch_args(&args(&["--allow-reversible", "--help"]))
            .unwrap()
            .help
    );
    assert!(parse_launch_args(&args(&["not-a-flag"])).is_err());
    assert!(parse_launch_args(&args(&["--allow-reversible", "typo"])).is_err());
}

#[test]
fn mcp_help_text_lists_both_flags_and_derives_its_default_list_from_tools() {
    let text = mcp_help_text();
    assert!(text.contains("--allow-reversible"));
    assert!(text.contains("--allow-irreversible"));
    for name in [
        "best-matches",
        "job",
        "profile",
        "automations",
        "commands",
        "call-read",
    ] {
        assert!(text.contains(name), "missing default tool `{name}`: {text}");
    }
    let default_line = text
        .lines()
        .find(|l| l.starts_with("Default"))
        .expect("must have a 'Default (no flags): ...' line");
    assert!(
        !default_line.contains("call-reversible") && !default_line.contains("call-irreversible"),
        "the default-tool-list line must not name a gated tool: {default_line}"
    );
}

#[test]
fn print_help_never_adds_a_trailing_blank_line() {
    let mut buf = Vec::new();
    print_help(&mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert!(
        text.ends_with('\n') && !text.ends_with("\n\n"),
        "must end in exactly one newline, no extra blank line: {text:?}"
    );
}

// ── every POLICY row is served by exactly one call-* tool, or refused
// everywhere if NotExposed (item 12 rewrites the NotExposed branch) ─────

#[test]
fn every_policy_row_is_routed_to_exactly_one_call_tool_or_refused_everywhere_if_not_exposed() {
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
                0,
                "{}: a NotExposed row must refuse locally on every call-* tool (MUST FIX — \
                 review round 2) — got {accepted_by:?}",
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

#[test]
fn extension_bridge_status_the_token_row_refuses_locally_on_every_call_tool() {
    // The HIGH-1 row: it returns the plaintext pairing token verbatim. A cross-version peer (an
    // updater-staged newer exe, an older still-running paired app) must not be the only thing
    // standing between this row and an MCP caller — refuse it locally too, no bridge involved.
    let verb = Verb::Call {
        namespace: "extension_bridge".to_string(),
        command: "extension_bridge_status".to_string(),
        input: json!({}),
        confirm: None,
    };
    for tool in [TOOL_CALL_READ, TOOL_CALL_REVERSIBLE, TOOL_CALL_IRREVERSIBLE] {
        let refusal = local_call_refusal(tool, &verb).expect("must refuse locally on every tool");
        assert_eq!(
            refusal["error"],
            crate::extension_bridge::agent_call::ERR_NOT_EXPOSED
        );
        assert!(refusal["detail"]
            .as_str()
            .unwrap()
            .contains("pairing token"));
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
            "arguments": { "namespace": "commands", "command": "documents_export_document" },
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

// ── commands (local, no bridge) ──────────────────────────────────────────

#[test]
fn commands_filters_by_effect_and_never_touches_the_bridge() {
    let all = commands_value(&json!({}), Tier::Irreversible);
    let read_only = commands_value(&json!({ "effect": "read" }), Tier::Irreversible);
    let all_rows = all["commands"].as_array().unwrap().len();
    let read_rows = read_only["commands"].as_array().unwrap().len();
    assert!(read_rows > 0 && read_rows < all_rows);
    for row in read_only["commands"].as_array().unwrap() {
        assert_eq!(row["effect"], "read");
        assert_eq!(row["tool"], TOOL_CALL_READ);
    }
}

#[test]
fn commands_names_the_right_tool_for_every_effect_class_with_all_flags_enabled() {
    // Pins the SECOND copy of the Effect→tool mapping (`tool_for`, used by both `commands_value`
    // and `local_call_refusal`) — the first copy was already covered by the `read`-only assertion
    // above, but nothing previously pinned `reversible`/`irreversible`/`not_exposed`.
    let all = commands_value(&json!({}), Tier::Irreversible);
    for row in all["commands"].as_array().unwrap() {
        match row["effect"].as_str().unwrap() {
            "read" => assert_eq!(row["tool"], TOOL_CALL_READ),
            "reversible" => assert_eq!(row["tool"], TOOL_CALL_REVERSIBLE),
            "irreversible" => assert_eq!(row["tool"], TOOL_CALL_IRREVERSIBLE),
            "not_exposed" => assert!(row.get("tool").is_none() && row.get("unavailable").is_none()),
            other => panic!("unexpected effect {other}"),
        }
    }
}

#[test]
fn commands_marks_irreversible_rows_unavailable_without_the_irreversible_flag() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Reversible);
    let rows = out["commands"].as_array().unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(
            row.get("tool").is_none(),
            "must not name a gated tool: {row}"
        );
        assert_eq!(
            row["unavailable"],
            "server started without --allow-irreversible"
        );
    }
}

#[test]
fn commands_marks_reversible_rows_unavailable_without_the_reversible_flag() {
    let out = commands_value(&json!({ "effect": "reversible" }), Tier::Read);
    let rows = out["commands"].as_array().unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(
            row.get("tool").is_none(),
            "must not name a gated tool: {row}"
        );
        assert_eq!(
            row["unavailable"],
            "server started without --allow-reversible"
        );
    }
}

#[test]
fn commands_with_an_unknown_effect_value_is_a_usage_error_not_a_silent_empty_success() {
    let server = Server::new(true, true);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({ "name": "commands", "arguments": { "effect": "bogus" } }),
        &server,
        &mut dispatch,
    )
    .unwrap();
    assert_eq!(
        outcome["isError"], true,
        "an unknown effect must not read as success"
    );
    assert_eq!(outcome["content"][1]["text"], "exitCode: 2");
    let text = outcome["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["error"], "usage");
}

#[test]
fn commands_with_a_non_string_effect_value_is_a_usage_error_for_every_json_type() {
    // item 20 — {"effect":5} (and bool/null/object) used to skip the old
    // `.and_then(Value::as_str)` gate entirely and answer with every row, isError:false.
    let server = Server::new(true, true);
    for bad in [json!(5), json!(true), Value::Null, json!({"nested": 1})] {
        let mut dispatch = stub_ok;
        let outcome = tool_call_result(
            &json!({ "name": "commands", "arguments": { "effect": bad.clone() } }),
            &server,
            &mut dispatch,
        )
        .unwrap();
        assert_eq!(
            outcome["isError"], true,
            "effect={bad:?} must not read as success"
        );
        let text = outcome["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["error"], "usage", "effect={bad:?}");
    }
}

#[test]
fn commands_names_the_proof_source_for_an_irreversible_row() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "ai_set_provider_key")
        .expect("ai_set_provider_key is a real Irreversible row");
    assert_eq!(row["proofFrom"], "ai:ai_has_provider_key");
    assert_eq!(row["proofInput"], "provider");
    assert!(
        row.get("proofInputValue").is_none(),
        "a FromCaller value is the caller's own input and must never be echoed: {row}"
    );
}

#[test]
fn commands_names_the_literal_proof_input_value_for_privacy_sign_out_all() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "privacy_sign_out_all")
        .expect("privacy_sign_out_all is a real Irreversible row with a Literal proof input");
    assert_eq!(row["proofInput"], "boardId");
    assert_eq!(
        row["proofInputValue"], "linkedin",
        "a Literal input's value is not secret and is the one thing this ceremony can't \
         otherwise complete from `commands` alone"
    );
}

// ── source hygiene guard (mutation-visible: adding any of these turns
// this test red immediately) ────────────────────────────────────────────

#[test]
fn mcp_source_never_prints_or_pretty_prints() {
    const SOURCE: &str = include_str!("../mcp.rs");
    for banned in ["println!(", "print!(", "to_string_pretty(", "eprintln!("] {
        assert!(
            !SOURCE.contains(banned),
            "mcp.rs must never call {banned} — see emit()'s own doc"
        );
    }
    assert_eq!(
        SOURCE.matches("stdout()").count(),
        1,
        "mcp.rs must call stdout() exactly once — see emit()'s own doc"
    );
}

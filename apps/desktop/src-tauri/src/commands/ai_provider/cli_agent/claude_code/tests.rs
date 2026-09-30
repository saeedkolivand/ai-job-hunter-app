use super::*;

#[test]
fn parses_text_delta() {
    let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}}"#;
    assert_eq!(
        ClaudeCodeAgent.parse_stream_line(line),
        Some(CliEvent::Delta("Hello".to_string()))
    );
}

#[test]
fn parses_thinking_delta() {
    let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hmm"}}}"#;
    assert_eq!(
        ClaudeCodeAgent.parse_stream_line(line),
        Some(CliEvent::Thinking("hmm".to_string()))
    );
}

#[test]
fn result_success_is_done() {
    let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"hi"}"#;
    assert_eq!(
        ClaudeCodeAgent.parse_stream_line(line),
        Some(CliEvent::Done)
    );
}

#[test]
fn result_error_is_error() {
    let line = r#"{"type":"result","subtype":"error","is_error":true,"result":"boom"}"#;
    assert_eq!(
        ClaudeCodeAgent.parse_stream_line(line),
        Some(CliEvent::Error("boom".to_string()))
    );
}

#[test]
fn ignores_system_and_assistant_events() {
    assert_eq!(
        ClaudeCodeAgent.parse_stream_line(r#"{"type":"system","subtype":"init"}"#),
        None
    );
    assert_eq!(
        ClaudeCodeAgent.parse_stream_line(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"x"}]}}"#
        ),
        None
    );
}

#[test]
fn complete_extracts_result() {
    let out = r#"{"type":"result","is_error":false,"result":"final text"}"#;
    assert_eq!(ClaudeCodeAgent.parse_complete(out).unwrap(), "final text");
}

#[test]
fn complete_errors_on_is_error() {
    let out = r#"{"type":"result","is_error":true,"result":"nope"}"#;
    assert!(ClaudeCodeAgent.parse_complete(out).is_err());
}

// ── Spec tests 1 & 2: isolation flags + effort on BOTH invocations ──────

/// Both invocations isolate the call from the user's own configuration:
/// `--strict-mcp-config`, the single-token `--setting-sources=` (see
/// [`isolation_args`] for why not the two-arg empty-string form), and
/// `--disable-slash-commands` — and never `--bare` (it would force
/// `ANTHROPIC_API_KEY` auth and break subscription logins).
#[test]
fn both_invocations_carry_isolation_flags_and_never_bare() {
    for inv in [
        ClaudeCodeAgent.stream_invocation("sonnet", "be brief", None),
        ClaudeCodeAgent.complete_invocation("sonnet", "be brief", None),
    ] {
        assert!(inv.args.iter().any(|a| a == "--strict-mcp-config"));
        assert!(inv.args.iter().any(|a| a == "--setting-sources="));
        assert!(inv.args.iter().any(|a| a == "--disable-slash-commands"));
        assert!(!inv.args.iter().any(|a| a == "--bare"));
        assert_eq!(inv.prompt, PromptDelivery::Stdin);
    }
}

/// `--effort <level>` is pushed for a known level and absent for `None`,
/// an unknown string, or a blank — the allowlist drops anything not in
/// [`EFFORT_LEVELS`], so an unknown value never reaches argv.
#[test]
fn effort_is_pushed_only_for_known_levels() {
    for inv in [
        ClaudeCodeAgent.stream_invocation("sonnet", "", Some("low")),
        ClaudeCodeAgent.complete_invocation("sonnet", "", Some("low")),
    ] {
        assert!(inv
            .args
            .windows(2)
            .any(|w| w[0] == "--effort" && w[1] == "low"));
    }
    for effort in [None, Some("bogus"), Some("  "), Some("high & calc")] {
        for inv in [
            ClaudeCodeAgent.stream_invocation("sonnet", "", effort),
            ClaudeCodeAgent.complete_invocation("sonnet", "", effort),
        ] {
            assert!(
                !inv.args.iter().any(|a| a == "--effort"),
                "effort {effort:?} must not reach argv"
            );
        }
    }
}

#[test]
fn stream_invocation_includes_model_and_system() {
    let inv = ClaudeCodeAgent.stream_invocation("sonnet", "be brief", None);
    assert!(inv.args.iter().any(|a| a == "stream-json"));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "--model" && w[1] == "sonnet"));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "--append-system-prompt" && w[1] == "be brief"));
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
}

#[test]
fn empty_model_omits_flag() {
    let inv = ClaudeCodeAgent.stream_invocation("", "", None);
    assert!(!inv.args.iter().any(|a| a == "--model"));
    assert!(!inv.args.iter().any(|a| a == "--append-system-prompt"));
}

// ── Spec tests 4 & 5: the structured (`--json-schema`) path ─────────────

/// The spec fixture: the schema-validated `structured_output` wins over
/// `.result` (which carries the same data as JSON text). Exercised through
/// the trait override — the exact call `run_structured_complete` makes.
#[test]
fn parse_structured_complete_prefers_structured_output() {
    // `result` deliberately differs (prose around the JSON, as a model may
    // write it): with identical payloads a parser that ignored
    // `structured_output` and fell back to `result` would pass too.
    let out = r#"{"type":"result","is_error":false,"result":"Here you go: {\"name\":\"Miso\"}","structured_output":{"name":"Miso"},"num_turns":3}"#;
    assert_eq!(
        ClaudeCodeAgent.parse_structured_complete(out).unwrap(),
        r#"{"name":"Miso"}"#
    );
}

/// Absent OR null `structured_output` falls back to `.result`.
#[test]
fn parse_structured_complete_falls_back_to_result_when_structured_output_missing() {
    let absent = r#"{"type":"result","is_error":false,"result":"{\"name\":\"Miso\"}"}"#;
    assert_eq!(
        ClaudeCodeAgent.parse_structured_complete(absent).unwrap(),
        r#"{"name":"Miso"}"#
    );
    let null = r#"{"type":"result","is_error":false,"result":"{\"name\":\"Miso\"}","structured_output":null}"#;
    assert_eq!(
        ClaudeCodeAgent.parse_structured_complete(null).unwrap(),
        r#"{"name":"Miso"}"#
    );
}

/// `is_error: true` errors exactly like `parse_complete` does today.
#[test]
fn parse_structured_complete_errors_on_is_error() {
    let out = r#"{"type":"result","is_error":true,"result":"boom"}"#;
    let err = ClaudeCodeAgent.parse_structured_complete(out).unwrap_err();
    assert!(format!("{err}").contains("boom"));
}

/// `structured_invocation` carries `--json-schema` (with the compact
/// schema) and `--effort` when given, and omits the schema flag entirely
/// when none is given — effort/schema/isolate all ride the same base.
#[test]
fn structured_invocation_carries_the_schema_and_effort() {
    let schema = r#"{"type":"object","properties":{"name":{"type":"string"}}}"#;
    let inv = structured_invocation("sonnet", "be brief", Some("xhigh"), Some(schema));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "--json-schema" && w[1] == schema));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "--effort" && w[1] == "xhigh"));
    assert!(inv.args.iter().any(|a| a == "--setting-sources="));

    let none = structured_invocation("sonnet", "", None, None);
    assert!(!none.args.iter().any(|a| a == "--json-schema"));
    assert!(!none.args.iter().any(|a| a == "--effort"));
}

/// The trait opt-in (`native_json_schema_invocation`) is how the cap reaches
/// the caller: a schema under the cap yields `Some` with `--json-schema` +
/// the compact schema on argv; one over the cap yields `None`, which
/// `CliAgentClient::complete_structured` turns into the prompt-discipline
/// fallback (spec test 5: "falls back when the schema exceeds the cap").
#[test]
fn native_schema_invocation_is_some_under_the_cap_and_none_above_it() {
    let small = serde_json::json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
    });
    let inv = ClaudeCodeAgent
        .native_json_schema_invocation("sonnet", "be brief", Some("max"), &small)
        .expect("a small schema must take the native path");
    assert!(inv.args.windows(2).any(|w| w[0] == "--json-schema"
        && w[1] == r#"{"properties":{"name":{"type":"string"}},"type":"object"}"#));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "--effort" && w[1] == "max"));

    let big = serde_json::json!({
        "type": "object",
        "properties": { "blob": { "type": "string", "description": "x".repeat(MAX_JSON_SCHEMA_ARG_CHARS * 3) } },
    });
    assert!(
        ClaudeCodeAgent
            .native_json_schema_invocation("sonnet", "be brief", None, &big)
            .is_none(),
        "an over-cap schema must decline the native path (prompt_only fallback)"
    );
}

/// The cap is the caller's fallback trigger: a schema under it is compact
/// and fits, one over it (a pathological nesting) is detected by
/// [`schema_is_under_argv_cap`] so the caller can degrade.
#[test]
fn schema_cap_detects_oversized_schemas() {
    let small = serde_json::json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
    });
    assert_eq!(
        compact_json_schema(&small),
        r#"{"properties":{"name":{"type":"string"}},"type":"object"}"#
    );
    assert!(schema_is_under_argv_cap(&small));

    // A schema of ~50k string characters (well over the cap) must be
    // detected, not silently shipped onto argv.
    let big = serde_json::json!({
        "type": "object",
        "properties": { "blob": { "type": "string", "description": "x".repeat(MAX_JSON_SCHEMA_ARG_CHARS * 3) } },
    });
    assert!(!schema_is_under_argv_cap(&big));
}

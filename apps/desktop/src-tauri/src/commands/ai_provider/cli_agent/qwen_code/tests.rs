use super::*;
use serde_json::json;

#[test]
fn parses_assistant_delta() {
    let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]}}"#;
    assert_eq!(
        QwenCodeAgent.parse_stream_line(line),
        Some(CliEvent::Delta("Hello".to_string()))
    );
}

#[test]
fn ignores_non_text_content() {
    let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_call"}]}}"#;
    assert_eq!(QwenCodeAgent.parse_stream_line(line), None);
}

#[test]
fn result_success_is_done() {
    let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"final"}"#;
    assert_eq!(QwenCodeAgent.parse_stream_line(line), Some(CliEvent::Done));
}

#[test]
fn result_error_is_error() {
    let line = r#"{"type":"result","subtype":"error","is_error":true,"result":"boom"}"#;
    assert_eq!(
        QwenCodeAgent.parse_stream_line(line),
        Some(CliEvent::Error("boom".to_string()))
    );
}

#[test]
fn complete_extracts_from_final_array_element() {
    let out = r#"[{"type":"assistant","message":{"content":[{"type":"text","text":"first"}]}},{"type":"result","subtype":"success","is_error":false,"result":"final answer"}]"#;
    assert_eq!(QwenCodeAgent.parse_complete(out).unwrap(), "final answer");
}

#[test]
fn complete_errors_on_is_error() {
    let out = r#"[{"type":"result","is_error":true,"result":"nope"}]"#;
    assert!(QwenCodeAgent.parse_complete(out).is_err());
}

#[test]
fn complete_errors_on_empty_array() {
    let out = "[]";
    assert!(QwenCodeAgent.parse_complete(out).is_err());
}

#[test]
fn argv_stream_has_isolation_flags_and_prompt_on_stdin() {
    let inv = QwenCodeAgent.stream_invocation("qwen-max", "system text", None);
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
    assert!(inv.args.iter().any(|a| a == "--approval-mode"));
    assert!(inv.args.iter().any(|a| a == "plan"));
    assert!(inv.args.iter().any(|a| a == "--max-session-turns"));
    assert!(inv.args.iter().any(|a| a == "1"));
    assert!(inv.args.iter().any(|a| a == "--allowed-mcp-server-names"));
    assert!(inv.args.iter().any(|a| a == "__ajh_none__"));
    assert!(inv.args.iter().any(|a| a == "--output-format"));
    assert!(inv.args.iter().any(|a| a == "stream-json"));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "-m" && w[1] == "qwen-max"));
    // -e none removed (it only disables extensions, not tools)
    assert!(!inv.args.iter().any(|a| a == "-e"));
    // Never --yolo
    assert!(!inv.args.iter().any(|a| a == "--yolo"));
}

#[test]
fn argv_complete_has_isolation_flags_and_prompt_on_stdin() {
    let inv = QwenCodeAgent.complete_invocation("qwen-max", "system text", None);
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
    assert!(inv.args.iter().any(|a| a == "--approval-mode"));
    assert!(inv.args.iter().any(|a| a == "plan"));
    assert!(inv.args.iter().any(|a| a == "--output-format"));
    assert!(inv.args.iter().any(|a| a == "json"));
    assert!(!inv.args.iter().any(|a| a == "--yolo"));
}

#[test]
fn native_json_schema_invocation_under_cap() {
    let schema = json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
    });
    let inv = QwenCodeAgent
        .native_json_schema_invocation("qwen-max", "be brief", None, &schema)
        .expect("small schema must take native path");
    assert!(inv.args.windows(2).any(|w| w[0] == "--json-schema"
        && w[1] == r#"{"properties":{"name":{"type":"string"}},"type":"object"}"#));
}

#[test]
fn native_json_schema_invocation_over_cap_is_none() {
    let big = json!({
        "type": "object",
        "properties": { "blob": { "type": "string", "description": "x".repeat(MAX_JSON_SCHEMA_ARG_CHARS * 3) } },
    });
    assert!(
        QwenCodeAgent
            .native_json_schema_invocation("qwen-max", "be brief", None, &big)
            .is_none(),
        "over-cap schema must decline native path"
    );
}

#[test]
fn system_settings_deny_every_tool_and_are_wired_through_the_env_var() {
    let files = QwenCodeAgent.workspace_files();
    assert_eq!(files.len(), 1);
    assert_eq!(
        QwenCodeAgent.workspace_env(),
        vec![("QWEN_CODE_SYSTEM_SETTINGS_PATH", files[0].0)]
    );
    let config: Value = serde_json::from_str(&files[0].1).unwrap();
    // Written out by hand on purpose: a check driven off QWEN_TOOLS itself
    // would still pass after a tool was deleted from it.
    for tool in [
        "run_shell_command",
        "read_file",
        "read_many_files",
        "write_file",
        "edit",
        "glob",
        "web_fetch",
        "web_search",
        "google_web_search",
        "save_memory",
        "tool_search",
        "tool_call",
    ] {
        for key in [&config["permissions"]["deny"], &config["tools"]["exclude"]] {
            assert!(
                key.as_array().unwrap().iter().any(|v| v == tool),
                "{tool} missing from {key}"
            );
        }
    }
}

#[test]
fn parse_structured_complete_prefers_structured_output() {
    let out = r#"[{"type":"result","is_error":false,"result":"Here you go: {\"name\":\"Miso\"}","structured_output":{"name":"Miso"}}]"#;
    assert_eq!(
        QwenCodeAgent.parse_structured_complete(out).unwrap(),
        r#"{"name":"Miso"}"#
    );
}

#[test]
fn parse_structured_complete_falls_back_to_result() {
    let out = r#"[{"type":"result","is_error":false,"result":"{\"name\":\"Miso\"}"}]"#;
    assert_eq!(
        QwenCodeAgent.parse_structured_complete(out).unwrap(),
        r#"{"name":"Miso"}"#
    );
    let null_out = r#"[{"type":"result","is_error":false,"result":"{\"name\":\"Miso\"}","structured_output":null}]"#;
    assert_eq!(
        QwenCodeAgent.parse_structured_complete(null_out).unwrap(),
        r#"{"name":"Miso"}"#
    );
}

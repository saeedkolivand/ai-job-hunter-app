use super::*;

#[test]
fn parses_stream_partial_delta() {
    // timestamp_ms present, model_call_id absent = new text delta
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello"}]},"timestamp_ms":12345}"#;
    assert_eq!(
        CursorAgent.parse_stream_line(line),
        Some(CliEvent::Delta("Hello".to_string()))
    );
}

#[test]
fn ignores_buffered_pre_tool_flush() {
    // Both timestamp_ms and model_call_id present = buffered pre-tool flush
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello"}]},"timestamp_ms":12345,"model_call_id":"call_123"}"#;
    assert_eq!(CursorAgent.parse_stream_line(line), None);
}

#[test]
fn ignores_final_flush() {
    // Both absent = final flush (would double text)
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello"}]}}"#;
    assert_eq!(CursorAgent.parse_stream_line(line), None);
}

#[test]
fn result_success_is_done() {
    let line = r#"{"type":"result","subtype":"success","is_error":false,"result":"final"}"#;
    assert_eq!(CursorAgent.parse_stream_line(line), Some(CliEvent::Done));
}

#[test]
fn result_error_is_error() {
    let line = r#"{"type":"result","subtype":"error","is_error":true,"result":"boom"}"#;
    assert_eq!(
        CursorAgent.parse_stream_line(line),
        Some(CliEvent::Error("boom".to_string()))
    );
}

#[test]
fn complete_extracts_result() {
    let out = r#"{"type":"result","subtype":"success","is_error":false,"result":"final text"}"#;
    assert_eq!(CursorAgent.parse_complete(out).unwrap(), "final text");
}

#[test]
fn complete_errors_on_is_error() {
    let out = r#"{"type":"result","is_error":true,"result":"nope"}"#;
    assert!(CursorAgent.parse_complete(out).is_err());
}

#[test]
fn argv_stream_has_correct_flags_and_prompt_on_stdin() {
    let inv = CursorAgent.stream_invocation("gpt-4o", "system text", None);
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
    // Fixed trusted prompt argument (compile-time constant)
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "-p" && w[1] == CURSOR_FIXED_PROMPT));
    assert!(inv.args.iter().any(|a| a == "--output-format"));
    assert!(inv.args.iter().any(|a| a == "stream-json"));
    assert!(inv.args.iter().any(|a| a == "--stream-partial-output"));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "--model" && w[1] == "gpt-4o"));
    // Never --force
    assert!(!inv.args.iter().any(|a| a == "--force"));
}

#[test]
fn argv_complete_has_correct_flags_and_prompt_on_stdin() {
    let inv = CursorAgent.complete_invocation("gpt-4o", "system text", None);
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
    // Fixed trusted prompt argument (compile-time constant)
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "-p" && w[1] == CURSOR_FIXED_PROMPT));
    assert!(inv.args.iter().any(|a| a == "--output-format"));
    assert!(inv.args.iter().any(|a| a == "json"));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "--model" && w[1] == "gpt-4o"));
    assert!(!inv.args.iter().any(|a| a == "--force"));
}

#[test]
fn workspace_files_returns_deny_config() {
    let files = CursorAgent.workspace_files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].0, ".cursor/cli.json");
    let config: Value = serde_json::from_str(&files[0].1).unwrap();
    let deny = config["permissions"]["deny"].as_array().unwrap();
    assert!(deny.iter().any(|v| v.as_str() == Some("Shell(*)")));
    assert!(deny.iter().any(|v| v.as_str() == Some("Read(**)")));
    assert!(deny.iter().any(|v| v.as_str() == Some("Write(**)")));
    assert!(deny.iter().any(|v| v.as_str() == Some("Mcp(*:*)")));
}

#[test]
fn parse_models_output_handles_model_lines() {
    let out = "gpt-4o\ngpt-4o-mini\n";
    let entries = parse_models_output(out).unwrap();
    assert_eq!(
        entries,
        vec![
            json!({ "name": "gpt-4o" }),
            json!({ "name": "gpt-4o-mini" }),
        ]
    );
}

#[test]
fn parse_models_output_empty_returns_none() {
    assert_eq!(parse_models_output(""), None);
    assert_eq!(parse_models_output("   \n  \n"), None);
}

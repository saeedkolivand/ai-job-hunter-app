//! Unit tests for the opencode CLI backend — moved to a sibling file purely to
//! keep the parent module under R8's LOC cap; nothing about the tests changed.

use super::*;

#[test]
fn parses_text_part_as_delta() {
    let line = r#"{"type":"text","part":{"type":"text","text":"Hello"}}"#;
    assert_eq!(
        OpencodeAgent.parse_stream_line(line),
        Some(CliEvent::Delta("Hello".to_string()))
    );
}

#[test]
fn ignores_non_text_parts() {
    let line = r#"{"type":"text","part":{"type":"tool_call","tool":"bash"}}"#;
    assert_eq!(OpencodeAgent.parse_stream_line(line), None);
}

#[test]
fn result_success_is_done() {
    let line = r#"{"type":"result","is_error":false}"#;
    assert_eq!(OpencodeAgent.parse_stream_line(line), Some(CliEvent::Done));
}

#[test]
fn result_error_is_error() {
    let line = r#"{"type":"result","is_error":true,"error":{"message":"boom"}}"#;
    assert_eq!(
        OpencodeAgent.parse_stream_line(line),
        Some(CliEvent::Error("boom".to_string()))
    );
}

#[test]
fn complete_replaces_an_updated_part_instead_of_duplicating_it() {
    let out = concat!(
        r#"{"type":"text","part":{"id":"p1","type":"text","text":"draft"}}"#,
        "\n",
        r#"{"type":"text","part":{"id":"p1","type":"text","text":"final answer"}}"#,
        "\n",
    );
    assert_eq!(OpencodeAgent.parse_complete(out).unwrap(), "final answer");
}

#[test]
fn complete_surfaces_result_error() {
    let out = r#"{"type":"result","is_error":true,"error":{"message":"Rate limit exceeded"}}"#;
    let err = OpencodeAgent.parse_complete(out).unwrap_err();
    assert!(format!("{err}").contains("rate limit"));
}

#[test]
fn complete_errors_on_empty() {
    let out = "{}";
    assert!(OpencodeAgent.parse_complete(out).is_err());
}

#[test]
fn argv_has_isolation_flags_and_model_only_prompt_on_stdin() {
    let inv = OpencodeAgent.stream_invocation("openai/gpt-4o", "system text", None);
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
    assert!(inv.args.iter().any(|a| a == "run"));
    assert!(inv.args.iter().any(|a| a == "--format"));
    assert!(inv.args.iter().any(|a| a == "json"));
    assert!(inv
        .args
        .windows(2)
        .any(|w| w[0] == "-m" && w[1] == "openai/gpt-4o"));
}

#[test]
fn argv_no_model_omits_flag() {
    let inv = OpencodeAgent.stream_invocation("", "", None);
    assert!(!inv.args.iter().any(|a| a == "-m"));
    assert_eq!(inv.prompt, PromptDelivery::Stdin);
}

#[test]
fn workspace_files_returns_deny_config() {
    let files = OpencodeAgent.workspace_files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].0, ".opencode/opencode.json");
    let config: Value = serde_json::from_str(&files[0].1).unwrap();
    // The whole rule set, exactly: one top-level ask-everything rule and
    // nothing else (see OPENCODE_CONFIG for why ask and not deny).
    assert_eq!(
        config["permissions"],
        json!([{ "action": "*", "resource": "*", "effect": "ask" }])
    );
    assert!(config.get("agent").is_none() && config.get("agents").is_none());
}

#[test]
fn parse_models_output_handles_provider_model_lines() {
    let out = "openai/gpt-4o\nanthropic/claude-3-5-sonnet\n";
    let entries = parse_models_output(out).unwrap();
    assert_eq!(
        entries,
        vec![
            json!({ "name": "openai/gpt-4o" }),
            json!({ "name": "anthropic/claude-3-5-sonnet" }),
        ]
    );
}

#[test]
fn parse_models_output_empty_returns_none() {
    assert_eq!(parse_models_output(""), None);
    assert_eq!(parse_models_output("   \n  \n"), None);
}

#[test]
fn quota_error_maps_to_friendly_message() {
    let out = r#"{"type":"result","is_error":true,"error":{"type":"provider.quota","message":"Rate limit exceeded. Please try again later.","status":429}}"#;
    let err = OpencodeAgent.parse_complete(out).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("rate limit exceeded"));
    assert!(msg.contains("try again later"));
}
#[test]
fn stream_maps_the_real_error_event() {
    let line = r#"{"type":"error","timestamp":1,"sessionID":"s","error":{"type":"provider.quota","message":"Rate limit exceeded. Please try again later.","status":429}}"#;
    match OpencodeAgent.parse_stream_line(line) {
        Some(CliEvent::Error(m)) => assert!(m.contains("rate limit exceeded"), "{m}"),
        other => panic!("expected an error event, got {other:?}"),
    }
}

#[test]
fn one_shot_joins_every_text_part() {
    let out = concat!(
        r#"{"type":"step_start","part":{"type":"step-start"}}"#,
        "\n",
        r#"{"type":"text","part":{"type":"text","text":"Hello, "}}"#,
        "\n",
        r#"{"type":"text","part":{"type":"text","text":"world"}}"#,
        "\n",
    );
    assert_eq!(OpencodeAgent.parse_complete(out).unwrap(), "Hello, world");
}

/// `ask` only refuses tools while nobody approves them: an auto-approve flag
/// would turn every `ask` into `allow`.
#[test]
fn argv_never_auto_approves() {
    for inv in [
        OpencodeAgent.stream_invocation("openai/gpt-4o", "", None),
        OpencodeAgent.complete_invocation("openai/gpt-4o", "", None),
    ] {
        for flag in ["--auto", "--yolo", "-y", "--dangerously-skip-permissions"] {
            assert!(!inv.args.iter().any(|a| a == flag), "{flag} present");
        }
    }
}

// ── #1293: `discover_models`' file-based capture ──────────────────────────────────

#[test]
fn read_models_file_reads_and_parses_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("models.out");
    std::fs::write(&path, "openai/gpt-4o\nanthropic/claude-3-5-sonnet\n").unwrap();
    assert_eq!(
        read_models_file(&path).unwrap(),
        vec![
            json!({ "name": "openai/gpt-4o" }),
            json!({ "name": "anthropic/claude-3-5-sonnet" }),
        ]
    );
}

#[test]
fn read_models_file_missing_file_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read_models_file(&dir.path().join("missing.out")), None);
}

#[tokio::test]
async fn retry_once_on_none_retries_exactly_once_after_a_blank_first_read() {
    let calls = std::cell::Cell::new(0);
    let result = retry_once_on_none(|| {
        calls.set(calls.get() + 1);
        let first = calls.get() == 1;
        async move {
            if first {
                None
            } else {
                Some(vec![json!({ "name": "a" })])
            }
        }
    })
    .await;
    assert_eq!(calls.get(), 2, "expected exactly one retry");
    assert_eq!(result, Some(vec![json!({ "name": "a" })]));
}

#[tokio::test]
async fn retry_once_on_none_does_not_retry_after_a_first_success() {
    let calls = std::cell::Cell::new(0);
    let result = retry_once_on_none(|| {
        calls.set(calls.get() + 1);
        async move { Some::<Vec<Value>>(vec![json!({ "name": "a" })]) }
    })
    .await;
    assert_eq!(calls.get(), 1, "must not call again after success");
    assert!(result.is_some());
}

#[tokio::test]
async fn retry_once_on_none_gives_up_after_two_blanks() {
    let calls = std::cell::Cell::new(0);
    let result: Option<Vec<Value>> = retry_once_on_none(|| {
        calls.set(calls.get() + 1);
        async move { None }
    })
    .await;
    assert_eq!(
        calls.get(),
        2,
        "no sleep loop — exactly two attempts, then give up"
    );
    assert_eq!(result, None);
}

//! `agent call <ns>:<command>`'s own argv.

use super::*;
use crate::extension_bridge::agent_cli::tests::support::s;

// ── `call` argv parsing (ADR-038 §2, Phase 2) ───────────────────────────

#[test]
fn parses_call_with_namespace_command_and_input() {
    assert_eq!(
        parse_verb(&s(&["call", "jobs:jobs_list", "--input", r#"{"a":1}"#])).unwrap(),
        Verb::Call {
            namespace: "jobs".to_string(),
            command: "jobs_list".to_string(),
            input: serde_json::json!({ "a": 1 }),
            confirm: None,
        }
    );
}

#[test]
fn parses_call_without_input_as_an_empty_object() {
    assert_eq!(
        parse_verb(&s(&["call", "jobs:jobs_list"])).unwrap(),
        Verb::Call {
            namespace: "jobs".to_string(),
            command: "jobs_list".to_string(),
            input: serde_json::json!({}),
            confirm: None,
        }
    );
}

#[test]
fn parses_call_with_confirm() {
    assert_eq!(
        parse_verb(&s(&[
            "call",
            "documents:documents_remove",
            "--confirm",
            "Resume A"
        ]))
        .unwrap(),
        Verb::Call {
            namespace: "documents".to_string(),
            command: "documents_remove".to_string(),
            input: serde_json::json!({}),
            confirm: Some("Resume A".to_string()),
        }
    );
}

#[test]
fn parses_call_with_both_input_and_confirm_in_either_order() {
    let forward = parse_verb(&s(&[
        "call",
        "documents:documents_remove",
        "--input",
        r#"{"id":"doc-1"}"#,
        "--confirm",
        "Resume A",
    ]))
    .unwrap();
    let backward = parse_verb(&s(&[
        "call",
        "documents:documents_remove",
        "--confirm",
        "Resume A",
        "--input",
        r#"{"id":"doc-1"}"#,
    ]))
    .unwrap();
    assert_eq!(forward, backward);
    assert_eq!(
        forward,
        Verb::Call {
            namespace: "documents".to_string(),
            command: "documents_remove".to_string(),
            input: serde_json::json!({ "id": "doc-1" }),
            confirm: Some("Resume A".to_string()),
        }
    );
}

#[test]
fn rejects_confirm_missing_its_value() {
    assert!(parse_verb(&s(&["call", "jobs:jobs_list", "--confirm"])).is_err());
}

#[test]
fn confirm_error_never_echoes_the_typed_value() {
    // Same path-privacy discipline as `--input` — `--confirm` is user data
    // (ADR-038 §4) and must never appear in a usage error either.
    let leaky = r"C:\Users\alice\Desktop\secret-notes";
    let err = parse_verb(&s(&[
        "call",
        "jobs:jobs_list",
        "--confirm",
        leaky,
        "--bogus",
    ]))
    .unwrap_err()
    .to_string();
    assert!(!err.contains(leaky), "must not echo --confirm: {err}");
}

#[test]
fn rejects_call_missing_the_namespace_command_token() {
    assert!(parse_verb(&s(&["call"])).is_err());
}

#[test]
fn rejects_call_target_missing_a_colon() {
    assert!(parse_verb(&s(&["call", "jobs_list"])).is_err());
}

#[test]
fn rejects_call_target_with_an_empty_namespace_or_command() {
    assert!(parse_verb(&s(&["call", ":jobs_list"])).is_err());
    assert!(parse_verb(&s(&["call", "jobs:"])).is_err());
}

#[test]
fn rejects_call_input_that_is_not_valid_json() {
    let err = parse_verb(&s(&["call", "jobs:jobs_list", "--input", "{not json"]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("valid JSON"));
}

#[test]
fn rejects_call_input_that_is_not_a_json_object() {
    assert!(parse_verb(&s(&["call", "jobs:jobs_list", "--input", "[1,2]"])).is_err());
    assert!(parse_verb(&s(&["call", "jobs:jobs_list", "--input", "\"x\""])).is_err());
}

#[test]
fn call_input_error_never_echoes_the_typed_value() {
    // Path privacy — `--input` may carry a path or other sensitive content.
    let leaky = r#"{"path":"C:\Users\alice\Desktop\secret"NOTJSON"#;
    let err = parse_verb(&s(&["call", "jobs:jobs_list", "--input", leaky]))
        .unwrap_err()
        .to_string();
    assert!(!err.contains("alice"), "must not echo --input: {err}");
}

use super::*;
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

/// Still the contract AT THIS LAYER: `tool_argv` never forwards `confirm` off
/// `call-irreversible`. A real `tools/call` no longer gets this far — `classify_tool_call`
/// refuses the undeclared key first (issue #1134, tested above) — but the layered guarantee is
/// what keeps a future caller of `tool_argv` from smuggling a proof through.
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

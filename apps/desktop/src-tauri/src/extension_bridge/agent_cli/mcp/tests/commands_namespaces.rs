use super::*;

/// Upper bound on a single catalogued description's length (MEDIUM — CLI review round 2,
/// issue #1163: "a one-line description"). `catalogueSummarize`'s short-sentence fallback used to
/// publish the ENTIRE first paragraph — up to 1096 chars of renderer/Settings implementation
/// detail for `ai_model_capabilities` — measured 416 as the longest row after the fix that pulls
/// in only the next sentence instead. Lower this constant (never raise it) if the generator gets
/// better at trimming; raising it silently re-permits a paragraph dump.
const MAX_CATALOGUE_DESCRIPTION_LENGTH: usize = 500;

#[test]
fn a_catalogued_description_never_grows_into_a_paragraph() {
    let out = commands_value(&json!({}), Tier::Irreversible);
    for row in out["commands"].as_array().unwrap() {
        let Some(description) = row["description"].as_str() else {
            continue;
        };
        assert!(
            description.len() <= MAX_CATALOGUE_DESCRIPTION_LENGTH,
            "{}: description is {} chars (cap {MAX_CATALOGUE_DESCRIPTION_LENGTH}), not a one-line \
             description: {description:?}",
            row["command"],
            description.len()
        );
    }
}

/// #1160's own target row must not silently lose its description again — `applications_delete`
/// was one of the 64 no-TSDoc rows a CLI review round flagged (MEDIUM), and it is the exact
/// command whose `keepDocuments: false` cascade a caller needs explained.
#[test]
fn applications_delete_carries_a_non_empty_description() {
    let out = commands_value(&json!({}), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "applications_delete")
        .expect("applications_delete is a real, catalogued row");
    let description = row["description"].as_str().unwrap_or("");
    // Both branches, not just the identifier (CLI review round 2 — MEDIUM): a description that
    // only says the flag is irrelevant ("always irreversible, regardless of `keepDocuments`")
    // satisfied a substring check on "keepDocuments" while explaining nothing a caller could
    // choose the flag on.
    assert!(
        description.to_lowercase().contains("false")
            && description.to_lowercase().contains("also deletes"),
        "must explain what keepDocuments: false does: {description:?}"
    );
    assert!(
        description.to_lowercase().contains("true")
            && description.to_lowercase().contains("detach"),
        "must explain what keepDocuments: true does: {description:?}"
    );
}

/// A wrapper arg whose type this generator RECOGNISED but could not resolve the fields of
/// (issue #1158 member 3, MEDIUM — CLI review round 1) carries an EXPLICIT `"fields": null`, not
/// an absent key — otherwise it is wire-identical to a plain scalar arg and a caller has no way
/// to know a nested object might be expected at all.
#[test]
fn commands_carries_a_null_fields_for_an_arg_whose_wrapper_type_did_not_resolve() {
    let out = commands_value(&json!({}), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "resume_pipeline_run")
        .expect("resume_pipeline_run is a real, catalogued row");
    let args = row["args"].as_array().expect("declared args, not null");
    let req = args
        .iter()
        .find(|a| a["name"] == "req")
        .expect("declares a req arg");
    assert!(
        req.get("fields").is_some_and(Value::is_null),
        "must be an explicit null, not an absent key: {req}"
    );
}

/// A scalar arg (no wrapper type at all) still omits `fields` entirely — the null-for-unresolved
/// fix must not turn every arg's `fields` key into a `null`.
#[test]
fn commands_omits_fields_entirely_for_a_scalar_arg() {
    let out = commands_value(&json!({}), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "applications_delete")
        .expect("applications_delete is a real, catalogued row");
    let args = row["args"].as_array().expect("declared args, not null");
    let id_arg = args
        .iter()
        .find(|a| a["name"] == "id")
        .expect("declares an id arg");
    assert!(
        id_arg.get("fields").is_none(),
        "a scalar arg must not carry a fields key at all: {id_arg}"
    );
}

/// A command absent from the generated catalogue (zero renderer `invoke()` references —
/// `policy.rs`'s own module doc) carries `args: null`, distinguishable from "this command
/// genuinely takes no arguments" (an empty array).
#[test]
fn commands_carries_a_null_args_for_an_uncatalogued_row() {
    let out = commands_value(&json!({}), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "boards_list")
        .expect("boards_list is a real POLICY row with zero renderer references");
    assert!(row["args"].is_null(), "{row}");
}

#[test]
fn commands_can_be_filtered_by_namespace() {
    let out = commands_value(&json!({ "namespace": "jobs" }), Tier::Irreversible);
    let rows = out["commands"].as_array().unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        assert_eq!(row["namespace"], "jobs", "{row}");
    }
}

#[test]
fn commands_namespace_and_effect_filters_compose() {
    let out = commands_value(
        &json!({ "namespace": "applications", "effect": "irreversible" }),
        Tier::Irreversible,
    );
    let rows = out["commands"].as_array().unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        assert_eq!(row["namespace"], "applications", "{row}");
        assert_eq!(row["effect"], "irreversible", "{row}");
    }
}

/// Same failure shape `effect` already guards against (issue #1134's own lesson, reapplied to
/// #1163's new filter): a typo'd namespace must be a usage error, never a silent empty success.
#[test]
fn commands_with_an_unknown_namespace_value_is_a_usage_error_not_a_silent_empty_success() {
    let server = Server::new(true, true);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({ "name": "commands", "arguments": { "namespace": "totally-not-a-real-namespace" } }),
        &server,
        &mut dispatch,
    )
    .unwrap();
    assert_eq!(outcome["isError"], true);
    let text = outcome["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["error"], "usage");
}

/// A REAL namespace passes the same gate straight through.
#[test]
fn commands_with_a_real_namespace_value_dispatches_locally() {
    let server = Server::new(true, true);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({ "name": "commands", "arguments": { "namespace": "jobs" } }),
        &server,
        &mut dispatch,
    )
    .unwrap();
    assert_eq!(outcome["isError"], false);
}

/// Issue #1163's own ask: a real command name typed under the wrong namespace must name the
/// right one in `unknown_command`'s local refusal — `jobs_list` is real, `wrongns` is not its
/// namespace.
#[test]
fn unknown_command_local_refusal_names_the_right_namespace_for_a_real_command_typed_wrong() {
    let verb = Verb::Call {
        namespace: "wrongns".to_string(),
        command: "jobs_list".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal =
        local_call_refusal(TOOL_CALL_READ, &verb, Tier::Irreversible).expect("must refuse");
    assert_eq!(refusal["error"], agent_call::ERR_UNKNOWN_COMMAND);
    assert!(
        refusal["detail"].as_str().unwrap().contains("jobs"),
        "{refusal}"
    );
}

/// The mirror case: a genuinely fictional command name must not fabricate a suggestion.
#[test]
fn unknown_command_local_refusal_names_no_namespace_for_a_command_that_does_not_exist() {
    let verb = Verb::Call {
        namespace: "nope".to_string(),
        command: "delete_everything".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal =
        local_call_refusal(TOOL_CALL_READ, &verb, Tier::Irreversible).expect("must refuse");
    assert!(
        !refusal["detail"]
            .as_str()
            .unwrap()
            .contains("registered under namespace"),
        "{refusal}"
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

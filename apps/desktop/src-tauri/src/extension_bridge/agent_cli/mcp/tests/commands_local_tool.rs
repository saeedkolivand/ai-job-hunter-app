use super::*;
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

/// Issue #1136's discoverability half: a caller must be able to LEARN that
/// these two rows answer with an envelope and take `limit`/`cursor`, rather
/// than discovering it by receiving a shape it did not expect. Asserted in
/// both directions — the PAGINATED_LIST_NOTE appears on exactly the paged
/// rows and on no others — so a `returns` key leaking onto some unrelated row
/// fails here too, `contact_profile_get`'s own unrelated note (P-r1-F4)
/// excepted and pinned separately below.
#[test]
fn commands_marks_the_paged_rows_and_only_those() {
    let all = commands_value(&json!({}), Tier::Irreversible);
    let mut noted: Vec<&str> = Vec::new();
    for row in all["commands"].as_array().unwrap() {
        let Some(returns) = row["returns"].as_str() else {
            continue;
        };
        let command = row["command"].as_str().unwrap();
        // `contact_profile_get` carries its OWN discovery note (P-r1-F4,
        // issue #1180), pinned by
        // `commands_marks_the_contact_profile_get_row_with_its_projection_note`
        // — this test's whole job is the PAGINATED_LIST_COMMANDS set, so it
        // is excluded by name rather than the assertion below being weakened
        // to "one of several known notes".
        if command == "contact_profile_get" {
            continue;
        }
        assert_eq!(returns, agent_call::reshape::PAGINATED_LIST_NOTE);
        noted.push(command);
    }
    noted.sort_unstable();
    assert_eq!(
        noted,
        vec!["ai_generations_list", "applications_list", "documents_list"]
    );
}

/// P-r1-F4 (round-1 review, issue #1180): `contact_profile_get`'s reshape is
/// the OTHER discovery gap the paged-rows test above already guards against
/// for paging — a plain `call-read` caller (or any `ajh-tauri agent call`
/// invocation) never sees the MCP tool description that used to carry the
/// only note about this projection.
#[test]
fn commands_marks_the_contact_profile_get_row_with_its_projection_note() {
    let all = commands_value(&json!({}), Tier::Irreversible);
    let row = all["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "contact_profile_get")
        .expect("contact_profile_get is a real POLICY row");
    assert_eq!(
        row["returns"],
        agent_call::reshape::CONTACT_PROFILE_GET_PROJECTION_NOTE
    );
}

/// P-r2-R2-F4 (round-2 review): the note above hand-copies the allowlist
/// into prose with nothing pinning it to the list it describes — the exact
/// drift `PAGINATED_LIST_NOTE`'s own sibling test guards against for its
/// pacing numbers. Compares the EXACT set named in the note's own
/// `{a,b,c}` literal against `CONTACT_PROFILE_AGENT_FIELDS`, not a
/// per-field `contains` (T0, PR #1184 CodeRabbit review): `contains` alone
/// would miss a field REMOVED from the note (every remaining name still
/// matches) and would wrongly accept `photo` being added back to the
/// allowlist, since the note already names `photo` in its own exclusion
/// clause ("`photo` is stripped before an agent ever sees it").
#[test]
fn contact_profile_get_projection_note_names_exactly_the_allowlisted_fields() {
    use crate::extension_bridge::autofill_profile::CONTACT_PROFILE_AGENT_FIELDS;
    use std::collections::HashSet;

    let note = agent_call::reshape::CONTACT_PROFILE_GET_PROJECTION_NOTE;
    let set_literal = note
        .split_once('{')
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(inside, _)| inside)
        .expect("the note must carry a `{a,b,c}` field-set literal");
    let named: HashSet<&str> = set_literal.split(',').collect();
    let allowlisted: HashSet<&str> = CONTACT_PROFILE_AGENT_FIELDS.iter().copied().collect();
    assert_eq!(named, allowlisted, "note: {note}");
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
    assert_eq!(row["proofField"], "has");
    assert_eq!(row["proofKind"], "field");
    assert!(
        row.get("proofInputValue").is_none(),
        "a FromCaller value is the caller's own input and must never be echoed: {row}"
    );
}

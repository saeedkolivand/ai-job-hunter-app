//! The catalogue's per-command JSON Schema (#1491): the list stays compact, one command's row
//! carries the nested contract.

use super::*;

fn row_for(command: &str) -> Value {
    let out = commands_value(&json!({ "command": command }), Tier::Irreversible);
    let rows = out["commands"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "an exact command filter yields one row");
    rows[0].clone()
}

fn req_schema(command: &str) -> Value {
    let row = row_for(command);
    let arg = row["args"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "req")
        .expect("a req arg")
        .clone();
    arg["schema"].clone()
}

/// The ask: enough to build a valid `autopilot_create` body without the source.
#[test]
fn autopilot_create_schema_carries_nested_fields_enums_and_ranges() {
    let schema = req_schema("autopilot_create");
    let props = &schema["properties"];
    assert_eq!(
        props["schedule"]["enum"],
        json!(["manual", "hourly", "daily", "twice_daily"])
    );
    assert_eq!(props["target"]["properties"]["pages"]["maximum"], 10);
    assert_eq!(props["target"]["properties"]["pages"]["minimum"], 1);
    assert_eq!(
        props["filter"]["properties"]["minMatchScore"]["maximum"],
        100
    );
    assert_eq!(props["name"]["maxLength"], 100);
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for field in ["name", "target", "filter", "schedule"] {
        assert!(
            required.contains(&field),
            "{field} is required: {required:?}"
        );
    }
    assert!(!required.contains(&"resumeText"));
    // The date filter vocabulary is the one the app refuses outside of (#1492).
    assert!(props["target"]["properties"]["dateFilter"]["enum"].is_array());
}

/// The six write commands #1491 names each resolve to a schema, and the three whose call site
/// the generator cannot resolve by type name (`SCHEMA_OVERRIDES`) are among them. A hand-written
/// literal: looping the catalogue would only cover what the generator already emitted.
#[test]
fn the_main_write_commands_each_carry_a_schema() {
    for (command, arg) in [
        ("autopilot_create", "req"),
        ("autopilot_update", "req"),
        ("scrape_boards", "req"),
        ("job_preferences_set", "prefs"),
        ("applications_update", "req"),
        ("resume_pipeline_run", "req"),
    ] {
        assert!(
            input_schema(command, arg).is_some(),
            "{command}.{arg} has no generated JSON Schema"
        );
    }
}

#[test]
fn autopilot_update_schema_is_a_partial_with_a_status_enum() {
    let schema = req_schema("autopilot_update");
    assert_eq!(
        schema["properties"]["status"]["enum"],
        json!(["active", "paused", "archived"])
    );
    assert!(
        schema
            .get("required")
            .is_none_or(|r| r.as_array().unwrap().is_empty()),
        "every update field is optional: {}",
        schema["required"]
    );
}

#[test]
fn every_generated_schema_is_valid_json_for_a_catalogued_arg() {
    let mut seen = 0;
    for entry in CATALOGUE.iter() {
        for arg in entry.args {
            if let Some(schema) = input_schema(entry.command, arg.name) {
                let parsed: Value = serde_json::from_str(schema)
                    .unwrap_or_else(|e| panic!("{}.{}: {e}", entry.command, arg.name));
                assert_eq!(parsed["type"], "object", "{}.{}", entry.command, arg.name);
                assert!(parsed.get("$schema").is_none());
                seen += 1;
            }
        }
    }
    assert!(seen >= 20, "only {seen} schemas reached the catalogue");
}

/// The list is every command, every call: it carries a `hasSchema` flag, never a schema.
#[test]
fn the_list_stays_compact_and_points_at_the_single_command_form() {
    let list = commands_value(&json!({}), Tier::Irreversible);
    let bytes = serde_json::to_string(&list).unwrap().len();
    println!("full commands list: {bytes} B");
    assert!(
        bytes < MCP_RESULT_MAX_BYTES / 2,
        "the full list is {bytes} B, over half the {MCP_RESULT_MAX_BYTES} B reply cap"
    );
    let create = list["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "autopilot_create")
        .unwrap();
    assert_eq!(create["args"][0]["hasSchema"], true);
    assert!(
        !list.to_string().contains("\"schema\""),
        "no schema in the list"
    );
}

/// Every single-command reply fits the cap, so the schema is always reachable even when the
/// whole set would not fit one reply.
#[test]
fn every_single_command_reply_fits_the_mcp_output_cap() {
    let mut largest = (0, "");
    for entry in POLICY.iter() {
        let command = agent_call::split_path(entry.path).1;
        let bytes = commands_value(&json!({ "command": command }), Tier::Irreversible)
            .to_string()
            .len();
        assert!(bytes < MCP_RESULT_MAX_BYTES, "{command} reply is {bytes} B");
        largest = largest.max((bytes, command));
    }
    println!(
        "largest single-command reply: {} B ({})",
        largest.0, largest.1
    );
}

#[test]
fn an_unknown_command_filter_is_a_usage_error_not_an_empty_success() {
    let server = Server::new(false, false);
    let ToolCall::Local(Ok(result)) = classify_tool_call(
        &json!({ "name": TOOL_COMMANDS, "arguments": { "command": "autopilot_creat" } }),
        &server,
    ) else {
        panic!("commands answers locally");
    };
    assert_eq!(result["isError"], true);
    let ToolCall::Local(Ok(ok)) = classify_tool_call(
        &json!({ "name": TOOL_COMMANDS, "arguments": { "command": "autopilot_create" } }),
        &server,
    ) else {
        panic!("commands answers locally");
    };
    assert_eq!(ok["isError"], false);
}

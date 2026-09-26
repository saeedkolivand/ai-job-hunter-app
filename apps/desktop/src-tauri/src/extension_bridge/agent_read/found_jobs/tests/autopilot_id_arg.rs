//! `parse_autopilot_id_arg`'s own argument-shape tests (B3-r1-F2).

use super::super::*;

#[test]
fn parse_autopilot_id_arg_treats_absent_or_null_as_spanning_every_autopilot() {
    assert_eq!(parse_autopilot_id_arg(&json!({})).unwrap(), None);
    assert_eq!(
        parse_autopilot_id_arg(&json!({ "autopilotId": null })).unwrap(),
        None
    );
}

#[test]
fn parse_autopilot_id_arg_accepts_a_real_id() {
    assert_eq!(
        parse_autopilot_id_arg(&json!({ "autopilotId": "ap-1" })).unwrap(),
        Some("ap-1".to_string())
    );
    // Surrounding whitespace is trimmed, same as every other string filter.
    assert_eq!(
        parse_autopilot_id_arg(&json!({ "autopilotId": "  ap-1  " })).unwrap(),
        Some("ap-1".to_string())
    );
}

/// The headline case (B3-r1-F2): a PRESENT-but-blank `autopilotId` used to
/// collapse silently to the same `None` an OMITTED one produces, widening a
/// one-autopilot selector into a spanning traversal of every autopilot with
/// no signal to the caller. Must now be a hard error, never "all".
#[test]
fn parse_autopilot_id_arg_rejects_a_blank_or_whitespace_only_value() {
    for value in [json!(""), json!("   ")] {
        let err = parse_autopilot_id_arg(&json!({ "autopilotId": value })).unwrap_err();
        assert_eq!(err.to_string(), BLANK_AUTOPILOT_ID_MESSAGE);
    }
}

/// Mirrors `agent_cli::mcp::tool_argv`'s own guard on the same field: a
/// flag-shaped value must never be forwarded as if it were a real id.
#[test]
fn parse_autopilot_id_arg_rejects_a_flag_shaped_value() {
    let err =
        parse_autopilot_id_arg(&json!({ "autopilotId": "--include-description" })).unwrap_err();
    assert_eq!(err.to_string(), BLANK_AUTOPILOT_ID_MESSAGE);
}

#[test]
fn parse_autopilot_id_arg_rejects_a_non_string_value() {
    let err = parse_autopilot_id_arg(&json!({ "autopilotId": 5 })).unwrap_err();
    assert_eq!(err.to_string(), BLANK_AUTOPILOT_ID_MESSAGE);
}

//! Tests for `build_input` (`proof.rs`).

use super::super::*;
use serde_json::json;

// ── build_input ─────────────────────────────────────────────────────

#[test]
fn build_input_forwards_the_named_caller_field_for_a_lookup() {
    let source = ProofSource::Lookup {
        read_command: "autopilot_get",
        key: "autopilotId",
        input: LookupInput::FromCaller(&["autopilotId"]),
        path: &["name"],
    };
    let caller_input = json!({ "autopilotId": "ap-1" });
    assert_eq!(
        build_input(source, &caller_input),
        json!({ "autopilotId": "ap-1" })
    );
}

/// HIGH fix (security review round 2): `resume_pipeline_regenerate_
/// section`'s own `#[tauri::command]` signature wraps its args in one
/// `req` struct, so its `runId` lives at `req.runId`, not the top level —
/// a multi-segment path is what makes `build_input` read the SAME
/// location the real command reads its target from.
#[test]
fn build_input_walks_a_multi_segment_path_for_a_wrapped_req_command() {
    let source = ProofSource::Lookup {
        read_command: "resume_pipeline_get",
        key: "runId",
        input: LookupInput::FromCaller(&["req", "runId"]),
        path: &["jobUrl"],
    };
    let caller_input = json!({ "req": { "runId": "run-B", "sectionKey": "summary" } });
    assert_eq!(
        build_input(source, &caller_input),
        json!({ "runId": "run-B" })
    );
}

/// The unbound-ceremony shape from the review finding: a top-level
/// `runId` alongside a DIFFERENT `req.runId` must resolve against the
/// WRAPPED value (what the real command actually acts on), never the
/// decoy top-level one — this is the exact defect the path-based
/// selector fixes.
#[test]
fn build_input_ignores_a_decoy_top_level_field_and_reads_only_the_wrapped_path() {
    let source = ProofSource::Lookup {
        read_command: "resume_pipeline_get",
        key: "runId",
        input: LookupInput::FromCaller(&["req", "runId"]),
        path: &["jobUrl"],
    };
    let caller_input = json!({ "runId": "run-A", "req": { "runId": "run-B" } });
    assert_eq!(
        build_input(source, &caller_input),
        json!({ "runId": "run-B" }),
        "must resolve against req.runId (what the command acts on), never the top-level decoy"
    );
}

#[test]
fn build_input_uses_a_literal_regardless_of_caller_input() {
    let source = ProofSource::Lookup {
        read_command: "boards_get_status",
        key: "boardId",
        input: LookupInput::Literal("linkedin"),
        path: &["connected"],
    };
    // Even a caller trying to steer the literal via its own input has no
    // effect — `Literal` never reads `caller_input` at all.
    let caller_input = json!({ "boardId": "attacker-controlled" });
    assert_eq!(
        build_input(source, &caller_input),
        json!({ "boardId": "linkedin" })
    );
}

#[test]
fn build_input_is_empty_for_every_no_input_read_command() {
    for source in [
        ProofSource::Scalar {
            read_command: "system_get_version",
            path: &[],
        },
        ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: &["id"],
            match_field: "_id",
            value_field: "name",
        },
        ProofSource::Count {
            read_command: "notifications_list",
        },
        ProofSource::MatchCount {
            read_command: "ai_generations_list",
            ids_field: &["ids"],
            match_field: "id",
        },
    ] {
        assert_eq!(build_input(source, &json!({ "id": "x" })), json!({}));
    }
}

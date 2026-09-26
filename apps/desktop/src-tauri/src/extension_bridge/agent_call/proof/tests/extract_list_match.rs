//! Tests for `extract`'s `ListMatch`/`Count`/`MatchCount` arms (`proof.rs`).

use super::super::super::super::agent_cli::policy::Effect;
use super::super::*;
use super::support::a_document_record;
use serde_json::json;

#[test]
fn extract_list_match_finds_the_record_by_the_callers_own_id() {
    let source = ProofSource::ListMatch {
        read_command: "documents_list",
        id_field: &["id"],
        match_field: "id",
        value_field: "name",
    };
    let response = json!([
        { "id": "doc-1", "name": "Resume A" },
        { "id": "doc-2", "name": "Resume B" },
    ]);
    assert_eq!(
        extract(source, &json!({ "id": "doc-2" }), &response),
        Some("Resume B".to_string())
    );
}

#[test]
fn extract_list_match_returns_none_when_the_id_is_not_in_the_list() {
    let source = ProofSource::ListMatch {
        read_command: "documents_list",
        id_field: &["id"],
        match_field: "id",
        value_field: "name",
    };
    let response = json!([{ "id": "doc-1", "name": "Resume A" }]);
    assert_eq!(
        extract(source, &json!({ "id": "doc-missing" }), &response),
        None
    );
}

#[test]
fn extract_list_match_returns_none_when_the_callers_own_id_field_is_absent() {
    // An empty/omitted selector must never widen to "the first record" —
    // absent input resolves to no proof, not an accidental match.
    let source = ProofSource::ListMatch {
        read_command: "documents_list",
        id_field: &["id"],
        match_field: "id",
        value_field: "name",
    };
    let response = json!([{ "id": "doc-1", "name": "Resume A" }]);
    assert_eq!(extract(source, &json!({}), &response), None);
}

#[test]
fn extract_list_match_matches_a_real_document_record_by_its_wire_id_field() {
    let response = json!([a_document_record("doc-1", "Resume A")]);
    let source = ProofSource::ListMatch {
        read_command: "documents_list",
        id_field: &["id"],
        match_field: "_id",
        value_field: "name",
    };
    assert_eq!(
        extract(source, &json!({ "id": "doc-1" }), &response),
        Some("Resume A".to_string())
    );
}

/// Mutation guard: the ORIGINAL bug (`match_field: "id"`) must fail
/// against a REAL wire response, proving the fixture above is not
/// accidentally satisfying both a correct and a buggy selector.
#[test]
fn extract_list_match_with_the_pre_fix_match_field_never_matches_a_real_document_record() {
    let response = json!([a_document_record("doc-1", "Resume A")]);
    let source = ProofSource::ListMatch {
        read_command: "documents_list",
        id_field: &["id"],
        match_field: "id",
        value_field: "name",
    };
    assert_eq!(
        extract(source, &json!({ "id": "doc-1" }), &response),
        None,
        "match_field: \"id\" must never match a real DocumentRecord, which has no such key"
    );
}

/// `resume_pipeline_run`'s exact shape: `id_field` is a PATH into a
/// `req`-wrapped `--input` body (its `#[tauri::command]` signature takes
/// one `req: ResumePipelineRunRequest` argument), and the response is
/// matched on the real `_id` wire field.
#[test]
fn extract_list_match_reads_a_wrapped_resume_id_and_matches_the_real_wire_field() {
    let response = json!([a_document_record("doc-9", "Resume B")]);
    let source = ProofSource::ListMatch {
        read_command: "documents_list",
        id_field: &["req", "resumeId"],
        match_field: "_id",
        value_field: "name",
    };
    let caller_input = json!({ "req": { "resumeId": "doc-9", "jobId": "job-1" } });
    assert_eq!(
        extract(source, &caller_input, &response),
        Some("Resume B".to_string())
    );
}

/// Closes the gap between "extract()'s ListMatch logic is correct in
/// general" (the hand-typed `ProofSource` tests above) and "the REAL
/// `documents_remove` POLICY row is configured correctly" — pulls the
/// row straight out of `POLICY` rather than typing its shape again, so
/// a future revert of that row's `match_field` back to `"id"` fails
/// HERE, not only against a hand-typed literal.
#[test]
fn the_real_documents_remove_policy_row_resolves_a_document_record_by_its_wire_id() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::documents::documents_remove")
        .expect("documents_remove is a real POLICY row");
    let Effect::Irreversible(source) = entry.effect else {
        panic!("documents_remove must be Irreversible: {:?}", entry.effect);
    };
    let response = json!([a_document_record("doc-1", "Resume A")]);
    assert_eq!(
        extract(source, &json!({ "id": "doc-1" }), &response),
        Some("Resume A".to_string())
    );
}

/// Same closing-the-gap discipline for `resume_pipeline_run`'s real
/// row — pins BOTH the wrapped `id_field` path AND the `_id` wire field
/// against the actual committed table, not a re-typed copy of it.
#[test]
fn the_real_resume_pipeline_run_policy_row_resolves_a_wrapped_resume_id() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::resume_pipeline::resume_pipeline_run")
        .expect("resume_pipeline_run is a real POLICY row");
    let Effect::Irreversible(source) = entry.effect else {
        panic!(
            "resume_pipeline_run must be Irreversible: {:?}",
            entry.effect
        );
    };
    let response = json!([a_document_record("doc-9", "Resume B")]);
    let caller_input = json!({ "req": { "resumeId": "doc-9", "jobId": "job-1" } });
    assert_eq!(
        extract(source, &caller_input, &response),
        Some("Resume B".to_string())
    );
}

/// The unbound-ceremony shape from the review finding, run against the
/// REAL `resume_pipeline_regenerate_section` row (not a re-typed copy):
/// `--input '{"runId":"run-A","req":{"runId":"run-B",...}}'` must
/// resolve the proof against `req.runId` (what the command actually
/// acts on), never the top-level decoy — a revert of this row's
/// `LookupInput::FromCaller` back to a flat `"runId"` fails HERE.
#[test]
fn the_real_resume_pipeline_regenerate_section_policy_row_ignores_a_decoy_top_level_run_id() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::resume_pipeline::resume_pipeline_regenerate_section")
        .expect("resume_pipeline_regenerate_section is a real POLICY row");
    let Effect::Irreversible(source) = entry.effect else {
        panic!(
            "resume_pipeline_regenerate_section must be Irreversible: {:?}",
            entry.effect
        );
    };
    let caller_input =
        json!({ "runId": "run-A", "req": { "runId": "run-B", "sectionKey": "summary" } });
    assert_eq!(
        build_input(source, &caller_input),
        json!({ "runId": "run-B" }),
        "must read req.runId (what the command acts on), never the top-level decoy"
    );
}

/// Closes the gap between "extract()'s Scalar logic is correct in
/// general" and "the REAL `ai_set_active_provider` row is configured
/// correctly" — pulls the row straight out of `POLICY` rather than
/// typing its shape again, so a future revert of its `path` back to
/// something else (or off `ai_active_config`) fails HERE against a real
/// fixture, not only against a hand-typed `ProofSource` literal.
/// `ai_set_provider_settings` used to be checked alongside this row —
/// security review round 4 moved it to `NotExposed` (its proof never
/// bound to the caller-chosen `provider` field the patch actually
/// rewrites; see `policy.rs`'s own comment on that row), so it no
/// longer has a `ProofSource` to resolve at all.
#[test]
fn the_real_ai_set_active_provider_row_resolves_a_real_active_ai_config_fixture() {
    let response = serde_json::to_value(crate::ai_config::ActiveAiConfig {
        active_provider: Some("anthropic".to_string()),
        ..Default::default()
    })
    .unwrap();
    let path = "commands::ai::ai_set_active_provider";
    let entry = POLICY
        .iter()
        .find(|e| e.path == path)
        .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
    let Effect::Irreversible(source) = entry.effect else {
        panic!("{path} must be Irreversible: {:?}", entry.effect);
    };
    assert_eq!(
        extract(source, &json!({}), &response),
        Some("anthropic".to_string()),
        "{path}'s real POLICY row must resolve against a real ActiveAiConfig fixture"
    );
}

#[test]
fn extract_count_is_the_array_length() {
    let source = ProofSource::Count {
        read_command: "notifications_list",
    };
    let response = json!([{}, {}, {}]);
    assert_eq!(
        extract(source, &json!({}), &response),
        Some("3".to_string())
    );
}

#[test]
fn extract_count_of_an_empty_list_is_zero() {
    let source = ProofSource::Count {
        read_command: "notifications_list",
    };
    assert_eq!(
        extract(source, &json!({}), &json!([])),
        Some("0".to_string())
    );
}

#[test]
fn extract_match_count_counts_only_the_targeted_ids_that_exist() {
    let source = ProofSource::MatchCount {
        read_command: "ai_generations_list",
        ids_field: &["ids"],
        match_field: "id",
    };
    let response = json!([
        { "id": "g-1" },
        { "id": "g-2" },
        { "id": "g-3" },
    ]);
    // Two of three requested ids actually exist; the third is a typo/stale id.
    let caller_input = json!({ "ids": ["g-1", "g-3", "g-nonexistent"] });
    assert_eq!(
        extract(source, &caller_input, &response),
        Some("2".to_string())
    );
}

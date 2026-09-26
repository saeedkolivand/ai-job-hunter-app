//! Tests for the success/refusal reply builder and the throttle-key router (`reply.rs`).

use super::super::*;

#[test]
fn call_result_reply_on_success_carries_dispatched_true_and_the_data_verbatim() {
    let text = call_result_reply(
        "req-1",
        "jobs",
        "jobs_list",
        Ok(json!({ "sample": "value" })),
    );
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["type"], super::super::super::msg::AGENT_CALL_RESULT);
    assert_eq!(v["payload"]["dispatched"], true);
    assert_eq!(v["payload"]["namespace"], "jobs");
    assert_eq!(v["payload"]["command"], "jobs_list");
    assert_eq!(v["payload"]["data"]["sample"], "value");
    assert!(v["payload"].get("ok").is_none(), "must never overload `ok`");
}

#[test]
fn call_result_reply_on_refusal_carries_dispatched_false_and_no_data_key() {
    let text = call_result_reply("req-2", "jobs", "bogus", Err(Refusal::UnknownCommand(None)));
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["payload"]["dispatched"], false);
    assert_eq!(v["payload"]["error"], "unknown_command");
    assert!(v["payload"]["detail"].as_str().unwrap().len() > 10);
    assert!(v["payload"].get("data").is_none());
}

#[test]
fn call_result_reply_for_confirmation_required_never_embeds_a_proof_value_in_the_reply() {
    let hint = proof::hint(
        super::super::super::agent_cli::policy::ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: &["id"],
            match_field: "id",
            value_field: "name",
        },
    );
    let text = call_result_reply(
        "req-3",
        "documents",
        "documents_remove",
        Err(Refusal::ConfirmationRequired(hint)),
    );
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["payload"]["dispatched"], false);
    assert_eq!(v["payload"]["error"], ERR_CONFIRMATION_REQUIRED);
    assert!(v["payload"]["detail"]
        .as_str()
        .unwrap()
        .contains("agent call documents:documents_list"));
    assert!(v["payload"].get("data").is_none());
}

/// End-to-end (of the pure parts) pin for Finding 1: a call whose command
/// dispatch produced `InvokeResponse::Err` must reach the wire as
/// `dispatched: false` with sentinel `invoke_error`, never `dispatched:
/// true` — the concrete `applications_delete`-without-`keepDocuments`
/// example the finding names.
#[test]
fn call_result_reply_for_invoke_error_never_claims_dispatched_true() {
    let text = call_result_reply(
        "req-4",
        "applications",
        "applications_delete",
        Err(Refusal::InvokeError(
            "missing required key keepDocuments".to_string(),
        )),
    );
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["payload"]["dispatched"], false);
    assert_eq!(v["payload"]["error"], "invoke_error");
    assert!(v["payload"]["detail"]
        .as_str()
        .unwrap()
        .contains("missing required key keepDocuments"));
    assert!(v["payload"].get("data").is_none());
}

// ── throttle_key ───────────────────────────────────────────────────

#[test]
fn throttle_key_routes_best_matches_command_into_the_shared_tight_bucket() {
    assert_eq!(throttle_key("autopilot_best_matches"), "best-matches");
}

#[test]
fn throttle_key_leaves_every_other_command_as_its_own_key() {
    assert_eq!(throttle_key("jobs_list"), "jobs_list");
    assert_eq!(throttle_key("scrape_resolve_url"), "scrape_resolve_url");
}

// ── fencing scraped job-posting text ──────────────────────────────

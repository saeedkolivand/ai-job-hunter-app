//! Tests for the paired-extension caller's own gating replies (`reply.rs`).

use super::super::super::agent_cli::policy::Effect;
use super::super::*;

#[test]
fn extension_may_dispatch_is_true_only_for_a_read_effect_row() {
    // `jobs_list` is a real Read row (used throughout this suite's `agent.call` fixtures).
    // Namespace is the WIRE shape `split_path` derives from the policy path's middle segment
    // (`"commands::jobs::jobs_list"` → `("jobs", "jobs_list")`), never the full `path` string.
    let read = json!({ "namespace": "jobs", "command": "jobs_list" });
    assert!(extension_may_dispatch(&read));

    // An unknown (namespace, command) pair must read as "not Read", never dispatched.
    let unknown = json!({ "namespace": "commands::nope", "command": "does_not_exist" });
    assert!(!extension_may_dispatch(&unknown));
}

/// Walks every real `POLICY` row (same exhaustive discipline as
/// `extension_bridge::test::agent_call_gate_matches_every_policy_rows_declared_effect`): the
/// extension tier's own gate must agree with EVERY row's declared `Effect`, not just a
/// hand-picked sample — `Read` dispatchable, everything else refused.
#[test]
fn extension_may_dispatch_matches_every_policy_rows_declared_effect() {
    use super::super::super::agent_cli::policy::POLICY;

    let mut checked = 0usize;
    for entry in POLICY {
        checked += 1;
        let (namespace, command) = split_path(entry.path);
        let payload = json!({ "namespace": namespace, "command": command });
        let expected_read = matches!(entry.effect, Effect::Read);
        assert_eq!(
            extension_may_dispatch(&payload),
            expected_read,
            "{} is {:?} — extension_may_dispatch disagreed with the declared Effect",
            entry.path,
            entry.effect
        );
    }
    assert_eq!(checked, 168);
}

#[test]
fn extension_gate_reply_carries_the_fixed_sentinel() {
    let payload = json!({ "namespace": "jobs", "command": "jobs_list" });
    let reply = extension_gate_reply("req-ext-1", &payload);
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], super::super::super::msg::AGENT_CALL_RESULT);
    assert_eq!(v["payload"]["dispatched"], false);
    assert_eq!(v["payload"]["error"], ERR_EXTENSION_READ_GATE);
    assert!(v["payload"]["detail"]
        .as_str()
        .unwrap()
        .contains("Assisted autofill"));
}

#[test]
fn effect_not_allowed_reply_carries_the_fixed_sentinel_and_never_a_confirm_hint() {
    // `applications_delete` is a real Irreversible row — see the exhaustive
    // `extension_may_dispatch_matches_every_policy_rows_declared_effect` above for why this must
    // be a GENUINE non-Read row, not an unknown-command typo that would refuse for a different
    // reason.
    let payload = json!({ "namespace": "applications", "command": "applications_delete" });
    let reply = effect_not_allowed_reply("req-ext-2", &payload);
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["payload"]["dispatched"], false);
    assert_eq!(v["payload"]["error"], ERR_EFFECT_NOT_ALLOWED_FOR_EXTENSION);
    let detail = v["payload"]["detail"].as_str().unwrap();
    assert!(
        !detail.to_lowercase().contains("confirm"),
        "the extension's refusal must never hint at the confirm ceremony: {detail:?}"
    );
}

#[test]
fn extension_capped_reply_passes_an_under_cap_reply_through_untouched() {
    let payload = json!({ "namespace": "jobs", "command": "jobs_list" });
    let reply = call_result_reply("req-ext-3", "jobs", "jobs_list", Ok(json!({})));
    let capped = extension_capped_reply("req-ext-3", &payload, reply.clone());
    assert_eq!(capped, reply);
}

#[test]
fn extension_capped_reply_refuses_an_over_cap_reply_with_its_own_smaller_cap_wording() {
    let payload = json!({ "namespace": "jobs", "command": "jobs_list" });
    let oversized = "x".repeat(super::super::super::EXTENSION_RESULT_MAX_BYTES + 1);
    let capped = extension_capped_reply("req-ext-4", &payload, oversized.clone());
    assert!(capped.len() < oversized.len());
    let v: Value = serde_json::from_str(&capped).unwrap();
    assert_eq!(v["payload"]["dispatched"], false);
    assert_eq!(v["payload"]["error"], ERR_RESULT_TOO_LARGE);
    assert_eq!(v["payload"]["namespace"], "jobs");
    assert_eq!(v["payload"]["command"], "jobs_list");
    assert!(v["payload"]["detail"]
        .as_str()
        .unwrap()
        .contains("extension caller's own"));
}

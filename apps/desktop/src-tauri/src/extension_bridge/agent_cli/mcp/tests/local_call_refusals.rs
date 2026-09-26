use super::*;
// ── MUST FIX — unknown_command / wrong_tool local refusals ──────────────

#[test]
fn call_read_refuses_a_namespace_command_the_local_policy_does_not_know() {
    let verb = Verb::Call {
        namespace: "nope".to_string(),
        command: "delete_everything".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal =
        local_call_refusal(TOOL_CALL_READ, &verb, Tier::Irreversible).expect("must refuse");
    assert_eq!(refusal["dispatched"], false);
    assert_eq!(refusal["error"], agent_call::ERR_UNKNOWN_COMMAND);
}

#[test]
fn call_read_refuses_a_real_reversible_row_naming_the_right_tool() {
    // `cli_agents_redetect` is a real Reversible POLICY row. `Tier::Irreversible` is what makes
    // this `wrong_tool` rather than `tier_not_enabled` — `call-reversible` IS registered here;
    // see the `tier_not_enabled` tests below for the launch where it is not.
    let verb = Verb::Call {
        namespace: "cli_agents".to_string(),
        command: "cli_agents_redetect".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal = local_call_refusal(TOOL_CALL_READ, &verb, Tier::Irreversible)
        .expect("must refuse — wrong tool");
    assert_eq!(refusal["error"], "wrong_tool");
    assert!(refusal["detail"]
        .as_str()
        .unwrap()
        .contains(TOOL_CALL_REVERSIBLE));
}

#[test]
fn call_read_accepts_a_real_read_row() {
    let verb = Verb::Call {
        namespace: "cli_agents".to_string(),
        command: "cli_agents_status".to_string(),
        input: json!({}),
        confirm: None,
    };
    assert!(local_call_refusal(TOOL_CALL_READ, &verb, Tier::Read).is_none());
}

// ── issue #1154 — `tier_not_enabled` vs `wrong_tool` ─────────────────────

/// The headline defect: at `Tier::Read`, a Reversible row called on `call-read` must NOT name
/// `call-reversible` as "call it there instead" — that tool is not registered on this launch, so
/// the refusal must say so and name the flag to relaunch with, never point at a dead end.
#[test]
fn tier_not_enabled_at_read_tier_for_a_reversible_row() {
    let verb = Verb::Call {
        namespace: "cli_agents".to_string(),
        command: "cli_agents_redetect".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal = local_call_refusal(TOOL_CALL_READ, &verb, Tier::Read)
        .expect("must refuse — tier not enabled");
    assert_eq!(refusal["error"], "tier_not_enabled");
    let detail = refusal["detail"].as_str().unwrap();
    assert!(
        detail.contains("--allow-reversible"),
        "must name the launch flag: {detail}"
    );
    assert!(
        !detail.contains("call it there instead"),
        "must not tell the model to retry on a tool it cannot see: {detail}"
    );
}

/// The other side: when the right tool IS registered, the SAME mismatch stays `wrong_tool` —
/// pinned at the most permissive launch so a caller for a Read row via `call-reversible` is told
/// to retry on `call-read`, never `tier_not_enabled` (every tool exists at `Tier::Irreversible`).
#[test]
fn wrong_tool_at_irreversible_tier_for_a_read_row_called_via_call_reversible() {
    let verb = Verb::Call {
        namespace: "cli_agents".to_string(),
        command: "cli_agents_status".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal = local_call_refusal(TOOL_CALL_REVERSIBLE, &verb, Tier::Irreversible)
        .expect("must refuse — wrong tool");
    assert_eq!(refusal["error"], "wrong_tool");
    assert!(refusal["detail"].as_str().unwrap().contains(TOOL_CALL_READ));
}

/// A1-r1-SEC-1 HIGH: `local_call_refusal` must catch a mis-keyed body itself, never rely on a
/// possibly stale PEER app process to be the only thing catching it (the same class this file
/// already fixed for `Effect::NotExposed`). `documents_remove` is a real Irreversible row whose
/// declared key is `id`; a caller who sends the wrong one must refuse `invalid_input` locally,
/// with no dispatch.
#[test]
fn call_irreversible_refuses_a_mis_keyed_body_locally_without_dispatching() {
    let verb = Verb::Call {
        namespace: "documents".to_string(),
        command: "documents_remove".to_string(),
        input: json!({ "documentId": "doc-1" }),
        confirm: None,
    };
    let refusal = local_call_refusal(TOOL_CALL_IRREVERSIBLE, &verb, Tier::Irreversible)
        .expect("must refuse — unknown key");
    assert_eq!(refusal["dispatched"], false);
    assert_eq!(refusal["error"], agent_call::ERR_INVALID_INPUT);
    let detail = refusal["detail"].as_str().unwrap();
    assert!(
        detail.contains("documentId") && detail.contains("id"),
        "{detail}"
    );
}

/// A correctly-keyed body on the right tool passes this local check — proves the new catalogue
/// check does not over-refuse a legitimate call.
#[test]
fn call_irreversible_accepts_a_correctly_keyed_body() {
    let verb = Verb::Call {
        namespace: "documents".to_string(),
        command: "documents_remove".to_string(),
        input: json!({ "id": "doc-1" }),
        confirm: None,
    };
    assert!(local_call_refusal(TOOL_CALL_IRREVERSIBLE, &verb, Tier::Irreversible).is_none());
}

/// A1-r1-AC-1 MEDIUM: `local_call_refusal` used to mirror `check_input` only, so issue #1158
/// member 3's exact shape (`{"req":{}}`, the empty write) still passed the local gate and
/// dispatched — reachable through `check_input`'s membership-only walk since `req` is a KNOWN key
/// with nothing unknown inside it. `applications_save_from_posting` is a real Reversible row whose
/// required `req` wrapper is fully resolved; an empty object for it must refuse locally, on the
/// right tool, with no dispatch — never depend on a possibly-stale peer app to catch it.
#[test]
fn call_reversible_refuses_an_empty_required_wrapper_locally_without_dispatching() {
    let verb = Verb::Call {
        namespace: "applications".to_string(),
        command: "applications_save_from_posting".to_string(),
        input: json!({ "req": {} }),
        confirm: None,
    };
    let refusal = local_call_refusal(TOOL_CALL_REVERSIBLE, &verb, Tier::Irreversible)
        .expect("must refuse — empty required wrapper");
    assert_eq!(refusal["dispatched"], false);
    assert_eq!(refusal["error"], agent_call::ERR_INVALID_INPUT);
    let detail = refusal["detail"].as_str().unwrap();
    assert!(
        detail.contains("req") && detail.contains("empty"),
        "{detail}"
    );
}

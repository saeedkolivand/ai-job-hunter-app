use super::*;
// ── Tier (item 21) ───────────────────────────────────────────────────────

#[test]
fn tier_irreversible_always_implies_reversible() {
    assert!(Tier::Irreversible.allows_reversible());
    assert!(Tier::Reversible.allows_reversible());
    assert!(!Tier::Read.allows_reversible());
    assert!(Tier::Irreversible.allows_irreversible());
    assert!(!Tier::Reversible.allows_irreversible());
    assert!(!Tier::Read.allows_irreversible());
}

#[test]
fn from_flags_resolves_every_combination_including_irreversible_alone() {
    assert_eq!(Tier::from_flags(false, false), Tier::Read);
    assert_eq!(Tier::from_flags(true, false), Tier::Reversible);
    assert_eq!(Tier::from_flags(false, true), Tier::Irreversible);
    assert_eq!(Tier::from_flags(true, true), Tier::Irreversible);
}

#[test]
fn server_new_false_true_still_resolves_the_full_irreversible_tier() {
    // The exact gap the review named: nothing previously constructed Server::new(false, true).
    let server = Server::new(false, true);
    assert_eq!(server.tier, Tier::Irreversible);
    assert_eq!(
        names(&server.tools),
        vec![
            "automations",
            "best-matches",
            "call-irreversible",
            "call-read",
            "call-reversible",
            "commands",
            "found-jobs",
            "job",
            "profile",
        ]
    );
}

// ── tools/list — the hand-written literal list, all three launch modes
// (item 11 — mutation-checked by deleting the reversible gate) ─────────

#[test]
fn tool_names_by_launch_mode_match_hand_written_literal_lists() {
    assert_eq!(
        names(&tools(Tier::Read)),
        vec![
            "automations",
            "best-matches",
            "call-read",
            "commands",
            "found-jobs",
            "job",
            "profile"
        ],
        "default server (no flags) must be read tier + commands only"
    );
    assert_eq!(
        names(&tools(Tier::Reversible)),
        vec![
            "automations",
            "best-matches",
            "call-read",
            "call-reversible",
            "commands",
            "found-jobs",
            "job",
            "profile",
        ],
        "--allow-reversible must add exactly call-reversible"
    );
    assert_eq!(
        names(&tools(Tier::Irreversible)),
        vec![
            "automations",
            "best-matches",
            "call-irreversible",
            "call-read",
            "call-reversible",
            "commands",
            "found-jobs",
            "job",
            "profile",
        ],
        "--allow-irreversible must add call-irreversible on top of call-reversible"
    );
}

#[test]
fn calling_the_reversible_tool_without_the_flag_is_invalid_params() {
    let server = Server::new(false, false);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({
            "name": "call-reversible",
            "arguments": { "namespace": "cli_agents", "command": "cli_agents_redetect" },
        }),
        &server,
        &mut dispatch,
    );
    assert_eq!(outcome.unwrap_err().0, -32602);
}

#[test]
fn calling_the_irreversible_tool_without_the_flag_is_invalid_params() {
    let server = Server::new(false, false);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({
            "name": "call-irreversible",
            "arguments": { "namespace": "documents", "command": "documents_remove" },
        }),
        &server,
        &mut dispatch,
    );
    assert_eq!(outcome.unwrap_err().0, -32602);
}

#[test]
fn every_curated_tool_and_call_tool_declares_a_bare_object_schema_with_no_ref() {
    for tool in tools(Tier::Irreversible) {
        let schema = &tool["inputSchema"];
        assert_eq!(
            schema["type"], "object",
            "{}: root must be type:object",
            tool["name"]
        );
        assert_eq!(
            schema["additionalProperties"], false,
            "{}: additionalProperties must be false",
            tool["name"]
        );
        assert!(schema.get("$ref").is_none(), "{}: no $ref", tool["name"]);
    }
}

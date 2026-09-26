use super::*;
// ── every POLICY row is served by exactly one call-* tool, or refused
// everywhere if NotExposed (item 12 rewrites the NotExposed branch) ─────

#[test]
fn every_policy_row_is_routed_to_exactly_one_call_tool_or_refused_everywhere_if_not_exposed() {
    for entry in POLICY {
        let (namespace, command) = agent_call::split_path(entry.path);
        let verb = Verb::Call {
            namespace: namespace.to_string(),
            command: command.to_string(),
            input: json!({}),
            confirm: None,
        };
        // ROUTING only, never validation: an `invalid_input` refusal (A1-r1-SEC-1 HIGH added
        // this local check) means the row's declared args reject a bare `{}` — orthogonal to
        // which TOOL it is classified for, and this test's probe never builds a real body. Counts
        // as "accepted" here so a row requiring args is not mistaken for one refused on every
        // tool (`invalid_input` refuses identically on all three, same as a routing accept would
        // look from this test's own PoV) — `local_call_refusal`'s own dedicated
        // `invalid_input`-refusal tests cover that check directly.
        // `Tier::Irreversible` — every `call-*` tool registered, so a mismatch can only be
        // `wrong_tool`/`invalid_input`, never `tier_not_enabled` (that gate has its own dedicated
        // tests below).
        let accepted_by: Vec<&str> = [TOOL_CALL_READ, TOOL_CALL_REVERSIBLE, TOOL_CALL_IRREVERSIBLE]
            .into_iter()
            .filter(
                |tool| match local_call_refusal(tool, &verb, Tier::Irreversible) {
                    None => true,
                    Some(refusal) => refusal["error"] == agent_call::ERR_INVALID_INPUT,
                },
            )
            .collect();
        match entry.effect {
            Effect::NotExposed(_) => assert_eq!(
                accepted_by.len(),
                0,
                "{}: a NotExposed row must refuse locally on every call-* tool (MUST FIX — \
                 review round 2) — got {accepted_by:?}",
                entry.path
            ),
            _ => assert_eq!(
                accepted_by.len(),
                1,
                "{}: exactly one call-* tool must accept this row locally — got {accepted_by:?}",
                entry.path
            ),
        }
    }
}

/// [A2-r2-AC-r2-2] The tier gate exercised across EVERY POLICY row at `Tier::Read` and
/// `Tier::Reversible` too — the prior test only ran it at `Tier::Irreversible`, where the gate is
/// always open by construction, so a divergence between `commands_value`'s gate and
/// `local_call_refusal`'s gate could never show up there. Both now call the ONE shared
/// [`tier_exposes`] (issue #1154), so this pins that `commands`' `"tool"`/`"unavailable"` split
/// and the refusal's `tier_not_enabled`/`wrong_tool` split agree for every row, at every tier.
///
/// [A2-r3-A3-AC-1] `gate_open` below is spelled out by hand rather than calling `tier_exposes`
/// itself — the function both call sites under test route through — so a break in the shared
/// gate has an independent expectation to disagree with, instead of a tautology that can only
/// ever agree with itself.
#[test]
fn the_tier_gate_agrees_between_commands_and_local_call_refusal_at_every_tier() {
    for tier in [Tier::Read, Tier::Reversible, Tier::Irreversible] {
        let commands = commands_value(&json!({}), tier);
        let rows = commands["commands"].as_array().unwrap();
        for entry in POLICY {
            if let Effect::NotExposed(_) = entry.effect {
                continue;
            }
            let (namespace, command) = agent_call::split_path(entry.path);
            let right_tool = tool_for(&entry.effect).unwrap();
            // Hand-written mirror of `tier_exposes`, not a call to it (see fn doc above).
            let gate_open = match entry.effect {
                Effect::Reversible => tier != Tier::Read,
                Effect::Irreversible(_) => tier == Tier::Irreversible,
                _ => true,
            };

            // `commands`' own row for this entry.
            let row = rows
                .iter()
                .find(|r| r["namespace"] == namespace && r["command"] == command)
                .unwrap_or_else(|| panic!("{}: missing from `commands` at {tier:?}", entry.path));
            assert_eq!(
                row.get("tool").is_some(),
                gate_open,
                "{}: `commands`' `tool` presence disagrees with tier_exposes at {tier:?}",
                entry.path
            );
            assert_eq!(
                row.get("unavailable").is_some(),
                !gate_open,
                "{}: `commands`' `unavailable` presence disagrees with tier_exposes at {tier:?}",
                entry.path
            );

            // `local_call_refusal` called on a WRONG tool for this row.
            let wrong_tool = [TOOL_CALL_READ, TOOL_CALL_REVERSIBLE, TOOL_CALL_IRREVERSIBLE]
                .into_iter()
                .find(|t| *t != right_tool)
                .unwrap();
            let verb = Verb::Call {
                namespace: namespace.to_string(),
                command: command.to_string(),
                input: json!({}),
                confirm: None,
            };
            let refusal = local_call_refusal(wrong_tool, &verb, tier)
                .unwrap_or_else(|| panic!("{}: must refuse on the wrong tool", entry.path));
            let expected_error = if gate_open {
                "wrong_tool"
            } else {
                "tier_not_enabled"
            };
            assert_eq!(
                refusal["error"], expected_error,
                "{}: local_call_refusal disagrees with tier_exposes at {tier:?}",
                entry.path
            );
        }
    }
}

#[test]
fn extension_bridge_status_the_token_row_refuses_locally_on_every_call_tool() {
    // The HIGH-1 row: it returns the plaintext pairing token verbatim. A cross-version peer (an
    // updater-staged newer exe, an older still-running paired app) must not be the only thing
    // standing between this row and an MCP caller — refuse it locally too, no bridge involved.
    let verb = Verb::Call {
        namespace: "extension_bridge".to_string(),
        command: "extension_bridge_status".to_string(),
        input: json!({}),
        confirm: None,
    };
    for tool in [TOOL_CALL_READ, TOOL_CALL_REVERSIBLE, TOOL_CALL_IRREVERSIBLE] {
        let refusal = local_call_refusal(tool, &verb, Tier::Irreversible)
            .expect("must refuse locally on every tool");
        assert_eq!(
            refusal["error"],
            crate::extension_bridge::agent_call::ERR_NOT_EXPOSED
        );
        assert!(refusal["detail"]
            .as_str()
            .unwrap()
            .contains("pairing token"));
    }
}

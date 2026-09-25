//! Local effect-class routing for `call-*` — the refusals that must never be forwarded to a
//! possibly stale peer app: an unknown target, a known target on the wrong tool, a
//! `NotExposed` row, and a body the bundled catalogue's own contract rejects.

use super::*;

/// Local effect-class routing for `call-*`: refuse a target the bundled [`POLICY`] copy does not
/// know at all (never forward it), refuse a KNOWN target on the wrong tool naming the right one —
/// or, when that right tool isn't even REGISTERED on this launch, `tier_not_enabled` naming the
/// flag to relaunch with instead (issue #1154: `wrong_tool` used to name `call-reversible`/
/// `call-irreversible` unconditionally, even on a read-only launch where the client's own
/// `tools/list` never advertised them — a dead end the model could not act on) — refuse a
/// [`Effect::NotExposed`] target on EVERY tool naming its own stored reason (MUST FIX — security
/// review round 2), and (A1-r1-SEC-1 HIGH) refuse a body that fails the bundled catalogue's own
/// declared contract with `invalid_input` — none of these forwarded, so a possibly stale PEER app
/// process (e.g. an updater-staged newer exe still paired with an older running app) is never the
/// only thing catching them, matching what [`instructions::INSTRUCTIONS`] promises the model
/// before any call runs. Never touches the wire.
pub(super) fn local_call_refusal(tool_name: &str, verb: &Verb, tier: Tier) -> Option<Value> {
    let Verb::Call {
        namespace,
        command,
        input,
        ..
    } = verb
    else {
        return None;
    };
    let entry = POLICY
        .iter()
        .find(|e| agent_call::split_path(e.path) == (namespace.as_str(), command.as_str()));
    let Some(entry) = entry else {
        // Same suggestion `agent_call::dispatch`'s own `UnknownCommand` refusal names — never a
        // second hand-typed scan of `POLICY` (issue #1163).
        let suggestion = agent_call::namespace_suggestion(command);
        return Some(json!({
            "dispatched": false,
            "namespace": namespace,
            "command": command,
            "error": agent_call::ERR_UNKNOWN_COMMAND,
            "detail": agent_call::unknown_command_detail(suggestion),
        }));
    };
    if let Effect::NotExposed(reason) = entry.effect {
        return Some(json!({
            "dispatched": false,
            "namespace": namespace,
            "command": command,
            "error": agent_call::ERR_NOT_EXPOSED,
            "detail": format!("not exposed to any CLI tier: {reason}"),
        }));
    }
    // `NotExposed` already returned above, so every remaining `Effect` has a right tool. If that
    // invariant ever breaks, forward to the app (which refuses on its own) rather than panic:
    // this path runs under `panic = "abort"`, where a panic is a silent server death.
    let right_tool = tool_for(&entry.effect)?;
    if right_tool != tool_name {
        // [`tier_exposes`] — the SAME fn `commands_value` calls per row (issue #1154), not a
        // second hand-typed copy, so the two can never disagree about which effects this Tier
        // exposes.
        let gate_open = tier_exposes(tier, &entry.effect);
        if !gate_open {
            return Some(json!({
                "dispatched": false,
                "namespace": namespace,
                "command": command,
                "error": "tier_not_enabled",
                "detail": format!(
                    "this command is classified for `{right_tool}`, but this server was \
                     launched without it registered ({}) — do not retry on `{right_tool}`, it is \
                     not in this session's tool list; ask the user to relaunch `ajh-tauri agent \
                     mcp` with that flag (Settings → Developer)",
                    unavailable_reason(&entry.effect),
                ),
            }));
        }
        return Some(json!({
            "dispatched": false,
            "namespace": namespace,
            "command": command,
            "error": "wrong_tool",
            "detail": format!("this command is classified for `{right_tool}`, not `{tool_name}` — call it there instead"),
        }));
    }
    // Catalogue validation (A1-r1-SEC-1 HIGH, widened for A1-r1-AC-1 MEDIUM to also cover an
    // empty required wrapper), same contract `agent_call::plan` enforces app-side — checked
    // locally so a mis-keyed or empty-wrapper body never depends on a possibly stale PEER app
    // process to catch it.
    if let Some(detail) = agent_call::invalid_input_detail(command, entry.effect, input) {
        return Some(json!({
            "dispatched": false,
            "namespace": namespace,
            "command": command,
            "error": agent_call::ERR_INVALID_INPUT,
            "detail": detail,
        }));
    }
    None
}

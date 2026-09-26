//! Reply builders — the one success-path builder plus the ONE refusal builder every refusal on
//! this surface goes through, and the per-caller reply-size ceilings layered on top of it. Split
//! out of `agent_call.rs` under the R8 LOC cap.

use serde_json::{json, Value};

use super::super::agent_cli::policy::Effect;
use super::policy_lookup::find_policy;
use super::refusal::{Refusal, ERR_RESULT_TOO_LARGE};

/// `dispatched`, never `ok` (ADR-038 §5): ~47 commands signal failure INSIDE
/// their own Ok payload, so this dispatcher cannot know whether the
/// underlying operation succeeded — only whether it ran. `data` is the
/// command's payload verbatim (no PII redaction — ADR-038's amendment to
/// ADR-0005, scoped to this generic tier by the owner's explicit decision).
pub(super) fn call_result_reply(
    req_id: &str,
    namespace: &str,
    command: &str,
    outcome: Result<Value, Refusal>,
) -> String {
    let payload = match outcome {
        Ok(data) => json!({
            "dispatched": true,
            "namespace": namespace,
            "command": command,
            "data": data,
        }),
        Err(refusal) => {
            let mut payload = json!({
                "dispatched": false,
                "namespace": namespace,
                "command": command,
                "error": refusal.sentinel(),
                "detail": refusal.detail(),
            });
            // `retryAfterMs` present ONLY on the throttle refusal (issue
            // #1155) — matching `agent_read::sentinel_refusal_reply`'s
            // `extra` merge, which likewise omits the key entirely for
            // every other refusal. A client keying on presence
            // (`if ('retryAfterMs' in p) wait(...)`) must see the SAME
            // presence/absence split on both tiers; an always-present
            // `null` would make it wait 0 ms on a non-throttle refusal.
            if let Refusal::RateLimited { retry_after_ms } = &refusal {
                payload["retryAfterMs"] = json!(retry_after_ms);
            }
            payload
        }
    };
    json!({
        "type": super::super::msg::AGENT_CALL_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Longest caller-supplied `reqId`/`namespace`/`command` a REFUSAL reply echoes back, in bytes —
/// all three arrive bounded only by the incoming frame cap (8 MiB), so echoing them verbatim could
/// itself exceed the cap it exists to enforce (HIGH — security review: a ~8.38 MB `command` blew
/// [`enforce_frame_cap`]'s own substitute measure). 256 is far above the real [`POLICY`] table's
/// longest command name; pinned against the TABLE, not a copy of this number.
pub(super) const REFUSAL_IDENT_CAP: usize = 256;

/// A [`REFUSAL_IDENT_CAP`]-bounded prefix of a caller-supplied identifier,
/// cut on a CHAR BOUNDARY — `&value[..256]` panics mid-codepoint, and this
/// crate is `panic = "abort"` in release, so a slice panic inside a frame
/// handler is a silent process death, not an error.
///
/// Applied on REFUSAL replies only, never on the success path: a real command
/// name is short, and a long one is already `unknown_command`, so clamping
/// there could only make a legitimate reply lie about which command produced
/// it. `pub(super)` (#1151) — reused by `agent_read`'s own refusal builder.
pub(in crate::extension_bridge) fn clamp_ident(value: &str) -> &str {
    if value.len() <= REFUSAL_IDENT_CAP {
        return value;
    }
    let mut end = REFUSAL_IDENT_CAP;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

/// `detail` on [`refusal_reply`]'s last-resort envelope. A FIXED string, so that envelope's size
/// cannot be influenced by anything the caller sent. `pub(super)` (#1151) — reused by `agent_read`.
pub(in crate::extension_bridge) const REFUSAL_UNDELIVERABLE_DETAIL: &str =
    "this call's refusal did not fit the bridge's frame cap, so its identifiers and detail \
     were dropped in order to deliver any reply at all";

/// The ONE builder for every refusal on this surface — the success path keeps calling
/// [`call_result_reply`] directly. Bounded material only (the three echoed identifiers go through
/// [`clamp_ident`]; every `detail` is a fixed literal, a `'static` reason, or an already-capped
/// string), AND measured, not assumed: re-measures the built reply anyway and degrades to a
/// minimal envelope if it somehow still doesn't fit, so a refusal that blew the cap never
/// reproduces the content-free `connection_lost` issue #1135 exists to eliminate.
pub(super) fn refusal_reply(
    req_id: &str,
    namespace: &str,
    command: &str,
    refusal: Refusal,
) -> String {
    let sentinel = refusal.sentinel();
    let reply = call_result_reply(
        clamp_ident(req_id),
        clamp_ident(namespace),
        clamp_ident(command),
        Err(refusal),
    );
    if reply.len() <= super::super::MAX_FRAME_BYTES {
        return reply;
    }
    json!({
        "type": super::super::msg::AGENT_CALL_RESULT,
        "reqId": "",
        "payload": {
            "dispatched": false,
            "namespace": "",
            "command": "",
            "error": sentinel,
            "detail": REFUSAL_UNDELIVERABLE_DETAIL,
        },
    })
    .to_string()
}

/// Reply for an `agent.call` arriving over a connection whose handshake
/// `Origin` wasn't `auth::AGENT_CLI_ORIGIN` — mirrors
/// `agent_read::origin_refused_reply` exactly, one wire type over. Built
/// through [`refusal_reply`]: this path never reaches [`enforce_frame_cap`]
/// (`mod.rs` writes what it returns straight to the socket), so the clamp is
/// the ONLY thing bounding what it echoes.
pub(in crate::extension_bridge) fn origin_refused_reply(req_id: &str, payload: &Value) -> String {
    let (namespace, command) = payload_target(payload);
    refusal_reply(req_id, namespace, command, Refusal::OriginRefused)
}

/// Same never-reaches-[`enforce_frame_cap`] path as [`origin_refused_reply`], bounded the same
/// way. `retry_after_ms` comes from the one caller, `mod.rs` (issue #1155).
pub(in crate::extension_bridge) fn throttled_reply(
    req_id: &str,
    payload: &Value,
    retry_after_ms: u64,
) -> String {
    let (namespace, command) = payload_target(payload);
    let refusal = Refusal::RateLimited { retry_after_ms };
    refusal_reply(req_id, namespace, command, refusal)
}

/// Whether an `agent.call` payload names a policy row whose [`Effect`] is `Read` — the extension
/// read tier's own gate (PR1, decision 1). Unlike the CLI (which also dispatches `Reversible`,
/// and — after a confirm ceremony — `Irreversible`), the paired extension may reach ONLY `Read`
/// rows; an unknown `(namespace, command)` pair reads as "not Read" here too (refused, never
/// dispatched — [`dispatch`]'s own `UnknownCommand` refusal is what a caller sees either way).
/// Pure — reuses the SAME [`find_policy`] lookup [`dispatch`] itself uses, never a second policy
/// table or a duplicated `(namespace, command)` split.
pub(in crate::extension_bridge) fn extension_may_dispatch(payload: &Value) -> bool {
    let (namespace, command) = payload_target(payload);
    matches!(
        find_policy(namespace, command).map(|entry| entry.effect),
        Some(Effect::Read)
    )
}

/// Reply for an `agent.call` from the paired EXTENSION caller while Assisted autofill is off
/// (PR1, decision 2) — mirrors `agent_read::extension_gate_reply` one wire type over; see
/// [`Refusal::ExtensionReadGate`].
pub(in crate::extension_bridge) fn extension_gate_reply(req_id: &str, payload: &Value) -> String {
    let (namespace, command) = payload_target(payload);
    refusal_reply(req_id, namespace, command, Refusal::ExtensionReadGate)
}

/// Reply for an `agent.call` from the paired EXTENSION caller that named a row whose `Effect`
/// isn't `Read` — see [`extension_may_dispatch`] and [`Refusal::EffectNotAllowedForExtension`].
pub(in crate::extension_bridge) fn effect_not_allowed_reply(
    req_id: &str,
    payload: &Value,
) -> String {
    let (namespace, command) = payload_target(payload);
    refusal_reply(
        req_id,
        namespace,
        command,
        Refusal::EffectNotAllowedForExtension,
    )
}

/// The extension caller's own reply cap, enforced ON TOP of [`enforce_frame_cap`]'s generic check
/// — an ordinary CLI reply legitimately runs larger, so `stream::spawn_agent_call` applies this
/// only for `CallerClass::Extension`. Built directly (not via [`refusal_reply`]) so `detail` names
/// the SMALLER cap actually enforced; still bounded the same way (`clamp_ident` on every identifier).
pub(in crate::extension_bridge) fn extension_capped_reply(
    req_id: &str,
    payload: &Value,
    reply: String,
) -> String {
    if reply.len() <= super::super::EXTENSION_RESULT_MAX_BYTES {
        return reply;
    }
    let (namespace, command) = payload_target(payload);
    json!({
        "type": super::super::msg::AGENT_CALL_RESULT,
        "reqId": clamp_ident(req_id),
        "payload": {
            "dispatched": false,
            "namespace": clamp_ident(namespace),
            "command": clamp_ident(command),
            "error": ERR_RESULT_TOO_LARGE,
            "detail": format!(
                "the command RAN, but its reply ({} B) exceeds the extension caller's own {} \
                 KiB cap and was discarded rather than truncated",
                reply.len(),
                super::super::EXTENSION_RESULT_MAX_BYTES / 1024
            ),
        },
    })
    .to_string()
}

pub(super) fn payload_target(payload: &Value) -> (&str, &str) {
    let namespace = payload
        .get("namespace")
        .and_then(Value::as_str)
        .unwrap_or("");
    let command = payload.get("command").and_then(Value::as_str).unwrap_or("");
    (namespace, command)
}

/// The throttle key `agent.call` draws from — reuses `BridgeState::try_acquire_agent`'s EXISTING
/// two buckets: `autopilot_best_matches` runs the SAME uncapped clustering pass `agent_read`'s
/// `best-matches` resource already rate-limits tightly, so it maps into that resource's own bucket
/// key rather than letting a caller double an allowance by alternating tiers. Every other command
/// falls into the shared cheap bucket. Note: an `Irreversible` row's proof-resolution read
/// (`proof::resolve`) dispatches a second, internal command not separately throttled here — bounded
/// to one extra read per confirm attempt, too small to need its own bucket.
pub(in crate::extension_bridge) fn throttle_key(command: &str) -> &str {
    if command == "autopilot_best_matches" {
        "best-matches"
    } else {
        command
    }
}

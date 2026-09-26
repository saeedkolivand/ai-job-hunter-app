//! Reply builders for `agent.query` — the bounded/clamped envelope every outcome on this tier
//! goes through, plus the throttle/origin/extension-gate refusals. Split out of `agent_read.rs`
//! under the R8 LOC cap; `handle_agent_query` itself stays in the entry file.

use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

use super::job::resolve::{JOB_NOT_FOUND_DETAIL, JOB_NOT_FOUND_MESSAGE};
use super::{RES_FOUND_JOBS, RES_JOB};

// ── Dispatch ─────────────────────────────────────────────────────────────

/// The resource named by an `agent.query` payload — `""` when absent/not a
/// string. Used both to route dispatch and to pick the throttle bucket.
pub(in crate::extension_bridge) fn resource_name(payload: &Value) -> &str {
    payload
        .get("resource")
        .and_then(Value::as_str)
        .unwrap_or("")
}

/// `(resource, fixed error sentinel) -> fixed detail`, for the handful of
/// refusals whose caller needs a next step rather than a bare sentinel
/// (issue #1166). Both sides of the match are compile-time constants, so
/// this can never echo caller-supplied content into `detail`.
pub(in crate::extension_bridge::agent_read) fn error_detail(
    resource: &str,
    error: &str,
) -> Option<&'static str> {
    match (resource, error) {
        (RES_JOB, JOB_NOT_FOUND_MESSAGE) => Some(JOB_NOT_FOUND_DETAIL),
        _ => None,
    }
}

pub(in crate::extension_bridge::agent_read) fn agent_result_reply(
    req_id: &str,
    resource: &str,
    outcome: AppResult<Value>,
) -> String {
    let payload = match outcome {
        Ok(data) => json!({ "ok": true, "resource": resource, "data": data }),
        // Wire-error discipline: `AppError`'s `Display` here is always a fixed sentinel or an echo
        // of the CALLER'S OWN `resource`/`url` input, never path/PII content. `detail` (issue
        // #1166) is looked up off that SAME fixed sentinel and omitted when there is none.
        Err(e) => {
            let error = e.to_string();
            let mut payload = json!({ "ok": false, "resource": resource, "error": error });
            if let Some(detail) = error_detail(resource, &error) {
                payload["detail"] = json!(detail);
            }
            payload
        }
    };
    json!({
        "type": super::super::msg::AGENT_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

// ── Bounded refusals (issue #1151 — this tier had no equivalent to `agent_call::refusal_reply`/
// `enforce_frame_cap`, so a refusal built from a near-cap `resource`/`reqId` could itself exceed
// the frame cap, and an uncapped `job`/`best-matches` success reply closed the socket with no
// refusal at all) ──

/// A [`super::super::agent_call::clamp_ident`]-bounded, sentinel+detail refusal — the shape `agent_call::refusal_reply` uses,
/// for the MACHINE-READABLE refusals this tier gained from issues #1151/#1155. Every OTHER refusal
/// keeps its existing bare-`error` shape via [`bounded_result_reply`] instead. `extra` merges
/// additional fields onto the payload. Re-measures the built reply and degrades to a minimal
/// envelope if it still doesn't fit.
fn sentinel_refusal_reply(
    req_id: &str,
    resource: &str,
    error: &'static str,
    detail: String,
    extra: Value,
) -> String {
    let mut payload = json!({
        "ok": false,
        "resource": super::super::agent_call::clamp_ident(resource),
        "error": error,
        "detail": detail,
    });
    if let (Value::Object(base), Value::Object(more)) = (&mut payload, &extra) {
        for (k, v) in more {
            base.insert(k.clone(), v.clone());
        }
    }
    let reply = json!({
        "type": super::super::msg::AGENT_RESULT,
        "reqId": super::super::agent_call::clamp_ident(req_id),
        "payload": payload,
    })
    .to_string();
    if reply.len() <= super::super::MAX_FRAME_BYTES {
        return reply;
    }
    json!({
        "type": super::super::msg::AGENT_RESULT,
        "reqId": "",
        "payload": {
            "ok": false,
            "resource": "",
            "error": error,
            "detail": super::super::agent_call::REFUSAL_UNDELIVERABLE_DETAIL,
        },
    })
    .to_string()
}

/// [`agent_result_reply`], with `resource`/`reqId` pre-clamped and the built reply re-measured
/// against [`super::super::MAX_FRAME_BYTES`] (issue #1151), applied to EVERY reply this tier builds
/// (success included, mirroring `agent_call::handle_agent_call`'s single `enforce_frame_cap` call
/// site): an oversized reply is substituted with a `result_too_large` refusal rather than closing
/// the socket with nothing (the #1135 failure mode this mirrors).
pub(super) fn bounded_result_reply(
    req_id: &str,
    resource: &str,
    outcome: AppResult<Value>,
) -> String {
    let reply = agent_result_reply(
        super::super::agent_call::clamp_ident(req_id),
        super::super::agent_call::clamp_ident(resource),
        outcome,
    );
    if reply.len() <= super::super::MAX_FRAME_BYTES {
        return reply;
    }
    sentinel_refusal_reply(
        req_id,
        resource,
        super::super::agent_call::ERR_RESULT_TOO_LARGE,
        format!(
            "the reply ({} B) exceeds the bridge's own frame cap and was discarded rather than \
             truncated — narrow the request (a smaller `limit`, a `found-jobs` page) if this \
             resource takes one",
            reply.len()
        ),
        json!({}),
    )
}

// `pub(super)` — reused verbatim by `agent_call`'s own throttle refusal
// (Phase 2, ADR-038 §2) so the two tiers report identical wording for the
// identical shared-bucket cause, never a second hand-typed copy.
pub(in crate::extension_bridge) const THROTTLED_MESSAGE: &str =
    "Too many requests — try again shortly.";

/// The caller-supplied argument that names WHICH request a throttle refusal belongs to, beyond
/// `resource` alone (issue #1155 — a throttled `job` lookup used to echo only `"resource":"job"`,
/// never which of several in-flight urls was refused). `job` keys on `url`, `found-jobs` on
/// `autopilotId`. Clamped like every other echoed identifier here.
fn identity_arg<'a>(resource: &str, payload: &'a Value) -> Option<(&'static str, &'a str)> {
    let field = match resource {
        RES_JOB => "url",
        RES_FOUND_JOBS => "autopilotId",
        _ => return None,
    };
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(|v| (field, super::super::agent_call::clamp_ident(v)))
}

/// The read tier's own `rate_limited` refusal (issue #1155) — the SAME sentinel+detail shape,
/// SAME `retryAfterMs`, as `agent_call::throttled_reply`'s: `retry_after_ms` is computed by the
/// ONE caller (`mod.rs`) right after the failed acquire, never invented here. Adds the refused
/// request's identity via [`identity_arg`] so a caller juggling several in-flight lookups can tell
/// WHICH one was blocked.
pub(in crate::extension_bridge) fn throttled_reply(
    req_id: &str,
    payload: &Value,
    retry_after_ms: u64,
) -> String {
    let resource = resource_name(payload);
    let mut extra = json!({ "retryAfterMs": retry_after_ms });
    if let Some((field, value)) = identity_arg(resource, payload) {
        extra[field] = json!(value);
    }
    sentinel_refusal_reply(
        req_id,
        resource,
        super::super::agent_call::ERR_RATE_LIMITED,
        THROTTLED_MESSAGE.to_string(),
        extra,
    )
}

/// Fixed sentinel — `msg::AGENT_QUERY`'s doc; never dynamic content (matches
/// every other refusal on this surface).
pub(super) const CLI_ONLY_MESSAGE: &str =
    "agent.query is only available to the ajh-tauri agent CLI";

/// Reply for an `agent.query` arriving over a connection whose handshake `Origin` wasn't
/// `auth::AGENT_CLI_ORIGIN` (finding #5, security review) — same `agent.result` envelope shape as
/// every other outcome here. Routed through [`bounded_result_reply`] (issue #1151) rather than
/// [`agent_result_reply`] directly since this path writes straight to the socket.
pub(in crate::extension_bridge) fn origin_refused_reply(req_id: &str, payload: &Value) -> String {
    bounded_result_reply(
        req_id,
        resource_name(payload),
        Err(AppError::Validation(CLI_ONLY_MESSAGE.to_string())),
    )
}

/// [`ERR_EXTENSION_READ_GATE`]'s detail text — the extension read tier's own consent gate (PR1,
/// decision 2), distinct from [`CLI_ONLY_MESSAGE`] (the CLI has no such gate at all) and from
/// `super::super::AUTOFILL_OFF_MESSAGE` (that one names `profile.get`/`answers.save` specifically; this
/// one is the generic agent surface).
const EXTENSION_READ_GATE_DETAIL: &str =
    "Turn on Assisted autofill in AI Job Hunter → Settings → Browser extension to let \
     the paired browser extension read your data.";

/// Reply for an `agent.query` from the paired EXTENSION caller while Assisted autofill is off
/// (PR1, decision 2) — no partial data, ever. Distinct from [`origin_refused_reply`]: this caller
/// origin-checked fine, but hasn't opted the extension into reading its data yet. Sentinel reused
/// verbatim from `agent_call::ERR_EXTENSION_READ_GATE` so both tiers report the same cause identically.
pub(in crate::extension_bridge) fn extension_gate_reply(req_id: &str, payload: &Value) -> String {
    sentinel_refusal_reply(
        req_id,
        resource_name(payload),
        super::super::agent_call::ERR_EXTENSION_READ_GATE,
        EXTENSION_READ_GATE_DETAIL.to_string(),
        json!({}),
    )
}

/// The extension caller's own reply cap, enforced ON TOP of the generic
/// [`super::super::MAX_FRAME_BYTES`] cap [`bounded_result_reply`] already applies, since an
/// ordinary CLI reply legitimately runs larger (`stream::spawn_agent_query` applies this only for
/// `CallerClass::Extension`).
pub(in crate::extension_bridge) fn extension_capped_reply(
    req_id: &str,
    payload: &Value,
    reply: String,
) -> String {
    if reply.len() <= super::super::EXTENSION_RESULT_MAX_BYTES {
        return reply;
    }
    sentinel_refusal_reply(
        req_id,
        resource_name(payload),
        super::super::agent_call::ERR_RESULT_TOO_LARGE,
        format!(
            "the reply ({} B) exceeds the extension caller's own {} KiB cap and was discarded \
             rather than truncated",
            reply.len(),
            super::super::EXTENSION_RESULT_MAX_BYTES / 1024
        ),
        json!({}),
    )
}

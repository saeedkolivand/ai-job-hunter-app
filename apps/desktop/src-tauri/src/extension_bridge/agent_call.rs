//! `agent.call` → `agent.call.result` — ADR-038 §2's generic dispatch tier
//! (`agent call <namespace>:<command> --input '<json>'`). [`Effect::Read`]
//! AND [`Effect::Reversible`] rows dispatch directly through
//! [`tauri::Webview::on_message`] (Phase 4) — the caller can undo either
//! through the app, which is what those two classes mean. An
//! [`Effect::Irreversible`] row dispatches only after a `--confirm` ceremony
//! (Phase 3, ADR-038 §4): a call with no `confirm` refuses with
//! [`Refusal::ConfirmationRequired`], naming WHICH other read surface the
//! proof value comes from and NEVER the value itself; a wrong `confirm`
//! refuses with [`Refusal::ConfirmationMismatch`], which likewise never
//! discloses the expected value. [`Effect::NotExposed`] always refuses. A
//! dispatched command that comes back as `InvokeResponse::Err` — the body
//! ran and returned a typed `Err`, or Tauri rejected the call before the
//! body ran at all (bad args, ACL denial, unknown command) — ALSO refuses,
//! with [`Refusal::InvokeError`]: it is never folded into `dispatched: true`
//! (see that variant's own doc for why the two causes are indistinguishable
//! on the wire and both must refuse).
//!
//! ## Dispatch mechanism (verified against the vendored tauri 2.11.5
//! source, not docs.rs — ADR-038's own "verified" note)
//! `Webview::on_message` is `pub`; every `InvokeRequest` field is `pub`;
//! `AppHandle::invoke_key` is `pub` and its own doc names this EXACT use
//! ("Gets the invoke key that must be referenced when using
//! `crate::webview::InvokeRequest`"). Driving it this way runs the REAL,
//! registered command body in the app's own process against its single
//! managed state — so `limits::Limiter`/`charge_provider_daily` (which live
//! INSIDE command bodies, never in a wrapper — `commands/ai/mod.rs`) still
//! apply exactly as they do for the renderer. No codegen, no second copy of
//! any command's logic, no call-the-Rust-fn-directly shortcut that would
//! bypass those limits. The SAME mechanism resolves an `Irreversible` row's
//! proof value too (`proof::resolve` dispatches its `read_command` through
//! this exact path) — never a second implementation of a command's logic.
//!
//! `url` is the running app's OWN "main" `WebviewWindow`'s CURRENT url
//! (`WebviewWindow::url()`), never a guessed/hardcoded literal —
//! `on_message`'s private `is_local_url` only compares scheme+domain against
//! the app's own protocol origin, so reading the real webview's real address
//! is what makes this genuinely mirror what the renderer itself sends, on
//! every platform and dev-vs-prod combination, rather than hardcoding one of
//! `tauri://localhost` / `https://tauri.localhost` and silently breaking on
//! the other. `invoke_key` is read fresh off `AppHandle::invoke_key()` on
//! every call and NEVER logged/echoed/returned — its own doc: "DO NOT expose
//! this key to third party scripts as might grant access to the backend
//! from external URLs and iframes."
//!
//! R8 LOC-cap split (`docs/architecture-rules.md`): this file keeps the module doc, the `mod`
//! table and [`handle_agent_call`] itself (the one entry point); everything else moved to a
//! sibling file by concern — namespace/command lookup ([`policy_lookup`]), the `Refusal`
//! vocabulary (`refusal`), the reply builders (`reply`), and the real dispatch machinery
//! (`dispatch`) — alongside the pre-existing `proof`/`reshape`/`validate`/`dispatch_plan`/`fence`
//! splits.

use serde_json::{json, Value};
use tauri::AppHandle;

mod dispatch;
mod dispatch_plan;
mod policy_lookup;
mod proof;
// The agent layer's own payload reshaping — the outbound fence/page/base64
// order and the inbound fence-strip mirror — lives in its own file under the
// R8 LOC cap; see `agent_call/reshape.rs`. Visible to the rest of
// `extension_bridge` for the three items `agent_cli::mcp` and `agent_read`
// read through it, the same shape `agent_read` uses for `found_jobs`.
mod refusal;
mod reply;
pub(in crate::extension_bridge) mod reshape;
// Dispatch-time input-key validation against the generated
// `agent_cli::catalogue` (issues #1163, #1158, #1160) — its own file under
// the same R8 LOC-cap reasoning as `proof`/`reshape` above.
mod validate;

use dispatch::{dispatch, invoke_command, InvokeOutcome};
// The pure `gate`/`plan` ordering decision — same R8 LOC-cap reasoning again. `dispatch.rs` reads
// both through its own direct sibling import, not this re-export; this one is solely for
// `#[cfg(test)]` consumers outside `agent_call::dispatch`'s own subtree —
// `extension_bridge::test`'s exhaustive `agent_call::gate`/`Dispatch` walk (a COUSIN module) and
// `validate::tests::ordering`'s real-`plan`-driven ordering tests — so the whole re-export is
// `#[cfg(test)]` too.
#[cfg(test)]
pub(super) use dispatch_plan::{gate, plan, Dispatch};
pub(super) use policy_lookup::{
    invalid_input_detail, namespace_suggestion, proof_field_for, proof_kind_for, split_path,
    unknown_command_detail,
};
use refusal::Refusal;
#[cfg(test)]
use refusal::ERR_EFFECT_NOT_ALLOWED_FOR_EXTENSION;
pub(super) use refusal::{
    ERR_CONFIRMATION_REQUIRED, ERR_EXTENSION_READ_GATE, ERR_INVALID_INPUT, ERR_NOT_EXPOSED,
    ERR_RATE_LIMITED, ERR_RESULT_TOO_LARGE, ERR_UNKNOWN_COMMAND,
};
use reply::{call_result_reply, payload_target, refusal_reply};
pub(super) use reply::{
    clamp_ident, effect_not_allowed_reply, extension_capped_reply, extension_gate_reply,
    extension_may_dispatch, origin_refused_reply, throttle_key, throttled_reply,
    REFUSAL_UNDELIVERABLE_DETAIL,
};

// ── Fencing scraped job-posting text (a different axis from the raw-data
// decision above — ADR-038's own amendment paragraph) — moved to its own
// file under the R8 LOC cap; see `agent_call/fence.rs`. `fence_scraped_fields`
// is the one entry point `reshape.rs`/`proof.rs` (their own `use super::*`)
// call; `dispatch_direct` (below) reaches it only indirectly, through
// `reshape_reply`/`fence_reply`, so this import is `#[cfg(test)]`-only —
// `agent_call::tests` (`use super::*`) is the one caller left that exercises
// it directly, against hand-built fixtures, the same reason `dispatch_plan::gate`
// just above is re-exported test-only too.
#[cfg(test)]
use fence::fence_scraped_fields;
mod fence;

/// Substitute a [`Refusal::ResultTooLarge`] reply for any `reply` the bridge
/// could not actually deliver — over [`super::MAX_FRAME_BYTES`], the cap both
/// ends of this socket configure (issue #1135; see that variant's own doc for
/// why an outgoing frame is otherwise unchecked and what the caller saw
/// instead). Pure, and returns the RECOMPUTED `dispatched` alongside the
/// reply so the observability span records what actually went on the wire
/// rather than what dispatch alone decided — measuring the built reply is the
/// only way to know, so this cannot live any earlier.
///
/// Note the asymmetry it deliberately preserves: `dispatched` on the wire
/// becomes `false` (no result was delivered, and every consumer — including
/// `agent_cli::exit_code_for_reply`'s exit-2 mapping — reads it that way),
/// while the refusal's own `detail` states plainly that the command RAN.
///
/// The substitute is NOT "always far smaller than the original" — that was
/// the false absolute this doc used to claim (HIGH, security review). Its
/// `data` is gone, but it still echoes `reqId`/`namespace`/`command`, and all
/// three are caller-supplied and bounded only by the 8 MiB INCOMING frame: a
/// ~8.38 MB `command` produced a substitute of 8,389,135 B against an
/// 8,388,608 B ceiling, i.e. a refusal that reproduced the failure it
/// reports. What holds instead is a BOUNDED argument, and it lives in
/// [`refusal_reply`]: the identifiers are clamped to [`REFUSAL_IDENT_CAP`],
/// every `detail` is bounded by construction, and the built reply is
/// re-measured with a minimal-envelope fallback. Hence this fn no longer
/// builds the substitute itself.
fn enforce_frame_cap(
    req_id: &str,
    namespace: &str,
    command: &str,
    reply: String,
    dispatched: bool,
) -> (String, bool) {
    if reply.len() <= super::MAX_FRAME_BYTES {
        return (reply, dispatched);
    }
    let refused = refusal_reply(
        req_id,
        namespace,
        command,
        Refusal::ResultTooLarge(reply.len()),
    );
    (refused, false)
}

/// Answer an authenticated, throttle-admitted, origin-checked `agent.call`.
/// Never panics — [`dispatch`] degrades to a [`Refusal`] on every failure
/// path (unknown command, wrong effect, or the dispatch itself erroring).
/// Logs the command identity + whether it dispatched (MEDIUM fix — security
/// review: every other privileged bridge path leaves an observability
/// record, this one didn't) — NEVER `input`/`confirm`/the response `data`,
/// every one of which can carry PII or a résumé/cover-letter body.
pub(super) async fn handle_agent_call(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let (namespace, command) = {
        let (ns, cmd) = payload_target(payload);
        (ns.to_string(), cmd.to_string())
    };
    let input = payload.get("input").cloned().unwrap_or_else(|| json!({}));
    let confirm = payload.get("confirm").and_then(Value::as_str);

    let span = crate::observability::Span::begin(
        "agent_call",
        format!("namespace={namespace} command={command}"),
    );
    let outcome = dispatch(app, &namespace, &command, input, confirm).await;
    let dispatched = outcome.is_ok();
    // Success builds from the raw builder (identifiers verbatim); EVERY
    // refusal goes through `refusal_reply`, which is where the identifier
    // clamp lives — a caller-supplied `command` of any length arrives here as
    // `Refusal::UnknownCommand` long before it could reach the frame cap.
    let reply = match outcome {
        Ok(data) => call_result_reply(req_id, &namespace, &command, Ok(data)),
        Err(refusal) => refusal_reply(req_id, &namespace, &command, refusal),
    };
    let (reply, dispatched) = enforce_frame_cap(req_id, &namespace, &command, reply, dispatched);
    span.end_with(&format!("dispatched={dispatched}"), dispatched);
    reply
}

#[cfg(test)]
mod tests;

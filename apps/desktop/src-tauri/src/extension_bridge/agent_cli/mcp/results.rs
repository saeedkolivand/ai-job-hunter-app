//! Reply-shaping for `tools/call` — the `CallToolResult` envelope every dispatched or locally
//! refused payload goes through on the way out, plus the three fixed refusal shapes this module
//! owns (`result_too_large`, `server_busy`, `shutting_down`). R8 LOC-cap split
//! (`docs/architecture-rules.md`), the same move `mcp/instructions.rs`/`mcp/schemas.rs` already
//! made: this is the RESULT-WRAPPING unit, so nothing about the protocol loop, dispatch or
//! classification travelled with it — those stay in `mcp.rs`, which imports the three items it
//! calls (`tool_result`, `busy_result`, `shutting_down_result`). `mcp::tests` reaches
//! [`oversized_result`]/[`MCP_RESULT_MAX_BYTES`] directly through this module's own path, the
//! same shape it already uses for `mcp::instructions`'s items.

use serde_json::{json, Value};

use super::agent_call;
use super::MCP_CALL_QUEUE_MAX;

const CONFIRMATION_NOTE: &str = "This command is Effect::Irreversible and was called with no \
    proof (exitCode 4). \"detail\" above names the read command and field the proof comes from. \
    Call call-read for that command, take the named field from its result, then retry this exact \
    call-irreversible with confirm set to it VERBATIM (including any fence wrapper and its \
    newlines) — the value is never disclosed by this refusal.";

/// A payload's serialized `content[0].text` length above which [`tool_result`] refuses rather
/// than returning it: `documents_export_document` (PDF bytes as a `number[]`) and
/// `documents_render_preview_images` are `Read` and auto-approved by most clients, and a local
/// refusal can echo a caller-chosen `namespace`/`command` of any length — nothing else bounded
/// either path but the bridge's own 8 MiB `MAX_FRAME_BYTES` WS frame limit. 256 KiB is
/// comfortably above every legitimate payload observed and comfortably below either oversized
/// case.
pub(super) const MCP_RESULT_MAX_BYTES: usize = 256 * 1024;

/// The refusal [`tool_result`] substitutes for ANY payload over [`MCP_RESULT_MAX_BYTES`] — this
/// fn no longer takes the triggering `Verb` (review round 3): `detail` is addressed to the HUMAN
/// reading the transcript, never to the model (MEDIUM fix — naming `agent call ns:cmd` here was a
/// working bypass recipe handed to the exact agent this cap exists to bound, since Claude Code has
/// Bash), and this refusal now also fires from local refusals that have no single command to name.
/// `bytes` is the length actually measured, never an estimate. Mirrors every other `Verb::Call`
/// refusal's own `dispatched:false` shape rather than a bespoke `ok:false` (LOW fix) — no
/// `namespace`/`command` here, since not every payload this wraps has one.
pub(super) fn oversized_result(bytes: usize) -> Value {
    json!({
        "dispatched": false,
        // The app-side frame cap refuses with this SAME sentinel (issue #1135) — one
        // definition of the string, in `agent_call`, never a second hand-typed copy here.
        "error": agent_call::ERR_RESULT_TOO_LARGE,
        "bytes": bytes,
        // Same warning the app-side twin carries (`agent_call::Refusal::ResultTooLarge`):
        // `dispatched:false` here means no result was delivered, NOT that nothing happened —
        // this cap can fire on the reply to a call that already took effect.
        "detail": format!(
            "payload exceeds the server's result cap ({bytes} B); narrow the query, or ask \
             the user to run it outside this session. The command may already have run and \
             only its reply was discarded, so do not re-send a mutating call on this refusal."
        ),
    })
}

/// The refusal answered when the call queue is full — see `mcp.rs`'s own module doc concurrency
/// section for why the excess call is refused rather than blocking the writer thread until there
/// is room. `dispatched:false`, like every other refusal that never reached the wire; the
/// `detail` is a plain instruction to wait, because unlike `result_too_large` this one IS worth
/// repeating.
pub(super) fn busy_result() -> Value {
    json!({
        "dispatched": false,
        "error": "server_busy",
        "detail": format!(
            "this server dispatches one call at a time and its queue is full \
             ({MCP_CALL_QUEUE_MAX} waiting); wait for an outstanding call's reply, then send \
             this one again."
        ),
    })
}

/// The refusal written for a call the EOF drain deadline expired on — see `mcp.rs`'s own module
/// doc EOF bullet. Two shapes behind one sentinel, because the honest answer differs by exactly
/// one fact the loop knows: `in_flight` is `dispatch` reaching `false` for a call the worker
/// never started (`abandoned` is set before this is written, so it never will) and `true` for the
/// one call single-flight FIFO order allows to be running, whose reply was never received.
///
/// `dispatched` therefore means what it means everywhere else here — did this call reach the app
/// — and the uncertainty that belongs to the `true` case (did it take effect?) is stated in
/// `detail` rather than smuggled into that boolean. Answering both as `dispatched:false` would be
/// the dangerous direction: a client re-sending a write it was told never landed.
pub(super) fn shutting_down_result(in_flight: bool) -> Value {
    json!({
        "dispatched": in_flight,
        "error": "shutting_down",
        "detail": if in_flight {
            "this server's input closed and its shutdown deadline expired while this call was \
             still in flight; its result was never received and it may already have taken \
             effect — re-read the affected resource before sending it again."
        } else {
            "this server's input closed and its shutdown deadline expired before this call was \
             dispatched; it never reached the app. Send it again to a new server."
        },
    })
}

/// One `CallToolResult`: `content[0].text` is the payload byte-for-byte, `content[1]` names the
/// exit code, and a `confirmation_required` refusal gets one more block mapping `--confirm` to
/// this tool's `confirm` argument. No `structuredContent` field (SHOULD fix — no observed client
/// surfaces it to the model, and it doubled every PII-bearing payload in the client's persisted
/// transcript for nothing). ALSO the ONE place [`MCP_RESULT_MAX_BYTES`] is enforced (moved here,
/// review round 3 — see [`oversized_result`]'s own doc), so every payload this fn ever wraps is
/// covered, not only a dispatched command's own reply; the size is measured exactly once, via the
/// SAME `to_string()` this fn needs anyway for `content[0].text`.
pub(super) fn tool_result(payload: Value, exit_code: i32) -> Value {
    let text = payload.to_string();
    let (text, exit_code, payload) = if text.len() > MCP_RESULT_MAX_BYTES {
        let refusal = oversized_result(text.len());
        (refusal.to_string(), 2, refusal)
    } else {
        (text, exit_code, payload)
    };
    let mut content = vec![
        json!({ "type": "text", "text": text }),
        json!({ "type": "text", "text": format!("exitCode: {exit_code}") }),
    ];
    if payload.get("error").and_then(Value::as_str) == Some(agent_call::ERR_CONFIRMATION_REQUIRED) {
        content.push(json!({ "type": "text", "text": CONFIRMATION_NOTE }));
    }
    json!({
        "content": content,
        "isError": exit_code != 0,
    })
}

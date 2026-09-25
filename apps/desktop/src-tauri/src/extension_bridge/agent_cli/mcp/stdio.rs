//! The stdio JSON-RPC transport — `agent mcp`'s default wire, split out from `mcp.rs` for the
//! same R8 LOC-cap reason `instructions.rs`/`schemas.rs`/`results.rs`/`resources.rs`/`prompts.rs`
//! already were, and the sibling unit to `mcp/http.rs`: that module owns the Streamable HTTP
//! wire, this one owns the three-thread stdio wire the module doc's "Concurrency" section
//! describes in full — [`serve`] is the loop itself, [`Event`]/[`emit`]/[`forget_in_flight`]/
//! [`stop_serving`] are its own bookkeeping. Reaches everything else in `mcp.rs` (`Server`,
//! `Routed`, `PendingKind`, `route_line`, `reply_frame`, `rpc_error`, `dispatched_tool_result`,
//! `resources::dispatched_resource_result`, `results::{busy_result, shutting_down_result,
//! tool_result}`, the `MCP_CALL_QUEUE_MAX`/`MCP_EVENT_QUEUE_MAX` bounds) through its own
//! `use super::*;`, never a second copy of any of it.

use super::*;

/// What the main thread selects over: input from the reader thread, replies from the worker
/// thread. One channel, two producers — so a reply and a line can never be missed for each other.
pub(super) enum Event {
    Line(String),
    /// The input ended (EOF, or a read error, which ends it the same way).
    Eof,
    Reply(Value),
}

/// The sole stdout writer once the JSON-RPC loop is running (see the module doc's "Stdout/stderr
/// discipline" section) — a compact `Value`'s own `Display` (never a pretty-printed one), one
/// `writeln!` call. `Err` here (EPIPE once the client closes its end of the pipe) is the caller's
/// cue to stop, never retried and never a panic.
///
/// The flush is explicit rather than left to [`std::io::Stdout`]'s own line buffering: this
/// server is a request/response peer whose client blocks on a reply before sending its next
/// frame, so a frame still sitting in a buffer is a deadlock, not a latency detail — and the
/// handle we write through is chosen for `Send`-ness (module doc), not for its buffering
/// strategy, so this must not depend on which one it happens to be.
pub(super) fn emit(output: &mut impl Write, frame: &Value) -> std::io::Result<()> {
    writeln!(output, "{frame}")?;
    output.flush()
}

/// Drop the id `frame` answers from the still-owed list, if it is there — the bookkeeping half of
/// the EOF guarantee (module doc). Removes ONE entry, so a client that reused an id across two
/// calls still has both tracked; a frame whose id matches nothing leaves the list untouched
/// rather than shortening it under a later, real reply.
pub(super) fn forget_in_flight(in_flight: &mut Vec<(Value, PendingKind)>, frame: &Value) {
    let Some(id) = frame.get("id") else { return };
    if let Some(pos) = in_flight.iter().position(|(owed, _)| owed == id) {
        in_flight.remove(pos);
    }
}

/// The ONE way [`serve`] leaves early, and the reason it is a function: every exit that stops
/// WRITING must first stop DISPATCHING, in that order. The drain-deadline exit did; the
/// write-failure ones returned a bare `0`, leaving the worker free to spend a bridge round trip
/// — and, at a write tier, a real mutation — on a call whose reply can no longer be delivered,
/// which is precisely what `abandoned` exists to prevent. Returns the exit code so a call site
/// reads `return stop_serving(&abandoned);` and cannot set the flag without also stopping, or
/// stop without setting it. A dead pipe is still a CLEAN exit (spec: end promptly once the
/// client is gone), hence 0.
pub(super) fn stop_serving(abandoned: &AtomicBool) -> i32 {
    abandoned.store(true, Ordering::SeqCst);
    0
}

mod serve;
pub(super) use serve::serve;

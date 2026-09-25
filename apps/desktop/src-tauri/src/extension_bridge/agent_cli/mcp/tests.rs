//! Split out of `mcp.rs` (R8’s hard LOC cap) — a suite this long earns a hub of its
//! own: one file per topic under `tests/`, declared below. Every test moved verbatim; the
//! only changes are the ones a split forces, all mechanical:
//!
//! - each `tests/*.rs` topic carries its own `use super::*;`;
//! - `SOURCE` is module-level rather than function-local, because two topics read it;
//! - the three drain-deadline consts are module-level, because two topics read them;
//! - the hand-instrumented writers/readers and the prose-window assertions two topics share
//!   moved to `tests/support.rs`;
//! - `SOURCE` concatenates `mcp.rs` and the sub-modules split out of it, so the source scans
//!   still cover every line they covered before the split.
//!
//! The fixtures below are the ones more than one topic reaches for. Anything a single
//! topic uses stays with that topic.

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::instructions::{EXPLAINED_IN_PROSE, INSTRUCTIONS};
use super::launch::{mcp_help_text, parse_launch_args, print_help, LaunchArgs};
use super::protocol::{initialize_result, DEFAULT_VERSION};
use super::results::{oversized_result, MCP_RESULT_MAX_BYTES};
use super::*;

use support::*;

mod argument_typing;
mod commands_local_tool;
mod commands_namespaces;
mod commands_proof_fields;
mod confirm_passthrough;
mod curated_tool_descriptions;
mod dispatch_queue_bound;
mod documents_read_prose;
mod eof_drain;
mod error_sentinels;
mod event_queue_bound;
mod found_jobs_argv;
mod instructions_prose;
mod launch_args;
mod local_call_refusals;
mod policy_row_coverage;
mod prompts_surface;
mod resource_result_envelope;
mod resources_end_to_end;
mod resources_read;
mod result_envelope;
mod result_too_large_refusals;
mod schema_text;
mod serve_loop;
mod source_hygiene;
mod support;
mod tier_and_launch_modes;
mod tool_titles_and_order;
mod version_negotiation;

/// How long a test waits for a signal that a correct [`serve`] always sends — long enough that a
/// loaded CI machine never trips it, short enough that a real deadlock fails the run rather than
/// hanging it.
const SIGNAL_BUDGET: Duration = Duration::from_secs(10);

fn line(v: Value) -> String {
    format!("{v}\n")
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// The two halves of one `tools/call`, composed. Production never needs this — [`serve`] runs
/// [`classify_tool_call`] on its writer thread and [`dispatched_tool_result`] on the worker,
/// which is the whole point of the split — so the composition lives here rather than as a
/// never-called fn in `mcp.rs`. The real loop's own composition is covered by the `serve` tests
/// below, not by this helper.
fn tool_call_result(
    params: &Value,
    server: &Server,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
) -> Result<Value, (i64, &'static str)> {
    match classify_tool_call(params, server) {
        ToolCall::Local(outcome) => outcome,
        ToolCall::Bridge(verb) => Ok(dispatched_tool_result(&verb, dispatch)),
    }
}

fn stub_ok(_verb: &Verb) -> Result<Value, &'static str> {
    Ok(json!({ "ok": true, "resource": "stub", "data": {} }))
}

/// A poisoned `Mutex` in a test is a panic in ANOTHER test thread; surface it as a failure here
/// rather than propagating an `unwrap` chain through every assertion.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Drive [`serve`] over an in-memory [`Cursor`] with a stub dispatcher — no runtime, no socket, no
/// live app. Always the most permissive launch mode (both flags) unless a test needs otherwise.
/// The dispatch stub now runs on `serve`'s own worker thread, so it must be `Send + 'static`:
/// state a test wants to observe travels through an `Arc` (or a channel), never a borrow.
fn serve_with(
    input: &str,
    output: impl Write,
    dispatch: impl FnMut(&Verb) -> Result<Value, &'static str> + Send + 'static,
) -> i32 {
    serve_with_drain_budget(input, output, dispatch, INVOCATION_TIMEOUT)
}

/// [`serve_with`] with the EOF drain deadline injected — production's own
/// [`INVOCATION_TIMEOUT`] everywhere except the two tests that measure the deadline itself, which
/// would otherwise have to wait it out.
fn serve_with_drain_budget(
    input: &str,
    output: impl Write,
    dispatch: impl FnMut(&Verb) -> Result<Value, &'static str> + Send + 'static,
    drain_budget: Duration,
) -> i32 {
    let server = Server::new(true, true);
    serve(
        Cursor::new(input.to_string()),
        output,
        &server,
        dispatch,
        drain_budget,
    )
}

/// [`serve_with`] into a plain buffer — returns the raw stdout bytes as a `String` so a test can
/// assert exact line counts / content.
fn run_serve(
    input: &str,
    dispatch: impl FnMut(&Verb) -> Result<Value, &'static str> + Send + 'static,
) -> String {
    let mut output = Vec::new();
    let code = serve_with(input, &mut output, dispatch);
    assert_eq!(code, 0);
    String::from_utf8(output).expect("valid utf8")
}

/// Every `id` [`serve`] wrote, in write order.
fn reply_ids(text: &str) -> Vec<i64> {
    text.lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).expect("each line is one JSON-RPC reply")["id"]
                .as_i64()
                .expect("every reply here carries a numeric id")
        })
        .collect()
}

/// The drain budget, and the sleep a blocking dispatch holds the worker for. An ORDER OF
/// MAGNITUDE apart in each direction from the wall each measures (CodeRabbit, PR #1092 — at
/// 50 ms/300 ms the "returned on its own deadline" assertion below had only 250 ms of scheduling
/// slack, so a loaded CI runner could fail a correct build): `serve` must return on the 50 ms
/// deadline, the assertion allows it 20× that, and the dispatch it must NOT wait out runs 40×
/// it. Only `DRAIN_EXIT_MAX` sits between the two, and it is nowhere near either.
const DRAIN_BUDGET: Duration = Duration::from_millis(50);
const DISPATCH_HOLD: Duration = Duration::from_secs(2);
const DRAIN_EXIT_MAX: Duration = Duration::from_secs(1);

// Everything that was `mcp.rs` before the R8 split, so source scans cover the same code.
const SOURCE: &str = concat!(
    include_str!("../mcp.rs"),
    include_str!("../mcp/tier.rs"),
    include_str!("../mcp/protocol.rs"),
    include_str!("../mcp/commands_tool.rs"),
    include_str!("../mcp/argv.rs"),
    include_str!("../mcp/refusal.rs"),
    include_str!("../mcp/tool_call.rs"),
    include_str!("../mcp/launch.rs"),
);

fn tool_description(list: &[Value], tool: &str) -> String {
    list.iter()
        .find(|t| t["name"] == tool)
        .unwrap_or_else(|| panic!("{tool} must be listed"))["description"]
        .as_str()
        .expect("a description")
        .to_string()
}

fn best_matches(limit: Option<u64>, cursor: Option<&str>, query: Option<&str>) -> Verb {
    Verb::BestMatches {
        limit,
        cursor: cursor.map(str::to_string),
        query: query.map(str::to_string),
    }
}

fn names(list: &[Value]) -> Vec<&str> {
    let mut n: Vec<&str> = list.iter().map(|t| t["name"].as_str().unwrap()).collect();
    n.sort_unstable();
    n
}

fn frame_with_id(frames: &[Value], id: i64) -> &Value {
    frames
        .iter()
        .find(|f| f["id"].as_i64() == Some(id))
        .unwrap_or_else(|| panic!("no frame with id {id} in {frames:?}"))
}

fn parsed_frames(text: &str) -> Vec<Value> {
    text.lines()
        .map(|l| serde_json::from_str(l).expect("each line is one JSON-RPC frame"))
        .collect()
}

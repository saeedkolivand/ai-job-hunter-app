//! `ajh-tauri agent mcp [--allow-reversible] [--allow-irreversible]` — an MCP (Model Context
//! Protocol) stdio server MODE of the agent CLI (see ADR-040), never a second binary, Tauri
//! command, or reader of the app's stores. Speaks the legacy 2025-11-25 JSON-RPC stdio lifecycle
//! Claude Code 2.1.258 and Codex 0.144.6 actually open (`initialize` →
//! `notifications/initialized` → `ping`/`tools/list`/`tools/call`), never the 2026-07-28 stateless
//! era: `server/discover` answers plain `-32601`, the signal that spec itself defines as the
//! legacy fallback.
//!
//! ## Three launch tiers over the [`Effect`] boundary
//! Six curated, `readOnlyHint:true`, names/base descriptions derived from [`super::VERB_TABLE`]
//! (never a second hand-typed copy): `best-matches`, `job`, `profile`, `automations`,
//! `found-jobs` (issue #1115 — paginated, filtered found-jobs traversal, one autopilot or every
//! one; issues #1167/#1168), and a LOCAL
//! `commands` (no bridge call — works with the app closed) enumerating [`POLICY`] by `effect`.
//! Three generic dispatch tools sit over that SAME table: `call-read` (always present),
//! `call-reversible` (`--allow-reversible`), and `call-irreversible` (`--allow-irreversible`,
//! which IMPLIES `--allow-reversible` — three strict-superset tiers, [`tier::Tier`]). MCP annotations
//! are PER TOOL, so one monolithic `call` tool would be `destructiveHint:true` as a whole;
//! splitting by [`Effect`] lets a client auto-approve a read while still prompting on a delete.
//!
//! Both gates exist for the same reason: Codex never reads the Anthropic-only
//! `_meta["anthropic/requiresUserInteraction"]` hint, so it cannot be the only thing gating
//! `call-irreversible` — and on that same client `call-reversible` had neither `_meta` NOR a
//! flag, only `destructiveHint:false`, a HINT a client is free to ignore (HIGH fix, security
//! review round 2). Naming a gated tool without its flag is `-32602`, same as any unknown tool;
//! `commands` marks a gated row `"unavailable"` rather than omitting it, still naming the flag
//! that would expose it.
//!
//! `call-*` looks its target up in this binary's own bundled [`POLICY`] copy first — routing +
//! annotation only, the RUNNING APP'S gate (reached over the wire by [`super::query`]) stays
//! authoritative regardless. An unknown `<namespace>:<command>` refuses locally, never forwarded;
//! a KNOWN row on the wrong tool refuses naming the right one; a [`Effect::NotExposed`] row ALSO
//! refuses locally now, on every tool, naming its own stored reason (HIGH fix, security review
//! round 2 — the running app is a SEPARATE, possibly OLDER process, e.g. an updater-staged newer
//! exe still paired with it, so this binary's own gate must not lean on the peer's). `confirm`
//! reaches `call-irreversible` verbatim and is absent from the other two schemas by construction
//! (ADR-038 §4: fetching then confirming here "stops nothing").
//!
//! ## The output contract
//! Every model-actionable signal lives in `content[].text`: `content[0]` is the CLI's own JSON
//! payload byte-for-byte, `content[1]` names the exit code, and a `confirmation_required` refusal
//! gets one more block mapping `--confirm` to this tool's own `confirm` argument. No
//! `structuredContent` (SHOULD fix — no observed client surfaces it, and it doubled every
//! PII-bearing payload in a persisted transcript for nothing). [`results::MCP_RESULT_MAX_BYTES`] bounds
//! EVERY payload [`tool_result`] wraps, dispatched or locally refused alike (review round 3 — a
//! local refusal echoing an oversized `namespace`/`command` used to return before any cap check),
//! refusing as `result_too_large` rather than returning a payload whole or truncated; its
//! `detail` is addressed to the human reading the transcript, never to the model — never the CLI
//! invocation, which would be a working bypass recipe handed to the exact agent the cap bounds.
//! Every outcome is a tool RESULT, never a JSON-RPC error.
//!
//! ## Stdout/stderr discipline
//! [`stdio::emit`] is the ONE stdout writer once the JSON-RPC loop starts, `writeln!` on a compact
//! [`Value`] (never pretty-printed) — release is `panic="abort"` above `crash_reporting::init`,
//! so a bare `println!` after the client closes its pipe would be a silent abort; `emit`'s `Err`
//! (EPIPE) ends [`serve`] cleanly instead. [`run`] writes stdout exactly once more, for `--help`,
//! BEFORE any JSON-RPC frame is read — nothing negotiated yet to break. Every stderr write here is
//! content-free and never touches stdout; most are pre-protocol usage/runtime failures in [`run`],
//! and [`serve`] may write one MID-protocol — when the dispatch thread is gone, next to the
//! `-32603` it answers the caller with.
//!
//! ## Concurrency — one reader, one dispatcher, one writer (ADR-040 §12's named follow-up)
//! Three threads and one [`stdio::Event`] channel: a READER thread turns the input into `Event::Line`s
//! and one final `Event::Eof`; ONE WORKER thread owns the tokio runtime and runs the bridge-backed
//! tool calls (each under its own [`super::INVOCATION_TIMEOUT`]), sending an `Event::Reply` per
//! call; the MAIN thread consumes those events, classifies and answers everything else itself,
//! and is the only thread that ever writes. What a caller may rely on:
//!
//! - **Only bridge-backed tool calls queue; local tools and protocol methods are answered
//!   immediately.** [`tool_call::classify_tool_call`] runs on the writer thread, so `commands`, an unknown
//!   tool or bad params, a [`parse_verb`] usage error and every [`refusal::local_call_refusal`] are
//!   answered on the spot — none of them touches the wire, so none of them waits on something
//!   that does. `initialize`, `ping` and `tools/list` are answered the same way, even mid-call:
//!   a liveness ping can no longer be mistaken for a hung server.
//! - **Bridge-backed calls are still dispatched single-flight, in input order.** Exactly ONE
//!   dispatch runs at any instant and queued calls run strictly FIFO, so the throttle bound
//!   ADR-040 §12 rests on (one bridge connection per process) is unchanged by this split.
//! - **The queue between them is BOUNDED at [`MCP_CALL_QUEUE_MAX`], and a full queue is answered,
//!   never waited on.** The pipe this split replaced had the OS's own socket backpressure; an
//!   unbounded channel would have traded it for unbounded memory, since a pipelining client can
//!   write `tools/call` frames far faster than one bridge round trip completes. Of the two ways
//!   to keep it bounded, blocking the writer thread on a full queue is exactly the stall this
//!   split exists to remove (a `ping` behind it would go unanswered), so the excess call is
//!   REFUSED instead: a `server_busy` tool result, `isError`, exit code 2, telling the client to
//!   wait for an outstanding reply and retry that one call. Nothing is dropped silently and the
//!   loop stays responsive.
//! - **The [`stdio::Event`] queue is BOUNDED too, at [`MCP_EVENT_QUEUE_MAX`] — and here the producers
//!   DO block.** Bounding the dispatch queue only restored half the backpressure the reader
//!   split lost: the other half is the reader itself. A client that stops draining stdout parks
//!   this loop inside [`stdio::emit`], and a stdin that never blocks on its own (a file-fed input, or a
//!   client pipelining faster than stdout drains) would let an unbounded reader queue buffer the
//!   whole input in memory. Bounded, the reader instead parks in its own `send`, stops reading,
//!   and the OS pipe pushes back on the client — which is precisely what the single-threaded
//!   loop did before the split, and the reader is the one thread whose blocking costs nothing
//!   (it answers nothing and writes nothing).
//! - **Why a bounded reader queue still cannot deadlock.** Both producers may block on it; the
//!   sole CONSUMER — the writer/main thread — never blocks on any channel send, which is what
//!   rules out a cycle. It hands work to the worker with `try_send` (a full dispatch queue is
//!   refused, above), so it never waits on the worker to make room; its only waits are
//!   `incoming.recv`, which by definition frees a slot, and [`stdio::emit`], which waits on the CLIENT.
//!   A worker blocked in `send(Event::Reply)` therefore stalls only the worker: the writer is
//!   already on its way back to `recv`, and once the client drains stdout both producers are
//!   released in order. The one place the writer waits on the worker is the final
//!   `worker.join()`, and the loop reaches it only after breaking with nothing left in flight
//!   — every reply already received, so the worker is idle at `queued.recv()` and cannot be
//!   holding a `send`. The drain-deadline and broken-pipe exits do not join at all.
//! - **The EOF drain is bounded too**, by ONE absolute deadline (`drain_budget`, one
//!   [`super::INVOCATION_TIMEOUT`] in production) started when `Eof` arrives — not one budget per
//!   queued call, which is the N × timeout worst case a full queue could otherwise hold the exit
//!   open for. On expiry the worker is told to stop dispatching whatever is still queued, every
//!   call still owed a reply is ANSWERED (below), and the process exits 0 with the in-flight
//!   dispatch abandoned rather than joined (joining is the wait the deadline exists to cap).
//! - **Replies are ordered per kind, never globally.** The cost of the first two, stated plainly:
//!   an immediately-answered frame MAY be written before the reply of a bridge call that arrived
//!   EARLIER. Every reply still carries the `id` it answers, which is how a JSON-RPC client pairs
//!   them; nothing here reorders two bridge replies against each other.
//! - **Exactly one writer, one line per frame.** Only the main thread touches the output handle
//!   (which is why it takes the [`std::io::Stdout`] VALUE, not a `StdoutLock` — the latter is not
//!   `Send`, and locking per write is what keeps a partially-written frame impossible), so two
//!   frames can never interleave.
//! - **EOF drains, and nothing handed to the worker is left unanswered.** Once the input ends,
//!   the loop keeps writing until every already-queued call has replied, then exits 0. Stated
//!   precisely, because the earlier wording ("a call in flight when stdin closes is never
//!   dropped") was true only of the dispatch already RUNNING at `Eof`: a call the worker starts
//!   during the drain can outlive the deadline, and one still queued when it expires never runs
//!   at all — both used to exit silently, leaving a client waiting on a reply that could never
//!   come. So an expired deadline now ANSWERS them. The `abandoned` flag stops the queue first,
//!   any reply that landed in the same instant is written, and every id still owed one gets a
//!   [`shutting_down_result`]: `dispatched:false` for a call that provably never reached the app,
//!   and — for the FIRST unanswered id, the only one single-flight FIFO order allows to be
//!   running — `dispatched:true`, whose result was never received and may already have taken
//!   effect. An [`stdio::emit`] error (EPIPE: the client closed its pipe) still ends the server
//!   immediately, exit 0, and is the one case that answers nothing further: there is nowhere
//!   left to write it.
//! - **A `tools/call` with a null/absent `id` is dropped before classification**, so it neither
//!   dispatches nor answers: nothing is listening for the result, exactly as before.
//!
//! ## Two transports, one handler (issue #1173, ADR-040 amendment)
//! `--http <port>` (`mcp::http`) serves the SAME per-request behaviour over Streamable HTTP
//! instead of stdio — [`protocol::handle_message`] is the one function both wires call: [`protocol::route_value`]
//! classifies, [`tool_call::dispatched_tool_result`] dispatches the bridge-backed outcome, and the tiers,
//! the per-call [`INVOCATION_TIMEOUT`]-bounded fresh [`super::query`] connection, and
//! [`results::tool_result`]'s [`results::MCP_RESULT_MAX_BYTES`] cap are unchanged either way —
//! `Server::new`, `tools()` and `dispatch` are literally the same values [`run`] hands to
//! whichever wire it picks. What differs is ONLY the framing: HTTP has no stdin/stdout to
//! protect from a slow dispatch, so it answers one request at a time on its own accept loop
//! rather than reproducing the three-thread stdio split; there is no SSE stream and no session —
//! every `POST /mcp` is a single self-contained JSON-RPC request/response, so nothing here needs
//! the queueing this module's stdio half exists for. See `mcp::http`'s own module doc for the
//! bind/auth/framing contract.
//!
//! ## What this is NOT
//! Never wrapped in [`super::run_verb_within`]'s whole-invocation [`super::INVOCATION_TIMEOUT`] —
//! each `tools/call` gets its own budget via the same constant. Never a second validator of
//! argument VALUES: `tools/call` arguments become this CLI's own argv and run through
//! [`super::parse_verb`], inheriting its never-echo-the-value discipline for free. The one thing
//! [`tool_call::classify_tool_call`] checks first is the argument object's KEY SET, which argv cannot carry
//! at all — the `additionalProperties:false` every schema here advertises (issue #1134), read off
//! that tool's own already-built schema rather than a second list, minus the protocol's own
//! reserved `_`-prefixed keys (see [`tool_call::is_reserved_argument_key`]).

use std::io::{stdin, stdout, BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;

use serde_json::Value;

use super::agent_call;
use super::catalogue::{CatalogueEntry, CATALOGUE};
use super::policy::{Effect, LookupInput, ProofSource, POLICY};
use super::*;
// The ENFORCING constants (issue #1129): both `limit` descriptions are `format!`ed from these,
// never retyped — a hand-typed copy is how the advertised 50/100 drifted from the enforced 25/50.
use crate::extension_bridge::agent_read::{
    best_matches::{DEFAULT_BEST_MATCHES_LIMIT, MAX_BEST_MATCHES_LIMIT},
    found_jobs::{DEFAULT_FOUND_JOBS_LIMIT, MAX_FOUND_JOBS_LIMIT},
};
// ── Tool names (the hand-written literal list a drift test pins) ──────────

const TOOL_BEST_MATCHES: &str = "best-matches";
const TOOL_JOB: &str = "job";
const TOOL_PROFILE: &str = "profile";
const TOOL_AUTOMATIONS: &str = "automations";
const TOOL_FOUND_JOBS: &str = "found-jobs";
const TOOL_COMMANDS: &str = "commands";
const TOOL_CALL_READ: &str = "call-read";
const TOOL_CALL_REVERSIBLE: &str = "call-reversible";
const TOOL_CALL_IRREVERSIBLE: &str = "call-irreversible";
// The `initialize` instructions text lives in its own file (R8 LOC cap): a prose unit, not a
// protocol one — see `mcp/instructions.rs`. `INSTRUCTIONS` itself is read only by that file and
// by this module's tests, so only the builder is imported here.
mod instructions;
use instructions::build_instructions;

// `tools/list`'s catalogue — every tool's title, description, annotations and
// input schema — lives in its own file for the same R8 reason; see
// `mcp/schemas.rs`. Only the four items the protocol half calls are imported.
mod schemas;
use schemas::{proof_from, tier_exposes, tool_for, tools, unavailable_reason};

// `resources/*` (issue #1146 P4) and `prompts/*` (P5) each get their own R8-capped unit for the
// same reason `schemas`/`instructions` do — only the items `route_line`/the worker thread call
// are imported; both modules reach everything else here (`Verb`, `dispatch_payload`, the
// `TOOL_*` name consts, `results`) through their own `use super::*;`.
mod prompts;
mod resources;

// One unit per responsibility, split out under R8's LOC cap. Each takes what it
// needs from this module through its own `use super::*;`, and every name the other
// units share is re-exported below, so no unit reaches into a sibling directly.
mod argv;
mod commands_tool;
mod launch;
mod protocol;
mod refusal;
mod tier;
mod tool_call;

use argv::tool_argv;
use commands_tool::{commands_value, EFFECT_FILTER_VALUES};
use protocol::{handle_message, reply_frame, route_line, rpc_error, PendingKind, Routed, Server};
use refusal::local_call_refusal;
use tier::Tier;
use tool_call::{classify_tool_call, dispatched_tool_result, ToolCall};

// `agent_cli::entrypoint` dispatches this one by path; everything else reaches it
// through [`launch`].
pub(crate) use launch::run;

/// How many classified `tools/call`s may WAIT on the single-flight dispatch thread, on top of the
/// one it is running. Small on purpose: dispatch is strictly serial and each call is bounded by
/// [`super::INVOCATION_TIMEOUT`], so a deep queue only means a client waiting minutes for a reply
/// it could have re-sent — and every queued frame is memory this process holds for it. 8 is above
/// anything a request/response client ever produces (they send one and wait) and low enough that
/// a pipelining one is told to slow down almost immediately.
const MCP_CALL_QUEUE_MAX: usize = 8;

/// How many [`Event`]s (input lines from the reader thread + replies from the dispatch thread) may
/// WAIT on the single writer thread. The reader→writer half of the backpressure the reader split
/// lost: a client that stops draining stdout parks this loop inside [`emit`], and a stdin that
/// never blocks (a file, or a pipelining client) would otherwise let the reader buffer the whole
/// input in memory. Bounding it hands the pressure back to the OS pipe — the reader parks in its
/// `send`, stops reading, and the writer on the other end of the pipe blocks — which is the one
/// thread whose blocking costs nothing here (see the module doc's concurrency section).
///
/// 64 rather than [`MCP_CALL_QUEUE_MAX`]'s 8: this queue is a lookahead buffer, not a work queue,
/// and it also carries the replies. A request/response client never puts more than one or two
/// events in it, and the deepest legitimate burst — a pipelining client filling the dispatch
/// queue — is answered (dispatched or `server_busy`) as fast as this loop can read, so 64 is
/// slack the loop never has to grow into rather than a depth anyone waits out.
const MCP_EVENT_QUEUE_MAX: usize = 64;
// `tools/call`'s reply-shaping — the `CallToolResult` envelope plus the three fixed refusal
// shapes it wraps — lives in its own file for the same R8 reason `instructions.rs`/`schemas.rs`
// do; see `mcp/results.rs`. Only the three items this module's protocol loop calls are imported;
// `mcp::tests` reaches `oversized_result`/`MCP_RESULT_MAX_BYTES` through that module's own path.
mod results;
use results::{busy_result, shutting_down_result, tool_result};
// The stdio JSON-RPC transport — the DEFAULT wire `run` serves over — lives in its own file
// for the same R8 reason `instructions.rs`/`schemas.rs`/`results.rs`/`resources.rs`/`prompts.rs`
// do; see `mcp/stdio.rs`. Only `serve` is called from this module; the queue bounds above are
// read by that file through its own `use super::*;`.
mod stdio;
use stdio::serve;

// `agent mcp --http <port>` — the Streamable HTTP transport sharing this module's classifier and
// dispatch (issue #1173). R8 LOC-cap split, the same move `instructions.rs`/`schemas.rs`/
// `results.rs` already made: this is the WIRE unit for that one transport, so nothing about the
// stdio loop or the shared per-request handler travelled with it.
mod http;

#[cfg(test)]
mod tests;

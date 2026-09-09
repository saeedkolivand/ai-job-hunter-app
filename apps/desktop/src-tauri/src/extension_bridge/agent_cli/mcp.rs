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
//! which IMPLIES `--allow-reversible` — three strict-superset tiers, [`Tier`]). MCP annotations
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
//! PII-bearing payload in a persisted transcript for nothing). [`MCP_RESULT_MAX_BYTES`] bounds
//! EVERY payload [`tool_result`] wraps, dispatched or locally refused alike (review round 3 — a
//! local refusal echoing an oversized `namespace`/`command` used to return before any cap check),
//! refusing as `result_too_large` rather than returning a payload whole or truncated; its
//! `detail` is addressed to the human reading the transcript, never to the model — never the CLI
//! invocation, which would be a working bypass recipe handed to the exact agent the cap bounds.
//! Every outcome is a tool RESULT, never a JSON-RPC error.
//!
//! ## Stdout/stderr discipline
//! [`emit`] is the ONE stdout writer once the JSON-RPC loop starts, `writeln!` on a compact
//! [`Value`] (never pretty-printed) — release is `panic="abort"` above `crash_reporting::init`,
//! so a bare `println!` after the client closes its pipe would be a silent abort; `emit`'s `Err`
//! (EPIPE) ends [`serve`] cleanly instead. [`run`] writes stdout exactly once more, for `--help`,
//! BEFORE any JSON-RPC frame is read — nothing negotiated yet to break. Every stderr write here is
//! content-free and never touches stdout; most are pre-protocol usage/runtime failures in [`run`],
//! and [`serve`] may write one MID-protocol — when the dispatch thread is gone, next to the
//! `-32603` it answers the caller with.
//!
//! ## Concurrency — one reader, one dispatcher, one writer (ADR-040 §12's named follow-up)
//! Three threads and one [`Event`] channel: a READER thread turns the input into `Event::Line`s
//! and one final `Event::Eof`; ONE WORKER thread owns the tokio runtime and runs the bridge-backed
//! tool calls (each under its own [`super::INVOCATION_TIMEOUT`]), sending an `Event::Reply` per
//! call; the MAIN thread consumes those events, classifies and answers everything else itself,
//! and is the only thread that ever writes. What a caller may rely on:
//!
//! - **Only bridge-backed tool calls queue; local tools and protocol methods are answered
//!   immediately.** [`classify_tool_call`] runs on the writer thread, so `commands`, an unknown
//!   tool or bad params, a [`parse_verb`] usage error and every [`local_call_refusal`] are
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
//! - **The [`Event`] queue is BOUNDED too, at [`MCP_EVENT_QUEUE_MAX`] — and here the producers
//!   DO block.** Bounding the dispatch queue only restored half the backpressure the reader
//!   split lost: the other half is the reader itself. A client that stops draining stdout parks
//!   this loop inside [`emit`], and a stdin that never blocks on its own (a file-fed input, or a
//!   client pipelining faster than stdout drains) would let an unbounded reader queue buffer the
//!   whole input in memory. Bounded, the reader instead parks in its own `send`, stops reading,
//!   and the OS pipe pushes back on the client — which is precisely what the single-threaded
//!   loop did before the split, and the reader is the one thread whose blocking costs nothing
//!   (it answers nothing and writes nothing).
//! - **Why a bounded reader queue still cannot deadlock.** Both producers may block on it; the
//!   sole CONSUMER — the writer/main thread — never blocks on any channel send, which is what
//!   rules out a cycle. It hands work to the worker with `try_send` (a full dispatch queue is
//!   refused, above), so it never waits on the worker to make room; its only waits are
//!   `incoming.recv`, which by definition frees a slot, and [`emit`], which waits on the CLIENT.
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
//!   effect. An [`emit`] error (EPIPE: the client closed its pipe) still ends the server
//!   immediately, exit 0, and is the one case that answers nothing further: there is nowhere
//!   left to write it.
//! - **A `tools/call` with a null/absent `id` is dropped before classification**, so it neither
//!   dispatches nor answers: nothing is listening for the result, exactly as before.
//!
//! ## Two transports, one handler (issue #1173, ADR-040 amendment)
//! `--http <port>` (`mcp::http`) serves the SAME per-request behaviour over Streamable HTTP
//! instead of stdio — [`handle_message`] is the one function both wires call: [`route_value`]
//! classifies, [`dispatched_tool_result`] dispatches the bridge-backed outcome, and the tiers,
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
//! [`classify_tool_call`] checks first is the argument object's KEY SET, which argv cannot carry
//! at all — the `additionalProperties:false` every schema here advertises (issue #1134), read off
//! that tool's own already-built schema rather than a second list, minus the protocol's own
//! reserved `_`-prefixed keys (see [`is_reserved_argument_key`]).

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
    found_jobs::{DEFAULT_FOUND_JOBS_LIMIT, MAX_FOUND_JOBS_LIMIT},
    DEFAULT_BEST_MATCHES_LIMIT, MAX_BEST_MATCHES_LIMIT,
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

/// `commands`' own `effect` filter values — builds its `inputSchema` enum AND validates an
/// incoming call (MUST FIX — previously nothing validated this at all, so a typo'd filter matched
/// zero rows and answered `{"commands":[]}` with `isError:false` exit 0: a refusal disguised as an
/// empty success).
const EFFECT_FILTER_VALUES: &[&str] = &["read", "reversible", "irreversible", "not_exposed"];

/// The three strictly-nested launch tiers this server can run at (MEDIUM fix, security review
/// round 3 — item 21): replaces a raw `(allow_reversible, allow_irreversible)` bool pair that let
/// `Server::new(false, true)` compile and pass every existing test even though no real launch can
/// ever produce it (`--allow-irreversible` alone always implies the reversible tier too). The
/// type itself makes that state unconstructable, rather than a "callers MUST resolve the
/// implication first" comment on every function that used to take the raw pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    Read,
    Reversible,
    Irreversible,
}

impl Tier {
    /// The ONE place raw launch flags become a [`Tier`] — `--allow-irreversible` implies
    /// `--allow-reversible` here, once.
    fn from_flags(allow_reversible: bool, allow_irreversible: bool) -> Self {
        if allow_irreversible {
            Tier::Irreversible
        } else if allow_reversible {
            Tier::Reversible
        } else {
            Tier::Read
        }
    }

    fn allows_reversible(self) -> bool {
        matches!(self, Tier::Reversible | Tier::Irreversible)
    }

    fn allows_irreversible(self) -> bool {
        matches!(self, Tier::Irreversible)
    }
}

// ── Version negotiation (Claude Code's own hard list; never the 2026-07-28 era) ────────────────

const SUPPORTED_VERSIONS: &[&str] = &[
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];
const DEFAULT_VERSION: &str = "2025-11-25";

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

fn initialize_result(params: &Value, instructions: &str) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = requested
        .filter(|v| SUPPORTED_VERSIONS.contains(v))
        .unwrap_or(DEFAULT_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
        "serverInfo": { "name": "ai-job-hunter", "version": env!("CARGO_PKG_VERSION") },
        "instructions": instructions,
    })
}

// ── `commands` (local — no bridge call) ────────────────────────────────

/// `command`'s row in the generated [`CATALOGUE`], or `None` when it is absent (an `invoke()` call
/// the generator could not parse with confidence, or one with no call site at all — its own module
/// doc). `commands` marks that absence with `args: null` (issue #1163) rather than an empty list,
/// which would otherwise be indistinguishable from "this command genuinely takes no arguments".
fn catalogue_lookup(command: &str) -> Option<&'static CatalogueEntry> {
    CATALOGUE.iter().find(|entry| entry.command == command)
}

fn commands_value(arguments: &Value, tier: Tier) -> Value {
    let effect_filter = arguments.get("effect").and_then(Value::as_str);
    let namespace_filter = arguments.get("namespace").and_then(Value::as_str);
    let rows: Vec<Value> = POLICY
        .iter()
        .filter_map(|entry| {
            let (namespace, command) = agent_call::split_path(entry.path);
            if namespace_filter.is_some_and(|n| n != namespace) {
                return None;
            }
            let effect_name = match entry.effect {
                Effect::Read => "read",
                Effect::Reversible => "reversible",
                Effect::Irreversible(_) => "irreversible",
                Effect::NotExposed(_) => "not_exposed",
            };
            if effect_filter.is_some_and(|f| f != effect_name) {
                return None;
            }
            let mut row =
                json!({ "namespace": namespace, "command": command, "effect": effect_name });
            match catalogue_lookup(command) {
                Some(catalogued) => {
                    if !catalogued.description.is_empty() {
                        row["description"] = json!(catalogued.description);
                    }
                    row["args"] = json!(catalogued
                        .args
                        .iter()
                        .map(|arg| {
                            let mut value = json!({ "name": arg.name, "required": arg.required });
                            // `None` (scalar arg) omits the key entirely — unchanged. `Some(&[])`
                            // (a wrapper type this generator could not resolve — see
                            // `CatalogueArg::fields`'s own doc) is surfaced as an explicit
                            // `null`, distinct from omission, so a caller can tell "known to
                            // take no nested fields" apart from "unknown nested shape" (MEDIUM —
                            // CLI review).
                            match arg.fields {
                                None => {}
                                Some([]) => {
                                    value["fields"] = Value::Null;
                                }
                                Some(fields) => {
                                    value["fields"] = json!(fields);
                                }
                            }
                            value
                        })
                        .collect::<Vec<_>>());
                }
                // `args: null`, never an absent key or an empty array — see this fn's own doc.
                None => row["args"] = Value::Null,
            }
            // A paged row's reply is an ENVELOPE, not the bare array its name suggests
            // (issue #1136). Both the list and the note come from `agent_call`, so this
            // row cannot drift from the behaviour `dispatch_direct` actually applies.
            if agent_call::reshape::PAGINATED_LIST_COMMANDS.contains(&command) {
                row["returns"] = json!(agent_call::reshape::PAGINATED_LIST_NOTE);
            }
            // Same discovery precedent as the paging note just above, for the
            // OTHER reply reshape a generic-tier caller cannot otherwise learn
            // about (round-1 review, issue #1180): plain `call-read` never sees
            // an MCP tool description.
            if command == agent_call::reshape::CONTACT_PROFILE_GET_COMMAND {
                row["returns"] = json!(agent_call::reshape::CONTACT_PROFILE_GET_PROJECTION_NOTE);
            }
            let gate_open = tier_exposes(tier, &entry.effect);
            match tool_for(&entry.effect) {
                Some(tool) if gate_open => row["tool"] = json!(tool),
                Some(_) => row["unavailable"] = json!(unavailable_reason(&entry.effect)),
                None => {}
            }
            match entry.effect {
                Effect::Irreversible(source) => {
                    if let Some(pf) = proof_from(source) {
                        row["proofFrom"] = json!(pf);
                    }
                    // The field a confirm ceremony will require (issue #1160: "what would
                    // deleting this require?" answerable without dispatching) — derived, never
                    // hand-typed, the same as `proofFrom`/`hint`'s own `field` clause.
                    if let Some(field) = agent_call::proof_field_for(source) {
                        row["proofField"] = json!(field);
                    }
                    // What an ABSENT `proofField` means for this row (CLI review round 2 —
                    // MEDIUM): `"count"` — pass the array length / `total`; `"response_value"` —
                    // pass the whole response value; `"field"` — a field IS named above. Carried
                    // on every Irreversible row, not just the ones with a named field, so a
                    // caller never has to dispatch the destructive command just to discover which
                    // shape its own refusal would have described.
                    row["proofKind"] = json!(agent_call::proof_kind_for(source));
                    if let ProofSource::Lookup { key, input, .. } = source {
                        row["proofInput"] = json!(key);
                        // A `Literal` input's VALUE (e.g. `privacy_sign_out_all`'s `boardId` =
                        // `"linkedin"`) is not secret and is otherwise the one thing this
                        // ceremony can't complete from `commands` alone; a `FromCaller` value is
                        // the caller's own input and deliberately never echoed here.
                        if let LookupInput::Literal(value) = input {
                            row["proofInputValue"] = json!(value);
                        }
                    }
                }
                Effect::NotExposed(reason) => row["reason"] = json!(reason),
                _ => {}
            }
            Some(row)
        })
        .collect();
    json!({ "commands": rows })
}

// ── `tools/call` → argv → `parse_verb` (one validator, reused) ────────────

fn value_as_arg(v: &Value) -> String {
    v.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| v.to_string())
}

/// Best-effort `tools/call` arguments → this CLI's own argv. Never validates anything itself — a
/// wrong shape (a missing `url`, a non-integer `limit`, a non-object `input`) produces
/// plausible-looking argv that [`parse_verb`] then rejects with ITS OWN, already-hardened,
/// never-echo-the-value error text; this fn's only job is building that argv, not judging it.
///
/// Two conventions EVERY optional argument below follows, stated once rather than re-argued per
/// arm (which is how they drifted apart): [`value_as_arg`], never `.and_then(Value::as_str)`, so
/// a JSON NUMBER (`{"cursor": 100}`, a numeric `confirm` proof) reaches [`parse_verb`] as its
/// string form instead of vanishing as "absent" (HIGH fix, round 2 — a dropped cursor reset the
/// traversal to page 0); and `.filter(|v| !v.is_null())`, so an explicit `null` reads as ABSENT
/// rather than as the literal string `"null"` (issue #1137 — mirrors `parse_found_jobs_cursor`'s
/// own `None | Some(Value::Null)` arm; a strict schema unions optionals with `null`).
fn tool_argv(name: &str, arguments: &Value) -> Vec<String> {
    match name {
        TOOL_BEST_MATCHES => {
            let mut argv = vec!["best-matches".to_string()];
            if let Some(limit) = arguments.get("limit").filter(|v| !v.is_null()) {
                argv.push("--limit".to_string());
                argv.push(value_as_arg(limit));
            }
            if let Some(cursor) = arguments.get("cursor").filter(|v| !v.is_null()) {
                argv.push("--cursor".to_string());
                argv.push(value_as_arg(cursor));
            }
            if let Some(query) = arguments.get("query").filter(|v| !v.is_null()) {
                argv.push("--query".to_string());
                argv.push(value_as_arg(query));
            }
            argv
        }
        TOOL_JOB => vec![
            "job".to_string(),
            arguments
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ],
        TOOL_PROFILE => vec!["profile".to_string()],
        TOOL_AUTOMATIONS => vec!["automations".to_string()],
        // Issue #1168 — `autopilotId` is now OPTIONAL (omitted spans every
        // autopilot). Forwarded as the SAME bare leading positional as
        // before when present, simply omitted when absent — `parse_found_jobs`
        // only reads the first token as `autopilotId` when it does not look
        // like a flag, so an omitted id here correctly falls through to
        // "start flag parsing at index 0".
        TOOL_FOUND_JOBS => {
            let mut argv = vec!["found-jobs".to_string()];
            if let Some(id) = arguments
                .get("autopilotId")
                .filter(|v| !v.is_null())
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                argv.push(id.to_string());
            }
            if let Some(limit) = arguments.get("limit").filter(|v| !v.is_null()) {
                argv.push("--limit".to_string());
                argv.push(value_as_arg(limit));
            }
            if let Some(cursor) = arguments.get("cursor").filter(|v| !v.is_null()) {
                argv.push("--cursor".to_string());
                argv.push(value_as_arg(cursor));
            }
            if let Some(min_score) = arguments.get("minScore").filter(|v| !v.is_null()) {
                argv.push("--min-score".to_string());
                argv.push(value_as_arg(min_score));
            }
            if let Some(country) = arguments.get("country").filter(|v| !v.is_null()) {
                argv.push("--country".to_string());
                argv.push(value_as_arg(country));
            }
            if let Some(remote) = arguments.get("remote").filter(|v| !v.is_null()) {
                argv.push("--remote".to_string());
                argv.push(value_as_arg(remote));
            }
            if let Some(applied) = arguments.get("applied").filter(|v| !v.is_null()) {
                argv.push("--applied".to_string());
                argv.push(value_as_arg(applied));
            }
            if let Some(query) = arguments.get("query").filter(|v| !v.is_null()) {
                argv.push("--query".to_string());
                argv.push(value_as_arg(query));
            }
            if arguments.get("includeDescription").and_then(Value::as_bool) == Some(true) {
                argv.push("--include-description".to_string());
            }
            argv
        }
        TOOL_CALL_READ | TOOL_CALL_REVERSIBLE | TOOL_CALL_IRREVERSIBLE => {
            let namespace = arguments
                .get("namespace")
                .and_then(Value::as_str)
                .unwrap_or("");
            let command = arguments
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("");
            let mut argv = vec!["call".to_string(), format!("{namespace}:{command}")];
            if let Some(input) = arguments.get("input") {
                argv.push("--input".to_string());
                argv.push(input.to_string());
            }
            // `confirm` is read for `call-irreversible` ONLY (MUST FIX — the other two tools'
            // schemas have no `confirm` property BY CONSTRUCTION; a misbehaving client sending
            // one anyway is silently ignored here rather than forwarded). Both conventions from
            // this fn's own doc apply (issue #1140): a NON-STRING proof is coerced, not dropped —
            // real proofs include bare numbers (`ProofSource::Count`), and dropping one answered
            // `confirmation_required` exactly as if none had been sent, collapsing the gate's
            // deliberate absent-vs-mismatch distinction.
            if name == TOOL_CALL_IRREVERSIBLE {
                if let Some(confirm) = arguments
                    .get("confirm")
                    .filter(|v| !v.is_null())
                    .map(value_as_arg)
                {
                    argv.push("--confirm".to_string());
                    argv.push(confirm);
                }
            }
            argv
        }
        _ => Vec::new(),
    }
}

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
fn local_call_refusal(tool_name: &str, verb: &Verb, tier: Tier) -> Option<Value> {
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

/// What a `tools/call` frame turns out to be, once classified. The split exists because only
/// [`ToolCall::Bridge`] costs a bridge round trip: everything else is decided from this binary's
/// own bundled tables and is answered on the spot, never queued behind an in-flight dispatch (see
/// the module doc's concurrency guarantees).
enum ToolCall {
    /// Answered with no wire traffic at all — `commands`, an unknown tool or bad params, a
    /// `parse_verb` usage error, or any [`local_call_refusal`].
    Local(Result<Value, (i64, &'static str)>),
    /// The one outcome that needs the app: dispatch this verb and wrap the reply.
    Bridge(Verb),
}

/// MCP RESERVES every `_`-prefixed key for the protocol itself (`_meta` is the one in use today,
/// and a client may attach it to ANY tool call's `arguments`), so the key-set gate below must skip
/// them: no tool schema declares `_meta`, and refusing it would refuse a spec-conformant call.
/// Matched on the PREFIX rather than an `_meta` literal, because the reservation is on the prefix
/// — and a plain typo (`limt`) carries no `_`, so it is still refused. Nothing downstream reads
/// these keys: [`tool_argv`] builds argv from named keys only, so a reserved key is inert, never
/// forwarded.
fn is_reserved_argument_key(key: &str) -> bool {
    key.starts_with('_')
}

/// Everything about a `tools/call` that can be decided WITHOUT the bridge. Pure — no dispatch
/// closure in its signature at all, which is what makes "local tools never queue" a property of
/// the type rather than of a comment: [`serve`] can run this on its writer thread precisely
/// because there is nothing here to block on.
fn classify_tool_call(params: &Value, server: &Server) -> ToolCall {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return ToolCall::Local(Err((-32602, "Invalid params")));
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    // Bound ONCE here: a non-object `arguments` is a protocol error before anything reads it, so
    // the key-set gate below — the only reader — needs no second, unreachable `as_object()` arm.
    let Some(given) = arguments.as_object() else {
        return ToolCall::Local(Err((-32602, "Invalid params")));
    };
    let Some(tool) = server
        .tools
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
    else {
        return ToolCall::Local(Err((-32602, "Unknown tool")));
    };
    // `additionalProperties:false` is advertised on every schema `schema_object` builds and, until
    // now, enforced by nothing (issue #1134): a typo'd OPTIONAL key (`limt`) was dropped in
    // silence and answered with that field's DEFAULT — a quietly wrong page, isError:false. Read
    // off this tool's OWN already-built `inputSchema.properties`, never a second hand-written key
    // list per tool (`tool_for`/`mcp_help_text`'s rule). A `usage` result, not `-32602`, so it
    // keeps the exitCode block every refusal carries; the offending key is caller-authored text
    // and so is never echoed — the detail names the DECLARED set instead.
    let declared: Vec<&str> = tool["inputSchema"]["properties"]
        .as_object()
        .map_or_else(Vec::new, |p| p.keys().map(String::as_str).collect());
    if given
        .keys()
        .any(|k| !is_reserved_argument_key(k) && !declared.contains(&k.as_str()))
    {
        let detail = if declared.is_empty() {
            "unknown argument (this tool accepts none)".to_string()
        } else {
            format!(
                "unknown argument (this tool accepts: {})",
                declared.join(", ")
            )
        };
        return ToolCall::Local(Ok(tool_result(usage_error_value(&detail), 2)));
    }

    if name == TOOL_COMMANDS {
        // MUST FIX — an `effect` outside the declared enum, or not even a STRING (`{"effect":5}`
        // skipped the old `and_then(Value::as_str)` gate entirely — review round 3, item 20),
        // used to fall through and match nothing, answering `{"commands":[]}` isError:false exit
        // 0. A PRESENT `effect` must be a valid string or this is a usage error; an ABSENT one
        // means "no filter" and is fine.
        if let Some(effect_value) = arguments.get("effect") {
            let valid = effect_value
                .as_str()
                .is_some_and(|s| EFFECT_FILTER_VALUES.contains(&s));
            if !valid {
                return ToolCall::Local(Ok(tool_result(
                    usage_error_value(
                        "effect must be one of read, reversible, irreversible, not_exposed",
                    ),
                    2,
                )));
            }
        }
        // Same reasoning as `effect` just above, for the SAME failure shape (issue #1163's
        // `namespace` filter): a typo'd namespace would otherwise match zero rows and answer
        // `{"commands":[]}` isError:false exit 0 — a refusal disguised as an empty success.
        // `namespace` has no small enum to advertise in the schema (unlike `effect`), so it is
        // checked against POLICY's own real namespace set rather than a hand-typed list.
        if let Some(namespace_value) = arguments.get("namespace") {
            let valid = namespace_value.as_str().is_some_and(|s| {
                POLICY
                    .iter()
                    .any(|entry| agent_call::split_path(entry.path).0 == s)
            });
            if !valid {
                return ToolCall::Local(Ok(tool_result(
                    usage_error_value("namespace does not match any real command's namespace"),
                    2,
                )));
            }
        }
        return ToolCall::Local(Ok(tool_result(commands_value(&arguments, server.tier), 0)));
    }

    // B3-r1-F2 — a PRESENT-but-blank `autopilotId` used to collapse to the
    // same argv [`tool_argv`] builds for an OMITTED one (`.filter(|s|
    // !s.is_empty())` before the push below), silently widening a
    // one-autopilot selector into a spanning traversal
    // (`agent-cli-standards`: an empty selector must never mean "all"). A
    // flag-shaped value (`"--include-description"`) was WORSE: forwarded as
    // the bare leading positional [`tool_argv`] builds, [`parse_found_jobs`]
    // reads it as a real flag rather than as an id, since it doesn't look
    // like one — turning on a filter the caller never asked for. Checked
    // HERE, before argv is built, rather than inside [`tool_argv`] (which
    // never validates anything itself, by its own documented contract) —
    // mirrors `found_jobs::parse_autopilot_id_arg`'s identical guard on the
    // SAME field one hop further in.
    if name == TOOL_FOUND_JOBS {
        if let Some(id) = arguments.get("autopilotId").filter(|v| !v.is_null()) {
            let usable = id
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && !s.trim().starts_with("--"));
            if !usable {
                return ToolCall::Local(Ok(tool_result(
                    usage_error_value(
                        "autopilotId must be a non-empty id, not blank or flag-shaped — omit \
                         the key entirely to span every autopilot",
                    ),
                    2,
                )));
            }
        }
        // Round 2 fix (B3-r2-F4) — `tool_argv`'s `includeDescription` arm used to read this
        // value with `.and_then(Value::as_bool)`, the exact silent-drop combinator this fn's own
        // doc says every optional argument avoids: a non-bool (`"true"`, `1`) vanished as
        // "absent" rather than reaching `parse_verb`, so the resource-level refusal for the
        // identical value one hop further in (`found_jobs::bool_filter`, via
        // `FoundJobsFilters::from_payload`) could never fire — the caller got compact rows with
        // no error and no signal that `description` was silently dropped. Checked HERE, before
        // argv is built, mirroring the `autopilotId` guard above on the SAME tool.
        if let Some(v) = arguments.get("includeDescription").filter(|v| !v.is_null()) {
            if v.as_bool().is_none() {
                return ToolCall::Local(Ok(tool_result(
                    usage_error_value("includeDescription must be a boolean"),
                    2,
                )));
            }
        }
    }

    let argv = tool_argv(name, &arguments);
    let verb = match parse_verb(&argv) {
        Ok(v) => v,
        Err(e) => return ToolCall::Local(Ok(tool_result(usage_error_value(&e.to_string()), 2))),
    };

    if let Some(refusal) = local_call_refusal(name, &verb, server.tier) {
        return ToolCall::Local(Ok(tool_result(refusal, 2)));
    }

    ToolCall::Bridge(verb)
}

/// The bridge-backed TAIL of a `tools/call` — the only part that touches the wire, and so the
/// only part [`serve`] hands to its worker thread. Split out of [`tool_call_result`] so the
/// dispatch closure appears in exactly one signature. Shares [`results::dispatch_payload`] with
/// [`resources::dispatched_resource_result`] (issue #1146 P4) — see that fn's own doc.
fn dispatched_tool_result(
    verb: &Verb,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
) -> Value {
    let (payload, code) = results::dispatch_payload(verb, dispatch);
    tool_result(payload, code)
}

// ── The JSON-RPC loop ───────────────────────────────────────────────────

/// One launched server's fixed state: its `tools/list` answer, its (tier-dependent) `initialize`
/// instructions, and the [`Tier`] `commands`/`tool_call_result` both need. Built once in [`run`]
/// (or by a test) and threaded through the read loop instead of a growing positional parameter
/// list.
struct Server {
    tools: Vec<Value>,
    instructions: String,
    tier: Tier,
}

impl Server {
    /// Still takes the raw launch-flag pair (matches [`run`]'s own [`LaunchArgs`]) but resolves it
    /// to a [`Tier`] via [`Tier::from_flags`] exactly once, here — `tools`/`commands_value`/
    /// `build_instructions` never see the raw pair at all.
    fn new(allow_reversible: bool, allow_irreversible: bool) -> Self {
        let tier = Tier::from_flags(allow_reversible, allow_irreversible);
        Self {
            tools: tools(tier),
            instructions: build_instructions(tier),
            tier,
        }
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// One JSON-RPC reply frame around an already-computed outcome.
fn reply_frame(id: Value, outcome: Result<Value, (i64, &'static str)>) -> Value {
    match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => rpc_error(id, code, message),
    }
}

/// What the main thread does with one input line. [`Routed::Drop`] means "no reply, ever" — a
/// notification (no `id` member), an explicit `id: null`, or any `notifications/*` method
/// regardless of `id`; a `tools/call` in that state never becomes a [`Routed::Call`] and so never
/// reaches the worker at all: nothing is listening for the result.
enum Routed {
    Drop,
    /// Answerable without touching the bridge — written immediately, even mid-call. Every
    /// protocol method AND every [`ToolCall::Local`] outcome lands here.
    Reply(Value),
    /// A bridge-backed call, already classified and parsed: the ONLY thing that queues behind an
    /// earlier one (see the module doc). `kind` decides the reply SHAPE once dispatched — a
    /// `tools/call` and a `resources/read` share this one queue and worker (issue #1146 P4), so
    /// the busy/shutting-down refusals below need it too, not just the happy path.
    Call {
        id: Value,
        verb: Verb,
        kind: PendingKind,
    },
}

/// Which reply shape a queued bridge call is owed once dispatched, decided at classification
/// time — `tools/call` becomes a `CallToolResult` ([`dispatched_tool_result`]), `resources/read`
/// becomes a `contents` envelope naming its own `uri` ([`resources::dispatched_resource_result`]).
/// Threaded through the dispatch queue AND `in_flight` so the busy ([`TrySendError::Full`]) and
/// shutting-down (EOF drain) refusals answer in the SAME shape a successful dispatch would have,
/// never a tool-shaped refusal for a resource read or vice versa. [`handle_message`] (the HTTP
/// transport's shared per-request handler) switches on the SAME `kind` to build its own reply,
/// so a resource read answers identically over either wire.
#[derive(Debug, Clone)]
enum PendingKind {
    Tool,
    Resource(String),
}

/// Route one already-read JSON-RPC line: parse, then hand off to [`route_value`]. Pure — never
/// dispatches. Split from [`route_value`] so the HTTP transport (`mcp::http`), which already
/// receives a parsed request body rather than a text line, can call the shared classifier
/// directly without re-serializing its body just to re-parse it here.
fn route_line(line: &str, server: &Server) -> Routed {
    let parsed: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Routed::Reply(rpc_error(Value::Null, -32700, "Parse error")),
    };
    route_value(parsed, server)
}

/// The classifier both transports share (issue #1173): given one already-parsed JSON-RPC
/// message, decide whether it is dropped (a notification, or `id: null`), answerable locally, or
/// needs the bridge. Identical to what `route_line` did inline before this split — no behaviour
/// change, only a parse/classify split so a caller that already holds a [`Value`] (the HTTP
/// transport's request body) skips the string round-trip.
fn route_value(parsed: Value, server: &Server) -> Routed {
    let Some(obj) = parsed.as_object() else {
        return Routed::Reply(rpc_error(Value::Null, -32600, "Invalid Request"));
    };
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    if id.is_null() {
        return Routed::Drop;
    }
    let method = obj.get("method").and_then(Value::as_str);
    let params = obj.get("params").cloned().unwrap_or_else(|| json!({}));

    let outcome: Result<Value, (i64, &'static str)> = match method {
        None => Err((-32600, "Invalid Request")),
        Some(m) if m.starts_with("notifications/") => return Routed::Drop,
        Some("initialize") => Ok(initialize_result(&params, &server.instructions)),
        Some("ping") => Ok(json!({})),
        Some("tools/list") => Ok(json!({ "tools": server.tools })),
        // Classified HERE, on the writer thread: only a target that really needs the app is
        // handed to the worker; a local tool, a usage error and every local refusal are
        // answered like any other immediate method (module doc).
        Some("tools/call") => match classify_tool_call(&params, server) {
            ToolCall::Local(outcome) => outcome,
            ToolCall::Bridge(verb) => {
                return Routed::Call {
                    id,
                    verb,
                    kind: PendingKind::Tool,
                }
            }
        },
        // `resources/list`/`resources/templates/list` are pure catalogue reads, answered exactly
        // like `tools/list` — no bridge call, no `Tier` gate (issue #1146 P4: every resource here
        // mirrors a curated Read tool, so there is nothing to gate along the `Effect` boundary).
        Some("resources/list") => Ok(json!({ "resources": resources::resources_list() })),
        Some("resources/templates/list") => {
            Ok(json!({ "resourceTemplates": resources::resource_templates() }))
        }
        Some("resources/read") => match resources::classify_resource_read(&params) {
            resources::ResourceCall::Local(outcome) => outcome,
            resources::ResourceCall::Bridge(uri, verb) => {
                return Routed::Call {
                    id,
                    verb,
                    kind: PendingKind::Resource(uri),
                }
            }
        },
        // `prompts/*` never touches the bridge (see `mcp/prompts.rs`'s own doc): both are
        // answered locally, the same as `commands`.
        Some("prompts/list") => Ok(json!({ "prompts": prompts::prompts_list() })),
        Some("prompts/get") => prompts::prompts_get(&params),
        // Everything else — `server/discover` included — is a plain "Method not found", the
        // legacy-fallback signal the 2025-11-25 spec itself defines (see the module doc).
        Some(_) => Err((-32601, "Method not found")),
    };

    Routed::Reply(reply_frame(id, outcome))
}

/// The one per-request handler both transports call (issue #1173): classify an already-parsed
/// JSON-RPC message via [`route_value`] and, for the one outcome that needs the app, dispatch it
/// and wrap the reply — `None` for a dropped notification, `Some(frame)` for everything else. The
/// stdio [`serve`] loop does NOT call this directly: its three-thread split (module doc) exists so
/// a bridge-backed call can be classified without blocking the writer and dispatched without
/// blocking a `ping` behind it, so it composes the same two calls (`route_value` on the writer
/// thread, [`dispatched_tool_result`]/[`resources::dispatched_resource_result`] on the worker)
/// across that boundary instead of in one frame. The stateless HTTP transport (`mcp::http`) has no
/// writer thread to protect and answers one request at a time, so it calls this directly and
/// synchronously — same classifier, same tiers, same throttle (a fresh [`super::query`] bridge
/// round trip per call either way), same result cap, byte-for-byte the same [`tool_result`]/
/// refusal shapes. `kind` (issue #1146 P4) picks the same reply shape [`serve`]'s worker picks for
/// the identical queued call, so a `resources/read` answers identically over either wire, not just
/// `tools/call`. Never used by [`route_line`]/[`route_value`] themselves, which stay pure and
/// dispatch-free.
fn handle_message(
    parsed: Value,
    server: &Server,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
) -> Option<Value> {
    match route_value(parsed, server) {
        Routed::Drop => None,
        Routed::Reply(frame) => Some(frame),
        Routed::Call { id, verb, kind } => {
            // `dispatched_resource_result` (T8, PR #1184) can itself be `Err` — an oversized
            // reply capped by `results::capped_result_text` — so both arms are unified as a
            // `Result` here rather than always wrapping in `Ok`, letting `reply_frame` write a
            // real JSON-RPC error for that case exactly as it does for any other refusal.
            let result = match &kind {
                PendingKind::Tool => Ok(dispatched_tool_result(&verb, dispatch)),
                PendingKind::Resource(uri) => {
                    resources::dispatched_resource_result(uri, &verb, dispatch)
                }
            };
            Some(reply_frame(id, result))
        }
    }
}

// The stdio JSON-RPC transport — the DEFAULT wire `run` serves over — lives in its own file
// for the same R8 reason `instructions.rs`/`schemas.rs`/`results.rs`/`resources.rs`/`prompts.rs`
// do; see `mcp/stdio.rs`. Only `serve` is called from this module; the queue bounds above are
// read by that file through its own `use super::*;`.
mod stdio;
use stdio::serve;

/// `agent mcp [--allow-reversible] [--allow-irreversible] [--http <port>] [--help]` argv — any
/// subset of the flags, in any order; `--help`/`-h`/`help` anywhere short-circuits everything
/// else. Anything not in this set is a hard failure (MUST FIX — security review round 2: argv is
/// the only path to any gate, env vars are never consulted, and this parser must never grow a
/// fuzzy/prefix match that could nudge a typo into an elevated launch).
///
/// `--http` takes exactly one following token, parsed as a bare `u16` — nothing else is a valid
/// shape for it. This is also the WHOLE non-loopback-bind refusal (issue #1173): there is no flag
/// that can express a host or address at all, so `--http 0.0.0.0:9000`, `--http=9000`, and a bare
/// `--http` with nothing after it are all a parse-time `Err(())` here, before a socket is ever
/// touched — never a runtime validation `http::run` has to perform on a value that already parsed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct LaunchArgs {
    help: bool,
    allow_reversible: bool,
    allow_irreversible: bool,
    http: Option<u16>,
}

fn parse_launch_args(args: &[String]) -> Result<LaunchArgs, ()> {
    let mut parsed = LaunchArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" | "help" => parsed.help = true,
            "--allow-reversible" => parsed.allow_reversible = true,
            "--allow-irreversible" => parsed.allow_irreversible = true,
            "--http" => {
                let port = args.get(i + 1).ok_or(())?.parse::<u16>().map_err(|_| ())?;
                parsed.http = Some(port);
                i += 1;
            }
            _ => return Err(()),
        }
        i += 1;
    }
    Ok(parsed)
}

/// `agent mcp --help`: pure local text, exactly like the top-level `--help` — this runs BEFORE the
/// JSON-RPC loop starts, so a human-readable stdout line here breaks no protocol discipline. The
/// default tool list is DERIVED from [`tools`] itself, never a second hand-typed name list.
fn mcp_help_text() -> String {
    let default_tools = tools(Tier::Read);
    let default_names: Vec<&str> = default_tools
        .iter()
        .map(|t| t["name"].as_str().unwrap_or_default())
        .collect();
    format!(
        "ajh-tauri agent mcp [--allow-reversible] [--allow-irreversible] [--http <port>]\n\n\
         Run as an MCP (Model Context Protocol) server for Claude Code/Codex/any client, over \
         stdio by default; the desktop app must be running for any tool except `commands`.\n\n\
         FLAGS:\n\
         \x20 --allow-reversible     expose call-reversible (mutates state, undoable via the app)\n\
         \x20 --allow-irreversible   expose call-irreversible too (implies --allow-reversible)\n\
         \x20 --http <port>          serve MCP Streamable HTTP on 127.0.0.1:<port> instead of \
           stdio (no other bind shape is accepted); prints one \
           {{\"transport\":\"http\",\"url\":...,\"token\":...}} line to stdout, once, before \
           serving\n\
         \x20 --help, -h, help       show this help and exit (works even if the app is closed)\n\n\
         Default (no flags): {}.\n",
        default_names.join(", "),
    )
}

/// Writes [`mcp_help_text`] to `out` without an extra trailing blank line (LOW fix, review round
/// 3 — the text already ends in exactly one `\n`; `writeln!` doubled it). `write!`, never
/// `writeln!`.
fn print_help(out: &mut impl Write) -> std::io::Result<()> {
    write!(out, "{}", mcp_help_text())
}

/// `agent mcp [flags]` entrypoint — called from [`super::run`]'s own argv sentinel, before
/// [`super::parse_verb`], exactly like `--help`. Never wrapped in [`super::run_verb_within`]'s
/// whole-invocation [`super::INVOCATION_TIMEOUT`] (that would kill a long-lived server after
/// 90s); each `tools/call` gets its own budget via the SAME constant instead. A fresh
/// [`super::query`] call — one HMAC handshake — runs per tool call rather than holding one socket
/// open, so token freshness, `token.revoked` handling, and the shared `BridgeState` throttle all
/// behave exactly as they do for the plain CLI, for free.
pub(super) fn run(args: &[String]) -> i32 {
    let Ok(launch) = parse_launch_args(args) else {
        // Pre-protocol: no JSON-RPC frame exists yet, so stdout must stay silent — stderr only.
        // Never echoes the actual bad token (path privacy — a stray path-like argument must not
        // be reflected back).
        let _ = writeln!(
            std::io::stderr(),
            "unknown argument to `agent mcp` (expected: --allow-reversible, \
             --allow-irreversible, --http <port>, --help)"
        );
        return 2;
    };

    let out = stdout();
    if launch.help {
        let mut lock = out.lock();
        let _ = print_help(&mut lock);
        return 0;
    }

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => {
            let _ = writeln!(std::io::stderr(), "could not start an async runtime");
            return 2;
        }
    };
    let server = Server::new(launch.allow_reversible, launch.allow_irreversible);
    // Moves onto the dispatch thread WITH the runtime it owns, so `block_on` still runs from a
    // plain sync context (never inside the reactor) — just not on the thread that writes.
    let dispatch = move |verb: &Verb| -> Result<Value, &'static str> {
        rt.block_on(async {
            match timeout(INVOCATION_TIMEOUT, query(verb)).await {
                Ok(result) => result,
                Err(_) => Err(ERR_TIMEOUT),
            }
        })
    };
    // `--http` swaps the WIRE, never the handler: `http::run` shares `server` and `dispatch` with
    // the stdio path below verbatim — same tiers, same per-call bridge round trip, same
    // `handle_message` classifier (module doc's "Two transports, one handler" section).
    if let Some(port) = launch.http {
        return http::run(port, &server, dispatch);
    }
    // Never `stdin().lock()`/`out.lock()`: a `StdinLock`/`StdoutLock` is not `Send`, and reading
    // and writing now happen on different threads. `Stdin` itself is `Read` but not `BufRead`,
    // hence the `BufReader`; both handles lock internally per call, so the one-frame-per-line
    // discipline is unchanged (module doc).
    serve(
        BufReader::new(stdin()),
        out,
        &server,
        dispatch,
        INVOCATION_TIMEOUT,
    )
}

// `agent mcp --http <port>` — the Streamable HTTP transport sharing this module's classifier and
// dispatch (issue #1173). R8 LOC-cap split, the same move `instructions.rs`/`schemas.rs`/
// `results.rs` already made: this is the WIRE unit for that one transport, so nothing about the
// stdio loop or the shared per-request handler travelled with it.
mod http;

#[cfg(test)]
mod tests;

//! `ajh-tauri agent <verb>` — a thin CLI client over the loopback bridge
//! (issue #1084 PR 1, CLIENT half; the server half is [`super::agent_read`]).
//!
//! Runs as a MODE of the existing `ajh-tauri` binary, selected by an argv
//! sentinel in `main.rs`/`lib::run_agent_cli_if_invoked` — never a second
//! `[[bin]]` (the release upload globs only read `target/release/bundle/**`,
//! so a second binary would ship to nobody). The app must already be
//! running: this sends ONE `agent.query` frame over the same v2
//! mutual-HMAC-authenticated WebSocket the browser extension uses, prints the
//! `agent.result` payload as JSON on stdout, and exits. No DB access, no HTTP
//! server, no new port.
//!
//! ## Why not [`super::native_host::connect_bridge`]
//! That function returns on the first successful WS UPGRADE, before any
//! protocol frame — a port-squatter (anything else listening in
//! [`super::PORT_RANGE`]) would take a native-host relay down; for the CLI it
//! would misreport a squatter as a successful connection with no way to send
//! `agent.query` at all. [`handshake_client::connect_authenticated`] instead drives the FULL
//! v2 handshake (`hello`→`challenge`→`auth`→`auth.ok`) per candidate port and
//! only accepts the one whose **server** proof verifies (see
//! [`super::handshake::verify_server_proof`], added for this client — there
//! was previously no Rust-side implementation of this handshake; the browser
//! extension's lives in TS, `apps/extension/src/lib/bridge.ts`).
//!
//! **This defeats a dumb port squatter (one with no way to answer the
//! challenge), not a RELAYING one** — a local process that transparently
//! proxies bytes between us and the real app would pass the server-proof
//! check too, since it never has to know the token itself, only forward it.
//! The v2 handshake has no channel binding to close that gap; this is a
//! pre-existing, inherent limitation shared with the browser extension's own
//! handshake, not something this client-side change introduces or fixes.
//!
//! ## Exit codes (the process-level contract — see [`entrypoint::run`])
//! - `0` — `agent.result` replied `{"ok":true,...}`; the payload is on stdout.
//! - `1` — `agent.result` replied `{"ok":false,...}` (a server-side refusal:
//!   rate-limited, validation, not-found, autofill off, …) — the payload
//!   (including the fixed-sentinel `error` text) is still on stdout.
//! - `2` — no result was delivered: bad CLI usage, the app is not running,
//!   the connection failed for a reason that says nothing about whether the
//!   pairing token itself is valid, or the app itself refused/discarded the
//!   reply (any generic-tier `dispatched:false` — including
//!   [`agent_call::Refusal::ResultTooLarge`], where the command itself may
//!   ALREADY have run, and [`agent_call::Refusal::InvalidCursor`]). When the
//!   round trip never completed, a synthesized
//!   `{"ok":false,"resource":…,"error":<fixed sentinel>}` is printed instead
//!   of the (nonexistent) server payload. Never a raw absolute path or an
//!   echoed I/O error string — only fixed sentinels, so this CLI's own stdout
//!   never leaks a path into whatever reads it (an LLM agent's context).
//! - `4` — `call` only (ADR-038 §4, Phase 3): the target is
//!   `Effect::Irreversible` and no `--confirm` was supplied. The reply's
//!   `detail` names WHICH other read command/resource to read the proof
//!   value from and NEVER the value itself — a distinct outcome from a
//!   refusal (exit 2), never collapsed into it.

use std::path::Path;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::time::{timeout, Instant};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{ClientRequestBuilder, Message};

use crate::error::{AppError, AppResult};

use super::{
    agent_call, auth, handshake, msg, MAX_FRAME_BYTES, PORT_RANGE, PROTOCOL_VERSION, TOKEN_FILE,
};

pub(super) type WsStream = tokio_tungstenite::WebSocketStream<TcpStream>;

// ── Error sentinels (the exit-2 `error` field) ──────────────────────────────
// Named once, referenced everywhere they're emitted AND by `help_text()`'s
// own listing — never a second hand-typed copy, so `--help` can't drift from
// what this CLI actually returns (the same anti-drift discipline as
// `agent_read::RESOURCES`).
const ERR_APP_NOT_LOCATED: &str = "app_not_located";
const ERR_PAIRING_TOKEN_UNAVAILABLE: &str = "pairing_token_unavailable";
const ERR_APP_NOT_RUNNING: &str = "app_not_running";
const ERR_PAIRING_REJECTED: &str = "pairing_rejected";
const ERR_CONNECTION_ERROR: &str = "connection_error";
const ERR_RUNTIME_UNAVAILABLE: &str = "runtime_unavailable";
const ERR_CONNECTION_LOST: &str = "connection_lost";
const ERR_TIMEOUT: &str = "timeout";
const ERR_UNSUPPORTED_BY_APP: &str = "unsupported_by_app";
const ERR_USAGE: &str = "usage";

/// `(sentinel, meaning)` — [`entrypoint::help_text`] lists these verbatim.
const ERROR_SENTINELS: &[(&str, &str)] = &[
    (
        ERR_APP_NOT_LOCATED,
        "the app has not written its pointer file yet (needs a newer build, or has never launched)",
    ),
    (
        ERR_PAIRING_TOKEN_UNAVAILABLE,
        "no pairing token on disk yet",
    ),
    (
        ERR_APP_NOT_RUNNING,
        "nothing answered a connect on any candidate port",
    ),
    (
        ERR_PAIRING_REJECTED,
        "every reachable port rejected this token — re-pair from Settings",
    ),
    (
        ERR_CONNECTION_ERROR,
        "a handshake started but failed before authenticating — not evidence of a bad token",
    ),
    (
        ERR_UNSUPPORTED_BY_APP,
        "the running app doesn't understand this verb yet — update it",
    ),
    (
        ERR_TIMEOUT,
        "no reply within the round-trip budget, or the whole invocation ran past its overall deadline",
    ),
    (
        ERR_CONNECTION_LOST,
        "the socket closed or errored mid-round-trip",
    ),
    (ERR_RUNTIME_UNAVAILABLE, "could not start an async runtime"),
    (
        ERR_USAGE,
        "bad CLI usage — see \"detail\" for what was wrong",
    ),
];

/// Wall-clock bound on each individual step of [`handshake_client::attempt_port`] — the raw
/// `TcpStream::connect`, the WS upgrade (`client_async_with_config`), send
/// hello → await challenge, and send auth → await auth.ok. Generous for a
/// loopback round trip; short enough that one hung/squatting port can't
/// stall the whole invocation across [`PORT_RANGE`] (MAJOR fix — security
/// review round 2: `connect`/the WS upgrade used to be the two UNBOUNDED
/// exceptions to that claim — a local process that accepts on a candidate
/// port and never completes the upgrade, including a wedged previous app
/// instance whose accept loop stopped running but whose listener is still
/// bound, parked this fn, and so the whole CLI invocation, forever).
const HANDSHAKE_STEP_TIMEOUT: Duration = Duration::from_secs(5);

/// Wall-clock bound on the `agent.query` round trip itself, AFTER
/// authentication — sized above `best-matches`' measured worst case (~12.3s
/// at 4000 found jobs; see `agent_read`'s throttle doc), not the handshake
/// budget above.
const QUERY_REPLY_TIMEOUT: Duration = Duration::from_secs(30);

/// The WHOLE invocation's outer deadline (MAJOR fix — security review round
/// 2) — wraps [`entrypoint::run_verb`] in [`entrypoint::run`], so no COMBINATION of slow/hung
/// candidate ports can exceed it, even though each individual step above
/// already has its own bound. Derived from the worst *legitimate* sweep, not
/// just one port: up to 5 non-real ports in [`PORT_RANGE`] each maximally
/// stalling both connect and upgrade (2 × [`HANDSHAKE_STEP_TIMEOUT`] = 10s
/// apiece = 50s) before the real app's own port is even reached, PLUS that
/// real port's own worst-case full round trip (2 × `HANDSHAKE_STEP_TIMEOUT`
/// for challenge/auth-ok + [`QUERY_REPLY_TIMEOUT`] for a slow `best-matches`
/// ≈ 40s) — roughly 90s, so this sits right at that sum rather than
/// padding it further: a real hang should surface promptly, not merely
/// "eventually". On expiry [`entrypoint::run`] reports [`ERR_TIMEOUT`] — the same
/// sentinel `send_agent_query_within`'s own post-auth timeout uses, since
/// from the caller's side both mean the identical thing: the CLI gave up
/// after its round-trip budget, whichever phase burned it.
const INVOCATION_TIMEOUT: Duration = Duration::from_secs(90);

// ── the argv → verb parsers, each verb's own sub-module ─────────────────────
// Split out under R8's LOC cap; see each file's own doc for what it holds.
mod entrypoint;
mod parse;
mod parse_call;
mod parse_found_jobs;
mod pointer;

// ── the v2 handshake's client half and the round trip built on it ───────────
mod handshake_client;
mod query;

// ── the verb vocabulary every parser above fills in ─────────────────────────
mod verb;

// `lib::run_agent_cli_if_invoked` calls this from OUTSIDE this module tree, so
// the entrypoint itself stays `pub` at the `agent_cli` path it has always had.
pub use entrypoint::run;

// The vocabulary the sub-modules above and `mcp` below all resolve through.
// One `use` per owning sub-module, so the graph stays readable: every name is
// still declared exactly once, in its own file, and every consumer reaches it
// exactly where it did when all of this was one file.
use entrypoint::{exit_code_for_reply, usage_error_value};
use handshake_client::{connect_authenticated, next_json, pairing_failure_sentinel};
use parse::{parse_bool_flag, parse_verb};
use parse_call::parse_call;
use parse_found_jobs::parse_found_jobs;
use pointer::{read_agent_pointer, read_pairing_token};
use query::query;
use verb::{verb_names_joined, Verb, VERB_TABLE};

// ADR-038 §1 — the command policy table (row count pinned by
// `policy::tests::policy_table_row_count_is_pinned`, never restated here)
// + its exactness test against `generate_handler!`. Data only in this
// phase: nothing here dispatches yet (§2's generic `agent call
// <ns>:<command>` tier is later).
pub(crate) mod policy;

// @generated — the declared input contract for every dispatchable command
// (issues #1163, #1158, #1160): a one-line description, top-level argument
// keys, required-ness, and nested field names for a wrapper key. Emitted by
// `pnpm gen:agent-catalogue` from the tauri-client `invoke()` call sites +
// the IPC contract TSDoc — never hand-edited, wired the same way
// `ipc_contracts` is. Read by `agent_call`'s dispatch-time key validation and
// by the MCP `commands` tool.
pub(crate) mod catalogue;

// The MCP (Model Context Protocol) stdio server mode — `agent mcp`.
mod mcp;

#[cfg(test)]
mod tests;

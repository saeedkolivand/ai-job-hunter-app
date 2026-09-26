//! `agent mcp --http <port>` — the MCP Streamable HTTP transport (issue #1173, amending
//! ADR-040), sharing [`super::handle_message`] and every tier/throttle/cap decision with the
//! stdio mode (see `mcp.rs`'s "Two transports, one handler" doc). This module owns ONLY the
//! wire: binding, one bearer token, the `Origin` refusal, and hand-rolled HTTP/1.1 framing —
//! never a second copy of the JSON-RPC classifier or dispatch path.
//!
//! ## Why hand-rolled, not `hyper`
//! `hyper`, `httparse` and `hyper-util` are already in this workspace's `Cargo.lock`
//! transitively (via `tokio-tungstenite`'s WebSocket stack), but wiring `hyper`'s async
//! `Service`/`Body` traits for one fixed route is more machinery than this surface needs — the
//! same reasoning ADR-040 §3 already gave for hand-rolling the stdio JSON-RPC wire instead of an
//! SDK crate. A request here is one method, one path, one JSON body bounded by
//! [`crate::extension_bridge::MAX_FRAME_BYTES`]: a blocking `std::net::TcpListener` accept loop
//! plus a request-line/header reader sized to that shape is smaller and more auditable than an
//! async service stack, and it costs no new dependency at all (not even a promoted transitive
//! one).
//!
//! ## Bind, auth, framing (every rule pinned by a test in this module)
//! - **`127.0.0.1` only, by construction.** [`run`]'s [`TcpListener::bind`] call is hardcoded to
//!   that host; `--http` (`mcp.rs`'s [`super::LaunchArgs`]/[`super::parse_launch_args`]) has no
//!   flag shape that can express a different host or address at all, so there is nothing for
//!   this module to validate at runtime — the refusal is at argv-parse time, before a socket
//!   exists.
//! - **One per-launch bearer token**, `rand`-backed (the same OS-seeded generator
//!   `extension_bridge::persist::new_token` already uses for the pairing token), printed to
//!   stdout exactly once, before the accept loop starts, as one compact JSON line —
//!   `{"transport":"http","url":"http://127.0.0.1:<port>/mcp","token":"<hex>"}` — and NEVER
//!   written to any file. A client that loses it must relaunch the server; there is no recovery
//!   path, by design (a recoverable token defeats the point of a random one).
//! - **`Origin` refuses BEFORE anything else** — before the token is even checked. This
//!   transport has no CORS support and expects a non-browser caller (a local script, or an
//!   agent framework's own HTTP client); a request carrying `Origin` at all came from something
//!   that behaves like a browser (a page's `fetch`, or a DNS-rebinding attempt against this
//!   loopback port), and gets `403` regardless of whether it also carried a valid token.
//! - **No SSE, no sessions.** `GET`/`DELETE /mcp` both answer `405`: this server never opens a
//!   server-initiated stream and issues no `Mcp-Session-Id`, so there is nothing for either verb
//!   to do. `POST /mcp` is the only request/response the client ever needs — a JSON-RPC request
//!   body answers with one JSON body (`Content-Type: application/json`); a notification body
//!   (no `id`, or `id: null`, or any `notifications/*` method) answers `202` with an empty body,
//!   matching [`super::Routed::Drop`] exactly as the stdio wire does.
//! - **Framing is checked before a body is ever read, never silently treated as empty (issue
//!   #1184).** A `POST` with no `Content-Length` at all answers `411 Length Required` — an absent
//!   declared length is a transport error, not "an empty body". A `Transfer-Encoding` header of
//!   any kind answers `501 Not Implemented`: this server never decodes chunked (or any other)
//!   transfer coding, so treating one as an empty, already-fully-read body would silently drop
//!   the request's real payload. Only once both are ruled out is the DECLARED `Content-Length`
//!   checked against [`crate::extension_bridge::MAX_FRAME_BYTES`] for the `413` below — an
//!   inbound body over that cap never reaches the JSON-RPC layer at all, so an oversized request
//!   costs this server nothing but the headers. The `result_too_large` sentinel (`mcp/results.rs`)
//!   is unrelated: that one is for an outbound REPLY that grew too large to answer honestly, never
//!   for inbound framing.
//! - **Every response closes the connection** (`Connection: close`): this is a request/response
//!   transport with no pipelining and no keep-alive reuse to get right, so closing after each
//!   reply is the simplest correct behaviour rather than an optimization left undone.
//! - **Every accepted connection carries a read/write deadline** ([`CONNECTION_IO_TIMEOUT`]):
//!   this loop serves one connection at a time (see [`run`]'s own doc), so a peer that connects
//!   and never completes a request — or never drains a reply — would otherwise stall every later
//!   request behind it forever. The gate this would stall is pre-auth, so no token is needed to
//!   trigger it; loopback-only narrows who can connect, not how long a connection can be held
//!   open.
//!
//! ## Shutdown
//! Ctrl-C is unhandled here, exactly as in the stdio mode: `SIGINT`'s OS default (process
//! termination) is the whole mechanism, on both wires. A background thread mirrors the stdio
//! mode's stdin-EOF drain by watching stdin itself and exiting the process the moment it closes
//! — this transport answers every request inline before returning to the accept loop, so there
//! is no in-flight call to drain and no deadline to bound; on EOF there is simply nothing left
//! to wait for.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use super::{handle_message, rpc_error, Server, Verb};
/// Read/write deadline armed on every accepted connection, before [`handle_connection`] ever
/// touches it (issue #1184 T5): [`run`]'s accept loop serves one connection at a time, so a peer
/// that never finishes sending its request — or never reads its reply — would otherwise hold this
/// whole server hostage indefinitely, pre-auth. A few seconds is generous for a same-machine
/// loopback client and short enough that a hung peer costs this server almost nothing.
const CONNECTION_IO_TIMEOUT: Duration = Duration::from_secs(5);

// One unit per responsibility, split out under R8's LOC cap; each reaches the shared
// state, the types and the two deadlines through its own `use super::*;`.
mod auth;
mod connection;
mod request;
mod response;

// Re-exported for the units above and for this module's own tests, so no sibling ever
// reaches into another directly.
use auth::{constant_time_eq, new_bearer_token};
use connection::serve_one_connection;
use request::{read_request_head, Headers};
use response::{write_json, write_status};

/// `agent mcp --http <port>` entrypoint, called from [`super::run`] with the SAME `server` and
/// `dispatch` the stdio path would have used (module doc). Binds `127.0.0.1:<port>` (`0` asks
/// the OS for an ephemeral port; the ACTUAL bound port, read back via [`TcpListener::local_addr`],
/// is what the startup line reports, so a caller never has to guess which one it got), prints the
/// startup line, then serves forever — one connection at a time, matching the stdio wire's own
/// single-flight dispatch guarantee — until the process exits via [`std::process::exit`] on
/// stdin EOF (module doc) or is killed by `SIGINT`/`SIGTERM`.
pub(super) fn run(
    port: u16,
    server: &Server,
    mut dispatch: impl FnMut(&Verb) -> Result<Value, &'static str>,
) -> i32 {
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(_) => {
            let _ = writeln!(std::io::stderr(), "could not bind 127.0.0.1:{port}");
            return 2;
        }
    };
    let bound_port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    let token = new_bearer_token();
    // The ONE stdout write this transport ever makes (module doc) — before the accept loop, the
    // same "one line, once, before serving" shape `--help` and the stdio `initialize` reply use
    // for their own single pre-loop writes.
    println!(
        "{}",
        serde_json::json!({
            "transport": "http",
            "url": format!("http://127.0.0.1:{bound_port}/mcp"),
            "token": token,
        })
    );
    let _ = std::io::stdout().flush();

    // Mirrors the stdio mode's shutdown-on-EOF (module doc): this transport has no in-flight
    // call to drain (every request is answered before `handle_connection` returns), so there is
    // nothing to wait for — the instant stdin closes, exit.
    let _ = thread::Builder::new()
        .name("mcp-http-stdin-watch".to_string())
        .spawn(|| {
            let mut discard = String::new();
            while std::io::stdin().lock().read_line(&mut discard).unwrap_or(0) > 0 {
                discard.clear();
            }
            std::process::exit(0);
        });

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        serve_one_connection(stream, server, &mut dispatch, &token, CONNECTION_IO_TIMEOUT);
    }
    0
}

#[cfg(test)]
mod tests;

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
//! - **Body cap is a transport-level `413`, not the JSON-RPC `result_too_large` sentinel.** The
//!   `result_too_large` refusal (`mcp/results.rs`) is for an outbound REPLY that grew too large
//!   to answer honestly; an inbound body over [`crate::extension_bridge::MAX_FRAME_BYTES`] never
//!   reaches the JSON-RPC layer at all — checked against the declared `Content-Length` before a
//!   byte of body is read, so an oversized request costs this server nothing but the headers.
//! - **Every response closes the connection** (`Connection: close`): this is a request/response
//!   transport with no pipelining and no keep-alive reuse to get right, so closing after each
//!   reply is the simplest correct behaviour rather than an optimization left undone.
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

use serde_json::Value;

use super::{handle_message, rpc_error, Server, Verb};

/// Total request-line + header bytes this server reads before giving up on a connection without
/// answering it. Loopback-only and gated by the same bearer token every real request needs, so
/// this is a defensive cap on a misbehaving peer, not a security boundary in itself — sized well
/// above any header set a real MCP client sends and well below anything worth allocating for.
const MAX_HEADER_BYTES: usize = 64 * 1024;

/// A 32-byte random bearer token, lowercase hex — the same shape and generator
/// `extension_bridge::persist::new_token` uses for the pairing token (issue #1173: ">= 128 bits
/// from the OS RNG the app already uses"; 32 bytes is 256 bits, matching that sibling rather
/// than cutting a new, smaller convention).
fn new_bearer_token() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Fixed-time comparison — a bearer token is a secret compared against caller-supplied input on
/// every request, so a length-then-byte early-exit (`==` on `&str`) would leak how many leading
/// bytes matched through response timing. `a`'s length varies only with what this process itself
/// generated, so branching on a length MISMATCH first leaks nothing about the token's content.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Request line + headers, lower-cased header names — everything [`handle_connection`] needs to
/// route and gate one request. Bounded by [`MAX_HEADER_BYTES`]; `None` on a malformed request, an
/// oversized header section, or a connection that closed before headers finished — all answered
/// the same way (silently dropped): nothing has been read that is safe to reply to yet.
fn read_request_head(
    reader: &mut impl BufRead,
) -> Option<(String, String, HashMap<String, String>)> {
    let mut total = 0usize;
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).ok()? == 0 {
        return None;
    }
    total += request_line.len();
    let mut parts = request_line.trim_end().splitn(3, ' ');
    let method = parts.next()?.to_string();
    let raw_path = parts.next()?.to_string();
    parts.next()?; // HTTP version — unread past the presence check
    let path = raw_path.split('?').next().unwrap_or_default().to_string();

    let mut headers = HashMap::new();
    loop {
        if total > MAX_HEADER_BYTES {
            return None;
        }
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None; // connection closed before the blank line that ends headers
        }
        total += n;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    Some((method, path, headers))
}

/// `HTTP/1.1 <code> <reason>` with a bare body (or none) and `Connection: close`. Used for every
/// non-200 outcome; the one 200 case ([`write_json`]) always carries a JSON body, so it has its
/// own writer rather than an empty-`Content-Type` special case here.
fn write_status(stream: &mut TcpStream, code: u16, reason: &str, body: &[u8]) {
    let _ = write!(
        stream,
        "HTTP/1.1 {code} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

/// The one 200 response shape this server ever writes: a JSON-RPC reply frame, verbatim, as
/// `application/json` — byte-for-byte the same [`Value`] the stdio wire would have written as a
/// line (module doc's "Two transports, one handler").
fn write_json(stream: &mut TcpStream, frame: &Value) {
    let body = frame.to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

/// `POST /mcp` past the `Origin`/auth/path/method gates in [`handle_connection`]: read the body
/// (capped against the DECLARED `Content-Length`, never against bytes actually read, so an
/// oversized request is refused before this server reads a single body byte), parse it, and run
/// it through the SAME [`handle_message`] the stdio worker thread calls for a bridge-backed
/// `tools/call` — the one function this whole module exists to reuse rather than reimplement.
fn handle_post(
    stream: &mut TcpStream,
    reader: &mut impl BufRead,
    headers: &HashMap<String, String>,
    server: &Server,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
) {
    let content_length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if content_length > crate::extension_bridge::MAX_FRAME_BYTES {
        write_status(stream, 413, "Payload Too Large", b"");
        return;
    }
    let mut body = vec![0u8; content_length];
    if std::io::Read::read_exact(reader, &mut body).is_err() {
        write_status(stream, 400, "Bad Request", b"");
        return;
    }
    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        // Same shape `route_line` returns for an unparsable stdio line: the TRANSPORT succeeded,
        // the JSON-RPC payload didn't parse, so this is still a 200 carrying a JSON-RPC error —
        // never a raw 400, which would be a second, undocumented error channel.
        Err(_) => {
            write_json(stream, &rpc_error(Value::Null, -32700, "Parse error"));
            return;
        }
    };
    match handle_message(parsed, server, dispatch) {
        Some(frame) => write_json(stream, &frame),
        // A notification: no reply, ever — `202` with an empty body is the closest HTTP has to
        // "accepted, nothing to say back", matching `Routed::Drop` on the stdio wire exactly.
        None => write_status(stream, 202, "Accepted", b""),
    }
}

/// One connection, start to finish: read the request head, gate it (`Origin` first, then the
/// bearer token, then path/method), and answer. Every early return has already written (or
/// deliberately not written) a response and closed the socket — there is nothing left to do
/// after this function returns.
fn handle_connection(
    stream: TcpStream,
    server: &Server,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
    token: &str,
) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut writer = stream;

    let Some((method, path, headers)) = read_request_head(&mut reader) else {
        return; // malformed / closed before headers finished — nothing safe to answer with yet
    };

    // `Origin` refuses BEFORE the token check (module doc): a browser-originated request is
    // refused outright regardless of whether it also carries a valid bearer token.
    if headers.contains_key("origin") {
        write_status(&mut writer, 403, "Forbidden", b"");
        return;
    }

    let authorized = headers
        .get("authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|presented| constant_time_eq(presented, token));
    if !authorized {
        write_status(&mut writer, 401, "Unauthorized", b"");
        return;
    }

    if path != "/mcp" {
        write_status(&mut writer, 404, "Not Found", b"");
        return;
    }
    match method.as_str() {
        "POST" => handle_post(&mut writer, &mut reader, &headers, server, dispatch),
        // No SSE, no sessions (module doc) — GET would open a server-initiated stream and
        // DELETE would end a session; this server offers neither, so both are a plain 405
        // rather than an empty stream or a no-op success that would misstate the contract.
        _ => write_status(&mut writer, 405, "Method Not Allowed", b""),
    }
}

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
        handle_connection(stream, server, &mut dispatch, &token);
    }
    0
}

#[cfg(test)]
mod tests;

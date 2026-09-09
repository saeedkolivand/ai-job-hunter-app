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

/// Total request-line + header bytes this server reads before giving up on a connection without
/// answering it. Loopback-only and gated by the same bearer token every real request needs, so
/// this is a defensive cap on a misbehaving peer, not a security boundary in itself — sized well
/// above any header set a real MCP client sends and well below anything worth allocating for.
/// Enforced per LINE, not only on the running total (issue #1184 T2) — see [`read_capped_line`].
const MAX_HEADER_BYTES: usize = 64 * 1024;

/// Read/write deadline armed on every accepted connection, before [`handle_connection`] ever
/// touches it (issue #1184 T5): [`run`]'s accept loop serves one connection at a time, so a peer
/// that never finishes sending its request — or never reads its reply — would otherwise hold this
/// whole server hostage indefinitely, pre-auth. A few seconds is generous for a same-machine
/// loopback client and short enough that a hung peer costs this server almost nothing.
const CONNECTION_IO_TIMEOUT: Duration = Duration::from_secs(5);

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

/// Bounded alternative to [`BufRead::read_line`] (issue #1184 T2): reads one line, through its
/// trailing `\n` inclusive, off `reader`'s OWN internal buffer via [`BufRead::fill_buf`]/
/// [`BufRead::consume`] — never a second buffering layer wrapped around it, which would silently
/// strand bytes belonging to the NEXT line inside a throwaway buffer. Refuses (`Err(true)`) the
/// instant reading one more byte would exceed `limit`, so an unterminated line cannot grow past
/// what remains under the caller's cap — unlike `read_line`, whose only check runs AFTER the
/// (unbounded) line has already been read in full. `Ok(vec![])` on immediate EOF, mirroring
/// `read_line`'s own `Ok(0)`; `Err(false)` on a genuine I/O error (a hung/reset peer — not a size
/// problem, so the caller must not answer `431` for it).
fn read_capped_line(reader: &mut impl BufRead, limit: usize) -> Result<Vec<u8>, bool> {
    let mut out = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(|_| false)?;
        if available.is_empty() {
            return Ok(out); // EOF — `out` holds whatever arrived before the peer closed
        }
        let newline_at = available.iter().position(|&b| b == b'\n').map(|p| p + 1);
        let take = newline_at.unwrap_or(available.len());
        if out.len() + take > limit {
            return Err(true);
        }
        out.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline_at.is_some() {
            return Ok(out);
        }
    }
}

/// Lower-cased header name → value, as parsed by [`read_request_head`]. Named rather than left as
/// an inline `HashMap<String, String>` purely so [`read_request_head`]'s own `Result<_, bool>`
/// signature never puts `Result<` and a `HashMap<String, String>` on the same source line (R6's
/// stringly-`Result` scan is a plain per-line text match — see `tests/architecture.rs` — and would
/// otherwise misread this tuple's UNRELATED `String` fields as a stringly error type; the actual
/// error here is `bool`).
type Headers = HashMap<String, String>;

/// Request line + headers, lower-cased header names — everything [`handle_connection`] needs to
/// route and gate one request. The WHOLE head (request line plus every header line) is bounded by
/// [`MAX_HEADER_BYTES`], enforced per LINE via [`read_capped_line`] against a single `remaining`
/// counter (issue #1184 T2) rather than checked only after each line, which let one line with no
/// `\n` grow without bound before the check ever ran. `Err(true)` once that cap is hit — the
/// caller answers `431`. `Err(false)` for a malformed request line, a connection that closed
/// before headers finished, or a genuine read error — nothing has been read that is safe to reply
/// to, so the caller drops the connection silently for those, exactly as before this fix.
fn read_request_head(reader: &mut impl BufRead) -> Result<(String, String, Headers), bool> {
    let mut remaining = MAX_HEADER_BYTES;

    let request_line = read_capped_line(reader, remaining)?;
    if request_line.is_empty() {
        return Err(false); // EOF before a byte of the request line arrived
    }
    remaining -= request_line.len();
    let request_line = String::from_utf8_lossy(&request_line);
    let mut parts = request_line.trim_end().splitn(3, ' ');
    let method = parts.next().ok_or(false)?.to_string();
    let raw_path = parts.next().ok_or(false)?.to_string();
    parts.next().ok_or(false)?; // HTTP version — unread past the presence check
    let path = raw_path.split('?').next().unwrap_or_default().to_string();

    let mut headers = HashMap::new();
    loop {
        let line = read_capped_line(reader, remaining)?;
        if line.is_empty() {
            return Err(false); // connection closed before the blank line that ends headers
        }
        remaining -= line.len();
        let line = String::from_utf8_lossy(&line);
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    Ok((method, path, headers))
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
///
/// Framing is checked BEFORE any of that (issue #1184 T3): an absent `Content-Length` used to
/// `unwrap_or(0)` into "read zero bytes", and a `Transfer-Encoding: chunked` body read the same
/// way since its framing is never decoded — both then reached the JSON-RPC parser as an empty
/// body, which reported `-32700 Parse error` for what is actually a transport problem, not a
/// malformed payload. `Transfer-Encoding` is checked first: a request carrying it is unsupported
/// regardless of what `Content-Length` says.
fn handle_post(
    stream: &mut TcpStream,
    reader: &mut impl BufRead,
    headers: &Headers,
    server: &Server,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
) {
    if headers.contains_key("transfer-encoding") {
        write_status(stream, 501, "Not Implemented", b"");
        return;
    }
    let Some(content_length) = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
    else {
        write_status(stream, 411, "Length Required", b"");
        return;
    };
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

    let (method, path, headers) = match read_request_head(&mut reader) {
        Ok(head) => head,
        // The header section itself exceeded MAX_HEADER_BYTES (issue #1184 T2) — the one case
        // here that IS safe to answer, since the cap fired on a bounded read, not a dead socket.
        Err(true) => {
            write_status(&mut writer, 431, "Request Header Fields Too Large", b"");
            return;
        }
        // Malformed request line, a connection closed before headers finished, or a genuine I/O
        // error — nothing has been read that is safe to reply to yet.
        Err(false) => return,
    };

    // `Origin` refuses BEFORE the token check (module doc): a browser-originated request is
    // refused outright regardless of whether it also carries a valid bearer token.
    if headers.contains_key("origin") {
        write_status(&mut writer, 403, "Forbidden", b"");
        return;
    }

    // Scheme compared case-insensitively (RFC 7235 §2.1: `auth-scheme` is a `token`, matched
    // case-insensitively) — issue #1184 T4 — the credential itself stays an exact,
    // constant-time comparison.
    let authorized = headers
        .get("authorization")
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .is_some_and(|(_, presented)| constant_time_eq(presented, token));
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

/// Arms [`CONNECTION_IO_TIMEOUT`] on `stream` before handing it to [`handle_connection`] (issue
/// #1184 T5) — pulled out of [`run`]'s own accept loop, with the deadline as a PARAMETER rather
/// than reading the module constant directly, so a test can exercise a hung peer without actually
/// waiting out the production timeout (the same reason `mcp/stdio.rs`'s `serve` takes its own
/// `drain_budget` as a parameter rather than a constant). Silently drops the connection if either
/// deadline fails to set (an OS-level failure on a fresh socket, not expected in practice) rather
/// than serving it with no bound at all.
fn serve_one_connection(
    stream: TcpStream,
    server: &Server,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
    token: &str,
    io_timeout: Duration,
) {
    if stream.set_read_timeout(Some(io_timeout)).is_ok()
        && stream.set_write_timeout(Some(io_timeout)).is_ok()
    {
        handle_connection(stream, server, dispatch, token);
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
        serve_one_connection(stream, server, &mut dispatch, &token, CONNECTION_IO_TIMEOUT);
    }
    0
}

#[cfg(test)]
mod tests;

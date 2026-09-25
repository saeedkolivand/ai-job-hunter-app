//! One connection, start to finish: the `Origin`/auth/path/method gates, the `POST` body, and
//! the per-connection I/O deadline. Everything a peer can be refused for, in the order the
//! refusals are allowed to happen.

use super::*;

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
pub(super) fn handle_post(
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
pub(super) fn handle_connection(
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
pub(super) fn serve_one_connection(
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

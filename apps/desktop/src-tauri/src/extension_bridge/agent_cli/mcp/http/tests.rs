//! Issue #1173 — the HTTP transport shares [`super::super::handle_message`] with the stdio wire
//! (`mcp::tests` already pins that classifier/dispatch path), so this module's own tests cover
//! only what is genuinely NEW here: the bind/auth/framing gates in [`super::handle_connection`],
//! never a second copy of a `tools/call` classification test.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use super::super::{Server, Tier, Verb};
use super::*;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd";

fn stub_ok(_verb: &Verb) -> Result<Value, &'static str> {
    Ok(json!({ "ok": true, "resource": "stub", "data": {} }))
}

/// Bind an ephemeral loopback listener, accept exactly ONE connection on a background thread
/// running the real [`handle_connection`] with [`TOKEN`] as its bearer secret, and hand the test
/// the port to connect to plus the join handle to wait on. `Server::new(true, true)` (the
/// broadest tier) unless a test needs a narrower one.
fn spawn_one_connection(server: Server) -> (u16, thread::JoinHandle<()>) {
    spawn_one_connection_with(server, stub_ok)
}

/// [`spawn_one_connection`] with a caller-supplied dispatch stub, for the one test (T8) that
/// needs the bridge to answer with something other than the tiny [`stub_ok`] payload.
fn spawn_one_connection_with(
    server: Server,
    mut dispatch: impl FnMut(&Verb) -> Result<Value, &'static str> + Send + 'static,
) -> (u16, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let port = listener.local_addr().expect("local_addr").port();
    let join = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept the one test connection");
        handle_connection(stream, &server, &mut dispatch, TOKEN);
    });
    (port, join)
}

/// Connect, write `request` verbatim, then read until the server closes its end (every response
/// this transport writes carries `Connection: close` — module doc).
fn send_raw(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.write_all(request.as_bytes()).expect("write request");
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    response
}

fn status_line(response: &str) -> &str {
    response.lines().next().unwrap_or_default()
}

fn body_of(response: &str) -> &str {
    response.split_once("\r\n\r\n").map_or("", |(_, b)| b)
}

fn post_request(path: &str, extra_headers: &str, body: &str) -> String {
    format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{extra_headers}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
}

#[test]
fn post_without_a_bearer_token_is_401() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req = post_request("/mcp", "", r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#);
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 401 Unauthorized");
}

#[test]
fn post_with_the_wrong_bearer_token_is_401() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req = post_request(
        "/mcp",
        "Authorization: Bearer not-the-real-token\r\n",
        r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 401 Unauthorized");
}

/// The task's own ordering requirement: `Origin` refuses BEFORE the token check, so a request
/// carrying both a VALID token and an `Origin` header must still be refused.
#[test]
fn origin_header_is_403_even_with_a_valid_token() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req = post_request(
        "/mcp",
        &format!("Origin: http://evil.example\r\nAuthorization: Bearer {TOKEN}\r\n"),
        r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 403 Forbidden");
}

#[test]
fn post_tools_list_matches_the_stdio_tool_set_for_the_launched_tier() {
    let (port, join) = spawn_one_connection(Server::new(true, true));
    let req = post_request(
        "/mcp",
        &format!("Authorization: Bearer {TOKEN}\r\n"),
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/list"}"#,
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
    assert!(
        response.contains("Content-Type: application/json"),
        "{response}"
    );
    let parsed: Value = serde_json::from_str(body_of(&response)).expect("valid json body");
    assert_eq!(
        parsed["result"]["tools"],
        json!(super::super::tools(Tier::Irreversible)),
        "the HTTP `tools/list` reply must equal the stdio wire's own tool set for the same tier, \
         not a second hand-built list"
    );
}

#[test]
fn a_notification_gets_202_with_no_body() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req = post_request(
        "/mcp",
        &format!("Authorization: Bearer {TOKEN}\r\n"),
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 202 Accepted");
    assert_eq!(
        body_of(&response),
        "",
        "a notification must get no body back"
    );
}

#[test]
fn get_on_mcp_is_405_no_sse() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req =
        format!("GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\n\r\n");
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 405 Method Not Allowed");
}

#[test]
fn delete_on_mcp_is_405_no_sessions() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req =
        format!("DELETE /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\n\r\n");
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 405 Method Not Allowed");
}

#[test]
fn unknown_path_is_404() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req =
        format!("GET /nope HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\n\r\n");
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 404 Not Found");
}

/// The declared `Content-Length` alone must trigger the refusal — the test never actually sends
/// that many bytes, which is only possible if [`super::handle_post`] checks the header BEFORE
/// attempting to read the body (module doc).
#[test]
fn a_declared_content_length_over_the_frame_cap_is_413_before_any_body_is_read() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let declared = crate::extension_bridge::MAX_FRAME_BYTES + 1;
    let req = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {declared}\r\n\r\n"
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 413 Payload Too Large");
}

#[test]
fn malformed_json_body_is_a_200_carrying_a_jsonrpc_parse_error() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req = post_request(
        "/mcp",
        &format!("Authorization: Bearer {TOKEN}\r\n"),
        "not json",
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
    let parsed: Value = serde_json::from_str(body_of(&response)).expect("valid json body");
    assert_eq!(parsed["error"]["code"], json!(-32700));
}

#[test]
fn constant_time_eq_matches_ordinary_string_equality() {
    assert!(constant_time_eq("abc", "abc"));
    assert!(!constant_time_eq("abc", "abd"));
    assert!(!constant_time_eq("abc", "abcd"));
    assert!(!constant_time_eq("", "a"));
    assert!(constant_time_eq("", ""));
}

/// T3 (PR #1184 CodeRabbit review): a `POST` with no `Content-Length` at all must not read as an
/// empty body — that used to surface as a `200` carrying `-32700 Parse error`, telling the client
/// its JSON was malformed when the real problem was the missing transport framing.
#[test]
fn post_without_a_content_length_is_411() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\n\
         Content-Type: application/json\r\n\r\n"
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 411 Length Required");
}

/// T3: `Transfer-Encoding` framing is never decoded by this server, so a request carrying it must
/// be refused outright rather than read as an (empty) `Content-Length`-less body.
#[test]
fn post_with_a_transfer_encoding_header_is_501() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {TOKEN}\r\n\
         Transfer-Encoding: chunked\r\n\r\n"
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 501 Not Implemented");
}

/// T4 (PR #1184 CodeRabbit review): RFC 7235 §2.1 makes `auth-scheme` a case-insensitive token —
/// a real client that sends `bearer` (or `BEARER`) must still authenticate.
#[test]
fn bearer_scheme_is_accepted_case_insensitively() {
    let (port, join) = spawn_one_connection(Server::new(false, false));
    let req = post_request(
        "/mcp",
        &format!("Authorization: bearer {TOKEN}\r\n"),
        r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
}

/// T5 (PR #1184 CodeRabbit review): this accept loop serves one connection at a time
/// ([`super::run`]'s own doc), so a peer that connects and never sends a complete request must
/// not be able to stall it forever — [`super::serve_one_connection`]'s read/write deadline is
/// what bounds that stall. Uses a short deadline (never the real
/// [`super::CONNECTION_IO_TIMEOUT`]) so this test does not itself wait out production's timeout.
#[test]
fn a_hung_peer_does_not_block_the_next_connection_from_being_answered() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let port = listener.local_addr().expect("local_addr").port();
    let server = Server::new(false, false);
    let short_timeout = Duration::from_millis(200);
    let join = thread::spawn(move || {
        let mut dispatch = stub_ok;
        // First connection: a peer that connects and sends nothing, ever — the exact case a
        // missing read timeout would block this single-threaded loop on forever.
        let (hung, _) = listener.accept().expect("accept the hanging connection");
        serve_one_connection(hung, &server, &mut dispatch, TOKEN, short_timeout);
        // Second connection: an ordinary request, which must still be answered — proving the
        // first connection's stall was bounded rather than starving this loop.
        let (ok, _) = listener.accept().expect("accept the second connection");
        serve_one_connection(ok, &server, &mut dispatch, TOKEN, short_timeout);
    });
    let _hanging_client = TcpStream::connect(("127.0.0.1", port)).expect("connect (hangs)");
    // Gives the accept loop a moment to pick up the hanging connection first, so this test
    // actually exercises "hung connection first", not a race between the two connects.
    thread::sleep(Duration::from_millis(30));
    let req = post_request(
        "/mcp",
        &format!("Authorization: Bearer {TOKEN}\r\n"),
        r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
    );
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
}

/// T8, end to end through the REAL HTTP transport (not `resources::resource_result` alone): an
/// oversized `resources/read` reply must reach the wire as a JSON-RPC `error` member, never a
/// `result` member carrying the capped text as if it were the requested resource — the same
/// guarantee `mcp::tests`' stdio-path sibling test pins for the other transport.
#[test]
fn resources_read_over_the_size_cap_is_a_jsonrpc_error_over_http() {
    let huge = json!({
        "ok": true, "resource": "profile",
        "blob": "x".repeat(super::super::results::MCP_RESULT_MAX_BYTES + 10),
    });
    let (port, join) =
        spawn_one_connection_with(Server::new(true, true), move |_: &Verb| Ok(huge.clone()));
    let body = json!({
        "jsonrpc": "2.0", "id": 1, "method": "resources/read",
        "params": { "uri": super::super::resources::URI_PROFILE },
    })
    .to_string();
    let req = post_request("/mcp", &format!("Authorization: Bearer {TOKEN}\r\n"), &body);
    let response = send_raw(port, &req);
    join.join().expect("handler thread must not panic");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
    let parsed: Value = serde_json::from_str(body_of(&response)).expect("valid json body");
    assert!(
        parsed.get("result").is_none(),
        "an oversized resources/read reply must never carry a `result` member: {parsed}"
    );
    assert_eq!(parsed["error"]["code"], json!(-32603));
    assert_eq!(parsed["error"]["message"], json!("result_too_large"));
}

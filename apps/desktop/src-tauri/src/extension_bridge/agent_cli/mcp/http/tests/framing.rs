//! Request-framing gates: body size, Content-Length, Transfer-Encoding and malformed JSON.

use super::*;

/// The declared `Content-Length` alone must trigger the refusal — the test never actually sends
/// that many bytes, which is only possible if [`super::super::connection::handle_post`] checks the header BEFORE
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

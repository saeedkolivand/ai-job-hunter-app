//! The only two response shapes this server writes: a bare status line for every non-200, and
//! the one 200 JSON-RPC frame. Both close the socket themselves — there is no keep-alive.

use super::*;

/// `HTTP/1.1 <code> <reason>` with a bare body (or none) and `Connection: close`. Used for every
/// non-200 outcome; the one 200 case ([`write_json`]) always carries a JSON body, so it has its
/// own writer rather than an empty-`Content-Type` special case here.
pub(super) fn write_status(stream: &mut TcpStream, code: u16, reason: &str, body: &[u8]) {
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
pub(super) fn write_json(stream: &mut TcpStream, frame: &Value) {
    let body = frame.to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

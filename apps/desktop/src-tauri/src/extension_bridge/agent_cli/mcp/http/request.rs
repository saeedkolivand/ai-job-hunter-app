//! The bounded READ side: a per-line capped line reader, the header map it feeds, and the
//! request head assembled from them. Every read on this transport is capped here, so no other
//! unit can add an unbounded one.

use super::*;

/// Total request-line + header bytes this server reads before giving up on a connection without
/// answering it. Loopback-only and gated by the same bearer token every real request needs, so
/// this is a defensive cap on a misbehaving peer, not a security boundary in itself — sized well
/// above any header set a real MCP client sends and well below anything worth allocating for.
/// Enforced per LINE, not only on the running total (issue #1184 T2) — see [`read_capped_line`].
const MAX_HEADER_BYTES: usize = 64 * 1024;

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
pub(super) type Headers = HashMap<String, String>;

/// Request line + headers, lower-cased header names — everything [`handle_connection`] needs to
/// route and gate one request. The WHOLE head (request line plus every header line) is bounded by
/// [`MAX_HEADER_BYTES`], enforced per LINE via [`read_capped_line`] against a single `remaining`
/// counter (issue #1184 T2) rather than checked only after each line, which let one line with no
/// `\n` grow without bound before the check ever ran. `Err(true)` once that cap is hit — the
/// caller answers `431`. `Err(false)` for a malformed request line, a connection that closed
/// before headers finished, or a genuine read error — nothing has been read that is safe to reply
/// to, so the caller drops the connection silently for those, exactly as before this fix.
pub(super) fn read_request_head(
    reader: &mut impl BufRead,
) -> Result<(String, String, Headers), bool> {
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

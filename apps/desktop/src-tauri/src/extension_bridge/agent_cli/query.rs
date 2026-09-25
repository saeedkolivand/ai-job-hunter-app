//! The authenticated `agent.query` round trip: budgeted frame exchange on an
//! already-connected socket, and the one bridge call every verb (and the MCP wire) shares. Split
//! out of `agent_cli.rs` under R8's LOC cap.

use super::*;
// ── agent.query round trip ──────────────────────────────────────────────────

/// [`send_agent_query`], but with the overall budget as an explicit
/// parameter — directly unit-testable against a real (but fast) loopback
/// server without waiting out the real 30s [`QUERY_REPLY_TIMEOUT`].
/// Production always goes through the convenience wrapper below.
///
/// Waits for the matching `agent.result` (by `reqId`), within `budget`
/// overall. A `token.revoked` seen instead (the pairing was rotated
/// mid-session) is reported distinctly rather than left to time out. Any
/// OTHER frame carrying a DIFFERENT `reqId` is ignored — a fresh, one-shot
/// connection should never see one, but ignoring rather than failing on it
/// costs nothing and is more robust to a future additive frame.
async fn send_agent_query_within(
    mut ws: WsStream,
    verb: &Verb,
    budget: Duration,
) -> Result<Value, &'static str> {
    let req_id = uuid::Uuid::new_v4().to_string();
    let frame = json!({
        "type": verb.wire_type(),
        "reqId": req_id,
        "payload": verb.payload(),
    })
    .to_string();
    if ws.send(Message::text(frame)).await.is_err() {
        return Err(ERR_CONNECTION_LOST);
    }

    let deadline = Instant::now() + budget;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ERR_TIMEOUT);
        }
        let Some(v) = next_json(&mut ws, remaining).await else {
            // `next_json` returning `None` is EITHER a genuine timeout (it
            // burned the whole `remaining` budget waiting) OR a real
            // transport failure (a close/IO error/malformed frame arriving
            // well BEFORE the deadline) — the two used to collapse into one
            // `connection_lost`, so a real 30s round-trip timeout (measured
            // against v0.144.0: `agent schema` returned `connection_lost`
            // after burning the full budget) misreported as a transport
            // failure instead. Distinguish by checking the clock: only a
            // call that actually reached the deadline is a timeout (same
            // defect class as `6bdd6785` — a definite outcome misreported as
            // something else).
            return if Instant::now() >= deadline {
                Err(ERR_TIMEOUT)
            } else {
                Err(ERR_CONNECTION_LOST)
            };
        };
        match v.get("type").and_then(Value::as_str) {
            Some(t)
                if t == verb.reply_type()
                    && v.get("reqId").and_then(Value::as_str) == Some(req_id.as_str()) =>
            {
                return v.get("payload").cloned().ok_or(ERR_CONNECTION_LOST);
            }
            Some(t) if t == msg::TOKEN_REVOKED => return Err(ERR_PAIRING_REJECTED),
            _ if v.get("reqId").and_then(Value::as_str) == Some(req_id.as_str()) => {
                // Any OTHER frame carrying OUR OWN reqId is precisely
                // detectable: an app that doesn't understand this verb's
                // wire type replies via `advance_authenticated`'s "unknown
                // message type" fallback, echoing this exact reqId on an
                // `import.result` envelope. Fail fast instead of waiting out
                // the full `QUERY_REPLY_TIMEOUT` for a reply that will never
                // arrive.
                return Err(ERR_UNSUPPORTED_BY_APP);
            }
            _ => continue,
        }
    }
}

/// Send one `agent.query`, budgeted at [`QUERY_REPLY_TIMEOUT`].
async fn send_agent_query(ws: WsStream, verb: &Verb) -> Result<Value, &'static str> {
    send_agent_query_within(ws, verb, QUERY_REPLY_TIMEOUT).await
}
/// The bridge round trip common to every verb, minus the CLI's own
/// stdout/exit-code translation: pointer → token → [`super::handshake_client::connect_authenticated`]
/// → [`send_agent_query`]. [`super::entrypoint::run_verb`] is now just this plus its own
/// `println!`/[`super::entrypoint::exit_code_for_reply`] wrapping; [`mcp`] calls this directly
/// instead of `run_verb`, since the MCP wire owns its own stdout discipline
/// (a single `writeln!` site — see that module's doc) and must never emit
/// this fn's sibling's bare `println!` envelope.
pub(super) async fn query(verb: &Verb) -> Result<Value, &'static str> {
    let pointer = read_agent_pointer().ok_or(ERR_APP_NOT_LOCATED)?;
    let token = read_pairing_token(&pointer.data_dir).ok_or(ERR_PAIRING_TOKEN_UNAVAILABLE)?;
    let ws = connect_authenticated(&token)
        .await
        .map_err(pairing_failure_sentinel)?;
    send_agent_query(ws, verb).await
}

#[cfg(test)]
mod tests;

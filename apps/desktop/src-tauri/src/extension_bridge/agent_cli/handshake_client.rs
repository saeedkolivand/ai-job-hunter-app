//! The v2 mutual handshake's client half: the four wire frames, the bounded
//! per-step reads, the port sweep, and the `PortOutcome` → `PairingFailure` classification that
//! decides which exit-2 sentinel the whole invocation reports. Split out of `agent_cli.rs` under
//! R8's LOC cap.

use super::*;
// ── v2 mutual handshake — the client half (see the module doc) ─────────────

/// Build the `hello` frame (handshake step 1).
fn build_hello(client_nonce: &str) -> String {
    json!({
        "type": msg::HELLO,
        "reqId": "cli-hello",
        "payload": { "protocol": PROTOCOL_VERSION, "clientNonce": client_nonce },
    })
    .to_string()
}

/// Extract `serverNonce` from a `challenge` reply, validating its shape
/// (mirrors the server's own `is_valid_nonce` check on the client nonce).
/// `None` for anything that isn't a well-formed challenge.
fn parse_challenge(v: &Value) -> Option<String> {
    if v.get("type").and_then(Value::as_str) != Some(msg::CHALLENGE) {
        return None;
    }
    let nonce = v.get("payload")?.get("serverNonce")?.as_str()?;
    handshake::is_valid_nonce(nonce).then(|| nonce.to_string())
}

/// Build the `auth` frame (handshake step 3) carrying the client's proof.
fn build_auth(proof: &str) -> String {
    json!({
        "type": msg::AUTH,
        "reqId": "cli-auth",
        "payload": { "proof": proof },
    })
    .to_string()
}

/// Extract `serverProof` from an `auth.ok` reply. `None` for anything else
/// (a different type, a missing/non-string field).
fn parse_auth_ok(v: &Value) -> Option<String> {
    if v.get("type").and_then(Value::as_str) != Some(msg::AUTH_OK) {
        return None;
    }
    Some(v.get("payload")?.get("serverProof")?.as_str()?.to_string())
}

/// Read the next parseable JSON text frame within `dur`, silently skipping
/// ping/pong control frames. `None` on timeout, a transport error, a close,
/// or non-JSON content — every one of those collapses to the same "this port
/// gave us nothing usable" signal for the caller.
///
/// `dur` is a single deadline for the WHOLE call, computed once on entry —
/// NOT re-armed on every loop iteration. A peer that emits a ping/pong (or
/// any other non-Text/Binary/Close frame) faster than `dur` would otherwise
/// keep resetting `timeout`'s clock forever and this call — and everything
/// waiting on it, including [`super::query::send_agent_query`]'s own shrinking
/// `remaining` budget, which never gets a chance to re-run while this loop
/// is stuck — would never return.
pub(super) async fn next_json(ws: &mut WsStream, dur: Duration) -> Option<Value> {
    let deadline = Instant::now() + dur;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let msg = match timeout(remaining, ws.next()).await {
            Ok(Some(Ok(m))) => m,
            _ => return None,
        };
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => String::from_utf8(b.to_vec()).ok()?,
            Message::Close(_) => return None,
            _ => continue,
        };
        return serde_json::from_str(&text).ok();
    }
}

/// One candidate port's outcome, coarse enough to drive
/// [`classify_pairing_failure`] without leaking WHICH failure mode occurred
/// (see that function's doc for why only three buckets exist).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortOutcome {
    /// Nothing answered a TCP connect / WS upgrade on this port.
    NoUpgrade,
    /// The WS upgrade succeeded, but the connection ended BEFORE we ever sent
    /// our `auth` proof (an I/O error, a timeout, or a malformed/missing
    /// challenge). NOT evidence about our own token — issue #1084 PR1's own
    /// decision: "a crash between challenge and auth is not a pairing
    /// failure."
    PreAuthError,
    /// We sent `auth{proof}`, and the port failed to answer with a
    /// VERIFYING `auth.ok` — silence, a close, a malformed reply, or a
    /// `serverProof` that failed constant-time verification. Folded into one
    /// bucket because the server's own failed-proof path is, by design, a
    /// silent close indistinguishable from a crash (see
    /// `extension_bridge::advance_auth`'s doc) — once we have committed our
    /// proof, any non-verifying outcome is attributed to proof rejection.
    ProofRejected,
}

/// Drive the full handshake against one port. `Ok` only once the SERVER's
/// proof has verified (mutual auth complete); every other case reports
/// [`PortOutcome`] instead of the (now-dropped) socket.
///
/// Both `connect` and the WS upgrade below are wrapped in
/// [`HANDSHAKE_STEP_TIMEOUT`] (MAJOR fix — security review round 2): before
/// this fix they were the two UNBOUNDED steps in an otherwise fully-budgeted
/// function — a local process that accepts a connection on this port and
/// never completes either step (a wedged previous app instance whose
/// listener is still bound but whose accept loop stopped running is exactly
/// this: `connect` succeeds instantly off the kernel's own backlog, then the
/// upgrade read waits forever for a reply nothing will ever send) parked
/// this fn, and so [`connect_authenticated`]'s whole port loop, forever. Both
/// timeout outcomes fold into [`PortOutcome::NoUpgrade`] — "nothing usable
/// answered" is exactly what that variant already means, whether the cause
/// was a refused connect, a rejected upgrade, or one of these now-bounded
/// hangs.
async fn attempt_port(port: u16, token: &str) -> Result<WsStream, PortOutcome> {
    let tcp = timeout(
        HANDSHAKE_STEP_TIMEOUT,
        TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .map_err(|_| PortOutcome::NoUpgrade)?
    .map_err(|_| PortOutcome::NoUpgrade)?;
    let uri = format!("ws://127.0.0.1:{port}/")
        .parse()
        .map_err(|_| PortOutcome::NoUpgrade)?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));
    // Its OWN sentinel Origin (finding #5, security review) — distinct from
    // the native host's, so the server can tell "the CLI" apart from "the
    // browser extension arriving via the native-host relay" and gate
    // `agent.query` on it (see `auth::AGENT_CLI_ORIGIN`'s doc for exactly
    // what this label does and doesn't defend against). The origin check
    // remains defense-in-depth only; the mutual HMAC handshake below is the
    // real boundary.
    let request = ClientRequestBuilder::new(uri).with_header("Origin", auth::AGENT_CLI_ORIGIN);
    let (mut ws, _resp) = timeout(
        HANDSHAKE_STEP_TIMEOUT,
        tokio_tungstenite::client_async_with_config(request, tcp, Some(config)),
    )
    .await
    .map_err(|_| PortOutcome::NoUpgrade)?
    .map_err(|_| PortOutcome::NoUpgrade)?;

    let client_nonce = handshake::new_nonce();
    if ws
        .send(Message::text(build_hello(&client_nonce)))
        .await
        .is_err()
    {
        return Err(PortOutcome::PreAuthError);
    }
    let server_nonce = next_json(&mut ws, HANDSHAKE_STEP_TIMEOUT)
        .await
        .and_then(|v| parse_challenge(&v))
        .ok_or(PortOutcome::PreAuthError)?;

    let proof = handshake::client_proof(token, &server_nonce, &client_nonce);
    if ws.send(Message::text(build_auth(&proof))).await.is_err() {
        // Delivery itself is unconfirmed — we never received anything that
        // could be a rejection signal, so this is NOT a proof rejection.
        return Err(PortOutcome::PreAuthError);
    }

    let server_proof = next_json(&mut ws, HANDSHAKE_STEP_TIMEOUT)
        .await
        .and_then(|v| parse_auth_ok(&v));
    match server_proof {
        Some(proof)
            if handshake::verify_server_proof(token, &server_nonce, &client_nonce, &proof) =>
        {
            Ok(ws)
        }
        _ => Err(PortOutcome::ProofRejected),
    }
}

/// Why every candidate port fell short of authenticating, folded into ONE
/// process-level verdict — see the exit-code table in the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PairingFailure {
    /// No port in [`PORT_RANGE`] answered a TCP connect/WS upgrade at all.
    AppNotRunning,
    /// Every port that upgraded also rejected our proof — the persisted
    /// pairing token is stale (this CLI process's copy, read fresh from the
    /// token file every invocation, no longer matches the app's).
    PairingRejected,
    /// At least one upgraded port failed BEFORE the proof-rejection point —
    /// inconclusive, and specifically NOT evidence the token is wrong (see
    /// [`PortOutcome::PreAuthError`]'s doc).
    ConnectionError,
}

/// Pure aggregation over this invocation's [`PortOutcome`]s — kept separate
/// from the async I/O in [`connect_authenticated`] so it is directly
/// unit-testable. Only counts ports that actually upgraded ("every port
/// completed an upgrade and every one rejected the proof" from a range where,
/// in practice, only ONE port is ever bound — the rest are simply absent).
fn classify_pairing_failure(outcomes: &[PortOutcome]) -> PairingFailure {
    let saw_upgrade = outcomes.iter().any(|o| *o != PortOutcome::NoUpgrade);
    let saw_pre_auth_error = outcomes.contains(&PortOutcome::PreAuthError);
    if !saw_upgrade {
        PairingFailure::AppNotRunning
    } else if saw_pre_auth_error {
        PairingFailure::ConnectionError
    } else {
        PairingFailure::PairingRejected
    }
}

/// For each port in [`PORT_RANGE`], drive the full handshake and accept the
/// first one whose server proof verifies. See the module doc for why this
/// must not reuse [`super::super::native_host::connect_bridge`].
pub(super) async fn connect_authenticated(token: &str) -> Result<WsStream, PairingFailure> {
    let mut outcomes = Vec::new();
    for port in PORT_RANGE {
        match attempt_port(port, token).await {
            Ok(ws) => return Ok(ws),
            Err(outcome) => outcomes.push(outcome),
        }
    }
    Err(classify_pairing_failure(&outcomes))
}
pub(super) fn pairing_failure_sentinel(f: PairingFailure) -> &'static str {
    match f {
        PairingFailure::AppNotRunning => ERR_APP_NOT_RUNNING,
        PairingFailure::PairingRejected => ERR_PAIRING_REJECTED,
        PairingFailure::ConnectionError => ERR_CONNECTION_ERROR,
    }
}

#[cfg(test)]
mod tests;

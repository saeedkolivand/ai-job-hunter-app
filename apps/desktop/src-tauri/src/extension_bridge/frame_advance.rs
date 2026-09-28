//! The handshake state machine that PRODUCES a [`super::frame::FrameDecision`] — `ConnState`,
//! `advance_frame_from`, and the handshake step functions (`advance_hello`/`advance_auth`) plus
//! their reply builders. Split from `frame.rs` (R8 relief, PR4) so the DECISION TYPE and the
//! STATE MACHINE that produces it stay under the LOC cap as two separate concerns; behaviourally
//! identical to the pre-split module. `pub(super)` throughout — see `frame`'s own doc for how
//! `mod.rs`/`test.rs`/`caller_gate.rs` keep resolving these names unqualified.

use serde_json::{json, Value};

use super::frame::FrameDecision;
use super::{
    caller_gate::advance_authenticated,
    handshake, msg,
    req_id_cap::{oversized_req_id_reply, MAX_REQ_ID_BYTES},
    BridgeState, CallerClass, MAX_FRAME_BYTES, PROTOCOL_VERSION,
};

/// Per-connection handshake state. A socket starts `AwaitingHello`; a valid
/// protocol-2 `hello` moves it to `AwaitingAuth` (holding the two fresh nonces);
/// a **verified** client proof moves it to `Authenticated`. Only in
/// `Authenticated` are `import.request` / `profile.get` frames honored — the
/// socket is session-authenticated, so those frames carry no token.
#[cfg_attr(test, derive(Debug, Clone, PartialEq, Eq))]
pub(super) enum ConnState {
    /// Fresh socket — the next frame must be a protocol-2 `hello`.
    AwaitingHello,
    /// `hello` accepted; a `challenge` was sent. The next frame must be
    /// `auth { proof }`; these nonces bind the expected proof.
    AwaitingAuth {
        server_nonce: String,
        client_nonce: String,
    },
    /// Mutual handshake complete — subsequent frames are session-authorized.
    Authenticated,
}

/// The per-message handshake gate + dispatch routing (size cap → JSON parse →
/// state-machine step) — everything that does NOT need an `AppHandle`. Pure
/// aside from reading the pairing token off [`BridgeState`] for the
/// constant-time proof check; the loop performs the I/O and the app-stateful
/// import/profile work. See [`ConnState`] for the state transitions.
///
/// `caller` is THIS connection's own resolved `CallerClass`, resolved once
/// by `handle_connection` (finding #5, security review; extended in PR1) —
/// never re-derived here, since only the WS handshake ever sees the raw
/// header. Production (`handle_connection`) calls this directly; every
/// EXISTING test in this crate exercises extension-origin traffic and goes
/// through [`advance_frame`] below instead, so none of them had to learn a
/// new parameter.
pub(super) fn advance_frame_from(
    state: &BridgeState,
    conn: &ConnState,
    text: &str,
    caller: CallerClass,
) -> FrameDecision {
    if text.len() > MAX_FRAME_BYTES {
        return FrameDecision::CloseOverCap;
    }

    let envelope: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return FrameDecision::Drop, // not JSON — drop silently
    };

    let kind = envelope.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let req_id = envelope
        .get("reqId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let payload = envelope.get("payload");

    // Bound BEFORE type dispatch, every verb + handshake state (see `req_id_cap`'s doc).
    if req_id.len() > MAX_REQ_ID_BYTES {
        return FrameDecision::Reply(oversized_req_id_reply());
    }

    match conn {
        ConnState::AwaitingHello => advance_hello(kind, &req_id, payload),
        ConnState::AwaitingAuth {
            server_nonce,
            client_nonce,
        } => advance_auth(state, kind, &req_id, payload, server_nonce, client_nonce),
        ConnState::Authenticated => advance_authenticated(state, kind, req_id, &envelope, caller),
    }
}

/// [`advance_frame_from`] with `caller: CallerClass::Other` — extension-origin
/// traffic that is NOT the paired extension's own caller class, the shape
/// every PRE-EXISTING test in this crate already exercises.
#[cfg(test)]
pub(super) fn advance_frame(state: &BridgeState, conn: &ConnState, text: &str) -> FrameDecision {
    advance_frame_from(state, conn, text, CallerClass::Other)
}

/// Handshake step 1: the FIRST frame must be a valid protocol-2 `hello`. A legacy
/// `{type:'auth', token}` frame, a missing/older `protocol`, or a malformed
/// `clientNonce` are all treated as an outdated client → `update_required` + close.
fn advance_hello(kind: &str, req_id: &str, payload: Option<&Value>) -> FrameDecision {
    if kind != msg::HELLO {
        return FrameDecision::Outdated(update_required_reply(req_id));
    }
    let protocol = payload
        .and_then(|p| p.get("protocol"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let client_nonce = payload
        .and_then(|p| p.get("clientNonce"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if protocol < PROTOCOL_VERSION || !handshake::is_valid_nonce(client_nonce) {
        return FrameDecision::Outdated(update_required_reply(req_id));
    }
    // Fresh server nonce (CSPRNG, per connection). Bind it + the client nonce into
    // the next state so the proof is verified against exactly this pair.
    let server_nonce = handshake::new_nonce();
    let reply = challenge_reply(req_id, &server_nonce);
    FrameDecision::Challenge {
        reply,
        next: ConnState::AwaitingAuth {
            server_nonce,
            client_nonce: client_nonce.to_string(),
        },
    }
}

/// Handshake step 3: only an `auth { proof }` is valid here. The proof is verified
/// CONSTANT-TIME against `HMAC-SHA256(token, CLIENT_MSG)`; on success the desktop
/// proves ITSELF via `serverProof` (step 4). Any other frame, or a bad/absent
/// proof, closes the socket (never connected).
fn advance_auth(
    state: &BridgeState,
    kind: &str,
    req_id: &str,
    payload: Option<&Value>,
    server_nonce: &str,
    client_nonce: &str,
) -> FrameDecision {
    if kind != msg::AUTH {
        return FrameDecision::Unauthorized;
    }
    let proof = payload
        .and_then(|p| p.get("proof"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let token = state.token();
    if !handshake::verify_client_proof(&token, server_nonce, client_nonce, proof) {
        log::warn!("[extension_bridge] handshake: client proof failed constant-time verification");
        return FrameDecision::Unauthorized;
    }
    let server_proof = handshake::server_proof(&token, server_nonce, client_nonce);
    FrameDecision::AuthOk(auth_ok_reply(req_id, &server_proof))
}

/// Build the `challenge` reply (handshake step 2) carrying the fresh server nonce.
fn challenge_reply(req_id: &str, server_nonce: &str) -> String {
    json!({
        "type": msg::CHALLENGE,
        "reqId": req_id,
        "payload": { "serverNonce": server_nonce },
    })
    .to_string()
}

/// Build the `auth.ok` reply (handshake step 4) carrying the desktop's proof.
fn auth_ok_reply(req_id: &str, server_proof: &str) -> String {
    json!({
        "type": msg::AUTH_OK,
        "reqId": req_id,
        "payload": { "serverProof": server_proof },
    })
    .to_string()
}

/// Build the `update_required` force-cutover reply. Sent, then the socket closes,
/// when the first frame is not a valid protocol-2 `hello` (an old extension).
fn update_required_reply(req_id: &str) -> String {
    json!({
        "type": msg::UPDATE_REQUIRED,
        "reqId": req_id,
        "payload": {
            "error": "Update the AI Job Hunter browser extension to reconnect (bridge protocol v2)."
        },
    })
    .to_string()
}

#[cfg(test)]
mod tests;

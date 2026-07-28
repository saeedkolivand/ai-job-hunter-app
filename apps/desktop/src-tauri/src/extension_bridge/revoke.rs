//! The `token.revoked` wire surface — what a socket is told (and, for an
//! unauthenticated one, deliberately NOT told) when the pairing token rotates.
//!
//! Split out of `mod.rs` when the rotation-epoch work pushed that module past
//! the R8 hard LOC cap, and cohesive on its own terms: this is where the
//! no-oracle rule (ADR-0010) is expressed as a pure decision, so both arms are
//! unit-testable without a live socket or an `AppHandle`.

use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use super::msg;

/// Envelope `reqId` for the desktop-initiated [`msg::TOKEN_REVOKED`] frame.
/// Every other frame echoes the caller's id, but this one answers no request —
/// a fixed, non-empty sentinel keeps the envelope shape (`reqId` is required on
/// both sides) without pretending to correlate to anything.
pub(super) const REVOKE_REQ_ID: &str = "token-revoked";

/// The frames a socket is sent when its pairing is revoked — the no-oracle gate
/// itself, as a pure decision.
///
/// - `authenticated == true` → the `token.revoked` frame, then a clean WS close.
/// - `authenticated == false` → **nothing**. A mid-handshake / never-said-hello
///   peer is closed silently, exactly like a failed proof: a frame here would
///   confirm that the token it was proving against had been valid, which is the
///   oracle ADR-0010's reply-less close exists to deny.
pub(super) fn revoke_frames(authenticated: bool) -> Vec<Message> {
    if !authenticated {
        return Vec::new();
    }
    vec![Message::text(token_revoked_reply()), Message::Close(None)]
}

/// Build the `token.revoked` frame. NO payload and, deliberately, no token
/// material of any kind: a revoked peer is told only that its pairing is dead,
/// never anything about the old or the new secret.
pub(super) fn token_revoked_reply() -> String {
    json!({
        "type": msg::TOKEN_REVOKED,
        "reqId": REVOKE_REQ_ID,
        "payload": Value::Null,
    })
    .to_string()
}

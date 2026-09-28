//! Split by topic (R8 relief — this unit's tests alone exceed the LOC cap): `handshake` covers
//! the v2 mutual-HMAC state machine; `classify_import`/`classify_write` cover per-verb
//! `FrameDecision` classification (read verbs vs. write verbs); `frame_caps` covers the size cap
//! + SSRF host guard + non-JSON drop. The shared `awaiting_auth`/`reply_error` fixtures below.

use super::*;

mod classify_import;
mod classify_write;
mod frame_caps;
mod handshake;
mod req_id_cap;

pub(super) fn awaiting_auth(state: &BridgeState) -> (ConnState, String) {
    use crate::extension_bridge::handshake;
    let server_nonce = handshake::new_nonce();
    let client_nonce = handshake::new_nonce();
    let proof = handshake::client_proof(&state.token(), &server_nonce, &client_nonce);
    (
        ConnState::AwaitingAuth {
            server_nonce,
            client_nonce,
        },
        proof,
    )
}

/// The `error` string of an `import.result` reply, or `None` if it is a success
/// payload. Panics if the reply is not a well-formed `import.result` envelope.
pub(super) fn reply_error(reply: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(reply).expect("reply must be JSON");
    assert_eq!(
        v.get("type").and_then(|t| t.as_str()),
        Some(msg::IMPORT_RESULT),
        "reply must be an import.result envelope"
    );
    v.get("payload")
        .and_then(|p| p.get("error"))
        .and_then(|e| e.as_str())
        .map(str::to_string)
}

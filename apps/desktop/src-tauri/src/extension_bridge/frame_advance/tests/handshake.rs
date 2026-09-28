use super::super::*;

use super::super::super::test_support::bridge_state;
use super::awaiting_auth;

// ─────────────────────────────────────────────────────────────────────────────
// B1. v2 mutual-handshake state machine (the token-never-on-the-wire fix)
//
// `advance_frame` is the per-message gate the connection loop runs. The security
// invariant: an `import.request` / `profile.get` is dispatched ONLY from the
// `Authenticated` state (i.e. AFTER a verified client proof). A socket that has
// not completed the handshake can NEVER reach `handle_import` — so nothing is
// ever persisted for an unauthenticated peer.
// ─────────────────────────────────────────────────────────────────────────────

/// A valid protocol-2 `hello` (with a well-formed clientNonce) is accepted:
/// `Challenge` carrying a fresh `serverNonce`, advancing to `AwaitingAuth` bound
/// to that nonce pair. NOT yet connected.
#[test]
fn hello_v2_is_accepted_and_advances_to_awaiting_auth() {
    let (_dir, state) = bridge_state();
    let client_nonce = handshake::new_nonce();

    let frame = json!({
        "type": msg::HELLO,
        "reqId": "r-hello",
        "payload": { "protocol": PROTOCOL_VERSION, "clientNonce": client_nonce },
    })
    .to_string();

    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Challenge { reply, next } => {
            let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(v["type"], msg::CHALLENGE);
            assert_eq!(v["reqId"], "r-hello");
            let server_nonce = v["payload"]["serverNonce"].as_str().unwrap();
            assert!(
                handshake::is_valid_nonce(server_nonce),
                "challenge must carry a well-formed server nonce"
            );
            match next {
                ConnState::AwaitingAuth {
                    server_nonce: sn,
                    client_nonce: cn,
                } => {
                    assert_eq!(sn, server_nonce, "next state binds the sent server nonce");
                    assert_eq!(cn, client_nonce, "next state binds the client nonce");
                }
                other => panic!("hello must advance to AwaitingAuth, got {other:?}"),
            }
        }
        other => panic!("a valid v2 hello must be Challenge, got {other:?}"),
    }
}

/// A legacy `{type:'auth', token}` FIRST frame (an OLD extension) is `Outdated`:
/// `update_required` reply then close — the force cutover, never an import.
#[test]
fn legacy_auth_first_frame_is_outdated() {
    let (_dir, state) = bridge_state();

    let frame = json!({
        "type": msg::AUTH,
        "token": state.token(), // legacy plaintext token — must NOT authenticate
        "reqId": "r-legacy",
        "payload": serde_json::Value::Null,
    })
    .to_string();

    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(reply) => {
            let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(v["type"], msg::UPDATE_REQUIRED);
            assert_eq!(v["reqId"], "r-legacy");
            assert!(v["payload"]["error"].as_str().unwrap().contains("Update"));
        }
        other => panic!("a legacy token `auth` first frame must be Outdated, got {other:?}"),
    }
}

/// A `hello` carrying a lower/older protocol is treated as an outdated client.
#[test]
fn hello_with_lower_protocol_is_outdated() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::HELLO,
        "reqId": "r-old",
        "payload": { "protocol": 1, "clientNonce": handshake::new_nonce() },
    })
    .to_string();
    assert!(
        matches!(
            advance_frame(&state, &ConnState::AwaitingHello, &frame),
            FrameDecision::Outdated(_)
        ),
        "protocol < 2 must be Outdated"
    );
}

/// A `hello` with a malformed clientNonce (wrong shape) is rejected as outdated —
/// junk never reaches the HMAC.
#[test]
fn hello_with_malformed_nonce_is_outdated() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::HELLO,
        "reqId": "r-bad-nonce",
        "payload": { "protocol": PROTOCOL_VERSION, "clientNonce": "not-hex!!" },
    })
    .to_string();
    assert!(matches!(
        advance_frame(&state, &ConnState::AwaitingHello, &frame),
        FrameDecision::Outdated(_)
    ));
}

/// An `import.request` in the AwaitingHello state is NEVER dispatched — it is an
/// outdated first frame (not a hello). This is the core invariant: you cannot
/// import before completing the handshake.
#[test]
fn import_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::IMPORT_REQUEST,
        "reqId": "r-early",
        "payload": { "url": "https://jobs.example.com/posting/early" },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => panic!("an import.request before hello must NOT be an Import, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B1b. Handshake step 3: constant-time client-proof verification
//
// In the AwaitingAuth state, only an `auth { proof }` with a proof that verifies
// constant-time against HMAC-SHA256(token, CLIENT_MSG) advances to Authenticated
// (AuthOk, replying the server proof). A wrong/absent proof, or any non-auth
// frame, closes the socket (Unauthorized) and never marks it connected.
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: the AwaitingAuth state for a fixed nonce pair, plus the CORRECT client
/// proof the extension would compute for `state`'s token.
/// A correct client proof advances to `AuthOk`: the reply is an `auth.ok`
/// envelope whose `serverProof` equals `HMAC(token, SERVER_MSG)` for the bound
/// nonces — so the extension can verify the desktop is genuine.
#[test]
fn correct_proof_yields_auth_ok_with_matching_server_proof() {
    let (_dir, state) = bridge_state();
    let (conn, proof) = awaiting_auth(&state);
    let (server_nonce, client_nonce) = match &conn {
        ConnState::AwaitingAuth {
            server_nonce,
            client_nonce,
        } => (server_nonce.clone(), client_nonce.clone()),
        _ => unreachable!(),
    };

    let frame = json!({
        "type": msg::AUTH,
        "reqId": "r-auth",
        "payload": { "proof": proof },
    })
    .to_string();

    match advance_frame(&state, &conn, &frame) {
        FrameDecision::AuthOk(reply) => {
            let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(v["type"], msg::AUTH_OK);
            assert_eq!(v["reqId"], "r-auth");
            let server_proof = v["payload"]["serverProof"].as_str().unwrap();
            assert_eq!(
                server_proof,
                handshake::server_proof(&state.token(), &server_nonce, &client_nonce),
                "the desktop must return the exact server proof the extension expects"
            );
        }
        other => panic!("a correct client proof must be AuthOk, got {other:?}"),
    }
}

/// A WRONG proof (right shape, wrong bytes) is `Unauthorized` — the socket closes
/// and is never marked connected. This is the token-mismatch path (bad_token).
#[test]
fn wrong_proof_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    // A validly-shaped but wrong proof (all zeros).
    let frame = json!({
        "type": msg::AUTH,
        "reqId": "r-bad",
        "payload": { "proof": "0".repeat(64) },
    })
    .to_string();
    assert!(matches!(
        advance_frame(&state, &conn, &frame),
        FrameDecision::Unauthorized
    ));
}

/// An absent/empty proof is `Unauthorized` (never a panic, never connected).
#[test]
fn absent_proof_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": msg::AUTH,
        "reqId": "r-empty",
        "payload": serde_json::Value::Null,
    })
    .to_string();
    assert!(matches!(
        advance_frame(&state, &conn, &frame),
        FrameDecision::Unauthorized
    ));
}

/// A non-auth frame in AwaitingAuth (e.g. an import.request trying to skip the
/// proof) is `Unauthorized` — you cannot bypass step 3.
#[test]
fn non_auth_frame_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": msg::IMPORT_REQUEST,
        "reqId": "r-skip",
        "payload": { "url": "https://jobs.example.com/posting/skip" },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("an import.request mid-handshake must be Unauthorized, got {other:?}"),
    }
}

/// End-to-end pure handshake: AwaitingHello --hello--> Challenge(next) ; feed the
/// derived server+client nonces the REAL client proof --auth--> AuthOk. Proves the
/// two-frame mutual handshake composes with the actual nonces the desktop issues.
#[test]
fn full_handshake_hello_then_auth_authenticates() {
    let (_dir, state) = bridge_state();
    let client_nonce = handshake::new_nonce();

    let hello = json!({
        "type": msg::HELLO,
        "reqId": "h",
        "payload": { "protocol": PROTOCOL_VERSION, "clientNonce": client_nonce },
    })
    .to_string();

    let next = match advance_frame(&state, &ConnState::AwaitingHello, &hello) {
        FrameDecision::Challenge { next, .. } => next,
        other => panic!("hello must Challenge, got {other:?}"),
    };
    let server_nonce = match &next {
        ConnState::AwaitingAuth { server_nonce, .. } => server_nonce.clone(),
        _ => unreachable!(),
    };

    // The extension computes the client proof for the issued nonces.
    let proof = handshake::client_proof(&state.token(), &server_nonce, &client_nonce);
    let auth = json!({
        "type": msg::AUTH,
        "reqId": "a",
        "payload": { "proof": proof },
    })
    .to_string();

    assert!(
        matches!(
            advance_frame(&state, &next, &auth),
            FrameDecision::AuthOk(_)
        ),
        "the real client proof for the issued nonces must authenticate"
    );
}

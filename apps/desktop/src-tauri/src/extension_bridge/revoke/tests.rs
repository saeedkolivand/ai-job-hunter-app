use super::*;

#[test]
fn revoke_frames_tells_an_unauthenticated_socket_nothing() {
    // The desktop half of the no-oracle rule (ADR-0010). An unauthenticated
    // peer — mid-handshake, or one that never got past `hello` — is closed in
    // SILENCE: a `token.revoked` would confirm that the token it was proving
    // against had been the real one, which is exactly what the reply-less
    // failed-proof close exists to deny.
    assert!(
        revoke_frames(false).is_empty(),
        "an unauthenticated socket must be told nothing at all"
    );

    // An authenticated session gets the revoke, then a clean close.
    let frames = revoke_frames(true);
    assert_eq!(frames.len(), 2, "the revoke frame, then the close");
    let Message::Text(text) = &frames[0] else {
        panic!("the first frame must be the token.revoked text frame");
    };
    let parsed: Value = serde_json::from_str(text.as_str()).unwrap();
    assert_eq!(parsed["type"], msg::TOKEN_REVOKED);
    assert!(
        matches!(frames[1], Message::Close(None)),
        "the socket is closed right behind the revoke"
    );
}

#[test]
fn token_revoked_frame_carries_no_token_material() {
    let dir = tempfile::tempdir().unwrap();
    let s = super::super::BridgeState::load(dir.path());
    let old = s.token();
    let new = s.regenerate_token();

    let frame = token_revoked_reply();
    let parsed: Value = serde_json::from_str(&frame).unwrap();

    assert_eq!(parsed["type"], msg::TOKEN_REVOKED);
    assert_eq!(parsed["reqId"], REVOKE_REQ_ID, "reqId stays non-empty");
    assert!(parsed["payload"].is_null(), "the frame carries no payload");
    // The whole point of the no-oracle rule: a revoked peer learns that its
    // pairing is dead and NOTHING about either secret.
    assert!(
        !frame.contains(&old),
        "the old token must never be on the wire"
    );
    assert!(!frame.contains(&new), "nor the new one");
}

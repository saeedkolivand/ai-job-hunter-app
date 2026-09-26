use super::*;
use crate::extension_bridge::{advance_frame, BridgeState, ConnState, FrameDecision};
// `advance_frame`/`ConnState`/`FrameDecision` are private to the parent
// `extension_bridge` module — visible here because privacy in Rust
// extends to every DESCENDANT module, not just direct children, so this
// test module (a grandchild) can reach them exactly as
// `extension_bridge::test` does one level up.

// ── pairing-failure classification (pure) ───────────────────────────────
// Hand-written expected buckets, not derived from `classify_pairing_failure`
// itself — mirrors the repo's standing lesson to pair a loop/derived check
// with a literal.

#[test]
fn all_ports_absent_is_app_not_running() {
    assert_eq!(
        classify_pairing_failure(&[PortOutcome::NoUpgrade, PortOutcome::NoUpgrade]),
        PairingFailure::AppNotRunning
    );
    assert_eq!(classify_pairing_failure(&[]), PairingFailure::AppNotRunning);
}

#[test]
fn every_reachable_port_rejecting_the_proof_is_pairing_rejected() {
    assert_eq!(
        classify_pairing_failure(&[PortOutcome::NoUpgrade, PortOutcome::ProofRejected]),
        PairingFailure::PairingRejected
    );
}

#[test]
fn any_pre_auth_error_is_connection_error_not_pairing_rejected() {
    // Issue #1084 PR1's own decision: "a crash between challenge and auth
    // is not a pairing failure" — even alongside a genuine proof
    // rejection on another port, the mixed case must NOT be reported as
    // a wrong token.
    assert_eq!(
        classify_pairing_failure(&[PortOutcome::PreAuthError, PortOutcome::ProofRejected]),
        PairingFailure::ConnectionError
    );
    assert_eq!(
        classify_pairing_failure(&[PortOutcome::PreAuthError]),
        PairingFailure::ConnectionError
    );
}

// ── handshake wire-shape round trip against the REAL server state
// machine (`super::advance_frame`) — no socket, no AppHandle needed:
// `advance_hello`/`advance_auth` are pure functions of `&BridgeState`.
// This is the proof the client's frame-building/parsing is wire-compatible
// with the committed server half, not just internally self-consistent. ──

#[test]
fn handshake_round_trips_against_the_real_server_state_machine() {
    let dir = tempfile::TempDir::new().unwrap();
    let state = BridgeState::load(dir.path());
    let token = state.token();

    let client_nonce = handshake::new_nonce();
    let hello = build_hello(&client_nonce);

    let decision = advance_frame(&state, &ConnState::AwaitingHello, &hello);
    let FrameDecision::Challenge { reply, next } = decision else {
        panic!("expected Challenge, got {decision:?}");
    };
    let challenge_json: Value = serde_json::from_str(&reply).unwrap();
    let server_nonce = parse_challenge(&challenge_json).expect("well-formed challenge");

    let proof = handshake::client_proof(&token, &server_nonce, &client_nonce);
    let auth = build_auth(&proof);

    let decision = advance_frame(&state, &next, &auth);
    let FrameDecision::AuthOk(reply) = decision else {
        panic!("expected AuthOk, got {decision:?}");
    };
    let auth_ok_json: Value = serde_json::from_str(&reply).unwrap();
    let server_proof = parse_auth_ok(&auth_ok_json).expect("well-formed auth.ok");

    assert!(
        handshake::verify_server_proof(&token, &server_nonce, &client_nonce, &server_proof),
        "the client's own verification must accept the real server's serverProof"
    );
}

#[test]
fn handshake_round_trip_rejects_a_wrong_token() {
    let dir = tempfile::TempDir::new().unwrap();
    let state = BridgeState::load(dir.path());

    let client_nonce = handshake::new_nonce();
    let hello = build_hello(&client_nonce);
    let decision = advance_frame(&state, &ConnState::AwaitingHello, &hello);
    let FrameDecision::Challenge { reply, next } = decision else {
        panic!("expected Challenge, got {decision:?}");
    };
    let server_nonce =
        parse_challenge(&serde_json::from_str(&reply).unwrap()).expect("well-formed challenge");

    // A wrong token — the CLI's persisted copy is stale.
    let wrong_proof = handshake::client_proof("not-the-real-token", &server_nonce, &client_nonce);
    let auth = build_auth(&wrong_proof);
    let decision = advance_frame(&state, &next, &auth);
    assert!(
        matches!(decision, FrameDecision::Unauthorized),
        "expected Unauthorized, got {decision:?}"
    );
}

// ── the SAME round trip, but over a REAL loopback socket, driving the
// production `attempt_port` fn (not a reimplementation) against a
// minimal server that itself calls the real `advance_frame` state
// machine — the strongest available proof that the client's transport
// code (WS upgrade, frame send/receive) interoperates with the actual
// server, not just that the JSON shapes match in-process. ──

#[tokio::test]
async fn attempt_port_authenticates_over_a_real_socket_against_the_real_server() {
    use tokio::net::TcpListener;

    let dir = tempfile::TempDir::new().unwrap();
    let state = BridgeState::load(dir.path());
    let token = state.token();

    // Kernel-assigned ephemeral port (never collides with a real running
    // app or another test) — same hermetic pattern as
    // `import_tests::claim_busy_port`.
    let listener = TcpListener::bind(("127.0.0.1", 0u16)).await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        // No Origin check here (that gate is `handle_connection`'s own,
        // covered by `auth`'s tests) — everything past the WS upgrade is
        // the real per-frame `advance_frame` dispatch `handle_connection`
        // itself runs.
        let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
        let mut conn = ConnState::AwaitingHello;
        loop {
            let msg = ws.next().await.unwrap().unwrap();
            let text = match msg {
                tokio_tungstenite::tungstenite::Message::Text(t) => t.to_string(),
                other => panic!("expected a text frame, got {other:?}"),
            };
            match advance_frame(&state, &conn, &text) {
                FrameDecision::Challenge { reply, next } => {
                    conn = next;
                    ws.send(tokio_tungstenite::tungstenite::Message::text(reply))
                        .await
                        .unwrap();
                }
                FrameDecision::AuthOk(reply) => {
                    ws.send(tokio_tungstenite::tungstenite::Message::text(reply))
                        .await
                        .unwrap();
                    break;
                }
                other => panic!("unexpected FrameDecision in test server: {other:?}"),
            }
        }
    });

    let result = attempt_port(port, &token).await;
    assert!(
        result.is_ok(),
        "attempt_port must authenticate against the real server over a real socket, got {:?}",
        result.err()
    );
    server.await.unwrap();
}

// ── `attempt_port`'s `connect`/WS-upgrade steps must be bounded (MAJOR
// fix — security review round 2): a local process that accepts a TCP
// connection on a candidate port and never completes the HTTP upgrade —
// including a wedged previous app instance whose listener is still
// bound but whose accept loop stopped running — used to park this fn,
// and so the whole invocation, forever. Drives the real production
// `attempt_port`, the same pattern as the real-socket test above, but
// against a server that accepts and then goes silent instead of ever
// speaking WebSocket. ──

#[tokio::test]
async fn attempt_port_gives_up_on_a_peer_that_accepts_and_never_completes_the_upgrade() {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0u16)).await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Accept the TCP connection (the kernel-level handshake a squatter
    // or a wedged app's still-bound listener completes for free) and
    // then go silent for the rest of this test: never send an HTTP
    // upgrade response, never close. This is exactly the "accepts on a
    // PORT_RANGE port and never completes the upgrade" scenario the fix
    // closes — before it, `client_async_with_config`'s read had no
    // deadline at all.
    let _server = tokio::spawn(async move {
        let (_tcp, _) = listener.accept().await.unwrap();
        std::future::pending::<()>().await
    });

    // Bounded well past 2× HANDSHAKE_STEP_TIMEOUT (connect can't
    // meaningfully stall against a listener that DID accept, so the
    // upgrade step's own timeout is what must fire here) so a
    // regression that restores the unbounded `.await` hangs this test
    // instead of the whole suite.
    let outcome = tokio::time::timeout(
        HANDSHAKE_STEP_TIMEOUT * 3,
        attempt_port(port, "irrelevant-token"),
    )
    .await;
    // `WsStream` doesn't implement `Debug`/`PartialEq` (it wraps a live
    // socket), so match the shape rather than `assert_eq!` the whole
    // `Result` or interpolate it into a panic message.
    match outcome {
        Ok(Err(PortOutcome::NoUpgrade)) => {}
        Ok(Ok(_)) => {
            panic!("attempt_port must not authenticate against a peer that never upgraded")
        }
        Ok(Err(other)) => panic!(
            "expected PortOutcome::NoUpgrade for a peer that never completes the upgrade, \
             got {other:?}"
        ),
        Err(_) => panic!(
            "attempt_port must give up once the upgrade step's own deadline elapses, not \
             hang forever on a peer that accepted but never completes the WS upgrade"
        ),
    }
}

// ── `next_json`'s deadline must cover the WHOLE call, not be re-armed
// per iteration — a peer that floods control frames (ping/pong) faster
// than the budget must not stall it past that budget. Drives the real
// production `next_json` over a real loopback socket against a minimal
// server, the same pattern as `attempt_port_authenticates_over_a_real_
// socket_against_the_real_server` above. ──

#[tokio::test]
async fn next_json_returns_at_its_deadline_even_when_flooded_with_pings() {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0u16)).await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Server: upgrade, then flood Ping frames faster than the client's
    // own per-call budget below — for as long as this task keeps
    // running (it is dropped, not joined, at the end of this test), so
    // a regression (a timeout re-armed on every iteration) would hang
    // past the outer bound below instead of merely returning late.
    let _server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
        loop {
            if ws.send(Message::Ping(Vec::new().into())).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    let tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let uri = format!("ws://127.0.0.1:{port}/").parse().unwrap();
    let (mut ws, _resp) = tokio_tungstenite::client_async(ClientRequestBuilder::new(uri), tcp)
        .await
        .unwrap();

    // The server's 10ms ping cadence is far faster than this budget, so
    // a correctly-fixed `next_json` still returns `None` right at the
    // deadline; a per-iteration-re-armed `timeout` (the bug) never
    // would, since every ping resets its clock — bound the assertion in
    // an outer timeout well past the budget so a regression fails this
    // test instead of hanging the whole suite.
    let budget = Duration::from_millis(150);
    let outcome = tokio::time::timeout(budget * 4, next_json(&mut ws, budget)).await;
    assert_eq!(
        outcome.ok(),
        Some(None),
        "next_json must return None at its own deadline, not hang past it, when flooded \
         with pings faster than that deadline"
    );
}

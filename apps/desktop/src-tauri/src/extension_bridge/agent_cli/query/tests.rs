use super::*;

// ── `send_agent_query_within` (finding #7 fix — security review):
// distinguish a genuine timeout from an early transport failure, and
// fail fast on a same-`reqId` reply of the wrong type instead of waiting
// out the whole budget. All three drive the REAL production fn over a
// real loopback socket, the same pattern as `attempt_port_authenticates_
// over_a_real_socket_against_the_real_server`. ──

async fn connect_plain(port: u16) -> WsStream {
    let tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let uri = format!("ws://127.0.0.1:{port}/").parse().unwrap();
    tokio_tungstenite::client_async(ClientRequestBuilder::new(uri), tcp)
        .await
        .unwrap()
        .0
}

#[tokio::test]
async fn send_agent_query_within_reports_a_genuine_timeout_when_nothing_ever_replies() {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0u16)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let _server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
        // Read and discard the query, then go silent for the rest of
        // this test — never replies, never closes.
        let _ = ws.next().await;
        std::future::pending::<()>().await
    });

    let ws = connect_plain(port).await;
    let budget = Duration::from_millis(150);
    // Bounded well past `budget` so a regression that hangs past the
    // deadline fails this test instead of the whole suite.
    let outcome = tokio::time::timeout(
        budget * 4,
        send_agent_query_within(ws, &Verb::Schema, budget),
    )
    .await;
    assert_eq!(
        outcome.ok(),
        Some(Err(ERR_TIMEOUT)),
        "a call that genuinely exhausts its budget must report `timeout`, not `connection_lost`"
    );
}

#[tokio::test]
async fn send_agent_query_within_reports_connection_lost_fast_on_an_early_close() {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0u16)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let _server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
        let _ = ws.next().await; // read the query
                                 // Close immediately — well before any realistic budget.
    });

    let ws = connect_plain(port).await;
    // A generous budget — the point is proving this returns FAST, not
    // by waiting it out.
    let generous_budget = Duration::from_secs(5);
    let outcome = tokio::time::timeout(
        Duration::from_millis(500),
        send_agent_query_within(ws, &Verb::Schema, generous_budget),
    )
    .await;
    assert_eq!(
        outcome.ok(),
        Some(Err(ERR_CONNECTION_LOST)),
        "a close well before the deadline must never be reported as `timeout`"
    );
}

#[tokio::test]
async fn send_agent_query_within_fails_fast_on_a_same_req_id_wrong_type_reply() {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0u16)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let _server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
        let msg = ws.next().await.unwrap().unwrap();
        let text = match msg {
            Message::Text(t) => t.to_string(),
            other => panic!("expected a text frame, got {other:?}"),
        };
        let sent: Value = serde_json::from_str(&text).unwrap();
        let req_id = sent["reqId"].as_str().unwrap().to_string();
        // Mirrors `advance_authenticated`'s "unknown message type"
        // fallback — a real (old) app that doesn't understand
        // `agent.query` replies exactly this way, echoing OUR reqId on
        // an `import.result` envelope.
        let reply = json!({
            "type": "import.result",
            "reqId": req_id,
            "payload": { "error": "unknown message type 'agent.query'" },
        })
        .to_string();
        let _ = ws.send(Message::text(reply)).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    });

    let ws = connect_plain(port).await;
    let generous_budget = Duration::from_secs(5);
    let outcome = tokio::time::timeout(
        Duration::from_millis(500),
        send_agent_query_within(ws, &Verb::Schema, generous_budget),
    )
    .await;
    assert_eq!(
        outcome.ok(),
        Some(Err(ERR_UNSUPPORTED_BY_APP)),
        "a same-reqId reply of the wrong type must fail fast as `unsupported_by_app`, \
         not silently `continue` toward a 30s timeout"
    );
}

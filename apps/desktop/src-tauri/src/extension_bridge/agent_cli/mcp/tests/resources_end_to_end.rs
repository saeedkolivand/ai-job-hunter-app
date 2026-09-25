use super::*;
#[test]
fn resources_read_unknown_uri_answers_a_json_rpc_error_end_to_end() {
    let input = line(json!({
        "jsonrpc": "2.0", "id": 1, "method": "resources/read",
        "params": { "uri": "ajh://nope" },
    }));
    let text = run_serve(&input, stub_ok);
    let reply: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(reply["error"]["code"], -32002);
    assert_eq!(reply["error"]["message"], "Resource not found");
}

/// A `resources/read` shares [`MCP_CALL_QUEUE_MAX`]'s queue and single worker with `tools/call`
/// (issue #1146 P4), so a full queue must refuse it too — in the RESOURCE shape (`contents`),
/// never the tool-shaped `content` a `tools/call` refusal carries.
#[test]
fn a_full_dispatch_queue_refuses_a_resource_read_in_the_resource_shape() {
    let total = MCP_CALL_QUEUE_MAX + 2;
    let input: String = (1..=total)
        .map(|id| {
            line(json!({
                "jsonrpc": "2.0", "id": id, "method": "resources/read",
                "params": { "uri": resources::URI_PROFILE },
            }))
        })
        .collect();

    let buffer = Arc::new(Mutex::new(Vec::new()));
    let (seen_busy, busy_written) = std::sync::mpsc::channel::<()>();
    let writer = SignallingWriter {
        buffer: Arc::clone(&buffer),
        needle: "server_busy",
        signal: Some(seen_busy),
    };
    let (release, blocked) = std::sync::mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        serve_with(&input, writer, move |_: &Verb| {
            let _ = blocked.recv_timeout(SIGNAL_BUDGET);
            Ok(json!({ "ok": true, "resource": "profile", "data": {} }))
        })
    });

    busy_written
        .recv_timeout(SIGNAL_BUDGET)
        .expect("a server_busy refusal must be written while the dispatcher is blocked");
    drop(release);
    let code = server.join().expect("serve must not panic");
    assert_eq!(code, 0);

    let text = String::from_utf8(lock(&buffer).clone()).expect("valid utf8");
    let busy_frame = parsed_frames(&text)
        .into_iter()
        .find(|f| {
            f["result"]["contents"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("server_busy")
        })
        .expect("at least one resource-shaped busy refusal");
    assert!(
        busy_frame["result"]["content"].is_null(),
        "a resource's busy refusal must use `contents`, never the tool-shaped `content`: {busy_frame}"
    );
}

/// The EOF-drain mirror of the busy test above: a `resources/read` still QUEUED (never started)
/// when the drain deadline expires must get a `shutting_down` refusal in the resource shape too.
#[test]
fn an_expired_drain_deadline_answers_a_queued_resource_read_in_the_resource_shape() {
    let input = format!(
        "{}{}",
        line(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "profile", "arguments": {} },
        })),
        line(json!({
            "jsonrpc": "2.0", "id": 2, "method": "resources/read",
            "params": { "uri": resources::URI_PROFILE },
        })),
    );
    let mut output = Vec::new();
    let code = serve_with_drain_budget(
        &input,
        &mut output,
        move |_: &Verb| {
            std::thread::sleep(DISPATCH_HOLD);
            Ok(json!({ "ok": true, "resource": "profile", "data": {} }))
        },
        DRAIN_BUDGET,
    );
    assert_eq!(code, 0);
    let text = String::from_utf8(output).expect("valid utf8");
    let frames = parsed_frames(&text);
    assert_eq!(reply_ids(&text), vec![1, 2]);
    let queued_reply = frame_with_id(&frames, 2);
    assert!(
        queued_reply["result"]["content"].is_null(),
        "the queued RESOURCE read's refusal must never use the tool shape: {queued_reply}"
    );
    let payload: Value = serde_json::from_str(
        queued_reply["result"]["contents"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(payload["error"], "shutting_down");
    assert_eq!(
        payload["dispatched"], false,
        "the queued resource read provably never reached the app"
    );
}

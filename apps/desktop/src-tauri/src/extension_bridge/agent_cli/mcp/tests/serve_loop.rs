use super::*;
// ── serve — the pure loop over a Cursor (GRAFT: mutation-visible) ─────────

#[test]
fn serve_emits_exactly_one_line_per_request_and_none_for_notifications() {
    let input = format!(
        "{}{}{}",
        line(json!({"jsonrpc":"2.0","id":1,"method":"ping"})),
        line(json!({"jsonrpc":"2.0","method":"notifications/initialized"})),
        line(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})),
    );
    let text = run_serve(&input, stub_ok);
    assert_eq!(
        text.lines().count(),
        2,
        "the notification must produce no output line: {text:?}"
    );
}

/// The `[call, ping, call]` input both concurrency tests below drive.
fn sandwiched_ping_input() -> String {
    format!(
        "{}{}{}",
        line(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "job", "arguments": { "url": "https://example.com/first" } },
        })),
        line(json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" })),
        line(json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "job", "arguments": { "url": "https://example.com/second" } },
        })),
    )
}

#[test]
fn a_ping_is_answered_while_the_first_tools_call_is_still_in_flight() {
    // ADR-040 §12's follow-up, pinned: the first call's dispatch BLOCKS on the worker thread
    // until the main thread has written the sandwiched ping's reply, so the emitted ids must be
    // [2, 1, 3] — id 2 answered mid-call, ahead of the earlier request it overtook. Under the old
    // read→handle→write loop this deadlocks (the ping can't be written until the call it queues
    // behind returns), which is exactly the property under test.
    let (ping_seen, wait_for_ping) = std::sync::mpsc::channel::<()>();
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let output = SignallingWriter {
        buffer: Arc::clone(&buffer),
        // The ping's own reply frame; no other frame in this input carries it.
        needle: "\"id\":2",
        signal: Some(ping_seen),
    };

    let dispatch_order = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&dispatch_order);
    let code = serve_with(&sandwiched_ping_input(), output, move |verb: &Verb| {
        if let Verb::Job { url } = verb {
            let first = {
                let mut seen = lock(&recorded);
                seen.push(url.clone());
                seen.len() == 1
            };
            if first {
                // Hold the worker inside the FIRST dispatch until the ping has been written.
                let _ = wait_for_ping.recv_timeout(SIGNAL_BUDGET);
            }
        }
        Ok(json!({ "ok": true, "resource": "job", "data": {} }))
    });
    assert_eq!(code, 0);

    let text = String::from_utf8(lock(&buffer).clone()).expect("valid utf8");
    assert_eq!(
        reply_ids(&text),
        vec![2, 1, 3],
        "the ping (id 2) must be answered while the first call (id 1) is still in flight, and \
         the second call (id 3) only after it: {text:?}"
    );
    assert_eq!(
        *lock(&dispatch_order),
        vec!["https://example.com/first", "https://example.com/second"],
        "the two tools/call frames must still dispatch in INPUT order — the sandwiched ping \
         never dispatches at all"
    );
}

#[test]
fn a_local_tools_call_is_answered_while_a_bridge_call_is_still_in_flight() {
    // The follow-up to the ping guarantee: `commands` is LOCAL (it reads this binary's own POLICY
    // copy and never opens a bridge connection), so it must not wait behind a bridge-backed call
    // either. The stub blocks the `job` dispatch until the `commands` reply has been written, so
    // the ids must come back [2, 1] — and the stub must be entered exactly ONCE, since a local
    // tool never reaches the worker at all.
    let (local_seen, wait_for_local) = std::sync::mpsc::channel::<()>();
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let output = SignallingWriter {
        buffer: Arc::clone(&buffer),
        // The `commands` reply's own frame; the blocked `job` call carries id 1.
        needle: "\"id\":2",
        signal: Some(local_seen),
    };
    let input = format!(
        "{}{}",
        line(json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "job", "arguments": { "url": "https://example.com/slow" } },
        })),
        line(json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "commands", "arguments": { "effect": "read" } },
        })),
    );

    let dispatches = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&dispatches);
    let code = serve_with(&input, output, move |_: &Verb| {
        counted.fetch_add(1, Ordering::SeqCst);
        let _ = wait_for_local.recv_timeout(SIGNAL_BUDGET);
        Ok(json!({ "ok": true, "resource": "job", "data": {} }))
    });
    assert_eq!(code, 0);

    let text = String::from_utf8(lock(&buffer).clone()).expect("valid utf8");
    assert_eq!(
        reply_ids(&text),
        vec![2, 1],
        "the local `commands` call (id 2) must be answered while the bridge call (id 1) is \
         still in flight: {text:?}"
    );
    assert_eq!(
        dispatches.load(Ordering::SeqCst),
        1,
        "only the bridge-backed `job` call may reach the dispatcher — `commands` is answered \
         without ever touching the wire"
    );
}

#[test]
fn a_locally_refused_tools_call_is_answered_without_reaching_the_dispatcher() {
    // The other local class: `local_call_refusal` (here a wrong_tool refusal for a real
    // Reversible row named on call-read). It is decided from the bundled POLICY copy, so it must
    // never be queued behind a dispatch either — and must never BE one.
    let input = line(json!({
        "jsonrpc": "2.0", "id": 5, "method": "tools/call",
        "params": {
            "name": "call-read",
            "arguments": { "namespace": "cli_agents", "command": "cli_agents_redetect" },
        },
    }));
    let dispatched = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&dispatched);
    let text = run_serve(&input, move |_: &Verb| {
        flag.store(true, Ordering::SeqCst);
        Ok(json!({ "ok": true }))
    });

    let reply: Value = serde_json::from_str(text.trim()).expect("one reply frame");
    let payload: Value =
        serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap())
            .expect("content[0] is the refusal payload");
    assert_eq!(payload["error"], "wrong_tool");
    assert!(
        !dispatched.load(Ordering::SeqCst),
        "a local refusal must be answered without a bridge dispatch"
    );
}

#[test]
fn two_tools_calls_never_dispatch_concurrently() {
    // The other half of the guarantee: answering a ping mid-call must not have made DISPATCH
    // concurrent. The stub records entry/exit and the loop's peak occupancy must stay 1, so a
    // second bridge connection can never be open while the first is (the ADR-040 §12 throttle
    // bound). Mutation-visible: spawning per call instead of queueing onto one worker pushes the
    // peak to 2.
    let in_flight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let dispatched = Arc::new(AtomicUsize::new(0));
    let (entered, peaked, counted) = (
        Arc::clone(&in_flight),
        Arc::clone(&peak),
        Arc::clone(&dispatched),
    );

    let text = run_serve(&sandwiched_ping_input(), move |_: &Verb| {
        let now = entered.fetch_add(1, Ordering::SeqCst) + 1;
        peaked.fetch_max(now, Ordering::SeqCst);
        counted.fetch_add(1, Ordering::SeqCst);
        // A real dispatch is not instantaneous; give an overlapping one room to be observed.
        std::thread::sleep(Duration::from_millis(20));
        entered.fetch_sub(1, Ordering::SeqCst);
        Ok(json!({ "ok": true, "resource": "job", "data": {} }))
    });

    assert_eq!(
        dispatched.load(Ordering::SeqCst),
        2,
        "both tools/call frames must dispatch: {text:?}"
    );
    assert_eq!(
        peak.load(Ordering::SeqCst),
        1,
        "at most ONE dispatch may ever be in flight — see the module doc's single-flight guarantee"
    );
    assert_eq!(
        in_flight.load(Ordering::SeqCst),
        0,
        "every dispatch must have completed before serve returned"
    );
}

#[test]
fn a_write_failure_ends_serve_with_exit_zero_and_dispatches_nothing_further() {
    // EPIPE once the client closes its end of the pipe: `emit`'s `Err` is the cue to STOP, never
    // a retry and never a panic (release is `panic = "abort"`, where a panic is a silent death).
    // The second assertion is what makes this mutation-visible: dropping the early return leaves
    // the exit code 0 either way, but the `tools/call` behind the failed ping would then still
    // reach the bridge, on a connection whose reply can no longer be delivered.
    let input = format!(
        "{}{}",
        line(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" })),
        line(json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "profile", "arguments": {} },
        })),
    );
    let dispatched = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&dispatched);
    let code = serve_with(&input, BrokenWriter, move |_: &Verb| {
        flag.store(true, Ordering::SeqCst);
        Ok(json!({ "ok": true }))
    });
    assert_eq!(code, 0, "a dead pipe is a clean exit, never a non-zero one");
    assert!(
        !dispatched.load(Ordering::SeqCst),
        "the early return on a failed write must stop the loop from ROUTING the frames behind \
         it: the tools/call after the failed ping is never classified, so it never reaches the \
         dispatcher"
    );
}

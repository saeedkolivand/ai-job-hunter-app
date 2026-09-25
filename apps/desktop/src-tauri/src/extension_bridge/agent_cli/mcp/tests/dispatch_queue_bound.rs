use super::*;
// ── The bounded dispatch queue (item 6) ──────────────────────────────────

/// The queue between the writer thread and the single-flight dispatcher is BOUNDED, and a full
/// queue is answered rather than waited on: the excess `tools/call` comes back as a `server_busy`
/// tool result WHILE the first dispatch is still blocked. Mutation-visible twice over — an
/// unbounded `channel()` produces zero refusals, and a blocking `send` on a full one writes
/// nothing at all until the dispatch below is released, so the signal never pulses and this test
/// fails on the timeout instead of the assertion.
#[test]
fn a_full_dispatch_queue_is_refused_with_server_busy_while_a_call_is_in_flight() {
    let total = MCP_CALL_QUEUE_MAX + 4;
    let input: String = (1..=total)
        .map(|id| {
            line(json!({
                "jsonrpc": "2.0", "id": id, "method": "tools/call",
                "params": { "name": "profile", "arguments": {} },
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
    // Every dispatch parks here until the test drops the sender, so the worker is provably still
    // holding the first call when the refusals are written.
    let (release, blocked) = std::sync::mpsc::channel::<()>();
    let dispatched = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&dispatched);

    let server = std::thread::spawn(move || {
        serve_with(&input, writer, move |_: &Verb| {
            counted.fetch_add(1, Ordering::SeqCst);
            let _ = blocked.recv_timeout(SIGNAL_BUDGET);
            Ok(json!({ "ok": true, "resource": "profile", "data": {} }))
        })
    });

    busy_written
        .recv_timeout(SIGNAL_BUDGET)
        .expect("a server_busy refusal must be written while the dispatcher is blocked");
    // Read BEFORE releasing: once the blocker is gone the worker drains the queue and frees
    // slots, so how the remaining frames split between accepted and refused stops being a
    // property of the bound and starts being a race.
    let running_at_refusal = dispatched.load(Ordering::SeqCst);
    drop(release);
    let code = server.join().expect("serve must not panic");
    assert_eq!(code, 0);

    let text = String::from_utf8(lock(&buffer).clone()).expect("valid utf8");
    let replies: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).expect("each line is one reply"))
        .collect();
    assert_eq!(
        replies.len(),
        total,
        "every frame must be answered exactly once, refused or dispatched: {text:?}"
    );
    let busy: Vec<&Value> = replies
        .iter()
        .filter(|r| {
            r["result"]["content"][0]["text"]
                .as_str()
                .is_some_and(|t| t.contains("server_busy"))
        })
        .collect();
    assert!(
        !busy.is_empty(),
        "the queue is bounded at {MCP_CALL_QUEUE_MAX}, so {total} pipelined calls must produce \
         at least one refusal: {text:?}"
    );
    for refusal in &busy {
        assert_eq!(refusal["result"]["isError"], true);
        assert_eq!(refusal["result"]["content"][1]["text"], "exitCode: 2");
        let payload: Value =
            serde_json::from_str(refusal["result"]["content"][0]["text"].as_str().unwrap())
                .expect("content[0] is the refusal payload");
        assert_eq!(payload["error"], "server_busy");
        assert_eq!(payload["dispatched"], false);
    }
    assert_eq!(
        dispatched.load(Ordering::SeqCst),
        total - busy.len(),
        "a refused call must never also reach the bridge"
    );
    // `<= 1`, not `== 1`: at most one call may be RUNNING when the refusal is written, so the
    // rest were WAITING in a full queue rather than being dispatched. Zero is legitimate and was
    // a real flake — 2 of 6 local runs, on this assertion, before this branch changed anything:
    // the writer can fill a {MCP_CALL_QUEUE_MAX}-deep queue and refuse the next frame in
    // microseconds, and the dispatch thread is not guaranteed to have been SCHEDULED by then. A
    // full queue is precisely the state that does not require it to have run. The upper bound is
    // what carries the meaning and is the mutation-visible half: dispatch per call instead of
    // onto one worker and this reads well above 1 (if `busy_written` above even fires at all).
    assert!(
        running_at_refusal <= 1,
        "at most ONE call may have been running when the refusal was written, so the other \
         {MCP_CALL_QUEUE_MAX} were waiting in a full queue rather than being dispatched; \
         {running_at_refusal} were running"
    );
}

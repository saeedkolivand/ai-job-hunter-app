use super::*;
// ── The bounded EOF drain (item 7) ───────────────────────────────────────

/// After `Eof` the drain has ONE absolute deadline, not one `INVOCATION_TIMEOUT` per queued call:
/// with a blocking dispatcher and a short injected budget, `serve` must return 0 long before the
/// in-flight call finishes, and the calls still queued behind it must never dispatch at all.
/// Mutation-visible: remove the deadline and this waits out the sleep below; keep the deadline
/// but drop the worker's abandoned-flag check and the queued second call still dispatches.
///
/// And every call the client is still waiting on is ANSWERED before the exit — the half of the
/// guarantee that used to be silence. Mutation-visible on its own: delete the `shutting_down`
/// sweep and this writes nothing at all, which is what it asserted before the fix.
#[test]
fn an_expired_drain_deadline_answers_what_it_abandons_and_dispatches_nothing_further() {
    let input: String = (1..=2)
        .map(|id| {
            line(json!({
                "jsonrpc": "2.0", "id": id, "method": "tools/call",
                "params": { "name": "profile", "arguments": {} },
            }))
        })
        .collect();

    // The stub reports each entry and each exit, so "the second call never dispatched" is a
    // message that never arrives rather than a fixed sleep.
    let (report, progress) = std::sync::mpsc::channel::<&'static str>();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let mut output = Vec::new();

    let started = std::time::Instant::now();
    let code = serve_with_drain_budget(
        &input,
        &mut output,
        move |_: &Verb| {
            let first = counted.fetch_add(1, Ordering::SeqCst) == 0;
            let _ = report.send(if first { "enter-1" } else { "enter-2" });
            std::thread::sleep(DISPATCH_HOLD);
            let _ = report.send(if first { "exit-1" } else { "exit-2" });
            Ok(json!({ "ok": true, "resource": "profile", "data": {} }))
        },
        DRAIN_BUDGET,
    );

    assert_eq!(code, 0, "an expired drain is still a clean exit");
    assert!(
        started.elapsed() < DRAIN_EXIT_MAX,
        "serve must return on its own deadline, not wait out the in-flight dispatch \
         (took {:?})",
        started.elapsed()
    );
    // Both calls are answered — the abandoned one and the never-started one — and the two
    // answers differ in the one fact the client needs to decide whether repeating is safe.
    let text = String::from_utf8(output).expect("valid utf8");
    let replies: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).expect("each line is one reply"))
        .collect();
    assert_eq!(
        reply_ids(&text),
        vec![1, 2],
        "every call the client was still waiting on must be answered, in queue order: {text:?}"
    );
    let payload = |r: &Value| -> Value {
        serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap())
            .expect("content[0] is the refusal payload")
    };
    for reply in &replies {
        assert_eq!(reply["result"]["isError"], true);
        assert_eq!(reply["result"]["content"][1]["text"], "exitCode: 2");
        assert_eq!(payload(reply)["error"], "shutting_down");
    }
    assert_eq!(
        payload(&replies[0])["dispatched"],
        true,
        "the in-flight call reached the app and may have taken effect — saying otherwise would \
         invite a client to repeat a write that already landed"
    );
    assert_eq!(
        payload(&replies[1])["dispatched"],
        false,
        "the queued call provably never reached the app: {text:?}"
    );

    assert_eq!(
        progress.recv_timeout(SIGNAL_BUDGET).ok(),
        Some("enter-1"),
        "the first call must have started before the deadline expired"
    );
    assert_eq!(
        progress.recv_timeout(SIGNAL_BUDGET).ok(),
        Some("exit-1"),
        "the abandoned in-flight call still runs to completion on its own thread"
    );
    // The queued second call must never start. A violating build reaches it the INSTANT the
    // first returns — i.e. immediately after the `exit-1` just received — so this grace only has
    // to outlast a thread wake-up, and stays far below `DISPATCH_HOLD` so a correct build never
    // waits it out for nothing.
    assert!(
        progress.recv_timeout(DRAIN_EXIT_MAX).is_err(),
        "a call still queued when the drain deadline expired must never dispatch"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn eof_still_writes_the_reply_of_a_call_that_was_already_in_flight() {
    // The drain guarantee: stdin closes immediately after a single `tools/call` line, so the
    // `Eof` event reaches the loop while the worker is still inside the dispatch. The reply must
    // still be written before `serve` returns, never dropped on the floor.
    let input = line(json!({
        "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": { "name": "profile", "arguments": {} },
    }));
    let text = run_serve(&input, |_: &Verb| {
        std::thread::sleep(Duration::from_millis(100));
        Ok(json!({ "ok": true, "resource": "profile", "data": {} }))
    });
    assert_eq!(
        reply_ids(&text),
        vec![7],
        "the in-flight call's reply must survive EOF: {text:?}"
    );
}

#[test]
fn an_explicit_id_null_produces_no_output_and_no_dispatch() {
    let input = line(json!({
        "jsonrpc": "2.0", "id": null, "method": "tools/call",
        "params": { "name": "profile", "arguments": {} },
    }));
    let dispatched = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&dispatched);
    let text = run_serve(&input, move |_: &Verb| {
        flag.store(true, Ordering::SeqCst);
        Ok(json!({ "ok": true }))
    });
    assert!(
        text.is_empty(),
        "id:null must produce zero output: {text:?}"
    );
    assert!(
        !dispatched.load(Ordering::SeqCst),
        "id:null must never reach the worker — nothing is listening for the result"
    );
}

#[test]
fn a_missing_id_member_is_treated_as_a_notification() {
    let input = line(json!({"jsonrpc":"2.0","method":"ping"}));
    let text = run_serve(&input, stub_ok);
    assert!(text.is_empty());
}

#[test]
fn server_discover_is_plain_method_not_found() {
    let input = line(json!({"jsonrpc":"2.0","id":1,"method":"server/discover"}));
    let text = run_serve(&input, stub_ok);
    let reply: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(reply["error"]["code"], -32601);
}

#[test]
fn unparseable_json_is_a_parse_error() {
    let text = run_serve("not json at all\n", stub_ok);
    let reply: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(reply["error"]["code"], -32700);
}

#[test]
fn a_fenced_payload_containing_newlines_still_emits_exactly_one_line() {
    let fenced = crate::prompt_fence::fenced(
        "job_posting",
        "line one\nline two\nthree",
        crate::prompt_fence::JOB_CAP,
    );
    let input = line(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "job", "arguments": { "url": "https://example.com/1" } },
    }));
    let text = run_serve(&input, move |_: &Verb| {
        Ok(json!({ "ok": true, "resource": "job", "data": { "description": fenced.clone() } }))
    });
    assert_eq!(
        text.matches('\n').count(),
        1,
        "a fenced value's embedded newlines must JSON-escape, never split the line: {text:?}"
    );
}

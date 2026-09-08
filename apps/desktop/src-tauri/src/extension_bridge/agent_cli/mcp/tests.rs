use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::instructions::{EXPLAINED_IN_PROSE, INSTRUCTIONS};
use super::*;

/// How long a test waits for a signal that a correct [`serve`] always sends — long enough that a
/// loaded CI machine never trips it, short enough that a real deadlock fails the run rather than
/// hanging it.
const SIGNAL_BUDGET: Duration = Duration::from_secs(10);

fn line(v: Value) -> String {
    format!("{v}\n")
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// The two halves of one `tools/call`, composed. Production never needs this — [`serve`] runs
/// [`classify_tool_call`] on its writer thread and [`dispatched_tool_result`] on the worker,
/// which is the whole point of the split — so the composition lives here rather than as a
/// never-called fn in `mcp.rs`. The real loop's own composition is covered by the `serve` tests
/// below, not by this helper.
fn tool_call_result(
    params: &Value,
    server: &Server,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
) -> Result<Value, (i64, &'static str)> {
    match classify_tool_call(params, server) {
        ToolCall::Local(outcome) => outcome,
        ToolCall::Bridge(verb) => Ok(dispatched_tool_result(&verb, dispatch)),
    }
}

fn stub_ok(_verb: &Verb) -> Result<Value, &'static str> {
    Ok(json!({ "ok": true, "resource": "stub", "data": {} }))
}

/// A poisoned `Mutex` in a test is a panic in ANOTHER test thread; surface it as a failure here
/// rather than propagating an `unwrap` chain through every assertion.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Drive [`serve`] over an in-memory [`Cursor`] with a stub dispatcher — no runtime, no socket, no
/// live app. Always the most permissive launch mode (both flags) unless a test needs otherwise.
/// The dispatch stub now runs on `serve`'s own worker thread, so it must be `Send + 'static`:
/// state a test wants to observe travels through an `Arc` (or a channel), never a borrow.
fn serve_with(
    input: &str,
    output: impl Write,
    dispatch: impl FnMut(&Verb) -> Result<Value, &'static str> + Send + 'static,
) -> i32 {
    serve_with_drain_budget(input, output, dispatch, INVOCATION_TIMEOUT)
}

/// [`serve_with`] with the EOF drain deadline injected — production's own
/// [`INVOCATION_TIMEOUT`] everywhere except the two tests that measure the deadline itself, which
/// would otherwise have to wait it out.
fn serve_with_drain_budget(
    input: &str,
    output: impl Write,
    dispatch: impl FnMut(&Verb) -> Result<Value, &'static str> + Send + 'static,
    drain_budget: Duration,
) -> i32 {
    let server = Server::new(true, true);
    serve(
        Cursor::new(input.to_string()),
        output,
        &server,
        dispatch,
        drain_budget,
    )
}

/// [`serve_with`] into a plain buffer — returns the raw stdout bytes as a `String` so a test can
/// assert exact line counts / content.
fn run_serve(
    input: &str,
    dispatch: impl FnMut(&Verb) -> Result<Value, &'static str> + Send + 'static,
) -> String {
    let mut output = Vec::new();
    let code = serve_with(input, &mut output, dispatch);
    assert_eq!(code, 0);
    String::from_utf8(output).expect("valid utf8")
}

/// Every `id` [`serve`] wrote, in write order.
fn reply_ids(text: &str) -> Vec<i64> {
    text.lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).expect("each line is one JSON-RPC reply")["id"]
                .as_i64()
                .expect("every reply here carries a numeric id")
        })
        .collect()
}

/// [`serve`]'s single writer, instrumented: mirrors every byte into a shared buffer and, the
/// first time that buffer contains `needle`, pulses `signal` exactly once. Lets a dispatch stub
/// running on the WORKER thread block until a frame the MAIN thread emitted has really been
/// written — the only way to observe "answered mid-call" from outside.
struct SignallingWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
    needle: &'static str,
    signal: Option<std::sync::mpsc::Sender<()>>,
}

impl Write for SignallingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let seen = {
            let mut sink = lock(&self.buffer);
            sink.extend_from_slice(buf);
            String::from_utf8_lossy(&sink).contains(self.needle)
        };
        if seen {
            if let Some(tx) = self.signal.take() {
                let _ = tx.send(());
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A writer whose every write fails — the EPIPE a client that closed its pipe produces.
struct BrokenWriter;

impl Write for BrokenWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("client closed the pipe"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("client closed the pipe"))
    }
}

/// A writer that PARKS inside its FIRST `write` — pulsing `parked` first — until the test drops
/// its release sender, then accepts everything. The client that stopped draining stdout, held
/// still on purpose: while it is parked the whole loop is stuck inside [`emit`], which is the
/// only state in which the reader thread can be observed running ahead of the writer.
struct ParkingWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
    parked: Option<std::sync::mpsc::Sender<()>>,
    release: std::sync::mpsc::Receiver<()>,
}

impl Write for ParkingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(tx) = self.parked.take() {
            let _ = tx.send(());
            // Returns as soon as the test drops the sender (Disconnected); the budget is only
            // there so a broken test fails instead of hanging the run.
            let _ = self.release.recv_timeout(SIGNAL_BUDGET);
        }
        lock(&self.buffer).extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A [`BufRead`] that hands out ONE line per `fill_buf` and counts every line it has handed over.
/// A [`Cursor`] cannot answer the question the bound is about — "how far did the reader get before
/// it stopped?" — because it is consumed in whatever chunks the reader asks for; this counts the
/// lines the reader thread actually pulled, so a reader parked on a full queue and a reader that
/// swallowed the entire input are two different numbers.
struct PacedInput {
    lines: std::vec::IntoIter<String>,
    current: Vec<u8>,
    pos: usize,
    produced: Arc<AtomicUsize>,
}

impl std::io::Read for PacedInput {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let taken = {
            let available = self.fill_buf()?;
            let n = available.len().min(buf.len());
            buf[..n].copy_from_slice(&available[..n]);
            n
        };
        self.consume(taken);
        Ok(taken)
    }
}

impl std::io::BufRead for PacedInput {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.pos == self.current.len() {
            self.current = self.lines.next().unwrap_or_default().into_bytes();
            self.pos = 0;
            if !self.current.is_empty() {
                // Counted on HAND-OVER, so the count is "lines the reader has begun reading",
                // never "lines the test wrote".
                self.produced.fetch_add(1, Ordering::SeqCst);
            }
        }
        Ok(&self.current[self.pos..])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = (self.pos + amt).min(self.current.len());
    }
}

fn names(list: &[Value]) -> Vec<&str> {
    let mut n: Vec<&str> = list.iter().map(|t| t["name"].as_str().unwrap()).collect();
    n.sort_unstable();
    n
}

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

// ── The bounded reader → writer event queue ──────────────────────────────

/// The OTHER half of the backpressure the reader split lost. Bounding the DISPATCH queue only
/// stopped `tools/call` frames piling up; a client that stops draining stdout parks the loop
/// inside `emit`, and with an unbounded `Event` channel the reader would go on turning a
/// never-blocking stdin (a file, or a pipelining client) into `Event::Line`s without limit.
///
/// Measured as the reader's own progress, which is the only place the difference shows: with the
/// writer held still, the reader may hand over exactly [`MCP_EVENT_QUEUE_MAX`] queued lines, plus
/// the one the loop already took out of the queue, plus the one it is parked in `send` holding —
/// and then it must STOP. Mutation-visible and not by a hair: swap the `sync_channel` back for a
/// `channel()` and the reader swallows the whole input in the same microseconds, so the exact
/// count below is out by a factor of four rather than by one frame.
#[test]
fn a_parked_writer_stops_the_reader_at_the_event_queue_bound() {
    // Four times the bound, so "stopped at the bound" and "read the whole input" are nowhere
    // near each other.
    let total = MCP_EVENT_QUEUE_MAX * 4 + 16;
    let lines: Vec<String> = (1..=total)
        .map(|id| line(json!({ "jsonrpc": "2.0", "id": id, "method": "ping" })))
        .collect();
    let produced = Arc::new(AtomicUsize::new(0));
    let input = PacedInput {
        lines: lines.into_iter(),
        current: Vec::new(),
        pos: 0,
        produced: Arc::clone(&produced),
    };

    let buffer = Arc::new(Mutex::new(Vec::new()));
    let (parked_tx, writer_parked) = std::sync::mpsc::channel::<()>();
    let (release, blocked) = std::sync::mpsc::channel::<()>();
    let writer = ParkingWriter {
        buffer: Arc::clone(&buffer),
        parked: Some(parked_tx),
        release: blocked,
    };

    let server = std::thread::spawn(move || {
        let server = Server::new(true, true);
        serve(input, writer, &server, stub_ok, INVOCATION_TIMEOUT)
    });

    writer_parked
        .recv_timeout(SIGNAL_BUDGET)
        .expect("the writer must reach its first frame");

    // `+ 2`: the line the loop pulled out of the queue before parking in `emit`, and the line the
    // reader is parked in `send` holding. Everything else must still be unread.
    let ceiling = MCP_EVENT_QUEUE_MAX + 2;
    let deadline = std::time::Instant::now() + SIGNAL_BUDGET;
    while produced.load(Ordering::SeqCst) < ceiling && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    // The reader is now against the wall (or the assertion below says it never got there). The
    // settle exists for the MUTATION direction only — it gives an unbounded reader, which needs
    // microseconds for the remaining lines, all the room it could want to run away in. A correct
    // reader cannot move at all while the writer is parked, so this changes nothing here.
    std::thread::sleep(Duration::from_millis(50));
    let while_parked = produced.load(Ordering::SeqCst);
    assert_eq!(
        while_parked, ceiling,
        "with the writer parked, the reader must stop at the {MCP_EVENT_QUEUE_MAX}-deep event \
         queue (+2 in flight) rather than buffering all {total} lines of a stdin that never \
         blocks on its own"
    );

    drop(release);
    let code = server.join().expect("serve must not panic");
    assert_eq!(code, 0);

    // Backpressure, not loss: once the writer drains, the reader resumes and every frame is still
    // answered exactly once — the bound would be worthless if it dropped lines to hold.
    let text = String::from_utf8(lock(&buffer).clone()).expect("valid utf8");
    assert_eq!(
        reply_ids(&text),
        (1..=total as i64).collect::<Vec<i64>>(),
        "every line must still be answered once, in order, after the writer unblocks"
    );
    assert_eq!(
        produced.load(Ordering::SeqCst),
        total,
        "the reader must have gone on to read the rest of the input, not abandoned it"
    );
}

// ── The bounded EOF drain (item 7) ───────────────────────────────────────

/// The drain budget, and the sleep a blocking dispatch holds the worker for. An ORDER OF
/// MAGNITUDE apart in each direction from the wall each measures (CodeRabbit, PR #1092 — at
/// 50 ms/300 ms the "returned on its own deadline" assertion below had only 250 ms of scheduling
/// slack, so a loaded CI runner could fail a correct build): `serve` must return on the 50 ms
/// deadline, the assertion allows it 20× that, and the dispatch it must NOT wait out runs 40×
/// it. Only `DRAIN_EXIT_MAX` sits between the two, and it is nowhere near either.
const DRAIN_BUDGET: Duration = Duration::from_millis(50);
const DISPATCH_HOLD: Duration = Duration::from_secs(2);
const DRAIN_EXIT_MAX: Duration = Duration::from_secs(1);

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

// ── initialize — version negotiation ───────────────────────────────────

#[test]
fn an_unsupported_protocol_version_falls_back_to_the_default() {
    let result = initialize_result(&json!({ "protocolVersion": "2099-01-01" }), INSTRUCTIONS);
    assert_eq!(result["protocolVersion"], DEFAULT_VERSION);
}

#[test]
fn every_supported_older_version_is_echoed_back_verbatim() {
    for v in [
        "2025-11-25",
        "2025-06-18",
        "2025-03-26",
        "2024-11-05",
        "2024-10-07",
    ] {
        let result = initialize_result(&json!({ "protocolVersion": v }), INSTRUCTIONS);
        assert_eq!(result["protocolVersion"], v, "must echo {v}");
    }
}

#[test]
fn a_missing_protocol_version_answers_the_default() {
    let result = initialize_result(&json!({}), INSTRUCTIONS);
    assert_eq!(result["protocolVersion"], DEFAULT_VERSION);
}

#[test]
fn initialize_never_names_2026_07_28() {
    for v in ["2025-11-25", "unknown-future-version"] {
        let result = initialize_result(&json!({ "protocolVersion": v }), INSTRUCTIONS);
        assert_ne!(result["protocolVersion"], "2026-07-28");
    }
}

// ── INSTRUCTIONS / build_instructions (items 7, 14, 18, 24, 27) ─────────

#[test]
fn instructions_name_both_missing_pointer_and_app_closed_and_map_cli_phrasing_onto_tools() {
    assert!(INSTRUCTIONS.contains("app_not_located"));
    assert!(INSTRUCTIONS.contains("app_not_running"));
    assert!(INSTRUCTIONS.contains("call-read"));
    assert!(
        INSTRUCTIONS.contains("--confirm"),
        "must map CLI --confirm phrasing onto this tool's own confirm argument"
    );
}

#[test]
fn instructions_name_connection_lost_alongside_rate_limited_in_the_no_retry_sentence() {
    // item 18 — a payload too large for the bridge frame surfaces as connection_lost, which
    // reads as transient; naming only rate_limited invited a retry loop.
    assert!(INSTRUCTIONS.contains("connection_lost"));
    assert!(INSTRUCTIONS.contains("rate_limited"));
}

/// Issue #1170 — INSTRUCTIONS now points a caller at the résumé/document reads before it judges
/// fit. Every `ns:cmd`-shaped token the prose cites must be a REAL `POLICY` row AND an
/// `Effect::Read` row: the sentence tells a caller to reach it through `call-read`, so a row that
/// was ever anything else earns that caller a `wrong_tool` refusal (issue #1164 — the earlier
/// version of this test checked existence only, which a `git mv`-style rename would catch but a
/// reclassification would not). The one hand-written skip is `ns:cmd` itself — the earlier "a
/// detail that says `agent call ns:cmd`" sentence uses it as a PLACEHOLDER, not a real pair (same
/// "skip list, not a substring match" discipline [`EXPLAINED_IN_PROSE`] already uses above).
#[test]
fn instructions_ns_cmd_pairs_are_real_policy_rows() {
    const SKIP: &[&str] = &["ns:cmd"];
    let is_ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    let mut checked = 0usize;
    for word in INSTRUCTIONS.split_whitespace() {
        let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ':' && c != '_');
        let Some((ns, cmd)) = word.split_once(':') else {
            continue;
        };
        if !is_ident(ns) || !is_ident(cmd) || SKIP.contains(&word) {
            continue;
        }
        checked += 1;
        let entry = POLICY
            .iter()
            .find(|e| agent_call::split_path(e.path) == (ns, cmd))
            .unwrap_or_else(|| {
                panic!("INSTRUCTIONS names `{word}`, which is not a real POLICY row")
            });
        assert!(
            matches!(entry.effect, Effect::Read),
            "INSTRUCTIONS tells a caller to reach `{word}` via call-read, but its POLICY row is \
             not Effect::Read"
        );
    }
    assert_eq!(
        checked, 2,
        "expected exactly the 2 résumé/document ns:cmd pairs (round 5, `B1-r1-ACLI-R5-4`): \
         documents:documents_list (fenced/capped rows) AND documents:documents_get_text (the \
         SAME text by id, fenced and capped at the SAME limit — its `id` param maps to \
         documents_list's `_id` value, spelled out in the prose rather than dropping the \
         command entirely; its reply is now fenced too, `B1-r1-ACLI-R5-7`; neither call can \
         return more of a document than the fence cap, `B1-r2-ACLI-R6-1`): {INSTRUCTIONS}"
    );
}

/// Issue #1170 round-4 review (`B1-r1-ACLI-R4-2`): every generic-tier reply pipes `text` through
/// `agent_call::fence_scraped_fields`, which fences it with `prompt_fence::JOB_CAP` — so a document
/// longer than the cap comes back silently truncated, with no truncation marker on the wire. The
/// old prose promised documents_list rows "already carry the full `text`" on BOTH surfaces below;
/// neither may claim "full" again. The last assertion proves the claim really would be false: text
/// well over the cap comes back shorter than it went in.
///
/// Round 5 (`B1-r1-ACLI-R5-5`): the two negative assertions below only deny the EXACT substrings
/// the round-4 fix happened to write — "rows already carry the complete text", "rows carry the
/// whole document", or "the entire text" would all satisfy both negatives while overclaiming
/// exactly the same thing. Assert the POSITIVE clause on both surfaces too, so a rewrite that
/// drops the caveat (while carefully avoiding the two banned phrases) still fails.
///
/// Round 6 (`B1-r2-ACLI-R6-3`): round 5's positive clause was satisfied by EITHER command's
/// mention — a prose that says "documents_list rows are fenced and capped" once, then separately
/// claims documents_get_text returns the "FULL, uncapped text", passed both assertions unchanged
/// (`fenced and capped` was present; `carry the full`/`full text` were never the phrase actually
/// written). [`assert_document_read_prose_is_honest`] instead: (a) bans "uncapped" anywhere,
/// case-insensitively; (b) bans the STANDALONE word "full" anywhere, not just inside one
/// hand-picked phrase like "carry the full" — "FULL, uncapped text" fails on both grounds now;
/// (c) walks every literal `documents:<cmd>` token found IN the string and requires "capped" to
/// appear near THAT occurrence.
///
/// Round 7 (`B1-r3-ACLI-R7-2`): (c) used the same wide, backward-reaching `window` as (a)/(b), so
/// two `documents:<cmd>` tokens sitting close together (as they do in the real prose) let ONE
/// command's cap disclosure satisfy the OTHER's requirement — the exact failure (c) claims to
/// prevent. The cap check now uses a forward-only span from this token to the NEXT `documents:`
/// occurrence (or the end of the string), so a disclosure written only near a neighbouring
/// command's mention can no longer cover this one. `window` (backward+forward) stays for the
/// full/uncapped bans, which round 6 needs to catch banned words sitting BEFORE the token.
// Byte-safe window bounds — `prose` is human prose with non-ASCII chars (e.g. "résumé"), so an
// arbitrary `idx - 120` can land mid-character; walk to the nearest valid boundary rather than
// panicking on a sliced-through multi-byte char. Module-level (not nested in the test below) so
// `documents_text_prose_per_token_cap_disclosure_is_required` can reuse them against synthetic
// prose without duplicating the window logic.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    let mut i = index.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
fn ceil_char_boundary(s: &str, index: usize) -> usize {
    let mut i = index.min(s.len());
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn assert_document_read_prose_is_honest(prose: &str, label: &str) {
    let mut found_any = false;
    for cmd in ["documents_list", "documents_get_text"] {
        let token = format!("documents:{cmd}");
        let Some(idx) = prose.find(&token) else {
            continue;
        };
        found_any = true;
        // A window AROUND the token, not just after it — the round-6 defect's banned words
        // sat BEFORE the token ("for a document's FULL, uncapped text, call-read
        // documents:documents_get_text …"), so an after-only window would have missed it.
        let start = floor_char_boundary(prose, idx.saturating_sub(120));
        let end = ceil_char_boundary(prose, idx + token.len() + 250);
        let window = &prose[start..end];

        // Forward-only, and bounded by the NEXT `documents:` token — so a cap disclosure
        // sitting near a different command's mention (before this token, or past the next
        // one) can never satisfy this command's own requirement.
        let next_token_start = prose[idx + token.len()..]
            .find("documents:")
            .map(|p| idx + token.len() + p)
            .unwrap_or(prose.len());
        let cap_end = ceil_char_boundary(prose, (idx + token.len() + 250).min(next_token_start));
        let cap_window = &prose[idx..cap_end];
        assert!(
            cap_window.contains("capped"),
            "{label}'s mention of `{token}` must disclose a cap near ITS OWN occurrence, \
             not rely on a disclosure written only near a different command's mention: \
             …{cap_window}…"
        );
        assert!(
            !window.to_ascii_lowercase().contains("uncapped"),
            "{label}'s mention of `{token}` must never claim it is uncapped: …{window}…"
        );
        assert!(
            !window
                .split(|c: char| !c.is_ascii_alphabetic())
                .any(|word| word.eq_ignore_ascii_case("full")),
            "{label}'s mention of `{token}` must never claim it returns the FULL text: \
             …{window}…"
        );
    }
    assert!(
        found_any,
        "{label} must name at least one documents:<cmd> read: {prose}"
    );
}

#[test]
fn documents_text_prose_never_claims_full_past_the_fence_cap() {
    assert_document_read_prose_is_honest(INSTRUCTIONS, "INSTRUCTIONS");
    let list = tools(Tier::Read);
    let profile_description = list.iter().find(|t| t["name"] == TOOL_PROFILE).unwrap()
        ["description"]
        .as_str()
        .unwrap();
    assert_document_read_prose_is_honest(profile_description, "profile's description");

    let over_cap = "x".repeat(crate::prompt_fence::JOB_CAP + 500);
    let fenced =
        crate::prompt_fence::fenced("job_posting", &over_cap, crate::prompt_fence::JOB_CAP);
    assert!(
        fenced.len() < over_cap.len(),
        "premise: fencing must actually truncate text past the cap, or the prose fix above has \
         nothing to be honest about"
    );
}

/// `B2-r1-ACLI-R8-2` (MEDIUM, review round 8): `documents_get_text` returns the IDENTICAL empty
/// string for both an unresolved `id` and a stored document whose own extracted text is itself
/// empty (`commands/documents.rs`'s `store.get(&id).map(|doc| doc.text).unwrap_or_default()`
/// falls through to `""` either way) — so neither surface may claim the empty fenced block means
/// ONLY "no such document"; both must say the two causes are not distinguishable from the reply
/// alone and point the caller at `documents:documents_list` to tell them apart.
#[test]
fn documents_text_prose_never_claims_empty_means_only_no_such_document() {
    let list = tools(Tier::Read);
    let profile_description = list.iter().find(|t| t["name"] == TOOL_PROFILE).unwrap()
        ["description"]
        .as_str()
        .unwrap();
    for (prose, label) in [
        (INSTRUCTIONS, "INSTRUCTIONS"),
        (profile_description, "profile's description"),
    ] {
        assert!(
            !prose.contains("means \"no such document\", never \"this document has no text\""),
            "{label} must never claim the empty fenced block means ONLY \"no such document\" — \
             documents_get_text returns the identical empty string when a real document's own \
             extracted text is empty too: {prose}"
        );
        assert!(
            prose.contains("cross-check") && prose.contains("documents:documents_list"),
            "{label} must tell the caller how to tell the two empty-reply causes apart via \
             documents:documents_list: {prose}"
        );
    }
}

/// Regression for `B1-r3-ACLI-R7-2`: reproduces the review's mutation run B directly — two
/// `documents:<cmd>` tokens close together, where `documents_list`'s own cap disclosure sits in
/// the ~100-char gap before `documents_get_text`'s token but `documents_get_text` never discloses
/// its own cap. The old backward-reaching window let list's disclosure satisfy get_text's
/// requirement; the fix must reject that and only accept a disclosure near get_text's own token.
#[test]
fn documents_text_prose_per_token_cap_disclosure_is_required() {
    let borrowed_disclosure = "read documents:documents_list (fenced and capped at the fence \
        limit); documents:documents_get_text returns that same document text by id ";
    let result = std::panic::catch_unwind(|| {
        assert_document_read_prose_is_honest(borrowed_disclosure, "synthetic");
    });
    assert!(
        result.is_err(),
        "documents_get_text's own missing cap disclosure must fail even though \
         documents_list's disclosure sits nearby"
    );

    let own_disclosure = "read documents:documents_list (fenced and capped at the fence limit); \
        documents:documents_get_text returns that same document text by id, fenced and capped \
        at the same limit";
    assert_document_read_prose_is_honest(own_disclosure, "synthetic");
}

/// Every `"error":` STRING LITERAL mcp.rs's own source writes directly — never `agent_call`'s
/// `pub(super)` sentinels (`ERR_UNKNOWN_COMMAND`/`ERR_NOT_EXPOSED`/`ERR_CONFIRMATION_REQUIRED`),
/// referenced by path there and never respelled here. A test-only fixture (item 24): nothing in
/// production reads it, only the two tests below.
const MCP_SENTINELS: &[&str] = &[
    "wrong_tool",
    "result_too_large",
    "server_busy",
    "shutting_down",
];

#[test]
fn instructions_name_every_mcp_only_sentinel() {
    // item 24 — wrong_tool/result_too_large are MCP-only outcomes named nowhere else.
    for sentinel in MCP_SENTINELS {
        assert!(
            INSTRUCTIONS.contains(sentinel),
            "INSTRUCTIONS must name MCP-only sentinel `{sentinel}`"
        );
    }
}

/// Find the next `"error"` key in `source` at or after `from` whose value is a string literal,
/// tolerating ANY amount of whitespace (including a newline, i.e. rustfmt splitting key and value
/// across lines) between `"error"`, `:`, and the opening quote — CodeRabbit, PR #1092: the prior
/// scanner matched only the exact spelling `"error": "` (one space), so `"error":"x"` or a
/// line-split write would silently produce NO match while the "found is non-empty" sanity check
/// stayed green on whatever it DID happen to catch elsewhere in the file. Returns the literal's
/// value and the index just past its closing quote, so the caller can resume scanning from there;
/// a `"error"` occurrence whose value isn't a string (e.g. `"error": some_const`) is skipped, not
/// treated as a scan failure.
fn next_error_literal(source: &str, from: usize) -> Option<(&str, usize)> {
    let bytes = source.as_bytes();
    let mut search_from = from;
    loop {
        let key_pos = source[search_from..].find("\"error\"")?;
        let after_key = search_from + key_pos + "\"error\"".len();
        let mut i = after_key;
        while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if bytes.get(i) != Some(&b':') {
            search_from = after_key;
            continue;
        }
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if bytes.get(i) != Some(&b'"') {
            search_from = after_key;
            continue;
        }
        let value_start = i + 1;
        let value_end = value_start + source[value_start..].find('"')?;
        return Some((&source[value_start..value_end], value_end + 1));
    }
}

#[test]
fn every_error_literal_in_mcp_source_is_named_in_mcp_sentinels() {
    // item 24 — a drift guard: every `"error": "..."` string literal this file's own source
    // writes must be a member of MCP_SENTINELS (never `agent_call`'s shared sentinels, which are
    // referenced by path, not respelled here).
    const SOURCE: &str = include_str!("../mcp.rs");
    let mut idx = 0;
    let mut found = Vec::new();
    while let Some((value, next)) = next_error_literal(SOURCE, idx) {
        found.push(value);
        idx = next;
    }
    assert!(
        !found.is_empty(),
        "sanity: the scanner must find at least one literal"
    );
    for f in &found {
        assert!(
            MCP_SENTINELS.contains(f),
            "mcp.rs writes \"error\": \"{f}\" but MCP_SENTINELS doesn't name it"
        );
    }
}

#[test]
fn build_instructions_appends_one_sentence_per_enabled_tier_and_never_duplicates_the_base() {
    let none = build_instructions(Tier::Read);
    let reversible = build_instructions(Tier::Reversible);
    let irreversible = build_instructions(Tier::Irreversible);
    // Was an exact equality against bare INSTRUCTIONS; issue #1143 appends the derived sentinel
    // table at EVERY tier, so the invariant this test owns is "leads with the base text and adds
    // no TIER notice", not "is byte-identical to the base text".
    assert!(
        none.starts_with(INSTRUCTIONS),
        "must still lead with the base text: {none}"
    );
    assert!(
        !none.contains("tier is enabled"),
        "no flags must append no tier notice: {none}"
    );
    assert!(reversible.starts_with(INSTRUCTIONS));
    assert!(reversible.contains("reversible write tier is enabled"));
    assert!(
        !reversible.contains("irreversible tier is enabled"),
        "the irreversible notice must not appear at the reversible tier: {reversible}"
    );
    assert!(irreversible.contains("reversible write tier is enabled"));
    assert!(irreversible.contains("irreversible tier is enabled"));
    assert_eq!(
        irreversible.matches("loopback bridge").count(),
        1,
        "must append, never duplicate, the base INSTRUCTIONS text"
    );
}

/// Issue #1143 — the shipped instructions named 2 of the ~10 sentinels this CLI can return, so a
/// client hitting `pairing_token_unavailable`/`pairing_rejected` (a moved data dir, a re-pair)
/// got an unexplained string. Two-sided on purpose: every row must be REACHABLE from the final
/// text, and the one deliberate omission is asserted ABSENT and anchored to its own const, so
/// widening the exclusion silently is what fails here rather than passing quietly.
#[test]
fn every_error_sentinel_is_named_in_the_final_instructions_except_the_pre_protocol_one() {
    let text = build_instructions(Tier::Read);
    for (sentinel, meaning) in ERROR_SENTINELS {
        if *sentinel == ERR_RUNTIME_UNAVAILABLE {
            assert!(
                !text.contains(sentinel),
                "`{sentinel}` fires before the protocol starts (stderr + exit 2), so no tool \
                 result can carry it and the instructions must not promise it: {text}"
            );
            continue;
        }
        assert!(
            text.contains(sentinel),
            "instructions must name every sentinel a tool result can carry, missing `{sentinel}`"
        );
        // The MEANING travels with the name for the rows the skip list doesn't claim — a bare
        // name list would leave the client exactly as unable to recover as before. The escape
        // hatch is the SAME literal list the builder filters on (MEDIUM fix, review round 4): it
        // used to be `INSTRUCTIONS.contains(sentinel)`, so a sentinel the prose merely MENTIONED
        // satisfied both the filter and its own test.
        assert!(
            text.contains(meaning) || EXPLAINED_IN_PROSE.contains(sentinel),
            "`{sentinel}` is only listed, never explained: {text}"
        );
    }
}

/// The filter, not just the table: a sentinel the base prose already explains must NOT be
/// re-listed. Mutating `sentinel_table`'s `!EXPLAINED_IN_PROSE.contains(name)` away is what this
/// catches — otherwise a raw dump would repeat `app_not_running` in the same string twice.
#[test]
fn the_sentinel_table_skips_rows_the_base_prose_already_explains() {
    let text = build_instructions(Tier::Read);
    for already_named in EXPLAINED_IN_PROSE {
        assert_eq!(
            text.matches(already_named).count(),
            INSTRUCTIONS.matches(already_named).count(),
            "`{already_named}` must not be repeated by the derived table: {text}"
        );
    }
}

/// The skip list against a SECOND hand-written literal list — the repo's standing pairing rule
/// (a test that loops over the table it is checking can only catch additions). Removing a name
/// here is what re-adds a redundant table row for a sentinel the prose already explains; the
/// loop-over-EXPLAINED_IN_PROSE tests around this one cannot see that by construction.
#[test]
fn the_skip_list_matches_a_hand_written_literal_list() {
    assert_eq!(
        EXPLAINED_IN_PROSE,
        ["app_not_running", "app_not_located"],
        "changing the skip list means re-reading the prose: a name belongs here only if \
         INSTRUCTIONS says what the sentinel IS and what to do about it, not merely mentions it"
    );
}

/// The skip list is hand-written, so it needs both directions pinned (MEDIUM fix, review round 4
/// — a hand-written list nothing checks is exactly the drift the old substring filter had).
/// Forward: every name on it is a REAL `ERROR_SENTINELS` row (otherwise it is inert) that the
/// base prose really does mention. Backward: every sentinel a tool result can carry is either on
/// the list or has its own derived row — no third state.
#[test]
fn every_skip_list_name_is_a_real_sentinel_the_prose_names_and_every_other_row_is_in_the_table() {
    for name in EXPLAINED_IN_PROSE {
        assert!(
            ERROR_SENTINELS.iter().any(|(n, _)| n == name),
            "`{name}` is not an ERROR_SENTINELS row, so skipping it does nothing"
        );
        assert!(
            INSTRUCTIONS.contains(name),
            "the base prose must actually explain `{name}` — it is not even mentioned"
        );
    }
    let table = build_instructions(Tier::Read)
        .strip_prefix(INSTRUCTIONS)
        .expect("the derived table is appended to the base prose")
        .to_string();
    for (sentinel, meaning) in ERROR_SENTINELS {
        if *sentinel == ERR_RUNTIME_UNAVAILABLE {
            continue;
        }
        if EXPLAINED_IN_PROSE.contains(sentinel) {
            assert!(
                !table.contains(sentinel),
                "`{sentinel}` is claimed as explained by the prose, so the table must skip it"
            );
        } else {
            assert!(
                table.contains(sentinel) && table.contains(meaning),
                "`{sentinel}` is neither claimed by the skip list nor listed with its meaning: \
                 {table}"
            );
        }
    }
}

/// The row `connection_lost` lost to the old substring filter — the prose names it only inside
/// "don't retry in a loop", which never says what it IS. Anchored to the sentinel that produced
/// the finding rather than to the list, so removing it from the table fails here even if someone
/// adds it to `EXPLAINED_IN_PROSE` at the same time.
#[test]
fn connection_lost_gets_its_own_table_row_because_the_prose_only_mentions_it() {
    let text = build_instructions(Tier::Read);
    assert!(
        text.matches(ERR_CONNECTION_LOST).count()
            > INSTRUCTIONS.matches(ERR_CONNECTION_LOST).count(),
        "a merely-mentioned sentinel must still be defined by the table: {text}"
    );
    let (_, meaning) = ERROR_SENTINELS
        .iter()
        .find(|(n, _)| *n == ERR_CONNECTION_LOST)
        .expect("connection_lost is a sentinel");
    assert!(text.contains(meaning), "with its meaning: {text}");
}

#[test]
fn instructions_notices_are_worded_by_tier_not_by_the_literal_flag_typed() {
    // item 27 — launched with ONLY --allow-irreversible; Tier::Irreversible implies the
    // reversible tier too, so BOTH notices append, but neither may claim a flag never typed.
    let text = build_instructions(Tier::from_flags(false, true));
    assert!(
        !text.contains("--allow-reversible") && !text.contains("--allow-irreversible"),
        "notices must be worded by TIER, not by the literal flag: {text}"
    );
    assert!(text.contains("reversible write tier is enabled"));
    assert!(text.contains("irreversible tier is enabled"));
}

// ── curated_tool description join (item 8) ──────────────────────────────

#[test]
fn curated_tool_joins_base_and_extra_as_two_sentences_not_a_run_on() {
    let tool = tools(Tier::Read)
        .into_iter()
        .find(|t| t["name"] == TOOL_BEST_MATCHES)
        .unwrap();
    let description = tool["description"].as_str().unwrap().to_string();
    assert!(
        description.contains(". "),
        "base and extra must be joined as two sentences: {description}"
    );
    assert!(
        !description.contains(") title/company"),
        "must never join with a bare space (the live run-on this fix closed): {description}"
    );
}

#[test]
fn every_scraped_text_tool_carries_the_same_untrusted_fields_notice() {
    // pre-PR gate, extended in review round 2 (MEDIUM — `found-jobs` was added without
    // extending this pairwise check, the exact class of gap `#1088` warns about): every
    // curated tool that returns title/company/location/description scraped text must carry
    // the IDENTICAL notice as `best-matches`, never a fresh one-off pair test per new tool.
    let list = tools(Tier::Read);
    let notice = list
        .iter()
        .find(|t| t["name"] == TOOL_BEST_MATCHES)
        .unwrap()["description"]
        .as_str()
        .unwrap()
        .rsplit_once(". ")
        .unwrap()
        .1
        .to_string();
    for tool in [TOOL_JOB, TOOL_FOUND_JOBS] {
        let description = list.iter().find(|t| t["name"] == tool).unwrap()["description"]
            .as_str()
            .unwrap();
        assert!(
            description.contains(&notice),
            "{tool}'s description must carry the same untrusted-fields notice as best-matches: \
             {description}"
        );
    }
}

/// Issue #1170, round 5 (`B1-r1-ACLI-R5-4`): the `profile` tool must say up front it holds
/// contact fields only, and point at REAL `POLICY` reads for the résumé/document text itself (a
/// stale rename here would send a calling model at a command that no longer exists). Both
/// `documents:documents_list` (fenced/capped rows) AND `documents:documents_get_text` (the same
/// text by id, fenced and capped at the SAME limit — round 6, `B1-r2-ACLI-R6-1`: it does NOT
/// return more) are named — the earlier version of this description dropped `documents_get_text`
/// entirely rather than spelling out that its `id` param maps to `documents_list`'s `_id` value,
/// leaving an assistant with no way to reach a résumé by id at all, only by re-listing.
#[test]
fn profile_tool_description_names_a_real_document_read() {
    let list = tools(Tier::Read);
    let description = list.iter().find(|t| t["name"] == TOOL_PROFILE).unwrap()["description"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        description.contains("Contact fields only"),
        "must say the profile tool holds contact fields only: {description}"
    );
    // Every `ns:cmd`-shaped token is pulled OUT of the description text itself (same
    // discipline as `instructions_ns_cmd_pairs_are_real_policy_rows` below) — a hand-picked
    // pair list would keep passing after a rename inside the string, which is exactly how the
    // round-1 fix missed `documents_get_text` (issue #1164 round 2).
    let is_ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_');
    let mut checked = 0usize;
    for word in description.split_whitespace() {
        let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ':' && c != '_');
        let Some((ns, cmd)) = word.split_once(':') else {
            continue;
        };
        if !is_ident(ns) || !is_ident(cmd) {
            continue;
        }
        checked += 1;
        let entry = POLICY
            .iter()
            .find(|e| agent_call::split_path(e.path) == (ns, cmd))
            .unwrap_or_else(|| {
                panic!("profile's description names {word}, which is not a real POLICY row")
            });
        assert!(
            matches!(entry.effect, Effect::Read),
            "profile's description tells a caller to reach {word} via call-read, but its \
             POLICY row is not Effect::Read"
        );
    }
    assert_eq!(
        checked, 2,
        "expected exactly the 2 ns:cmd pairs the profile description names (round 5, \
         `B1-r1-ACLI-R5-4`) — documents:documents_list and documents:documents_get_text, \
         matching INSTRUCTIONS: {description}"
    );
}

// ── Advertised schema text (issues #1129, #1130, #1132, #1144) ──────────

/// One tool's `inputSchema.properties.<field>.description`.
fn property_description(list: &[Value], tool: &str, field: &str) -> String {
    list.iter()
        .find(|t| t["name"] == tool)
        .unwrap_or_else(|| panic!("{tool} must be listed"))["inputSchema"]["properties"][field]
        ["description"]
        .as_str()
        .unwrap_or_else(|| panic!("{tool}.{field} must declare a description"))
        .to_string()
}

fn tool_description(list: &[Value], tool: &str) -> String {
    list.iter()
        .find(|t| t["name"] == tool)
        .unwrap_or_else(|| panic!("{tool} must be listed"))["description"]
        .as_str()
        .expect("a description")
        .to_string()
}

/// Issue #1129 — the shipped schema advertised "default 50, server cap 100" while the server
/// enforced 25/50, and an AI client has no other source for those bounds before its first call.
/// Anchored to the CONSTANTS, never to today's numbers: a test retyping 25/50 would be the same
/// hand-typed copy that drifted. The stale literal is asserted absent so the old text cannot
/// creep back in beside a correct one. Each bound is asserted as the WHOLE derived phrase rather
/// than two independent `contains` calls: separate calls still pass with the default and the cap
/// transposed, which is exactly the drift this test exists to catch.
#[test]
fn both_limit_descriptions_are_derived_from_the_constants_the_server_enforces() {
    let list = tools(Tier::Read);
    let found = property_description(&list, TOOL_FOUND_JOBS, "limit");
    assert!(
        found.contains(&format!(
            "default {DEFAULT_FOUND_JOBS_LIMIT}, server cap {MAX_FOUND_JOBS_LIMIT}"
        )),
        "found-jobs' limit must advertise the enforced default/cap: {found}"
    );
    assert!(
        !found.contains("100"),
        "the drifted cap must be gone, not merely joined by the real one: {found}"
    );
    let best = property_description(&list, TOOL_BEST_MATCHES, "limit");
    assert!(
        best.contains(&format!(
            "default {DEFAULT_BEST_MATCHES_LIMIT}, server cap {MAX_BEST_MATCHES_LIMIT}"
        )),
        "best-matches' limit must advertise the enforced default/cap: {best}"
    );
}

/// Issue #1130 — the wire cursor is `<autopilotId>:<offset>`, so calling it an "offset" invited
/// exactly the cross-autopilot reuse the resolver now rejects.
#[test]
fn the_found_jobs_cursor_is_advertised_as_an_opaque_per_autopilot_token() {
    let cursor = property_description(&tools(Tier::Read), TOOL_FOUND_JOBS, "cursor");
    assert!(
        !cursor.contains("offset"),
        "the cursor is no longer a bare offset and must not be described as one: {cursor}"
    );
    assert!(
        cursor.contains("opaque") && cursor.contains("autopilotId"),
        "it must say it is opaque and bound to the id that issued it: {cursor}"
    );
}

/// Issue #1132 — `totalFound` is the LAST run's kept count and diverged from the traversable
/// total by up to ~24x on real data, with nothing on the surface saying so.
#[test]
fn the_automations_description_distinguishes_both_totals() {
    let description = tool_description(&tools(Tier::Read), TOOL_AUTOMATIONS);
    assert!(
        description.contains("totalFound") && description.contains("foundJobsTotal"),
        "both totals must be named where a client reads them: {description}"
    );
    assert!(
        description.contains("`totalFound` is the last run's"),
        "`totalFound` must be qualified as last-run-only: {description}"
    );
}

/// Issue #1144 — a well-formed payload sent as the bare `input` fails with an opaque
/// `invoke_error` on ~24 write commands; the wrapper key isn't derivable, so both channels a
/// client reads must say it.
#[test]
fn the_generic_input_schema_and_the_instructions_both_document_the_wrapper_key() {
    let input = property_description(&tools(Tier::Read), TOOL_CALL_READ, "input");
    assert!(
        input.contains("parameter") && input.contains("invoke_error"),
        "the input schema must name the wrapper key rule and the recovery signal: {input}"
    );
    assert!(
        INSTRUCTIONS.contains("invoke_error"),
        "a client that reads only `instructions` must learn the same recovery: {INSTRUCTIONS}"
    );
}

/// Roadmap #1146 P6 — `autopilot_run` does its whole scrape inside the app and can outlast this
/// server's per-call budget, so a timeout must not read as "the run stopped".
#[test]
fn call_irreversible_says_long_work_outlives_the_call_and_names_what_to_poll() {
    let description = tool_description(&tools(Tier::Irreversible), TOOL_CALL_IRREVERSIBLE);
    assert!(
        description.contains("autopilot_run") && description.contains("timeout"),
        "the async shape must be stated on the tool itself: {description}"
    );
    assert!(
        description.contains(TOOL_AUTOMATIONS),
        "and it must name what to poll instead: {description}"
    );
}

// ── title + deterministic order (roadmap #1146 P1, P10) ─────────────────

/// P1 — every client's tool-approval UI shows `title` when present and the raw wire `name` when
/// not, so a tool added without one is a visible regression. Derived sweep + the hand-written
/// literal list below, per the "a guard driven off its own data can't catch a deletion" rule.
#[test]
fn every_tool_carries_a_non_empty_human_title() {
    for tool in tools(Tier::Irreversible) {
        let title = tool["title"].as_str().unwrap_or_default();
        assert!(
            !title.is_empty(),
            "{} must carry a human title",
            tool["name"]
        );
        assert_ne!(
            title,
            tool["name"].as_str().unwrap_or_default(),
            "a title that just repeats the wire name adds nothing"
        );
    }
}

#[test]
fn tool_titles_match_a_hand_written_literal_list() {
    let list = tools(Tier::Irreversible);
    let titles: Vec<&str> = list
        .iter()
        .map(|t| t["title"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        titles,
        vec![
            "Best Matches",
            "Job by URL",
            "My Profile",
            "Automations",
            "Found Jobs",
            "Commands",
            "Call (read)",
            "Call (reversible)",
            "Call (irreversible)",
        ]
    );
}

/// P10 — deterministic ordering is what lets a client's prompt cache survive repeated
/// `tools/list` calls in a long session. Two properties, both mutation-visible: the order is
/// STABLE call-to-call, and every lower tier is a strict PREFIX of the next, so enabling a write
/// tier appends rather than reshuffling the read tools a cached prompt already holds.
#[test]
fn tools_list_order_is_stable_across_calls_and_prefix_stable_across_tiers() {
    let ordered = |tier| -> Vec<String> {
        tools(tier)
            .iter()
            .map(|t| t["name"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    let read = ordered(Tier::Read);
    assert_eq!(read, ordered(Tier::Read), "two calls must agree");
    assert_eq!(
        read,
        vec![
            "best-matches",
            "job",
            "profile",
            "automations",
            "found-jobs",
            "commands",
            "call-read",
        ],
        "the read tier's order is part of the contract, not an accident of construction"
    );
    let reversible = ordered(Tier::Reversible);
    let irreversible = ordered(Tier::Irreversible);
    assert_eq!(
        reversible[..read.len()],
        read[..],
        "read tier stays a prefix"
    );
    assert_eq!(
        irreversible[..reversible.len()],
        reversible[..],
        "reversible tier stays a prefix"
    );
    assert_eq!(reversible[read.len()..], ["call-reversible"]);
    assert_eq!(irreversible[reversible.len()..], ["call-irreversible"]);
}

// ── Tier (item 21) ───────────────────────────────────────────────────────

#[test]
fn tier_irreversible_always_implies_reversible() {
    assert!(Tier::Irreversible.allows_reversible());
    assert!(Tier::Reversible.allows_reversible());
    assert!(!Tier::Read.allows_reversible());
    assert!(Tier::Irreversible.allows_irreversible());
    assert!(!Tier::Reversible.allows_irreversible());
    assert!(!Tier::Read.allows_irreversible());
}

#[test]
fn from_flags_resolves_every_combination_including_irreversible_alone() {
    assert_eq!(Tier::from_flags(false, false), Tier::Read);
    assert_eq!(Tier::from_flags(true, false), Tier::Reversible);
    assert_eq!(Tier::from_flags(false, true), Tier::Irreversible);
    assert_eq!(Tier::from_flags(true, true), Tier::Irreversible);
}

#[test]
fn server_new_false_true_still_resolves_the_full_irreversible_tier() {
    // The exact gap the review named: nothing previously constructed Server::new(false, true).
    let server = Server::new(false, true);
    assert_eq!(server.tier, Tier::Irreversible);
    assert_eq!(
        names(&server.tools),
        vec![
            "automations",
            "best-matches",
            "call-irreversible",
            "call-read",
            "call-reversible",
            "commands",
            "found-jobs",
            "job",
            "profile",
        ]
    );
}

// ── tools/list — the hand-written literal list, all three launch modes
// (item 11 — mutation-checked by deleting the reversible gate) ─────────

#[test]
fn tool_names_by_launch_mode_match_hand_written_literal_lists() {
    assert_eq!(
        names(&tools(Tier::Read)),
        vec![
            "automations",
            "best-matches",
            "call-read",
            "commands",
            "found-jobs",
            "job",
            "profile"
        ],
        "default server (no flags) must be read tier + commands only"
    );
    assert_eq!(
        names(&tools(Tier::Reversible)),
        vec![
            "automations",
            "best-matches",
            "call-read",
            "call-reversible",
            "commands",
            "found-jobs",
            "job",
            "profile",
        ],
        "--allow-reversible must add exactly call-reversible"
    );
    assert_eq!(
        names(&tools(Tier::Irreversible)),
        vec![
            "automations",
            "best-matches",
            "call-irreversible",
            "call-read",
            "call-reversible",
            "commands",
            "found-jobs",
            "job",
            "profile",
        ],
        "--allow-irreversible must add call-irreversible on top of call-reversible"
    );
}

#[test]
fn calling_the_reversible_tool_without_the_flag_is_invalid_params() {
    let server = Server::new(false, false);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({
            "name": "call-reversible",
            "arguments": { "namespace": "cli_agents", "command": "cli_agents_redetect" },
        }),
        &server,
        &mut dispatch,
    );
    assert_eq!(outcome.unwrap_err().0, -32602);
}

#[test]
fn calling_the_irreversible_tool_without_the_flag_is_invalid_params() {
    let server = Server::new(false, false);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({
            "name": "call-irreversible",
            "arguments": { "namespace": "documents", "command": "documents_remove" },
        }),
        &server,
        &mut dispatch,
    );
    assert_eq!(outcome.unwrap_err().0, -32602);
}

#[test]
fn every_curated_tool_and_call_tool_declares_a_bare_object_schema_with_no_ref() {
    for tool in tools(Tier::Irreversible) {
        let schema = &tool["inputSchema"];
        assert_eq!(
            schema["type"], "object",
            "{}: root must be type:object",
            tool["name"]
        );
        assert_eq!(
            schema["additionalProperties"], false,
            "{}: additionalProperties must be false",
            tool["name"]
        );
        assert!(schema.get("$ref").is_none(), "{}: no $ref", tool["name"]);
    }
}

// ── additionalProperties:false is ENFORCED, not merely advertised (#1134) ─

/// The payload of a `ToolCall::Local(Ok(..))` outcome, parsed back from `content[0].text`.
fn local_payload(params: &Value, server: &Server) -> Value {
    let ToolCall::Local(Ok(result)) = classify_tool_call(params, server) else {
        panic!("expected a local result for {params}");
    };
    let text = result["content"][0]["text"].as_str().expect("a text block");
    let payload: Value = serde_json::from_str(text).expect("the payload is JSON");
    assert_eq!(result["isError"], true, "an unknown argument is an error");
    assert_eq!(result["content"][1]["text"], "exitCode: 2");
    payload
}

/// Issue #1134 — every curated schema advertised `additionalProperties:false` and nothing
/// validated against it, so a typo'd OPTIONAL key silently took that field's default and answered
/// a quietly-wrong result with `isError:false`.
#[test]
fn an_undeclared_argument_on_a_curated_tool_is_a_usage_error_not_a_silent_drop() {
    let server = Server::new(false, false);
    // A tool with NO declared properties, and one with a real property typo'd — the two shapes
    // the issue reproduced live.
    for arguments in [json!({ "bogusProp": true }), json!({ "limt": 5 })] {
        let payload = local_payload(
            &json!({ "name": TOOL_PROFILE, "arguments": arguments }),
            &server,
        );
        assert_eq!(payload["error"], ERR_USAGE);
    }
    let payload = local_payload(
        &json!({ "name": TOOL_FOUND_JOBS, "arguments": { "autopilotId": "ap-1", "limt": 5 } }),
        &server,
    );
    assert_eq!(payload["error"], ERR_USAGE);
    let detail = payload["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("limit") && detail.contains("cursor") && detail.contains("autopilotId"),
        "the detail must name the DECLARED set so a client can correct itself: {detail}"
    );
    assert!(
        !detail.contains("limt"),
        "the caller's own token is never echoed back (path privacy): {detail}"
    );
}

/// The generic tier is covered by the same gate — including `confirm` on `call-read`, whose
/// schema omits it BY CONSTRUCTION. It used to be accepted and dropped; now the client is told.
#[test]
fn an_undeclared_argument_on_call_read_is_a_usage_error() {
    let server = Server::new(false, false);
    let payload = local_payload(
        &json!({
            "name": TOOL_CALL_READ,
            "arguments": { "namespace": "jobs", "command": "jobs_list", "confirm": "x" },
        }),
        &server,
    );
    assert_eq!(payload["error"], ERR_USAGE);
    let detail = payload["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("namespace") && detail.contains("input"),
        "the detail must name call-read's own declared set: {detail}"
    );
}

/// The other half of the guard: a DECLARED key set still classifies exactly as before. Without
/// this, refusing everything would pass the test above.
#[test]
fn every_declared_argument_still_reaches_the_bridge_or_its_local_result() {
    let server = Server::new(false, false);
    assert!(matches!(
        classify_tool_call(
            &json!({
                "name": TOOL_FOUND_JOBS,
                "arguments": { "autopilotId": "ap-1", "limit": 5, "cursor": "ap-1:5" },
            }),
            &server,
        ),
        ToolCall::Bridge(_)
    ));
    assert!(matches!(
        classify_tool_call(&json!({ "name": TOOL_PROFILE, "arguments": {} }), &server),
        ToolCall::Bridge(_)
    ));
    let ToolCall::Local(Ok(result)) = classify_tool_call(
        &json!({ "name": TOOL_COMMANDS, "arguments": { "effect": "read" } }),
        &server,
    ) else {
        panic!("commands answers locally");
    };
    assert_eq!(result["isError"], false);
}

/// MEDIUM fix, review round 4 — the #1134 gate refused MCP's own reserved `_`-prefixed keys,
/// which no schema declares and any client may attach (`_meta` rides on `tools/list` results in
/// this very file). Both directions in one test: a reserved key passes, and the typo the gate
/// exists for is STILL refused when it rides alongside one.
#[test]
fn a_reserved_underscore_argument_key_is_ignored_but_a_typo_beside_it_is_still_refused() {
    let server = Server::new(false, false);
    for arguments in [
        json!({ "_meta": { "progressToken": 1 } }),
        json!({ "_vendorExtension": true }),
    ] {
        assert!(
            matches!(
                classify_tool_call(
                    &json!({ "name": TOOL_PROFILE, "arguments": arguments }),
                    &server,
                ),
                ToolCall::Bridge(_)
            ),
            "a protocol-reserved key must not turn a valid call into a usage error"
        );
    }
    let payload = local_payload(
        &json!({
            "name": TOOL_FOUND_JOBS,
            "arguments": { "autopilotId": "ap-1", "_meta": { "progressToken": 1 }, "limt": 5 },
        }),
        &server,
    );
    assert_eq!(
        payload["error"], ERR_USAGE,
        "skipping `_`-prefixed keys must not widen into skipping the typo'd ones"
    );
}

#[test]
fn call_irreversible_carries_the_requires_user_interaction_meta() {
    let tool_list = tools(Tier::Irreversible);
    let tool = tool_list
        .iter()
        .find(|t| t["name"] == TOOL_CALL_IRREVERSIBLE)
        .expect("present when allowed");
    assert_eq!(tool["_meta"]["anthropic/requiresUserInteraction"], true);
    assert_eq!(tool["annotations"]["destructiveHint"], true);
}

// ── mcp --help / launch-arg parsing (items 10, 11, 23, 28) ──────────────

#[test]
fn parse_launch_args_accepts_any_subset_of_the_two_flags_in_any_order() {
    assert_eq!(parse_launch_args(&[]).unwrap(), LaunchArgs::default());
    assert_eq!(
        parse_launch_args(&args(&["--allow-reversible"])).unwrap(),
        LaunchArgs {
            help: false,
            allow_reversible: true,
            allow_irreversible: false,
        }
    );
    assert_eq!(
        parse_launch_args(&args(&["--allow-irreversible", "--allow-reversible"])).unwrap(),
        LaunchArgs {
            help: false,
            allow_reversible: true,
            allow_irreversible: true,
        },
        "order must not matter"
    );
}

#[test]
fn parse_launch_args_accepts_help_anywhere_and_rejects_anything_else() {
    assert!(parse_launch_args(&args(&["--help"])).unwrap().help);
    assert!(
        parse_launch_args(&args(&["--allow-reversible", "--help"]))
            .unwrap()
            .help
    );
    assert!(parse_launch_args(&args(&["not-a-flag"])).is_err());
    assert!(parse_launch_args(&args(&["--allow-reversible", "typo"])).is_err());
}

#[test]
fn mcp_help_text_lists_both_flags_and_derives_its_default_list_from_tools() {
    let text = mcp_help_text();
    assert!(text.contains("--allow-reversible"));
    assert!(text.contains("--allow-irreversible"));
    for name in [
        "best-matches",
        "job",
        "profile",
        "automations",
        "commands",
        "call-read",
    ] {
        assert!(text.contains(name), "missing default tool `{name}`: {text}");
    }
    let default_line = text
        .lines()
        .find(|l| l.starts_with("Default"))
        .expect("must have a 'Default (no flags): ...' line");
    assert!(
        !default_line.contains("call-reversible") && !default_line.contains("call-irreversible"),
        "the default-tool-list line must not name a gated tool: {default_line}"
    );
}

#[test]
fn print_help_never_adds_a_trailing_blank_line() {
    let mut buf = Vec::new();
    print_help(&mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert!(
        text.ends_with('\n') && !text.ends_with("\n\n"),
        "must end in exactly one newline, no extra blank line: {text:?}"
    );
}

// ── every POLICY row is served by exactly one call-* tool, or refused
// everywhere if NotExposed (item 12 rewrites the NotExposed branch) ─────

#[test]
fn every_policy_row_is_routed_to_exactly_one_call_tool_or_refused_everywhere_if_not_exposed() {
    for entry in POLICY {
        let (namespace, command) = agent_call::split_path(entry.path);
        let verb = Verb::Call {
            namespace: namespace.to_string(),
            command: command.to_string(),
            input: json!({}),
            confirm: None,
        };
        let accepted_by: Vec<&str> = [TOOL_CALL_READ, TOOL_CALL_REVERSIBLE, TOOL_CALL_IRREVERSIBLE]
            .into_iter()
            .filter(|tool| local_call_refusal(tool, &verb).is_none())
            .collect();
        match entry.effect {
            Effect::NotExposed(_) => assert_eq!(
                accepted_by.len(),
                0,
                "{}: a NotExposed row must refuse locally on every call-* tool (MUST FIX — \
                 review round 2) — got {accepted_by:?}",
                entry.path
            ),
            _ => assert_eq!(
                accepted_by.len(),
                1,
                "{}: exactly one call-* tool must accept this row locally — got {accepted_by:?}",
                entry.path
            ),
        }
    }
}

#[test]
fn extension_bridge_status_the_token_row_refuses_locally_on_every_call_tool() {
    // The HIGH-1 row: it returns the plaintext pairing token verbatim. A cross-version peer (an
    // updater-staged newer exe, an older still-running paired app) must not be the only thing
    // standing between this row and an MCP caller — refuse it locally too, no bridge involved.
    let verb = Verb::Call {
        namespace: "extension_bridge".to_string(),
        command: "extension_bridge_status".to_string(),
        input: json!({}),
        confirm: None,
    };
    for tool in [TOOL_CALL_READ, TOOL_CALL_REVERSIBLE, TOOL_CALL_IRREVERSIBLE] {
        let refusal = local_call_refusal(tool, &verb).expect("must refuse locally on every tool");
        assert_eq!(
            refusal["error"],
            crate::extension_bridge::agent_call::ERR_NOT_EXPOSED
        );
        assert!(refusal["detail"]
            .as_str()
            .unwrap()
            .contains("pairing token"));
    }
}

// ── MUST FIX — unknown_command / wrong_tool local refusals ──────────────

#[test]
fn call_read_refuses_a_namespace_command_the_local_policy_does_not_know() {
    let verb = Verb::Call {
        namespace: "nope".to_string(),
        command: "delete_everything".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal = local_call_refusal(TOOL_CALL_READ, &verb).expect("must refuse");
    assert_eq!(refusal["dispatched"], false);
    assert_eq!(refusal["error"], agent_call::ERR_UNKNOWN_COMMAND);
}

#[test]
fn call_read_refuses_a_real_reversible_row_naming_the_right_tool() {
    // `cli_agents_redetect` is a real Reversible POLICY row.
    let verb = Verb::Call {
        namespace: "cli_agents".to_string(),
        command: "cli_agents_redetect".to_string(),
        input: json!({}),
        confirm: None,
    };
    let refusal = local_call_refusal(TOOL_CALL_READ, &verb).expect("must refuse — wrong tool");
    assert_eq!(refusal["error"], "wrong_tool");
    assert!(refusal["detail"]
        .as_str()
        .unwrap()
        .contains(TOOL_CALL_REVERSIBLE));
}

#[test]
fn call_read_accepts_a_real_read_row() {
    let verb = Verb::Call {
        namespace: "cli_agents".to_string(),
        command: "cli_agents_status".to_string(),
        input: json!({}),
        confirm: None,
    };
    assert!(local_call_refusal(TOOL_CALL_READ, &verb).is_none());
}

// ── confirm is passed through verbatim on call-irreversible only ────────

#[test]
fn confirm_reaches_the_verb_only_via_the_irreversible_tool() {
    let arguments =
        json!({ "namespace": "documents", "command": "documents_remove", "confirm": "Resume A" });
    let argv = tool_argv(TOOL_CALL_IRREVERSIBLE, &arguments);
    let verb = parse_verb(&argv).unwrap();
    assert_eq!(
        verb,
        Verb::Call {
            namespace: "documents".to_string(),
            command: "documents_remove".to_string(),
            input: json!({}),
            confirm: Some("Resume A".to_string()),
        }
    );
}

/// Still the contract AT THIS LAYER: `tool_argv` never forwards `confirm` off
/// `call-irreversible`. A real `tools/call` no longer gets this far — `classify_tool_call`
/// refuses the undeclared key first (issue #1134, tested above) — but the layered guarantee is
/// what keeps a future caller of `tool_argv` from smuggling a proof through.
#[test]
fn a_confirm_argument_sent_to_call_read_is_silently_ignored() {
    let arguments =
        json!({ "namespace": "jobs", "command": "jobs_list", "confirm": "should never forward" });
    let argv = tool_argv(TOOL_CALL_READ, &arguments);
    let verb = parse_verb(&argv).unwrap();
    assert_eq!(
        verb,
        Verb::Call {
            namespace: "jobs".to_string(),
            command: "jobs_list".to_string(),
            input: json!({}),
            confirm: None,
        }
    );
}

// ── found-jobs tool_argv mapping (MEDIUM fix, review round 2 — this new arm had no
// coverage at all) ───────────────────────────────────────────────────────────────

#[test]
fn found_jobs_tool_argv_maps_autopilot_id_limit_and_cursor() {
    let arguments = json!({ "autopilotId": "ap-1", "limit": 10, "cursor": "20" });
    let argv = tool_argv(TOOL_FOUND_JOBS, &arguments);
    assert_eq!(
        parse_verb(&argv).unwrap(),
        Verb::FoundJobs {
            autopilot_id: "ap-1".to_string(),
            limit: Some(10),
            cursor: Some("20".to_string()),
        }
    );
}

#[test]
fn found_jobs_tool_argv_omits_optional_flags_when_absent() {
    let argv = tool_argv(TOOL_FOUND_JOBS, &json!({ "autopilotId": "ap-1" }));
    assert_eq!(
        parse_verb(&argv).unwrap(),
        Verb::FoundJobs {
            autopilot_id: "ap-1".to_string(),
            limit: None,
            cursor: None,
        }
    );
}

/// HIGH fix, review round 2 — a JSON NUMBER `cursor` (as a real MCP client would send,
/// since the declared schema type is `string` but nothing on the wire enforces that) must
/// still reach `parse_verb` as a string, not be dropped as if the caller had sent nothing.
#[test]
fn found_jobs_tool_argv_forwards_a_numeric_cursor_rather_than_dropping_it() {
    let arguments = json!({ "autopilotId": "ap-1", "cursor": 100 });
    let argv = tool_argv(TOOL_FOUND_JOBS, &arguments);
    assert_eq!(
        parse_verb(&argv).unwrap(),
        Verb::FoundJobs {
            autopilot_id: "ap-1".to_string(),
            limit: None,
            cursor: Some("100".to_string()),
        }
    );
}

#[test]
fn found_jobs_tool_argv_treats_an_explicit_null_cursor_as_absent() {
    let arguments = json!({ "autopilotId": "ap-1", "cursor": null });
    let argv = tool_argv(TOOL_FOUND_JOBS, &arguments);
    assert_eq!(
        parse_verb(&argv).unwrap(),
        Verb::FoundJobs {
            autopilot_id: "ap-1".to_string(),
            limit: None,
            cursor: None,
        }
    );
}

/// Issue #1137 — `limit: null` was forwarded as the literal string `"null"` and failed
/// `--limit`'s integer parse, while the sibling `cursor` on the same tool already read an
/// explicit null as absent. BOTH `limit` arms are covered: shipping the fix on one of two
/// structurally identical arms guarantees a second issue.
#[test]
fn an_explicit_null_limit_reads_as_absent_on_both_tools_that_take_one() {
    assert_eq!(
        parse_verb(&tool_argv(
            TOOL_FOUND_JOBS,
            &json!({ "autopilotId": "ap-1", "limit": null })
        ))
        .unwrap(),
        Verb::FoundJobs {
            autopilot_id: "ap-1".to_string(),
            limit: None,
            cursor: None,
        }
    );
    assert_eq!(
        parse_verb(&tool_argv(TOOL_BEST_MATCHES, &json!({ "limit": null }))).unwrap(),
        Verb::BestMatches { limit: None }
    );
}

/// The direction the null-filter must NOT regress: a JSON number is still coerced, on the same
/// two arms. A `filter` that swallowed non-strings would pass the test above and break this one.
#[test]
fn a_numeric_limit_still_reaches_parse_verb_on_both_tools() {
    assert_eq!(
        parse_verb(&tool_argv(TOOL_BEST_MATCHES, &json!({ "limit": 7 }))).unwrap(),
        Verb::BestMatches { limit: Some(7) }
    );
    assert_eq!(
        parse_verb(&tool_argv(
            TOOL_FOUND_JOBS,
            &json!({ "autopilotId": "ap-1", "limit": 7 })
        ))
        .unwrap(),
        Verb::FoundJobs {
            autopilot_id: "ap-1".to_string(),
            limit: Some(7),
            cursor: None,
        }
    );
}

/// Issue #1140 — a numeric `confirm` (the shape `ProofSource::Count` proofs really take, e.g. a
/// token count read from `ai_spend_summary`) was dropped by `and_then(Value::as_str)` and
/// answered exactly like "confirm omitted", so a client could loop forever re-reading the same
/// proof and re-sending it the same way.
#[test]
fn a_non_string_confirm_reaches_the_verb_as_its_json_text_rather_than_being_dropped() {
    let arguments = json!({
        "namespace": "documents", "command": "documents_remove", "confirm": 12345,
    });
    let verb = parse_verb(&tool_argv(TOOL_CALL_IRREVERSIBLE, &arguments)).unwrap();
    assert_eq!(
        verb,
        Verb::Call {
            namespace: "documents".to_string(),
            command: "documents_remove".to_string(),
            input: json!({}),
            confirm: Some("12345".to_string()),
        }
    );
}

/// …and the other side of the same fix: an explicit `null` still means "no proof supplied", so a
/// strict-schema client gets the `confirmation_required` hint rather than a mismatch on a value
/// it never sent.
#[test]
fn an_explicit_null_confirm_still_reads_as_absent() {
    let arguments = json!({
        "namespace": "documents", "command": "documents_remove", "confirm": null,
    });
    let verb = parse_verb(&tool_argv(TOOL_CALL_IRREVERSIBLE, &arguments)).unwrap();
    assert_eq!(
        verb,
        Verb::Call {
            namespace: "documents".to_string(),
            command: "documents_remove".to_string(),
            input: json!({}),
            confirm: None,
        }
    );
}

#[test]
fn confirmation_required_result_carries_the_cli_payload_verbatim_plus_one_note() {
    let payload = json!({
        "dispatched": false, "namespace": "ai", "command": "ai_set_provider_key",
        "error": agent_call::ERR_CONFIRMATION_REQUIRED,
        "detail": "read `agent call ai:ai_has_provider_key` and pass its own `has` field as --confirm",
    });
    let result = tool_result(payload.clone(), 4);
    assert_eq!(result["isError"], true);
    let blocks = result["content"].as_array().unwrap();
    assert_eq!(
        blocks[0]["text"],
        payload.to_string(),
        "content[0] must be the CLI payload byte-for-byte"
    );
    assert_eq!(blocks[1]["text"], "exitCode: 4");
    assert!(
        blocks.len() >= 3,
        "a confirmation_required result must carry a third, mapping block"
    );
    assert!(blocks[2]["text"].as_str().unwrap().contains("call-read"));
}

// ── structuredContent is dropped everywhere (item 15) ───────────────────

#[test]
fn tool_result_never_carries_structured_content() {
    let result = tool_result(json!({ "ok": true }), 0);
    assert!(
        result.get("structuredContent").is_none(),
        "structuredContent must never appear — see the module doc's output-contract section"
    );
}

// ── result-size cap, checked in tool_result itself (items 13, 16, 17, 22, 25) ──

#[test]
fn oversized_result_detail_never_names_the_cli_invocation() {
    let refusal = oversized_result(MCP_RESULT_MAX_BYTES + 1);
    let detail = refusal["detail"].as_str().unwrap();
    assert!(
        !detail.contains("agent call") && !detail.contains("agent mcp"),
        "must not hand the model a bypass recipe: {detail}"
    );
    assert_eq!(
        refusal["dispatched"], false,
        "must mirror every other Verb::Call refusal's own shape"
    );
}

#[test]
fn a_dispatched_payload_over_the_byte_cap_refuses_and_never_truncates() {
    let server = Server::new(true, true);
    let huge =
        json!({ "ok": true, "resource": "call", "blob": "x".repeat(MCP_RESULT_MAX_BYTES + 10) });
    let mut dispatch = move |_: &Verb| Ok(huge.clone());
    let outcome = tool_call_result(
        &json!({
            "name": "call-read",
            "arguments": { "namespace": "commands", "command": "documents_export_document" },
        }),
        &server,
        &mut dispatch,
    )
    .unwrap();
    assert_eq!(outcome["isError"], true);
    let text = outcome["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).expect("must stay valid JSON — never truncated");
    assert_eq!(parsed["error"], "result_too_large");
    assert_eq!(
        parsed["dispatched"], false,
        "must mirror every other Verb::Call refusal's own shape"
    );
    assert!(parsed["bytes"].as_u64().unwrap() > MCP_RESULT_MAX_BYTES as u64);
    let detail = parsed["detail"].as_str().unwrap();
    assert!(
        !detail.contains("agent call"),
        "must never hand the model a bypass recipe: {detail}"
    );
    assert_eq!(outcome["content"][1]["text"], "exitCode: 2");
}

#[test]
fn a_locally_refused_oversized_namespace_never_gets_echoed_back_in_full() {
    // item 17 — a local refusal (unknown_command here) used to return BEFORE any cap check, so an
    // oversized caller-chosen `namespace` reproduced the exact frame size the cap exists to bound.
    let server = Server::new(true, true);
    let huge_namespace = "n".repeat(MCP_RESULT_MAX_BYTES + 10);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({
            "name": "call-read",
            "arguments": { "namespace": huge_namespace, "command": "whatever" },
        }),
        &server,
        &mut dispatch,
    )
    .unwrap();
    assert_eq!(outcome["isError"], true);
    let text = outcome["content"][0]["text"].as_str().unwrap();
    assert!(
        text.len() < MCP_RESULT_MAX_BYTES,
        "must refuse instead of echoing the oversized namespace back verbatim: {} bytes",
        text.len()
    );
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["error"], "result_too_large");
}

/// Issue #1138, measured against the REAL cap rather than a copy of it: a
/// realistic one-page-résumé PDF (180 KB of file bytes — the repro's own
/// export was 259,841 B of JSON at 99.1% of the cap, so its raw payload was
/// around this size) serialized as a `number[]` blows [`MCP_RESULT_MAX_BYTES`],
/// and the same bytes base64'd fit comfortably under it.
///
/// Both sides are asserted, so this cannot pass for the wrong reason: if the
/// "before" ever stopped exceeding the cap, the premise this fix rests on
/// would be gone and the test says so instead of quietly still passing.
/// Anchored here, beside the constant, precisely so that lowering the cap
/// re-runs this arithmetic rather than silently invalidating it.
#[test]
fn base64_takes_a_realistic_pdf_export_from_over_the_result_cap_to_under_it() {
    let pdf_bytes: Vec<u8> = (0..180_000u32).map(|i| (i % 251) as u8).collect();
    let mut payload = json!({
        "data": pdf_bytes,
        "mimeType": "application/pdf",
        "filename": "resume.pdf",
    });

    let before = serde_json::to_string(&payload).unwrap().len();
    assert!(
        before > MCP_RESULT_MAX_BYTES,
        "premise: the number[] encoding must exceed the cap for this fix to be needed \
         ({before} B vs {MCP_RESULT_MAX_BYTES})"
    );

    agent_call::reshape::base64_byte_fields("documents_export_document", &mut payload);

    let after = serde_json::to_string(&payload).unwrap().len();
    assert!(
        after < MCP_RESULT_MAX_BYTES,
        "the base64 payload must fit the cap ({after} B vs {MCP_RESULT_MAX_BYTES})"
    );
    assert_eq!(payload["dataEncoding"], "base64");
}

// ── commands (local, no bridge) ──────────────────────────────────────────

#[test]
fn commands_filters_by_effect_and_never_touches_the_bridge() {
    let all = commands_value(&json!({}), Tier::Irreversible);
    let read_only = commands_value(&json!({ "effect": "read" }), Tier::Irreversible);
    let all_rows = all["commands"].as_array().unwrap().len();
    let read_rows = read_only["commands"].as_array().unwrap().len();
    assert!(read_rows > 0 && read_rows < all_rows);
    for row in read_only["commands"].as_array().unwrap() {
        assert_eq!(row["effect"], "read");
        assert_eq!(row["tool"], TOOL_CALL_READ);
    }
}

/// Issue #1136's discoverability half: a caller must be able to LEARN that
/// these two rows answer with an envelope and take `limit`/`cursor`, rather
/// than discovering it by receiving a shape it did not expect. Asserted in
/// both directions — the note appears on exactly the paged rows and on no
/// others — so a `returns` key leaking onto every row fails here too.
#[test]
fn commands_marks_the_paged_rows_and_only_those() {
    let all = commands_value(&json!({}), Tier::Irreversible);
    let mut noted: Vec<&str> = Vec::new();
    for row in all["commands"].as_array().unwrap() {
        let Some(returns) = row["returns"].as_str() else {
            continue;
        };
        assert_eq!(returns, agent_call::reshape::PAGINATED_LIST_NOTE);
        noted.push(row["command"].as_str().unwrap());
    }
    noted.sort_unstable();
    assert_eq!(
        noted,
        vec!["ai_generations_list", "applications_list", "documents_list"]
    );
}

#[test]
fn commands_names_the_right_tool_for_every_effect_class_with_all_flags_enabled() {
    // Pins the SECOND copy of the Effect→tool mapping (`tool_for`, used by both `commands_value`
    // and `local_call_refusal`) — the first copy was already covered by the `read`-only assertion
    // above, but nothing previously pinned `reversible`/`irreversible`/`not_exposed`.
    let all = commands_value(&json!({}), Tier::Irreversible);
    for row in all["commands"].as_array().unwrap() {
        match row["effect"].as_str().unwrap() {
            "read" => assert_eq!(row["tool"], TOOL_CALL_READ),
            "reversible" => assert_eq!(row["tool"], TOOL_CALL_REVERSIBLE),
            "irreversible" => assert_eq!(row["tool"], TOOL_CALL_IRREVERSIBLE),
            "not_exposed" => assert!(row.get("tool").is_none() && row.get("unavailable").is_none()),
            other => panic!("unexpected effect {other}"),
        }
    }
}

#[test]
fn commands_marks_irreversible_rows_unavailable_without_the_irreversible_flag() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Reversible);
    let rows = out["commands"].as_array().unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(
            row.get("tool").is_none(),
            "must not name a gated tool: {row}"
        );
        assert_eq!(
            row["unavailable"],
            "server started without --allow-irreversible"
        );
    }
}

#[test]
fn commands_marks_reversible_rows_unavailable_without_the_reversible_flag() {
    let out = commands_value(&json!({ "effect": "reversible" }), Tier::Read);
    let rows = out["commands"].as_array().unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(
            row.get("tool").is_none(),
            "must not name a gated tool: {row}"
        );
        assert_eq!(
            row["unavailable"],
            "server started without --allow-reversible"
        );
    }
}

#[test]
fn commands_with_an_unknown_effect_value_is_a_usage_error_not_a_silent_empty_success() {
    let server = Server::new(true, true);
    let mut dispatch = stub_ok;
    let outcome = tool_call_result(
        &json!({ "name": "commands", "arguments": { "effect": "bogus" } }),
        &server,
        &mut dispatch,
    )
    .unwrap();
    assert_eq!(
        outcome["isError"], true,
        "an unknown effect must not read as success"
    );
    assert_eq!(outcome["content"][1]["text"], "exitCode: 2");
    let text = outcome["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["error"], "usage");
}

#[test]
fn commands_with_a_non_string_effect_value_is_a_usage_error_for_every_json_type() {
    // item 20 — {"effect":5} (and bool/null/object) used to skip the old
    // `.and_then(Value::as_str)` gate entirely and answer with every row, isError:false.
    let server = Server::new(true, true);
    for bad in [json!(5), json!(true), Value::Null, json!({"nested": 1})] {
        let mut dispatch = stub_ok;
        let outcome = tool_call_result(
            &json!({ "name": "commands", "arguments": { "effect": bad.clone() } }),
            &server,
            &mut dispatch,
        )
        .unwrap();
        assert_eq!(
            outcome["isError"], true,
            "effect={bad:?} must not read as success"
        );
        let text = outcome["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["error"], "usage", "effect={bad:?}");
    }
}

#[test]
fn commands_names_the_proof_source_for_an_irreversible_row() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "ai_set_provider_key")
        .expect("ai_set_provider_key is a real Irreversible row");
    assert_eq!(row["proofFrom"], "ai:ai_has_provider_key");
    assert_eq!(row["proofInput"], "provider");
    assert!(
        row.get("proofInputValue").is_none(),
        "a FromCaller value is the caller's own input and must never be echoed: {row}"
    );
}

#[test]
fn commands_names_the_literal_proof_input_value_for_privacy_sign_out_all() {
    let out = commands_value(&json!({ "effect": "irreversible" }), Tier::Irreversible);
    let row = out["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["command"] == "privacy_sign_out_all")
        .expect("privacy_sign_out_all is a real Irreversible row with a Literal proof input");
    assert_eq!(row["proofInput"], "boardId");
    assert_eq!(
        row["proofInputValue"], "linkedin",
        "a Literal input's value is not secret and is the one thing this ceremony can't \
         otherwise complete from `commands` alone"
    );
}

// ── source hygiene guard (mutation-visible: adding any of these turns
// this test red immediately) ────────────────────────────────────────────

#[test]
fn mcp_source_never_prints_or_pretty_prints() {
    const SOURCE: &str = include_str!("../mcp.rs");
    for banned in ["println!(", "print!(", "to_string_pretty(", "eprintln!("] {
        assert!(
            !SOURCE.contains(banned),
            "mcp.rs must never call {banned} — see emit()'s own doc"
        );
    }
    assert_eq!(
        SOURCE.matches("stdout()").count(),
        1,
        "mcp.rs must call stdout() exactly once — see emit()'s own doc"
    );
}

// ── result_too_large is not a "just narrow it and retry" refusal ──

/// The sentinel is SHARED with the app-side frame cap
/// (`agent_call::Refusal::ResultTooLarge`), whose own detail says outright
/// that the command RAN. Both strings a client can see for this cause must
/// therefore carry the same warning: `dispatched:false` here means no result
/// was delivered, NOT that nothing happened, and re-sending a mutating call on
/// it would repeat a mutation that already took effect.
#[test]
fn both_result_too_large_texts_warn_that_the_command_may_already_have_run() {
    assert!(
        INSTRUCTIONS.contains("result_too_large"),
        "the instructions must still name the sentinel"
    );
    assert!(
        INSTRUCTIONS.contains("ALREADY HAVE RUN"),
        "INSTRUCTIONS must warn that the call may have taken effect: {INSTRUCTIONS}"
    );
    assert!(
        INSTRUCTIONS.contains("never re-send a mutating call"),
        "INSTRUCTIONS must say what NOT to do: {INSTRUCTIONS}"
    );

    let detail = oversized_result(999_999)["detail"]
        .as_str()
        .expect("detail is a string")
        .to_string();
    assert!(
        detail.contains("may already have run"),
        "the refusal itself must carry the warning, not only the instructions: {detail}"
    );
    assert!(
        detail.contains("do not re-send a mutating call"),
        "{detail}"
    );
}

/// The generic `input` schema is the only place a caller learns that `limit`
/// and `cursor` on a paged row are the PAGING LAYER's arguments —
/// `agent_call::take_list_page_args` strips them before dispatch, so a caller
/// that expects the target command to see them is wrong about the contract.
#[test]
fn the_generic_input_schema_says_limit_and_cursor_belong_to_the_paging_layer() {
    let tool = tools(Tier::Read)
        .into_iter()
        .find(|t| t["name"] == TOOL_CALL_READ)
        .expect("call-read is always present");
    let description = tool["inputSchema"]["properties"]["input"]["description"]
        .as_str()
        .expect("the input property carries a description")
        .to_string();
    for clause in [
        "limit",
        "cursor",
        "paging layer",
        "stripped before dispatch",
    ] {
        assert!(
            description.contains(clause),
            "the input description must state `{clause}`: {description}"
        );
    }
}

/// Round 5 (`B1-r1-ACLI-R5-1`): `updater:updater_check` writes `UpdaterState` and emits a UI
/// event, so it must stay `Effect::Reversible`, NOT `Read` — `call-read`'s `readOnlyHint` is a
/// per-TOOL promise covering every current and future `Read` row, and reclassifying one
/// side-effecting row into `Read` would force that promise to `false` for all the genuinely
/// read-only rows too. `updater::updater_status` is the read-only alternative (its own POLICY row
/// comment).
#[test]
fn updater_check_is_not_dispatchable_as_read() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "updater::updater_check")
        .expect("updater_check has a POLICY row");
    assert_eq!(
        entry.effect,
        Effect::Reversible,
        "updater_check writes UpdaterState + emits updater:status — it must not be Read, or \
         call-read's readOnlyHint would have to go false for every Read row"
    );
}

/// Companion to the test above: `call-read`'s own annotations must still claim `readOnlyHint:
/// true` now that no side-effecting row (`updater_check`) is classified `Read` — this is the
/// promise every genuinely read-only row (63 of them) depends on for auto-approval.
#[test]
fn call_read_annotations_claim_read_only() {
    let tool = tools(Tier::Read)
        .into_iter()
        .find(|t| t["name"] == TOOL_CALL_READ)
        .expect("call-read is always present");
    assert_eq!(
        tool["annotations"]["readOnlyHint"],
        json!(true),
        "call-read must claim readOnlyHint: true — every row it can dispatch is genuinely \
         side-effect-free on the persisted+in-memory axis: {tool}"
    );
    let description = tool_description(&tools(Tier::Read), TOOL_CALL_READ);
    assert!(
        description.contains("no state change"),
        "call-read's description must say \"no state change\": {description}"
    );
}

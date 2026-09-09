//! The stdio JSON-RPC transport — `agent mcp`'s default wire, split out from `mcp.rs` for the
//! same R8 LOC-cap reason `instructions.rs`/`schemas.rs`/`results.rs`/`resources.rs`/`prompts.rs`
//! already were, and the sibling unit to `mcp/http.rs`: that module owns the Streamable HTTP
//! wire, this one owns the three-thread stdio wire the module doc's "Concurrency" section
//! describes in full — [`serve`] is the loop itself, [`Event`]/[`emit`]/[`forget_in_flight`]/
//! [`stop_serving`] are its own bookkeeping. Reaches everything else in `mcp.rs` (`Server`,
//! `Routed`, `PendingKind`, `route_line`, `reply_frame`, `rpc_error`, `dispatched_tool_result`,
//! `resources::dispatched_resource_result`, `results::{busy_result, shutting_down_result,
//! tool_result}`, the `MCP_CALL_QUEUE_MAX`/`MCP_EVENT_QUEUE_MAX` bounds) through its own
//! `use super::*;`, never a second copy of any of it.

use super::*;

/// What the main thread selects over: input from the reader thread, replies from the worker
/// thread. One channel, two producers — so a reply and a line can never be missed for each other.
enum Event {
    Line(String),
    /// The input ended (EOF, or a read error, which ends it the same way).
    Eof,
    Reply(Value),
}

/// The sole stdout writer once the JSON-RPC loop is running (see the module doc's "Stdout/stderr
/// discipline" section) — a compact `Value`'s own `Display` (never a pretty-printed one), one
/// `writeln!` call. `Err` here (EPIPE once the client closes its end of the pipe) is the caller's
/// cue to stop, never retried and never a panic.
///
/// The flush is explicit rather than left to [`std::io::Stdout`]'s own line buffering: this
/// server is a request/response peer whose client blocks on a reply before sending its next
/// frame, so a frame still sitting in a buffer is a deadlock, not a latency detail — and the
/// handle we write through is chosen for `Send`-ness (module doc), not for its buffering
/// strategy, so this must not depend on which one it happens to be.
fn emit(output: &mut impl Write, frame: &Value) -> std::io::Result<()> {
    writeln!(output, "{frame}")?;
    output.flush()
}

/// Drop the id `frame` answers from the still-owed list, if it is there — the bookkeeping half of
/// the EOF guarantee (module doc). Removes ONE entry, so a client that reused an id across two
/// calls still has both tracked; a frame whose id matches nothing leaves the list untouched
/// rather than shortening it under a later, real reply.
fn forget_in_flight(in_flight: &mut Vec<(Value, PendingKind)>, frame: &Value) {
    let Some(id) = frame.get("id") else { return };
    if let Some(pos) = in_flight.iter().position(|(owed, _)| owed == id) {
        in_flight.remove(pos);
    }
}

/// The ONE way [`serve`] leaves early, and the reason it is a function: every exit that stops
/// WRITING must first stop DISPATCHING, in that order. The drain-deadline exit did; the
/// write-failure ones returned a bare `0`, leaving the worker free to spend a bridge round trip
/// — and, at a write tier, a real mutation — on a call whose reply can no longer be delivered,
/// which is precisely what `abandoned` exists to prevent. Returns the exit code so a call site
/// reads `return stop_serving(&abandoned);` and cannot set the flag without also stopping, or
/// stop without setting it. A dead pipe is still a CLEAN exit (spec: end promptly once the
/// client is gone), hence 0.
fn stop_serving(abandoned: &AtomicBool) -> i32 {
    abandoned.store(true, Ordering::SeqCst);
    0
}

/// The whole loop — generic over `dispatch` so it is directly testable over a
/// [`std::io::Cursor`] with a stub closure (no runtime, no socket, no live tool table beyond what
/// the test supplies). See the module doc for the guarantees this shape buys and what it costs.
/// EOF (or a read error) drains the queue and ends the loop, exit 0 (spec: exit promptly once
/// stdin closes); so does a failed write.
///
/// `drain_budget` is the WHOLE drain's budget once `Eof` arrives — one
/// [`super::INVOCATION_TIMEOUT`] in production, a few milliseconds in the tests that measure the
/// deadline itself. A parameter rather than a constant read here because a test that had to wait
/// out the real one would be a 90-second test nobody runs.
pub(super) fn serve(
    input: impl BufRead + Send + 'static,
    mut output: impl Write,
    server: &Server,
    mut dispatch: impl FnMut(&Verb) -> Result<Value, &'static str> + Send + 'static,
    drain_budget: Duration,
) -> i32 {
    // BOUNDED (module doc): its two PRODUCERS may block on it, the sole consumer — this loop —
    // never does, which is what makes a full queue backpressure rather than a deadlock.
    let (events, incoming) = sync_channel::<Event>(MCP_EVENT_QUEUE_MAX);
    // BOUNDED (module doc): the writer never blocks on it — a full queue is refused with
    // `server_busy` instead — so this bound is a memory bound, not a latency one. Carries
    // `PendingKind` alongside the `Verb` (issue #1146 P4) so the worker below can build the right
    // reply shape for a `resources/read` too, not only a `tools/call`.
    let (calls, queued) = sync_channel::<(Value, Verb, PendingKind)>(MCP_CALL_QUEUE_MAX);

    // Set when the drain deadline expires: whatever is still queued must not be dispatched, since
    // this loop has stopped reading replies and would spend a bridge round trip per call for a
    // frame nobody will ever write.
    let abandoned = Arc::new(AtomicBool::new(false));
    let worker_abandoned = Arc::clone(&abandoned);
    let worker_events: SyncSender<Event> = events.clone();
    let worker = thread::Builder::new()
        .name("mcp-dispatch".to_string())
        .spawn(move || {
            // FIFO by construction: one receiver, one thread, each call run to completion before
            // the next is taken — this is the "single-flight, in input order" guarantee. The
            // queue carries an ALREADY-CLASSIFIED [`Verb`], so this thread needs no [`Server`]
            // and can do nothing but dispatch.
            while let Ok((id, verb, kind)) = queued.recv() {
                // Checked per call, not once: dropping `calls` is not enough on its own, because
                // a `Receiver` keeps yielding what was ALREADY buffered after its sender is gone.
                //
                // Honest about what this costs and covers: in practice the `send` below is what
                // stops this thread, since the `Event` receiver dies with `serve` and the failed
                // send returns. This flag closes the window between the loop breaking and that
                // receiver actually being dropped — which is why removing it fails no test, and
                // why it is 3 lines rather than a mechanism.
                if worker_abandoned.load(Ordering::SeqCst) {
                    return;
                }
                // The one place `PendingKind` decides the reply SHAPE — the payload underneath is
                // built by the SAME `dispatch_payload` either way (issue #1146 P4).
                let result = match &kind {
                    PendingKind::Tool => dispatched_tool_result(&verb, &mut dispatch),
                    PendingKind::Resource(uri) => {
                        resources::dispatched_resource_result(uri, &verb, &mut dispatch)
                    }
                };
                let reply = reply_frame(id, Ok(result));
                // MAY BLOCK, and that is safe — a blocking `send` here can never stall the
                // writer's drain, because the writer never waits on THIS thread while the loop
                // runs: it hands work over with `try_send` (a full dispatch queue is refused, not
                // waited on) and reaches its one `worker.join()` only after the loop has broken
                // with nothing left in flight, i.e. after every reply this thread produced was
                // already received. So the writer's only waits are on its own consumer end and
                // on stdout, and both free slots here rather than needing one. `is_err` = the
                // receiver is gone (`serve` returned), the same stop signal as before.
                if worker_events.send(Event::Reply(reply)).is_err() {
                    return;
                }
            }
        });
    let Ok(worker) = worker else {
        // Pre-protocol in practice (nothing has been read yet), so stderr only — same shape as
        // `run`'s own runtime-build failure.
        let _ = writeln!(std::io::stderr(), "could not start the MCP dispatch thread");
        return 2;
    };

    let reader = thread::Builder::new()
        .name("mcp-reader".to_string())
        .spawn(move || {
            for line in input.lines() {
                // A read error ends the input exactly like EOF does.
                let Ok(line) = line else { break };
                // MAY BLOCK once [`MCP_EVENT_QUEUE_MAX`] events are waiting — deliberately. This
                // is the thread whose blocking costs nothing: parking here stops the reads, and
                // the OS pipe pushes back on the client exactly as it did before this loop had a
                // reader thread at all. The alternative (an unbounded queue) buffers a
                // never-blocking stdin — a file, or a client that pipelines faster than stdout
                // drains — without limit.
                if events.send(Event::Line(line)).is_err() {
                    return;
                }
            }
            let _ = events.send(Event::Eof);
        });
    if reader.is_err() {
        drop(calls);
        let _ = worker.join();
        let _ = writeln!(std::io::stderr(), "could not start the MCP reader thread");
        return 2;
    }
    // Its handle is deliberately DROPPED (detached), never joined: the reader is parked inside a
    // blocking `stdin` read that only a closed pipe ends, so joining it would be the very hang
    // this split exists to remove. It is already finished by the time `Eof` reaches the loop.
    drop(reader);

    let mut input_ended = false;
    // Calls handed to the worker that have not replied yet, in the order they were queued — EOF
    // may not end the loop until this is empty, or a reply the client is waiting for would be
    // dropped on the floor. The IDS, not a count — paired with the [`PendingKind`] each is owed a
    // reply IN, so an expired drain deadline can answer whatever is left in the right SHAPE. An
    // expired deadline has to answer everything still here, and single-flight FIFO order is what
    // makes the head of this list the only entry that can be running (see
    // [`shutting_down_result`]).
    let mut in_flight: Vec<(Value, PendingKind)> = Vec::new();
    // When the drain started. ONE deadline for the whole drain (module doc), not one per queued
    // call: `drain_budget` is measured from this instant no matter how many replies are still
    // owed. `None` until `Eof`, which is when the loop first has a reason to stop waiting.
    let mut drain_started: Option<std::time::Instant> = None;
    let mut drain_expired = false;
    loop {
        let event = match drain_started {
            None => match incoming.recv() {
                Ok(event) => event,
                Err(_) => break,
            },
            Some(started) => {
                // `checked_sub` is `None` once the budget is spent — never a panicking
                // subtraction, and never a negative timeout.
                let remaining = drain_budget.checked_sub(started.elapsed());
                let Some(remaining) = remaining.filter(|r| !r.is_zero()) else {
                    drain_expired = true;
                    break;
                };
                match incoming.recv_timeout(remaining) {
                    Ok(event) => event,
                    Err(RecvTimeoutError::Timeout) => {
                        drain_expired = true;
                        break;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        };
        match event {
            Event::Line(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match route_line(line, server) {
                    Routed::Drop => {}
                    Routed::Reply(frame) => {
                        if emit(&mut output, &frame).is_err() {
                            return stop_serving(&abandoned);
                        }
                    }
                    Routed::Call { id, verb, kind } => {
                        // Cloned BEFORE the send, which consumes `kind`: cheap (a bare enum, or
                        // one `String` for a resource's `uri`), and it's what lets `in_flight`
                        // carry the SAME kind the queued tuple does without reaching back into a
                        // channel that only yields values once.
                        let in_flight_kind = kind.clone();
                        match calls.try_send((id.clone(), verb, kind)) {
                            Ok(()) => in_flight.push((id, in_flight_kind)),
                            Err(TrySendError::Full((_, _, kind))) => {
                                // `try_send`, never `send`: blocking here would stall the ONE
                                // thread that answers pings and writes replies — the stall this
                                // split exists to remove — so the excess call is refused instead,
                                // in the SAME shape a successful dispatch of this `kind` would
                                // have answered in (issue #1146 P4).
                                let busy = match kind {
                                    PendingKind::Tool => tool_result(busy_result(), 2),
                                    PendingKind::Resource(uri) => {
                                        resources::resource_result(&uri, busy_result())
                                    }
                                };
                                let refusal = reply_frame(id, Ok(busy));
                                if emit(&mut output, &refusal).is_err() {
                                    return stop_serving(&abandoned);
                                }
                            }
                            Err(TrySendError::Disconnected(_)) => {
                                // The worker is gone (only reachable if its thread died, which
                                // under `panic = "abort"` it cannot). Answer anyway rather than
                                // leave the client waiting on a reply that can never come.
                                let _ =
                                    writeln!(std::io::stderr(), "the MCP dispatch thread is gone");
                                if emit(&mut output, &rpc_error(id, -32603, "Internal error"))
                                    .is_err()
                                {
                                    return stop_serving(&abandoned);
                                }
                            }
                        }
                    }
                }
            }
            Event::Reply(frame) => {
                // Retain-by-id, never a bare `- 1`: a reply is only ever produced for a call this
                // loop queued, but an id that somehow matched nothing must leave the list alone
                // rather than shorten it under a later, real reply.
                forget_in_flight(&mut in_flight, &frame);
                if emit(&mut output, &frame).is_err() {
                    return stop_serving(&abandoned);
                }
            }
            Event::Eof => {
                input_ended = true;
                drain_started = Some(std::time::Instant::now());
            }
        }
        if input_ended && in_flight.is_empty() {
            break;
        }
    }

    if drain_expired {
        // Tell the worker to stop before closing the queue: a `Receiver` still yields what was
        // buffered before its sender was dropped, so the flag — not `drop(calls)` — is what
        // actually stops the queued calls from dispatching. Setting it FIRST is also what makes
        // the answers below true: past this point nothing new can be dispatched, so an id still
        // unanswered after the sweep is one that never will be.
        abandoned.store(true, Ordering::SeqCst);
        drop(calls);
        // A reply the worker sent in the instant the deadline fired is still a real reply — take
        // whatever is already in the channel before deciding who is owed one. Non-blocking, so
        // this cannot re-open the wait the deadline just closed.
        while let Ok(event) = incoming.try_recv() {
            // Only a reply can still be in there: `Eof` is the last thing the reader ever sends
            // and this loop has already taken it, so no `Line` can be queued behind it.
            let Event::Reply(frame) = event else { continue };
            forget_in_flight(&mut in_flight, &frame);
            if emit(&mut output, &frame).is_err() {
                return stop_serving(&abandoned);
            }
        }
        // Every call the client is still waiting on gets an answer rather than silence (module
        // doc's EOF bullet). Head of the list first: it is the only one that can be in flight.
        // Shaped by its own `kind` (issue #1146 P4) — a queued resource read gets a `contents`
        // envelope here too, never a tool-shaped refusal for a call it never was.
        for (i, (id, kind)) in in_flight.iter().enumerate() {
            let payload = shutting_down_result(i == 0);
            let result = match kind {
                PendingKind::Tool => tool_result(payload, 2),
                PendingKind::Resource(uri) => resources::resource_result(uri, payload),
            };
            let refusal = reply_frame(id.clone(), Ok(result));
            if emit(&mut output, &refusal).is_err() {
                break;
            }
        }
        // Deliberately NOT joined: the worker may be inside a dispatch bounded only by
        // `INVOCATION_TIMEOUT`, and waiting that out is exactly what the deadline exists to
        // prevent. The thread is detached like the reader's, and the process exits.
        return 0;
    }
    // Both threads are finished or about to be: the reader sent `Eof` before returning, and the
    // worker's queue closes with `calls`. Joining keeps a test from leaking a thread per case;
    // a join error (a panicked thread) is nothing this path can act on.
    drop(calls);
    let _ = worker.join();
    0
}

//! Unit tests for `stream.rs`, split into this sibling file (R8 line-budget
//! split — the same precedent `answer_assist_tests.rs`/`anthropic_tests.rs`
//! set: when a module hits the LOC cap, its TEST module moves out, not its
//! production code, so the invariants documented beside that code stay put).
//!
//! Wired via `#[path = "stream_tests.rs"] mod tests;` in `stream.rs` — that
//! keeps this a CHILD module of `stream` in the module tree (identical to the
//! inline `#[cfg(test)] mod tests { ... }` block it came from), so `use
//! super::*` below still reaches every private item there, while this file's
//! own filename (ending `tests.rs`) excludes it from the architecture test's
//! R8 LOC cap and from R3/R6's non-test scans.

use super::super::assist_registry::JobCanceller;
use super::*;

fn as_text(m: Message) -> String {
    match m {
        Message::Text(t) => t.to_string(),
        other => panic!("expected a text frame, got {other:?}"),
    }
}

// `AssistStreamRegistry`'s own state-machine tests (register/take/
// unregister, CWE-639 isolation, cancel/cancel_all, the pre-registration
// cancel race, duplicate-reqId rejection) plus `start_and_register`'s
// tests now live in `assist_registry` (R8 split — see that module).

/// A tiny local copy of `assist_registry::tests::RecordingCanceller` —
/// duplicated (not shared) so this file's test module stays independent
/// of that module's own private test internals. Used only by the ONE
/// test below that needs to prove a cancel finds the Pending marker
/// `begin_or_reject_duplicate` leaves behind.
#[derive(Default)]
struct RecordingCanceller {
    cancelled: std::cell::RefCell<Vec<String>>,
}

impl JobCanceller for RecordingCanceller {
    fn cancel_job(&self, job_id: &str) {
        self.cancelled.borrow_mut().push(job_id.to_string());
    }
}

// ── begin_or_reject_duplicate (HIGH fix: pre-begin cancel-drop —
// `begin` must run synchronously, on the read loop's own thread, BEFORE
// `tokio::spawn` ever schedules the streaming task) ────────────────────

#[test]
fn begin_or_reject_duplicate_marks_pending_synchronously_before_any_task_runs() {
    // This whole test has no `.await` at all — `begin_or_reject_duplicate`
    // is a plain, non-async fn — so a `Some(gen)` return, and `contains`
    // reporting `true` immediately after, already proves `begin` ran on
    // the CALLER's thread, not deferred into whatever thread a spawned
    // task eventually runs on.
    let registry = AssistStreamRegistry::default();
    let (out_tx, _out_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    let r#gen =
        begin_or_reject_duplicate(&registry, "req-1", &out_tx).expect("a fresh reqId is accepted");
    assert!(
        registry.contains("req-1"),
        "begin must have run synchronously — before any spawn, before any await"
    );

    // The exact race this fix closes: a same-connection `assist.cancel`
    // dispatched right after `spawn_answer_assist` returns (before the
    // spawned task has run AT ALL) must still find the Pending marker,
    // never nothing.
    let canceller = RecordingCanceller::default();
    registry.cancel(&canceller, "req-1");
    assert!(
        canceller.cancelled.borrow().is_empty(),
        "no job exists yet — Pending just becomes CancelledEarly, nothing to job_cancel"
    );
    assert!(
        !registry.register("req-1", r#gen, "job-1"),
        "the cancel that raced ahead of the spawned task's own register call must still win"
    );
}

#[test]
fn begin_or_reject_duplicate_rejects_an_already_active_req_id_via_out_tx() {
    let registry = AssistStreamRegistry::default();
    registry.begin("req-1"); // the original request is already in flight
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    assert!(begin_or_reject_duplicate(&registry, "req-1", &out_tx).is_none());

    let frame = out_rx
        .try_recv()
        .expect("a duplicate-rejection reply must be enqueued through out_tx directly");
    let v: Value = serde_json::from_str(&as_text(frame)).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(
        v["payload"]["error"],
        super::super::answer_assist::DUPLICATE_REQUEST_MESSAGE
    );
    assert!(
        registry.contains("req-1"),
        "the ORIGINAL request's entry must be left untouched by the rejected duplicate"
    );
}

// `start_and_register`'s tests (TOCTOU fix — job_start before register)
// now live in `assist_registry` alongside the function itself.

// ── ChannelFrameSink / channel multiplexing (HIGH fix mechanism) ───────

#[tokio::test]
async fn a_slow_streaming_producer_never_blocks_a_concurrently_enqueued_frame() {
    // Mirrors the HIGH fix this module exists for: before, a streaming
    // handler was awaited INLINE in the read loop, so nothing else —
    // including a same-connection `assist.cancel` reply — could reach
    // the writer until it finished. Now every producer (the read loop
    // itself, and any spawned streaming task) enqueues through its OWN
    // `ChannelFrameSink` clone into the SAME channel; a slow producer
    // must never delay another producer's frame from being observed.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    let mut slow_sink = ChannelFrameSink(tx.clone());
    tokio::spawn(async move {
        for i in 0..3 {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            slow_sink.send_frame(format!("chunk-{i}")).await;
        }
        slow_sink.send_frame("done".to_string()).await;
    });

    // A concurrent fast frame — e.g. the read loop's own dispatch for a
    // synchronous verb, or an `assist.cancel` acknowledgement — enqueued
    // through its OWN sink immediately, before any of the slow
    // producer's sleeps elapse.
    let mut fast_sink = ChannelFrameSink(tx.clone());
    fast_sink.send_frame("fast-reply".to_string()).await;

    let first = rx.recv().await.unwrap();
    assert_eq!(
        as_text(first),
        "fast-reply",
        "the fast frame must never queue behind the slow stream"
    );

    for i in 0..3 {
        let msg = rx.recv().await.unwrap();
        assert_eq!(as_text(msg), format!("chunk-{i}"));
    }
    assert_eq!(as_text(rx.recv().await.unwrap()), "done");
}

// ── run_writer (HIGH fix: write-backpressure / stalled-peer runaway) ───

/// A sink whose `poll_ready` never resolves `Ready` — mirrors a
/// TCP-open-but-not-reading peer: the OS write buffer stays full
/// forever, so a plain `writer.send(msg).await` would otherwise hang
/// this task indefinitely, with nothing ever erroring. Zero fields, so
/// it is `Unpin` automatically.
struct StalledSink;

impl futures::Sink<Message> for StalledSink {
    type Error = std::io::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Pending
    }

    fn start_send(self: std::pin::Pin<&mut Self>, _item: Message) -> Result<(), Self::Error> {
        unreachable!("poll_ready never resolves Ready, so start_send is never reached")
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Pending
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Pending
    }
}

#[tokio::test(start_paused = true)]
async fn run_writer_breaks_the_loop_once_a_write_stalls_past_write_stall() {
    // Mirrors the HIGH fix this closes: before, an unbounded channel plus
    // a peer that keeps the socket open but never reads meant
    // `writer.send(msg).await` parked forever — nothing ever errored, so
    // the receiver never dropped, `send_frame` kept reporting success,
    // and `forward_chunk` kept enqueueing frames for a consumer that
    // would never read them.
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    tx.send(Message::text("hello")).unwrap();

    let writer_task = tokio::spawn(run_writer(StalledSink, rx));

    // Let the spawned task actually run once, so its `WRITE_STALL`
    // timer registers with the (paused) clock before we advance past it.
    tokio::task::yield_now().await;
    tokio::time::advance(WRITE_STALL + std::time::Duration::from_millis(1)).await;

    writer_task
        .await
        .expect("run_writer must return, not panic, once its write stalls out");

    // The receiver `run_writer` owned is dropped once its loop breaks —
    // the NEXT `send_frame` on this same channel must now report the
    // sink gone, funneling into the EXISTING `SinkGone` → `job_cancel`
    // path unchanged (no new cancellation mechanism).
    assert!(
        !ChannelFrameSink(tx)
            .send_frame("after-stall".to_string())
            .await,
        "a subsequent send_frame must return false once run_writer's receiver is dropped"
    );
}

// ── next_step (CodeRabbit fix: propagate the writer-timeout into
// connection teardown — a DETACHED `run_writer` ending must not go
// unnoticed by the read loop until its own next inbound frame, which may
// never arrive) ─────────────────────────────────────────────────────────

#[tokio::test]
async fn next_step_reports_writer_ended_without_waiting_on_a_never_resolving_reader() {
    // Mirrors a stalled-but-open (or quiet/idle) connection: `reader_next`
    // here NEVER resolves — a real `reader.next()` on such a connection
    // would behave identically (no frame ever arrives). The writer future
    // resolves IMMEDIATELY (mirrors `run_writer`'s `JoinHandle` completing
    // once its `WRITE_STALL` timeout fires). This test completing at all
    // — rather than hanging forever — is the proof: `next_step` did not
    // block on the never-resolving reader, so the connection tears down
    // immediately instead of waiting indefinitely for a frame that may
    // never come.
    let reader_next = std::future::pending::<Option<i32>>();
    let writer_done = std::future::ready(());

    let outcome = next_step(reader_next, writer_done, never_revoked()).await;

    assert!(
        matches!(outcome, NextStep::WriterEnded),
        "the writer ending must win the race even though the reader never resolves"
    );
}

/// A revoke receiver that never fires — the shape of a healthy connection
/// whose pairing token is not being rotated.
fn never_revoked() -> std::future::Pending<Result<(), tokio::sync::broadcast::error::RecvError>> {
    std::future::pending()
}

#[tokio::test]
async fn next_step_reports_revoked_on_a_quiet_connection() {
    // A token rotation must reach an IDLE, healthy connection at once — a
    // paired browser that sends nothing (the normal state between clicks)
    // would otherwise never learn its pairing died. Both other arms here
    // never resolve, so this test completing at all is the proof.
    let reader_next = std::future::pending::<Option<i32>>();
    let writer_done = std::future::pending::<()>();

    let outcome = next_step(reader_next, writer_done, std::future::ready(Ok(()))).await;

    assert!(
        matches!(outcome, NextStep::Revoked),
        "a revoke must win against a quiet reader and a healthy writer"
    );
}

#[tokio::test]
async fn next_step_treats_a_lagged_receiver_as_a_revoke() {
    // A connection busy in a long dispatch await can miss the ring slot. A
    // `Lagged` receiver still means "a rotation happened while you weren't
    // looking" — silently skipping it would strand exactly the socket that
    // was too busy to notice its pairing died.
    use tokio::sync::broadcast::error::RecvError;
    let outcome = next_step(
        std::future::pending::<Option<i32>>(),
        std::future::pending::<()>(),
        std::future::ready(Err(RecvError::Lagged(3))),
    )
    .await;

    assert!(
        matches!(outcome, NextStep::Revoked),
        "a missed (lagged) rotation signal must revoke, never be skipped"
    );
}

#[tokio::test]
async fn next_step_never_revokes_when_the_channel_merely_closed() {
    // THE regression guard: a closed channel (app shutdown, or a refactor
    // that stops holding `revoke_tx`) is NOT a revocation. Mapping it to
    // `Revoked` would send `token.revoked` to every paired browser at once
    // and mass-unpair the install on a channel-lifecycle change.
    use tokio::sync::broadcast::error::RecvError;
    let outcome = next_step(
        std::future::pending::<Option<i32>>(),
        std::future::pending::<()>(),
        std::future::ready(Err(RecvError::Closed)),
    )
    .await;

    assert!(
        matches!(outcome, NextStep::RevokeWatchLost),
        "a closed revoke channel must tear down WITHOUT revoking the pairing"
    );
}

#[tokio::test]
async fn next_step_still_reports_a_frame_when_the_writer_is_still_alive() {
    // The normal case, unaffected by this fix: the writer task is still
    // running (never resolves in this test), so a frame arriving must
    // still be reported through — the writer race must never swallow or
    // delay a normal inbound frame while the writer is healthy.
    let reader_next = std::future::ready(Some(7));
    let writer_done = std::future::pending::<()>();

    let outcome = next_step(reader_next, writer_done, never_revoked()).await;

    let NextStep::Frame(value) = outcome else {
        panic!("expected NextStep::Frame — the writer must never win while a frame is ready");
    };
    assert_eq!(value, Some(7));
}

// ── `agent_query_or_cancelled` (MAJOR fix — security review round 2):
// an in-flight `agent.query` must never send its reply once this
// connection's cancellation token has fired — see `spawn_agent_query`'s
// doc for the token-revocation scenario this closes. ────────────────────

#[tokio::test]
async fn agent_query_or_cancelled_suppresses_the_reply_once_cancelled() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    // The query future here never resolves — proof this doesn't wait for
    // it once `cancel` has already fired. Bounded well past any
    // reasonable budget so a regression that ignores `cancel` hangs this
    // test instead of the whole suite.
    let outcome = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        agent_query_or_cancelled(std::future::pending::<String>(), &cancel),
    )
    .await;
    assert_eq!(
        outcome.ok(),
        Some(None),
        "a cancelled connection must suppress the query's reply, never send it"
    );
}

#[tokio::test]
async fn agent_query_or_cancelled_returns_the_reply_when_never_cancelled() {
    // The normal case, unaffected by this fix: an un-cancelled
    // connection must still deliver the query's own result unchanged.
    let cancel = CancellationToken::new();
    let outcome =
        agent_query_or_cancelled(std::future::ready("agent.result".to_string()), &cancel).await;
    assert_eq!(outcome, Some("agent.result".to_string()));
}

#[tokio::test]
async fn agent_query_or_cancelled_races_a_cancel_that_fires_mid_flight() {
    // A cancel arriving WHILE the query is still in flight (not already
    // cancelled before the race even starts) — the realistic timing for
    // a token revoked mid-`best-matches`.
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        cancel_clone.cancel();
    });
    let outcome = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        agent_query_or_cancelled(std::future::pending::<String>(), &cancel),
    )
    .await;
    assert_eq!(
        outcome.ok(),
        Some(None),
        "a cancel that fires mid-flight must still suppress the reply"
    );
}

// ── streaming: forwardable_delta / assist frame builders ────────────────

fn stream_chunk(delta: &str, done: bool, thinking: Option<bool>) -> AiStreamChunk {
    AiStreamChunk {
        job_id: "job-1".to_string(),
        delta: delta.to_string(),
        done,
        error: None,
        thinking,
    }
}

#[test]
fn forwardable_delta_forwards_a_plain_text_delta() {
    let chunk = stream_chunk("Because I ", false, None);
    assert_eq!(forwardable_delta(&chunk), Some("Because I "));
}

#[test]
fn forwardable_delta_skips_the_terminal_done_piece() {
    let chunk = stream_chunk("", true, None);
    assert_eq!(forwardable_delta(&chunk), None);
}

#[test]
fn forwardable_delta_skips_a_thinking_piece() {
    // A reasoning/thinking delta must never leak into the popup's
    // streaming preview — only the visible answer streams.
    let chunk = stream_chunk("pondering…", false, Some(true));
    assert_eq!(forwardable_delta(&chunk), None);
}

#[test]
fn forwardable_delta_skips_an_empty_delta() {
    let chunk = stream_chunk("", false, Some(false));
    assert_eq!(forwardable_delta(&chunk), None);
}

// ── forward_chunk (MEDIUM fix: live DRAFT_CAP enforcement; HIGH fix:
// dead-sink detection) ───────────────────────────────────────────────────

#[derive(Default)]
struct RecordingSink {
    sent: Vec<String>,
}

#[async_trait::async_trait]
impl FrameSink for RecordingSink {
    async fn send_frame(&mut self, text: String) -> bool {
        self.sent.push(text);
        true
    }
}

/// A sink whose transport is already gone — `send_frame` always reports
/// `false`, mirroring a disconnected client's outbound channel.
struct DeadSink;

#[async_trait::async_trait]
impl FrameSink for DeadSink {
    async fn send_frame(&mut self, _text: String) -> bool {
        false
    }
}

#[tokio::test]
async fn forward_chunk_stops_growing_accumulated_once_the_draft_cap_is_reached() {
    let mut sink = RecordingSink::default();
    let mut accumulated = String::new();

    // A single delta that exactly fills the cap.
    let cap = super::super::answer_assist::DRAFT_CAP;
    let first = stream_chunk(&"a".repeat(cap), false, None);
    let capped = forward_chunk(&first, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(capped, ForwardOutcome::CapReached);
    assert_eq!(accumulated.chars().count(), cap);

    // A second delta arriving after the cap must never grow the buffer
    // or send another frame.
    let second = stream_chunk("more text", false, None);
    let capped_again = forward_chunk(&second, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(capped_again, ForwardOutcome::CapReached);
    assert_eq!(
        accumulated.chars().count(),
        cap,
        "must never exceed the cap"
    );
    assert_eq!(
        sink.sent.len(),
        1,
        "the second delta must never be forwarded on the wire"
    );
}

/// A delta that would cross the cap is cut at the boundary, not dropped
/// whole. `cap_base = 0` is a single-attempt request (or attempt 1 of a
/// retried one); the REBASED window a retry gets is driven through this
/// same function by `answer_assist`'s
/// `compose_with_length_retry_*_draft_cap` tests.
#[tokio::test]
async fn forward_chunk_clamps_a_delta_that_would_cross_the_cap_mid_chunk() {
    let mut sink = RecordingSink::default();
    let cap = super::super::answer_assist::DRAFT_CAP;
    let mut accumulated = "x".repeat(cap - 5);

    // 10 chars incoming, only 5 fit before the cap.
    let chunk = stream_chunk("0123456789", false, None);
    let capped = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, 0).await;

    assert_eq!(capped, ForwardOutcome::CapReached);
    assert_eq!(accumulated.chars().count(), cap);
    assert_eq!(
        sink.sent.last().unwrap(),
        &assist_chunk_frame("req-1", "01234")
    );
}

/// A retry's window starts where its OWN text starts (`compose_attempts`
/// snapshots it), so a first attempt that forwarded
/// inline-`<think>` prose right up to the cap cannot leave the retry a stub:
/// same buffer, fresh budget.
///
/// Mutation checks (both executed): count the whole buffer instead of the
/// slice past `cap_base` (the pre-fix shape) and the first pair fails
/// (`CapReached`, and a 5-char frame instead of the whole 10-char delta);
/// ignore what the attempt already spent (`spent` returns 0 whenever
/// `cap_base > 0`) and the second pair fails — the attempt forwards `cap + 10`.
#[tokio::test]
async fn forward_chunk_gives_each_attempt_a_full_cap_past_cap_base() {
    let mut sink = RecordingSink::default();
    let cap = super::super::answer_assist::DRAFT_CAP;
    // Attempt 1 spent all but 5 chars of a cap; the retry's window starts here.
    let mut accumulated = "x".repeat(cap - 5);
    let cap_base = accumulated.len();

    let chunk = stream_chunk("0123456789", false, None);
    let outcome = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, cap_base).await;

    assert_eq!(
        outcome,
        ForwardOutcome::Continue,
        "10 chars is nowhere near the retry's own cap"
    );
    assert_eq!(
        sink.sent.last().unwrap(),
        &assist_chunk_frame("req-1", "0123456789"),
        "the retry's delta must reach the client whole"
    );

    // …and that window is still ONE cap, accumulated across the attempt's own
    // deltas: a second delta that would cross it is clamped, not waved through.
    let flood = stream_chunk(&"z".repeat(cap), false, None);
    let outcome = forward_chunk(&flood, "req-1", &mut sink, &mut accumulated, cap_base).await;

    assert_eq!(outcome, ForwardOutcome::CapReached);
    assert_eq!(
        accumulated.chars().count() - (cap - 5),
        cap,
        "the attempt forwarded exactly its own cap, never more"
    );
}

#[tokio::test]
async fn forward_chunk_reports_uncapped_while_under_the_limit() {
    let mut sink = RecordingSink::default();
    let mut accumulated = String::new();
    let chunk = stream_chunk("short delta", false, None);
    let capped = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(capped, ForwardOutcome::Continue);
    assert_eq!(accumulated, "short delta");
    assert_eq!(sink.sent, vec![assist_chunk_frame("req-1", "short delta")]);
}

#[tokio::test]
async fn forward_chunk_reports_sink_gone_when_send_frame_returns_false() {
    let mut sink = DeadSink;
    let mut accumulated = String::new();
    let chunk = stream_chunk("hello", false, None);
    let outcome = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(outcome, ForwardOutcome::SinkGone);
    assert_eq!(
        accumulated, "hello",
        "the delta is still accumulated locally even though the wire send failed"
    );
}

#[tokio::test]
async fn forward_chunk_never_reports_sink_gone_once_already_capped() {
    // Once the cap is reached, forward_chunk short-circuits before ever
    // touching the sink again — a dead sink discovered only AFTER the
    // cap must never surface, since there's nothing left to send.
    let mut sink = DeadSink;
    let cap = super::super::answer_assist::DRAFT_CAP;
    let mut accumulated = "x".repeat(cap);
    let chunk = stream_chunk("more", false, None);
    let outcome = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(outcome, ForwardOutcome::CapReached);
}

#[test]
fn assist_chunk_frame_carries_the_delta_under_the_reqs_id() {
    let frame = assist_chunk_frame("req-9", "Because I ");
    let v: Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(v["type"], msg::ASSIST_CHUNK);
    assert_eq!(v["reqId"], "req-9");
    assert_eq!(v["payload"]["delta"], "Because I ");
}

#[test]
fn assist_done_frame_carries_no_payload() {
    let frame = assist_done_frame("req-9");
    let v: Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(v["type"], msg::ASSIST_DONE);
    assert_eq!(v["reqId"], "req-9");
    assert!(v["payload"].is_null());
}

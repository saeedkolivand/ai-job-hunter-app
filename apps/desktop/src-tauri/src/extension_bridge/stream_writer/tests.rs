use super::*;

use super::super::stream::{ChannelFrameSink, FrameSink};

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

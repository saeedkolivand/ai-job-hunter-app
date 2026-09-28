//! The connection read/write race primitives — split from `stream.rs` (R8 relief): [`run_writer`]
//! is the ONE task that ever writes to a connection's live WS sink, and [`next_step`] is the
//! `tokio::select!` race `handle_connection`'s read loop runs every iteration (reader vs. writer-
//! task-ended vs. token-revoked). See `stream`'s own module doc for the whole streaming design.

use futures::SinkExt;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_tungstenite::tungstenite::Message;

/// How long a single `writer.send(msg).await` inside [`run_writer`] may take
/// before that peer is treated as stalled and the write loop breaks — see
/// `run_writer`'s doc for the exact failure this closes. Chosen generously so
/// a merely-slow-but-alive client is never killed (25s is well past any
/// realistic Wi-Fi/mobile round-trip hiccup).
///
/// **Verified before picking this** — no existing redundant layer to lean on
/// instead: `handle_connection`'s `WebSocketConfig` only sets
/// `max_message_size`/`max_frame_size`; nothing in `extension_bridge`
/// configures a periodic keepalive ping or any other read/write deadline on
/// this socket. tungstenite auto-replies to a received `Ping` with a `Pong`,
/// but neither tungstenite nor this module ever ORIGINATES its own periodic
/// ping to notice a peer that has simply stopped reading without closing —
/// so this timeout is the ONLY thing that ever detects that case, not a
/// belt-and-braces duplicate of something else already watching for it.
const WRITE_STALL: std::time::Duration = std::time::Duration::from_secs(25);

/// The ONE task that ever writes to the live WS sink for a connection. Every
/// outbound frame — handshake replies, synchronous verb replies, and every
/// streaming `assist.chunk`/`assist.done`/terminal reply from a
/// concurrently-running [`spawn_answer_assist`] task — funnels through `rx`,
/// so `handle_connection`'s read loop never itself awaits a socket write.
///
/// Exits once every sender clone is dropped (the read loop's own sender AND
/// every spawned streaming task's clone), OR once a single write stalls past
/// [`WRITE_STALL`]. The latter closes a real hole: a peer that keeps the TCP
/// connection open but stops reading parks a plain `writer.send(msg).await`
/// forever with nothing else ever erroring — the channel's receiver stays
/// alive, so `ChannelFrameSink::send_frame` keeps reporting success and
/// `forward_chunk` keeps enqueueing `assist.chunk` frames into the unbounded
/// channel for a consumer that will never read them (unbounded memory growth,
/// plus a billable generation left running all the way to its cap for
/// nobody). On timeout this loop breaks exactly like a closed-receiver error
/// would: `writer`/`rx` are dropped, so the NEXT `send_frame` on this
/// connection's channel returns `false` and the EXISTING `SinkGone` →
/// `job_cancel` path (see [`compose_draft_stream`]) fires unchanged — no new
/// cancellation mechanism, just an upper bound on how long a stalled write
/// can go undetected.
///
/// Generic over `S` (rather than the concrete
/// `SplitSink<WebSocketStream<TcpStream>, Message>`) purely so this is
/// unit-testable against a fake sink whose `poll_ready` never resolves,
/// without a live socket. Its OWN generation logic is still fire-and-forget
/// (a streaming task that never finishes can never hang the connection's own
/// cleanup) — but this function's `JoinHandle` itself is no longer purely
/// dropped: `handle_connection`'s read loop keeps it and races it via
/// [`next_step`], so this task ending (either `break` above) tears the
/// connection down immediately instead of going unnoticed.
pub(super) async fn run_writer<S>(mut writer: S, mut rx: UnboundedReceiver<Message>)
where
    S: SinkExt<Message> + Unpin,
{
    while let Some(msg) = rx.recv().await {
        match tokio::time::timeout(WRITE_STALL, writer.send(msg)).await {
            Ok(Ok(())) => continue,
            Ok(Err(_)) => break,
            Err(_) => {
                // Distinct from a genuine socket send error (the arm above) —
                // this is a peer that never errored at all, just stopped
                // reading. Worth its own log line so a stalled-peer teardown
                // is diagnosable in the field, not indistinguishable from a
                // normal disconnect.
                log::debug!(
                    "[extension_bridge] run_writer: write stalled past {WRITE_STALL:?} — \
                     treating the peer as gone and closing this connection's writer"
                );
                break;
            }
        }
    }
}

/// Outcome of one [`next_step`] race — see its doc.
pub(super) enum NextStep<T> {
    /// A frame arrived off the read side (`None` = the stream ended naturally,
    /// same as `reader.next()`'s own `None`).
    Frame(T),
    /// The writer task ended FIRST — a [`WRITE_STALL`] timeout, or a genuine
    /// socket send error (see [`run_writer`]). Nothing can ever reach this
    /// client again; the caller should tear its connection down immediately
    /// rather than keep waiting for a next inbound frame that may never
    /// arrive.
    WriterEnded,
    /// The pairing token was rotated (Settings → "Regenerate", or a factory
    /// reset) while this connection was live — see
    /// [`super::BridgeState::regenerate_token`]. Every socket that existed at
    /// rotation time gets this; the caller tells an AUTHENTICATED one
    /// `token.revoked` (never an unauthenticated one — that would be a token
    /// oracle) and tears the connection down either way.
    Revoked,
    /// The revocation channel closed — its sender was dropped (app shutdown, or
    /// a refactor that stops holding `BridgeState`'s `revoke_tx`). Distinct from
    /// [`NextStep::Revoked`] ON PURPOSE: the caller tears this connection down
    /// but sends NO `token.revoked`, so a channel-lifecycle change can never
    /// unpair every paired browser at once.
    RevokeWatchLost,
}

/// The exact `tokio::select!` race `handle_connection`'s read loop runs every
/// iteration: `reader_next` (in production, `reader.next()`) against
/// `writer_done` (in production, `&mut writer_task`, [`run_writer`]'s own
/// `JoinHandle`) — whichever resolves first wins. CodeRabbit finding: a
/// DETACHED `run_writer` task ending (its [`WRITE_STALL`] timeout, or a send
/// error) used to go unnoticed by the read loop until ITS OWN next inbound
/// frame — which, for a stalled-but-open peer or a quiet/idle connection, may
/// never come — so `cancel_all`/`dec_connected` stayed delayed
/// indefinitely. Racing the writer handle here closes that: the writer ending
/// now tears the connection down immediately, the SAME way a read error does.
///
/// Generic over both futures (not the concrete `WebSocketStream`/`JoinHandle`
/// types) so this race's OUTCOME is unit-testable without a live socket/
/// `AppHandle` (this crate has no `tauri::test` mock-app harness): a fake
/// "never resolves" reader racing an already-resolved writer proves the
/// `WriterEnded` arm wins without ever blocking on the reader, and vice
/// versa. Both real-world arms are cancel-safe (`futures::StreamExt::next`
/// and `tokio::task::JoinHandle` both are — `tokio::select!` may drop either
/// branch on any iteration without losing a frame or a writer-task
/// completion), so re-entering this fresh every loop iteration is safe.
///
/// `revoked` (in production, this connection's
/// [`super::BridgeState::subscribe_revoke`] receiver) is the third arm: a token
/// rotation must reach a QUIET connection immediately — the whole point of the
/// revoke is that the paired browser learns to re-pair, and an idle socket
/// sends nothing that would otherwise wake this loop. `broadcast::Receiver::recv`
/// is cancel-safe like the other two.
pub(super) async fn next_step<R, W, V>(
    reader_next: R,
    writer_done: W,
    revoked: V,
) -> NextStep<R::Output>
where
    R: std::future::Future,
    W: std::future::Future,
    V: std::future::Future<Output = Result<(), tokio::sync::broadcast::error::RecvError>>,
{
    use tokio::sync::broadcast::error::RecvError;
    tokio::select! {
        frame = reader_next => NextStep::Frame(frame),
        _ = writer_done => NextStep::WriterEnded,
        signal = revoked => match signal {
            // A rotation happened. `Lagged` means we missed one or more while
            // this connection was busy — still "your pairing is gone", so it
            // gets the SAME treatment (never a silent skip).
            Ok(()) | Err(RecvError::Lagged(_)) => NextStep::Revoked,
            // The sender is gone (shutdown, or a refactor that drops
            // `BridgeState`'s `revoke_tx`). This is emphatically NOT a
            // revocation: mapping it to `Revoked` would tell EVERY paired
            // browser its token died and mass-unpair the install on a channel
            // bookkeeping change. Tear this connection down without ever
            // sending the frame.
            Err(RecvError::Closed) => NextStep::RevokeWatchLost,
        },
    }
}

#[cfg(test)]
mod tests;

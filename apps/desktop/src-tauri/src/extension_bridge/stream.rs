//! The streaming relay — decouples `handle_connection`'s read loop from a
//! multi-second streaming handler (`answer.assist`) so a same-connection
//! `assist.cancel` can still be read and dispatched while a stream is in
//! flight.
//!
//! ## The bug this closes
//! The streaming `answer.assist` handler used to be awaited INLINE in the
//! read loop's match arm, so `reader.next()` was never polled again until it
//! finished — the same (only) connection could not read its own
//! `assist.cancel` mid-stream.
//!
//! ## The design
//! Every outbound frame for a connection — handshake replies, synchronous
//! verb replies, AND a concurrently-running streaming handler's
//! `assist.chunk`/`assist.done`/terminal reply — funnels through ONE
//! `tokio::sync::mpsc` channel that [`run_writer`] drains into the live WS
//! sink. `handle_connection`'s read loop never itself awaits a socket write;
//! it only enqueues (a synchronous, non-blocking channel `send`), so a
//! streaming `answer.assist` handler can be [`spawn_answer_assist`]ed onto
//! its OWN task and run fully concurrently with the loop continuing to poll
//! `reader.next()` — including reading THIS stream's own `assist.cancel`.
//! [`ChannelFrameSink`] is the [`super::FrameSink`] a spawned handler writes
//! through. Ordering per `reqId` (chunks, then `assist.done`, then the
//! terminal reply) is preserved because ONE task produces all of them, in
//! that order, into a FIFO channel — interleaving across DIFFERENT `reqId`s
//! is fine, since every frame carries its own `reqId` for correlation.
//!
//! ## Cancellation is per-connection (CWE-639 fix)
//! [`AssistStreamRegistry`] is created FRESH per connection in
//! `handle_connection` (never a field on the global `BridgeState`), so a
//! second authenticated connection's `assist.cancel` can never even NAME a
//! stream this connection started — there is structurally no shared map to
//! look it up in (an insecure direct object reference via a client-chosen
//! `reqId`, the prior design's bug: the registry lived globally on
//! `BridgeState`, shared by every socket).

use std::collections::HashMap;
use std::sync::Arc;

use futures::SinkExt;
use parking_lot::Mutex;
use serde_json::Value;
use tauri::AppHandle;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

/// A destination for streamed reply frames — abstracts the live WS writer so
/// the streaming `answer.assist` core ([`super::answer_assist`]) is
/// unit-testable against an in-memory recorder, without a live socket.
/// `send_frame` takes already-serialized JSON text (mirroring
/// `FrameDecision::Reply`'s shape) rather than a typed frame, so this trait
/// doesn't need to know about any particular wire message. Re-exported as
/// `super::FrameSink` (see `mod.rs`) so sibling modules keep referring to it
/// by that path.
#[async_trait::async_trait]
pub(crate) trait FrameSink: Send {
    /// Send one frame's raw JSON text. `false` = the transport is gone — the
    /// caller should stop sending further frames for this stream (but may
    /// still finish its own bookkeeping).
    async fn send_frame(&mut self, text: String) -> bool;
}

/// A [`FrameSink`] over a connection's outbound-frame channel. Lets a task
/// running OFF the read loop (a spawned streaming handler) enqueue frames
/// without ever touching the live WS writer directly — [`run_writer`] is the
/// only thing that ever calls the real socket's `send`.
pub(super) struct ChannelFrameSink(pub(super) UnboundedSender<Message>);

#[async_trait::async_trait]
impl FrameSink for ChannelFrameSink {
    async fn send_frame(&mut self, text: String) -> bool {
        self.0.send(Message::text(text)).is_ok()
    }
}

/// The ONE task that ever writes to the live WS sink for a connection. Every
/// outbound frame — handshake replies, synchronous verb replies, and every
/// streaming `assist.chunk`/`assist.done`/terminal reply from a
/// concurrently-running [`spawn_answer_assist`] task — funnels through `rx`,
/// so `handle_connection`'s read loop never itself awaits a socket write.
/// Exits once every sender clone is dropped (the read loop's own sender AND
/// every spawned streaming task's clone). Fire-and-forget, like every other
/// spawn in this module — never explicitly joined, so a streaming task that
/// never finishes can never hang the connection's own cleanup.
pub(super) async fn run_writer(
    mut writer: futures::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
    mut rx: UnboundedReceiver<Message>,
) {
    while let Some(msg) = rx.recv().await {
        if writer.send(msg).await.is_err() {
            break;
        }
    }
}

/// Drive a streaming `answer.assist` request on its OWN task, decoupled from
/// the connection's read loop — see the module doc. The read loop's dispatch
/// for `FrameDecision::AnswerAssist` calls this and moves on immediately (no
/// reply is returned inline); the terminal `answer.assist.result` is sent
/// from INSIDE this task once `handle_answer_assist` resolves, through the
/// SAME channel as every `assist.chunk`/`assist.done` it already sent, so
/// per-`reqId` ordering (chunks, then done, then result) holds.
pub(super) fn spawn_answer_assist(
    app: AppHandle,
    req_id: String,
    payload: Value,
    out_tx: UnboundedSender<Message>,
    registry: Arc<AssistStreamRegistry>,
) {
    tokio::spawn(async move {
        let mut sink = ChannelFrameSink(out_tx.clone());
        let reply = super::answer_assist::handle_answer_assist(
            &app, &req_id, &payload, &registry, &mut sink,
        )
        .await;
        let _ = out_tx.send(Message::text(reply));
    });
}

/// Per-connection registry of in-flight streaming `answer.assist` jobs
/// (`reqId -> jobId`) — deliberately scoped to ONE connection. See the
/// module doc's "Cancellation is per-connection" section.
#[derive(Default)]
pub(super) struct AssistStreamRegistry(Mutex<HashMap<String, String>>);

impl AssistStreamRegistry {
    /// Register an in-flight stream's `reqId -> jobId`. A re-registration
    /// under the same `reqId` replaces the prior mapping (a plain HashMap
    /// insert — never duplicates).
    pub(super) fn register(&self, req_id: &str, job_id: &str) {
        self.0.lock().insert(req_id.to_string(), job_id.to_string());
    }

    /// Forget a stream's registration once it ends (success, failure, or an
    /// already-raced cancel). A no-op when `req_id` is already gone — never
    /// an error.
    pub(super) fn unregister(&self, req_id: &str) {
        self.0.lock().remove(req_id);
    }

    /// Remove + return the job registered under `req_id` on THIS registry —
    /// pure (no `AppHandle`), so the security-relevant property is directly
    /// unit-testable: `None` when this registry never held `req_id` — never
    /// registered here, already finished, OR (the CWE-639 case this type
    /// exists to close) it belongs to a DIFFERENT connection's registry.
    pub(super) fn take(&self, req_id: &str) -> Option<String> {
        self.0.lock().remove(req_id)
    }

    /// Cancel the in-flight stream named by `req_id` on THIS registry, if
    /// any — marks its job cancelled via the shared `JobTracker` (the SAME
    /// mechanism `chat_stream`'s `is_cancelled` polls every chunk, so the
    /// provider call itself stops on its next read). A no-op (not an error)
    /// when `req_id` names no live stream on this connection — it may have
    /// already finished, or it belongs to a different connection entirely.
    pub(super) fn cancel(&self, app: &AppHandle, req_id: &str) {
        if let Some(job_id) = self.take(req_id) {
            crate::commands::jobs::job_cancel(app, &job_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_text(m: Message) -> String {
        match m {
            Message::Text(t) => t.to_string(),
            other => panic!("expected a text frame, got {other:?}"),
        }
    }

    // ── AssistStreamRegistry ──────────────────────────────────────────────

    #[test]
    fn register_then_take_returns_and_forgets_it() {
        let r = AssistStreamRegistry::default();
        r.register("req-1", "job-1");
        assert_eq!(r.take("req-1"), Some("job-1".to_string()));
        assert_eq!(r.take("req-1"), None, "take also forgets the mapping");
    }

    #[test]
    fn unregister_on_an_unknown_req_id_is_a_no_op() {
        let r = AssistStreamRegistry::default();
        r.unregister("never-registered"); // must not panic
        assert_eq!(r.take("never-registered"), None);
    }

    #[test]
    fn register_overwrites_a_prior_mapping_for_the_same_req_id() {
        let r = AssistStreamRegistry::default();
        r.register("req-1", "job-1");
        r.register("req-1", "job-2");
        assert_eq!(
            r.take("req-1"),
            Some("job-2".to_string()),
            "a re-registration under the same reqId must replace, not duplicate"
        );
    }

    #[test]
    fn unregister_then_register_again_reflects_the_new_mapping() {
        let r = AssistStreamRegistry::default();
        r.register("req-1", "job-1");
        r.unregister("req-1");
        r.register("req-1", "job-2");
        assert_eq!(r.take("req-1"), Some("job-2".to_string()));
    }

    // ── CWE-639 regression: a different connection's registry can never see
    // (let alone cancel) another connection's stream ──────────────────────

    #[test]
    fn take_on_a_different_connections_registry_never_sees_another_connections_stream() {
        // Two independent registries — one per connection, exactly as
        // `handle_connection` creates a fresh one per socket.
        let connection_a = AssistStreamRegistry::default();
        let connection_b = AssistStreamRegistry::default();

        connection_a.register("req-1", "job-1");

        // Connection B never registered "req-1" — it must be a no-op, NEVER
        // able to observe (let alone cancel) connection A's stream.
        assert_eq!(
            connection_b.take("req-1"),
            None,
            "a different connection's registry must never see this reqId"
        );

        // Connection A can still cancel its own stream — the isolation is
        // per-connection, not "nobody can ever cancel it".
        assert_eq!(connection_a.take("req-1"), Some("job-1".to_string()));
    }

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
}

//! The streaming relay — decouples `handle_connection`'s read loop from a
//! multi-second streaming handler (`answer.assist`) so a same-connection
//! `assist.cancel` can still be read and dispatched while a stream is in
//! flight, and owns the streaming compose internals themselves (moved here
//! from `answer_assist` in the R8 line-budget split — see
//! [`compose_draft_stream`]).
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
//! [`ChannelFrameSink`] is the [`FrameSink`] a spawned handler writes
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
//!
//! ## Four ways a stream ends early, none of them a "failure"
//! Besides a genuine provider/network error, a stream can end for FOUR
//! reasons that must never be mislabeled `Failed` in the job tracker:
//! 1. **[`DRAFT_CAP`](super::answer_assist::DRAFT_CAP) reached** — enforced
//!    live by [`forward_chunk`]; [`compose_draft_stream`] then cancels the
//!    job itself, a successful truncation.
//! 2. **The transport is gone** — [`forward_chunk`] reports
//!    [`ForwardOutcome::SinkGone`] when `sink.send_frame` returns `false`
//!    (the connection's outbound channel is closed), and
//!    [`compose_draft_stream`] cancels immediately: no consumer is left, so
//!    waiting for the cap or a natural finish only burns more provider spend
//!    for nobody. This also catches a peer that never explicitly closes —
//!    [`run_writer`]'s `WRITE_STALL` timeout closes the channel after a
//!    stalled write, so the NEXT `send_frame` reports gone the same way.
//! 3. **The whole connection drops mid-stream** — `handle_connection` calls
//!    [`AssistStreamRegistry::cancel_all`] once its read loop exits, so
//!    every stream still registered for that connection is cancelled too,
//!    not just the one an explicit `assist.cancel` might have named.
//! 4. **An `assist.cancel` races the pre-compose window** — the gate/
//!    resume/limiter/salary/web-notes awaits in `resolve_answer_assist` run
//!    BEFORE [`compose_draft_stream`] ever calls [`AssistStreamRegistry::register`];
//!    a cancel arriving in that window used to be silently swallowed (there
//!    was nothing yet to `take()`). [`AssistStreamRegistry::begin`] records a
//!    `Pending` placeholder — called SYNCHRONOUSLY in `spawn_answer_assist`,
//!    before `tokio::spawn` even schedules the task that runs those awaits —
//!    so a racing cancel is captured as [`StreamEntry::CancelledEarly`], and
//!    `register` reports that back so the caller never starts the billable
//!    job at all; [`compose_draft_stream`] itself now starts that job
//!    (`start_and_register`) BEFORE registering it, so a cancel racing that
//!    exact gap still finds `Pending`, never a not-yet-existing `Running` job.
//!
//! Whichever of these fires, [`compose_draft_stream`]'s own error-path fix
//! (item 2 of this pass) still only calls `job_fail` for a GENUINE failure —
//! it checks the job's live status first, so a cancellation (any of the
//! above, or an external `assist.cancel`) is never overwritten `Failed`.

use std::sync::Arc;

use futures::SinkExt;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Listener, Manager};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

use super::assist_registry::start_and_register;
use super::msg;
use crate::error::{AppError, AppResult};
use crate::events::{AiStreamChunk, AI_STREAM};
use crate::pipeline::Completer;

/// A destination for streamed reply frames — abstracts the live WS writer so
/// the streaming `answer.assist` core ([`compose_draft_stream`]) is
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
pub(super) async fn next_step<R, W>(reader_next: R, writer_done: W) -> NextStep<R::Output>
where
    R: std::future::Future,
    W: std::future::Future,
{
    tokio::select! {
        frame = reader_next => NextStep::Frame(frame),
        _ = writer_done => NextStep::WriterEnded,
    }
}

/// Drive a streaming `answer.assist` request on its OWN task, decoupled from
/// the connection's read loop — see the module doc. The read loop's dispatch
/// for `FrameDecision::AnswerAssist` calls this and moves on immediately (no
/// reply is returned inline for the normal path); the terminal
/// `answer.assist.result` is sent from INSIDE the spawned task once
/// `handle_answer_assist` resolves, through the SAME channel as every
/// `assist.chunk`/`assist.done` it already sent, so per-`reqId` ordering
/// (chunks, then done, then result) holds.
///
/// [`AssistStreamRegistry::begin`] is called HERE, synchronously, on the read
/// loop's own thread — BEFORE `tokio::spawn` — rather than inside the spawned
/// task (where it used to live, in `resolve_answer_assist`). `tokio::spawn`
/// only SCHEDULES the task; it does not run it. Left inside the task, the
/// single-threaded read loop could immediately read the NEXT frame — an
/// `assist.cancel` for this very `reqId` — and dispatch it to
/// `AssistStreamRegistry::cancel` (also synchronous) before the spawned task
/// ever got scheduled to run its own `begin`. `cancel` would then find no
/// entry at all, silently drop the cancel, and the task would go on to run
/// `begin`→register→`job_start` regardless, billing a request the client
/// already gave up on. Calling `begin` here closes that scheduling gap: the
/// `Pending` marker exists before this function returns, so a same-connection
/// `assist.cancel` dispatched anywhere after this call is guaranteed to see
/// it. A duplicate `reqId` (one already `Pending`/`Running`/`CancelledEarly`
/// on this connection) is rejected right here with its own
/// `answer.assist.result` error reply — the task is never spawned at all.
pub(super) fn spawn_answer_assist(
    app: AppHandle,
    req_id: String,
    payload: Value,
    out_tx: UnboundedSender<Message>,
    registry: Arc<AssistStreamRegistry>,
) {
    let Some(gen) = begin_or_reject_duplicate(&registry, &req_id, &out_tx) else {
        return;
    };
    tokio::spawn(async move {
        let mut sink = ChannelFrameSink(out_tx.clone());
        let reply = super::answer_assist::handle_answer_assist(
            &app, &req_id, gen, &payload, &registry, &mut sink,
        )
        .await;
        let _ = out_tx.send(Message::text(reply));
    });
}

/// The synchronous half of [`spawn_answer_assist`] — factored out so it is
/// directly unit-testable WITHOUT a live `AppHandle` (this crate has no
/// `tauri::test` mock-app harness). A plain (non-`async`) function, so a
/// caller observing `Some(_)` returned — or `registry.contains(req_id)` true
/// right after — has proof `begin` ran on ITS OWN thread, not deferred into
/// whatever thread `tokio::spawn`'s task eventually runs on. Returns
/// `Some(gen)` — the generation `begin` minted for this reqId, which the
/// caller MUST thread all the way to `handle_answer_assist`'s end-of-request
/// `unregister_gen` call (see [`super::assist_registry::StreamEntry`]'s doc
/// for the reused-reqId clobber this generation closes) — when `req_id` was
/// free (the caller should go on to spawn the actual task); `None` when it
/// already named an active entry — in which case this function has ALREADY
/// enqueued the `DUPLICATE_REQUEST_MESSAGE` reply through `out_tx` itself, so
/// the caller has nothing left to do but return.
fn begin_or_reject_duplicate(
    registry: &AssistStreamRegistry,
    req_id: &str,
    out_tx: &UnboundedSender<Message>,
) -> Option<u64> {
    if let Some(gen) = registry.begin(req_id) {
        return Some(gen);
    }
    let reply = super::answer_assist::answer_assist_reply(
        req_id,
        Err(AppError::Validation(
            super::answer_assist::DUPLICATE_REQUEST_MESSAGE.to_string(),
        )),
    );
    let _ = out_tx.send(Message::text(reply));
    None
}

// ── Per-connection stream registry — the state machine itself now lives in
// `assist_registry` (R8 split); re-exported here so every existing
// `stream::AssistStreamRegistry` reference (mod.rs, answer_assist.rs, and
// their tests) keeps resolving unchanged. ───────────────────────────────────
pub(super) use super::assist_registry::AssistStreamRegistry;

// ── Streaming compose internals (moved from `answer_assist` — R8 split) ─────

/// Whether `job_id` is currently `Cancelled` in the job tracker — used by
/// [`compose_draft_stream`]'s error path to distinguish an EXPECTED
/// cancellation (this function's own cap/dead-sink `job_cancel`, or an
/// external `assist.cancel`) from a genuine provider/network failure, so
/// `job_fail` never overwrites an already-`Cancelled` job with `Failed`.
fn is_job_cancelled(app: &AppHandle, job_id: &str) -> bool {
    app.state::<Mutex<crate::jobs::JobTracker>>()
        .lock()
        .get(job_id)
        .map(|j| j.status == crate::jobs::JobStatus::Cancelled)
        .unwrap_or(false)
}

/// Stream the ONE compose call for `answer.assist` — see the module doc's
/// "Three ways a stream ends early" section for the full picture. Registers
/// a fresh job under `req_id` on `registry` (THIS connection's own
/// [`AssistStreamRegistry`] — so a client `assist.cancel` can stop it
/// early; does NOT `unregister` it on any path out of this function —
/// `handle_answer_assist` (in `answer_assist`) is the SOLE unregister owner,
/// once per request, at its single return point, so `req_id` can never be
/// clobbered by two cleanup sites racing over the same key. See its doc),
/// drives [`Completer::stream_complete`], forwards every
/// visible-text delta through `sink` as a cap-clamped `assist.chunk` frame
/// (see [`forward_chunk`]), sends a terminal `assist.done` frame once the
/// stream ends, and returns the full accumulated text (or the provider's
/// error) for the caller to shape into the (unchanged) `answer.assist.result`
/// terminal reply.
///
/// `system`/`max_tokens` are CALLER-supplied (PR 11) rather than hardcoded to
/// [`super::answer_assist::ANSWER_ASSIST_SYSTEM`]/`ANSWER_ASSIST_MAX_TOKENS`
/// so a second prompt (rewrite mode's
/// [`super::answer_rewrite::REWRITE_SYSTEM`]) can reuse this SAME streaming
/// compose path instead of a parallel one — the draft caller passes the
/// draft system/cap unchanged, the rewrite caller passes its own.
///
/// Mechanism: `chat_stream` emits `ai:stream` Tauri events as it drives the
/// HTTP stream — the SAME channel the renderer's own provider hook listens
/// to. This registers a SECOND, Rust-side listener for this exact `job_id`
/// (`tauri::Listener`) and forwards each piece through `sink` instead of into
/// a webview. A synchronous listener callback can't itself `.await` a socket
/// write, so it pushes each event onto an unbounded channel that this
/// function drains CONCURRENTLY with the `stream_complete` future
/// (`tokio::select!`). Several deltas — including the terminal one — can be
/// emitted synchronously in one burst right before `stream_complete`
/// resolves, so a final `try_recv` drain AFTER the select loop breaks is
/// what guarantees every already-buffered delta is still forwarded, never
/// just the ones the loop happened to poll before the future won the race.
///
/// Reaching [`super::answer_assist::DRAFT_CAP`] mid-stream, or the sink
/// reporting the transport gone ([`ForwardOutcome::SinkGone`]), both cancel
/// the job THE SAME WAY an external `assist.cancel` would (`job_cancel`,
/// polled by `chat_stream`'s own `is_cancelled` check) — either is a
/// SUCCESSFUL early stop, not a failure, so neither is propagated as an
/// error; only a cap/sink-unrelated error still is, and even then only after
/// confirming (via [`is_job_cancelled`]) that the job isn't ALREADY
/// `Cancelled` for one of those reasons (or an external cancel) — otherwise
/// `job_fail` would wrongly overwrite it.
pub(super) async fn compose_draft_stream(
    app: &AppHandle,
    completer: &Completer,
    req_id: &str,
    registry: &AssistStreamRegistry,
    system: &str,
    max_tokens: u32,
    user: &str,
    sink: &mut dyn FrameSink,
) -> AppResult<String> {
    // `start_and_register` starts the job BEFORE registering it — see its own
    // doc for the TOCTOU this order closes. `None` means an `assist.cancel`
    // raced ahead of this call during the pre-compose window and the job
    // `start_and_register` itself just started has already been cancelled —
    // the client already gave up, so never proceed into the stream loop.
    let Some(job_id) = start_and_register(app, registry, req_id) else {
        return Err(AppError::Message("Job cancelled".to_string()));
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AiStreamChunk>();
    let listen_job_id = job_id.clone();
    let listener_id = app.listen(AI_STREAM, move |event| {
        if let Ok(chunk) = serde_json::from_str::<AiStreamChunk>(event.payload()) {
            if chunk.job_id == listen_job_id {
                let _ = tx.send(chunk);
            }
        }
    });

    let mut accumulated = String::new();
    let mut cap_reached = false;
    let mut sink_gone = false;
    let result: AppResult<()>;
    {
        let mut stream_fut =
            Box::pin(completer.stream_complete(&job_id, system, user, Some(0.5), Some(max_tokens)));
        loop {
            tokio::select! {
                maybe = rx.recv() => {
                    if let Some(chunk) = maybe {
                        match forward_chunk(&chunk, req_id, sink, &mut accumulated).await {
                            ForwardOutcome::Continue => {}
                            ForwardOutcome::CapReached if !cap_reached => {
                                cap_reached = true;
                                // Bound cost/latency live, not just the wire
                                // text — the SAME cancellation path
                                // `assist.cancel` drives.
                                crate::commands::jobs::job_cancel(app, &job_id);
                            }
                            ForwardOutcome::CapReached => {}
                            ForwardOutcome::SinkGone if !sink_gone => {
                                sink_gone = true;
                                // No consumer left for this stream — stop the
                                // billable generation immediately rather
                                // than waiting for the cap or a natural
                                // finish.
                                crate::commands::jobs::job_cancel(app, &job_id);
                            }
                            ForwardOutcome::SinkGone => {}
                        }
                    }
                }
                res = &mut stream_fut => {
                    result = res;
                    break;
                }
            }
        }
    }
    // Flush anything emitted in the same synchronous burst as the terminal
    // piece — see the doc above. Skipped once the cap/dead-sink already
    // closed the frame stream (nothing left to usefully forward).
    while !cap_reached && !sink_gone {
        match rx.try_recv() {
            Ok(chunk) => match forward_chunk(&chunk, req_id, sink, &mut accumulated).await {
                ForwardOutcome::Continue => {}
                ForwardOutcome::CapReached => cap_reached = true,
                ForwardOutcome::SinkGone => sink_gone = true,
            },
            Err(_) => break,
        }
    }

    app.unlisten(listener_id);
    // No `registry.unregister(req_id)` here — see this function's doc:
    // `handle_answer_assist` is the SOLE unregister owner (one call, at its
    // single return point, covering every outcome of this function too).
    sink.send_frame(assist_done_frame(req_id)).await;

    if cap_reached {
        return Ok(accumulated);
    }
    if let Err(e) = result {
        // A cancellation THIS function itself triggered (the cap above, or
        // `sink_gone`) is an expected outcome, not a real failure — the job
        // is already correctly `Cancelled` by the SAME `job_cancel` call
        // that caused it. An EXTERNAL `assist.cancel` lands here too and is
        // likewise already `Cancelled`. Only when NEITHER is true (a
        // genuine provider/network error) does this need `job_fail`:
        // `stream_complete`'s own error path never calls it (it's a raw
        // provider call, not job-aware), so without this the job is stuck
        // `Running` and a restart mislabels it "interrupted by app restart"
        // instead of the real cause.
        if !is_job_cancelled(app, &job_id) {
            crate::commands::jobs::job_fail(app, &job_id, e.to_string());
        }
        return Err(e);
    }
    Ok(accumulated)
}

/// Whether `chunk` (an `ai:stream` event for the job [`compose_draft_stream`]
/// is driving) carries visible answer text to forward as an `assist.chunk`
/// frame — `None` for the terminal `done` piece, a reasoning/`thinking`
/// piece (the popup streams only the visible answer, never chain-of-thought),
/// or an already-empty delta. Pure — directly unit-tested without a live
/// `AppHandle`/event.
pub(super) fn forwardable_delta(chunk: &AiStreamChunk) -> Option<&str> {
    if chunk.done || chunk.thinking == Some(true) || chunk.delta.is_empty() {
        None
    } else {
        Some(chunk.delta.as_str())
    }
}

/// Outcome of forwarding one delta through [`forward_chunk`] — tells
/// [`compose_draft_stream`] whether (and why) to stop the generation early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ForwardOutcome {
    /// Keep going — under the cap, sink still alive.
    Continue,
    /// [`super::answer_assist::DRAFT_CAP`] was reached (sink still alive) —
    /// stop forwarding, but the generation itself is left to finish
    /// naturally (already bounded by `max_tokens`), so real usage still
    /// gets recorded on the normal completion path.
    CapReached,
    /// `sink.send_frame` reported the transport is gone (returned `false`)
    /// — no consumer left; the caller should cancel the job immediately,
    /// not wait for the cap or a natural finish.
    SinkGone,
}

/// Forward one delta (if any, per [`forwardable_delta`]) through `sink`,
/// clamped so `accumulated` never grows past
/// [`super::answer_assist::DRAFT_CAP`] chars — the LIVE, mid-stream sibling
/// of `resolve_answer_assist`'s own terminal `clamp_chars` safety net. See
/// [`ForwardOutcome`] for what each return value tells the caller to do.
pub(super) async fn forward_chunk(
    chunk: &AiStreamChunk,
    req_id: &str,
    sink: &mut dyn FrameSink,
    accumulated: &mut String,
) -> ForwardOutcome {
    let cap = super::answer_assist::DRAFT_CAP;
    let Some(delta) = forwardable_delta(chunk) else {
        return if accumulated.chars().count() >= cap {
            ForwardOutcome::CapReached
        } else {
            ForwardOutcome::Continue
        };
    };
    let remaining = cap.saturating_sub(accumulated.chars().count());
    if remaining == 0 {
        return ForwardOutcome::CapReached;
    }
    let piece: std::borrow::Cow<'_, str> = if delta.chars().count() > remaining {
        delta.chars().take(remaining).collect::<String>().into()
    } else {
        delta.into()
    };
    if piece.is_empty() {
        return ForwardOutcome::Continue;
    }
    accumulated.push_str(&piece);
    if !sink.send_frame(assist_chunk_frame(req_id, &piece)).await {
        return ForwardOutcome::SinkGone;
    }
    if accumulated.chars().count() >= cap {
        ForwardOutcome::CapReached
    } else {
        ForwardOutcome::Continue
    }
}

/// Build an `assist.chunk { delta }` frame.
fn assist_chunk_frame(req_id: &str, delta: &str) -> String {
    json!({
        "type": msg::ASSIST_CHUNK,
        "reqId": req_id,
        "payload": { "delta": delta },
    })
    .to_string()
}

/// Build the terminal, no-payload `assist.done` frame for `req_id`.
fn assist_done_frame(req_id: &str) -> String {
    json!({
        "type": msg::ASSIST_DONE,
        "reqId": req_id,
        "payload": Value::Null,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
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

        assert!(begin_or_reject_duplicate(&registry, "req-1", &out_tx).is_some());
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
            !registry.register("req-1", "job-1"),
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

        let outcome = next_step(reader_next, writer_done).await;

        assert!(
            matches!(outcome, NextStep::WriterEnded),
            "the writer ending must win the race even though the reader never resolves"
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

        let outcome = next_step(reader_next, writer_done).await;

        let NextStep::Frame(value) = outcome else {
            panic!("expected NextStep::Frame — the writer must never win while a frame is ready");
        };
        assert_eq!(value, Some(7));
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
        let capped = forward_chunk(&first, "req-1", &mut sink, &mut accumulated).await;
        assert_eq!(capped, ForwardOutcome::CapReached);
        assert_eq!(accumulated.chars().count(), cap);

        // A second delta arriving after the cap must never grow the buffer
        // or send another frame.
        let second = stream_chunk("more text", false, None);
        let capped_again = forward_chunk(&second, "req-1", &mut sink, &mut accumulated).await;
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

    #[tokio::test]
    async fn forward_chunk_clamps_a_delta_that_would_cross_the_cap_mid_chunk() {
        let mut sink = RecordingSink::default();
        let cap = super::super::answer_assist::DRAFT_CAP;
        let mut accumulated = "x".repeat(cap - 5);

        // 10 chars incoming, only 5 fit before the cap.
        let chunk = stream_chunk("0123456789", false, None);
        let capped = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated).await;

        assert_eq!(capped, ForwardOutcome::CapReached);
        assert_eq!(accumulated.chars().count(), cap);
        assert_eq!(
            sink.sent.last().unwrap(),
            &assist_chunk_frame("req-1", "01234")
        );
    }

    #[tokio::test]
    async fn forward_chunk_reports_uncapped_while_under_the_limit() {
        let mut sink = RecordingSink::default();
        let mut accumulated = String::new();
        let chunk = stream_chunk("short delta", false, None);
        let capped = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated).await;
        assert_eq!(capped, ForwardOutcome::Continue);
        assert_eq!(accumulated, "short delta");
        assert_eq!(sink.sent, vec![assist_chunk_frame("req-1", "short delta")]);
    }

    #[tokio::test]
    async fn forward_chunk_reports_sink_gone_when_send_frame_returns_false() {
        let mut sink = DeadSink;
        let mut accumulated = String::new();
        let chunk = stream_chunk("hello", false, None);
        let outcome = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated).await;
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
        let outcome = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated).await;
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
}

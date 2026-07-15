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

use std::collections::HashMap;
use std::sync::Arc;

use futures::SinkExt;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Listener, Manager};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

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
/// without a live socket. Fire-and-forget, like every other spawn in this
/// module — never explicitly joined, so a streaming task that never finishes
/// can never hang the connection's own cleanup.
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
    if !begin_or_reject_duplicate(&registry, &req_id, &out_tx) {
        return;
    }
    tokio::spawn(async move {
        let mut sink = ChannelFrameSink(out_tx.clone());
        let reply = super::answer_assist::handle_answer_assist(
            &app, &req_id, &payload, &registry, &mut sink,
        )
        .await;
        let _ = out_tx.send(Message::text(reply));
    });
}

/// The synchronous half of [`spawn_answer_assist`] — factored out so it is
/// directly unit-testable WITHOUT a live `AppHandle` (this crate has no
/// `tauri::test` mock-app harness). A plain (non-`async`) function, so a
/// caller observing `true` returned — or `registry.contains(req_id)` true
/// right after — has proof `begin` ran on ITS OWN thread, not deferred into
/// whatever thread `tokio::spawn`'s task eventually runs on. Returns `true`
/// when `req_id` was free (the caller should go on to spawn the actual
/// task); `false` when it already named an active entry — in which case this
/// function has ALREADY enqueued the `DUPLICATE_REQUEST_MESSAGE` reply
/// through `out_tx` itself, so the caller has nothing left to do but return.
fn begin_or_reject_duplicate(
    registry: &AssistStreamRegistry,
    req_id: &str,
    out_tx: &UnboundedSender<Message>,
) -> bool {
    if registry.begin(req_id) {
        return true;
    }
    let reply = super::answer_assist::answer_assist_reply(
        req_id,
        Err(AppError::Validation(
            super::answer_assist::DUPLICATE_REQUEST_MESSAGE.to_string(),
        )),
    );
    let _ = out_tx.send(Message::text(reply));
    false
}

// ── Per-connection stream registry (register / cancel / pre-registration race) ──

/// Abstracts "cancel one job by id" so [`AssistStreamRegistry::cancel`]/
/// [`AssistStreamRegistry::cancel_all`]'s job-cancelling side effect is
/// unit-testable against a fake recorder, without a live `AppHandle` — this
/// crate has no `tauri::test` mock-app harness (mirrors the
/// `SalarySearcher`/`AnswerSearcher` genericization precedent in
/// `answer_assist`/`commands::ai`). The sole production implementor forwards
/// to [`crate::commands::jobs::job_cancel`].
pub(super) trait JobCanceller {
    fn cancel_job(&self, job_id: &str);
}

impl JobCanceller for AppHandle {
    fn cancel_job(&self, job_id: &str) {
        crate::commands::jobs::job_cancel(self, job_id);
    }
}

/// Abstracts "start a job by id" so [`start_and_register`]'s
/// start-before-register ordering is unit-testable against a fake recorder,
/// without a live `AppHandle` — mirrors [`JobCanceller`]'s existing seam. The
/// sole production implementor forwards to [`crate::commands::jobs::job_start`]
/// with this module's one fixed job kind (every job this registry ever tracks
/// is an `"extension.answer_assist"` job).
pub(super) trait JobStarter {
    fn start_job(&self, job_id: &str);
}

impl JobStarter for AppHandle {
    fn start_job(&self, job_id: &str) {
        crate::commands::jobs::job_start(self, job_id, "extension.answer_assist");
    }
}

/// Start a fresh job for `req_id` and register it with `registry` —
/// deliberately `start` BEFORE `register`, the reverse of this module's
/// original order. The original order had a TOCTOU: `register` published a
/// `Running(job_id)` entry before the job existed, so an `assist.cancel`
/// landing in that exact gap found `Running`, removed the entry, and called
/// `cancel_job` on an id nothing had started yet (a no-op) — the job then
/// started anyway, Running, with no cancel path left. Starting first closes
/// it: a cancel racing this same gap instead finds the `Pending` marker
/// [`AssistStreamRegistry::begin`] already left behind (set synchronously in
/// `spawn_answer_assist`, before this task was even spawned), so `register`
/// below correctly observes [`StreamEntry::CancelledEarly`] and this function
/// cancels the very job it just started before reporting failure. `None` on
/// that race (the caller should treat it as `"Job cancelled"`); `Some(job_id)`
/// otherwise. Safe to cancel unconditionally on the race path — `job_id` is a
/// fresh UUID ([`crate::db::new_job_id`]), so it can never collide with a
/// later, unrelated `start_job` call.
///
/// Generic over a combined [`JobStarter`] + [`JobCanceller`] recorder (not
/// the concrete `AppHandle`) so this ordering is directly unit-testable
/// without a live `AppHandle` — this crate has no `tauri::test` mock-app
/// harness. The sole production caller ([`compose_draft_stream`]) passes a
/// real `&AppHandle` (which implements both).
pub(super) fn start_and_register<T: JobStarter + JobCanceller>(
    starter: &T,
    registry: &AssistStreamRegistry,
    req_id: &str,
) -> Option<String> {
    let job_id = crate::db::new_job_id();
    starter.start_job(&job_id);
    if !registry.register(req_id, &job_id) {
        starter.cancel_job(&job_id);
        return None;
    }
    Some(job_id)
}

/// One `reqId`'s lifecycle in the per-connection registry — from the moment
/// `resolve_answer_assist` starts its pre-compose work (before ANY billable
/// spend) through to either a registered running job or an early
/// cancellation. See the module doc's "pre-registration cancel race" case.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StreamEntry {
    /// Pre-compose work (gate/resume/limiter/salary/web-notes) is in
    /// flight — no job exists yet.
    Pending,
    /// [`AssistStreamRegistry::register`] recorded its job id.
    Running(String),
    /// An `assist.cancel` arrived while still `Pending` — the pre-compose
    /// caller must short-circuit rather than proceed to the billable
    /// compose call. See [`AssistStreamRegistry::register`]'s return value.
    CancelledEarly,
}

/// Per-connection registry of in-flight/pending streaming `answer.assist`
/// requests (`reqId -> `[`StreamEntry`]) — deliberately scoped to ONE
/// connection. See the module doc's "Cancellation is per-connection" section.
#[derive(Default)]
pub(super) struct AssistStreamRegistry(Mutex<HashMap<String, StreamEntry>>);

impl AssistStreamRegistry {
    /// Mark `req_id` as `Pending` BEFORE any pre-compose await (the gate/
    /// resume/limiter/salary/web-notes lookups in `resolve_answer_assist`) —
    /// the realistic window (network round-trips) an `assist.cancel` could
    /// race ahead of [`Self::register`]. Returns `false` (leaving the
    /// existing entry untouched) when `req_id` already names ANY entry on
    /// this connection — `Pending`, `Running`, OR `CancelledEarly` — never
    /// silently overwriting it with a fresh `Pending`. Overwriting a
    /// `Running` entry would orphan its job (still running server-side, but
    /// no longer reachable from this registry, so a later `assist.cancel`
    /// for that reqId could never reach it again). Overwriting a
    /// `CancelledEarly` entry reopens the SAME hole from the other
    /// direction: that marker is NOT settled — it is awaiting consumption by
    /// [`Self::register`] (which sees it, removes it, and reports `false` so
    /// the run that raced the cancel never starts a billable job) or removal
    /// by [`Self::unregister`]/[`Self::cancel_all`]. Allowing a reuse before
    /// that consumption would let a second run's fresh `Pending` slip in
    /// under the same `req_id`; the FIRST (already-cancelled) run's later
    /// `register` call would then see that second run's `Pending` instead of
    /// its own `CancelledEarly` marker and register a billable job anyway —
    /// the cancel guarantee lost. It is always cleared within the original
    /// run's own lifecycle (consumed by `register`, or removed by an
    /// `unregister`/`cancel_all`), so a `req_id` can never get stuck rejected
    /// forever; a well-behaved client uses a fresh reqId per request anyway.
    /// Returns `true` — and inserts `Pending` — only when `req_id` names no
    /// entry at all: a fresh `req_id`, or one that already fully settled and
    /// was removed.
    pub(super) fn begin(&self, req_id: &str) -> bool {
        let mut guard = self.0.lock();
        if guard.contains_key(req_id) {
            return false;
        }
        guard.insert(req_id.to_string(), StreamEntry::Pending);
        true
    }

    /// Register an in-flight stream's `reqId -> jobId`, UNLESS `req_id` was
    /// already marked [`StreamEntry::CancelledEarly`] (an `assist.cancel`
    /// raced the pre-compose window) — in which case this is a no-op and
    /// returns `false`, so the caller aborts BEFORE ever starting the
    /// billable job, rather than registering (and thus only becoming
    /// cancellable from this point forward) a stream the client already
    /// gave up on. Returns `true` otherwise: the normal `Pending` →
    /// `Running` move, or a caller that never `begin`s at all.
    pub(super) fn register(&self, req_id: &str, job_id: &str) -> bool {
        let mut guard = self.0.lock();
        if matches!(guard.get(req_id), Some(StreamEntry::CancelledEarly)) {
            guard.remove(req_id);
            return false;
        }
        guard.insert(req_id.to_string(), StreamEntry::Running(job_id.to_string()));
        true
    }

    /// Forget a stream's registration once it ends (success, failure, or an
    /// already-raced cancel). A no-op when `req_id` is already gone — never
    /// an error.
    pub(super) fn unregister(&self, req_id: &str) {
        self.0.lock().remove(req_id);
    }

    /// Remove + return the RUNNING job registered under `req_id` on THIS
    /// registry — `None` when never registered here, already finished,
    /// still `Pending` (no job yet), or belonging to a DIFFERENT
    /// connection's registry (the CWE-639 case this type exists to close).
    /// Pure (no `AppHandle`, no [`JobCanceller`]), so this security-relevant
    /// property is directly unit-testable. Test-only: [`Self::cancel`] used
    /// to call this (a separate `lock()` acquisition), but that shape was a
    /// TOCTOU (a concurrent `register` could flip `Pending` -> `Running` in
    /// the gap between this method's lock and `cancel`'s own); `cancel` now
    /// inlines the same decision under ONE lock instead, leaving this method
    /// only as a test seam.
    #[cfg(test)]
    pub(super) fn take(&self, req_id: &str) -> Option<String> {
        let mut guard = self.0.lock();
        // Checked BEFORE removing — a naive unconditional `remove` would
        // destroy a `Pending`/`CancelledEarly` entry it isn't actually
        // returning, silently losing that state for anyone who checks
        // `req_id` afterward (this was a real bug caught by this file's own
        // pre-registration-race test).
        match guard.get(req_id) {
            Some(StreamEntry::Running(_)) => match guard.remove(req_id) {
                Some(StreamEntry::Running(job_id)) => Some(job_id),
                _ => None,
            },
            _ => None,
        }
    }

    /// Test-only seam: whether ANY entry (`Pending`, `Running`, or
    /// `CancelledEarly`) exists for `req_id` — unlike [`Self::take`] (which
    /// only ever observes a `Running` job), this is what a leak-detection
    /// test needs to assert a `Pending` entry was actually `unregister`ed,
    /// not just left un-taken.
    #[cfg(test)]
    pub(super) fn contains(&self, req_id: &str) -> bool {
        self.0.lock().contains_key(req_id)
    }

    /// Cancel the stream named by `req_id` on THIS registry, if any. A
    /// `Running` entry is job-cancelled via `canceller` (the SAME mechanism
    /// `chat_stream`'s `is_cancelled` polls every chunk, so the provider
    /// call itself stops on its next read) and forgotten. A still-`Pending`
    /// entry (no job yet — the pre-compose window) is marked
    /// [`StreamEntry::CancelledEarly`] instead, so [`Self::register`]
    /// reports `false` once the pre-compose caller reaches it. A no-op when
    /// `req_id` names nothing on this connection at all. Generic over
    /// [`JobCanceller`] (not the concrete `AppHandle`) so this is
    /// unit-testable against a fake recorder — the sole production caller
    /// passes a real `&AppHandle` (which implements it).
    pub(super) fn cancel<C: JobCanceller>(&self, canceller: &C, req_id: &str) {
        // The whole "is it Running, or still Pending, or neither" decision
        // happens under ONE lock acquisition — splitting it into `take`
        // (its own lock) followed by a second, separate `self.0.lock()` (the
        // original shape) leaves a gap between the two where a concurrent
        // `register` can flip `Pending` -> `Running` on the multi-threaded
        // runtime, so this call would see neither case and silently miss
        // the cancel (TOCTOU). Same per-variant behavior as before, just
        // decided in one critical section.
        let job_id = {
            let mut guard = self.0.lock();
            match guard.get(req_id) {
                Some(StreamEntry::Running(_)) => match guard.remove(req_id) {
                    Some(StreamEntry::Running(job_id)) => Some(job_id),
                    _ => None,
                },
                Some(StreamEntry::Pending) => {
                    guard.insert(req_id.to_string(), StreamEntry::CancelledEarly);
                    None
                }
                _ => None,
            }
        };
        if let Some(job_id) = job_id {
            canceller.cancel_job(&job_id);
        }
    }

    /// Cancel EVERY stream currently registered on THIS connection's
    /// registry — called once the connection's read loop exits (socket
    /// closed/errored) so a client disconnect stops every billable
    /// generation still running for it, not just the one an explicit
    /// `assist.cancel` might have named (the CWE-639 fix stays: this only
    /// ever touches THIS connection's own map). A `Running` entry is
    /// cancelled via `canceller`; a still-`Pending` entry is marked
    /// `CancelledEarly` (mirrors [`Self::cancel`]'s `Pending` arm) so
    /// in-flight pre-compose work also short-circuits instead of reaching a
    /// now-pointless billable compose call. Generic over [`JobCanceller`] —
    /// see [`Self::cancel`]'s doc.
    pub(super) fn cancel_all<C: JobCanceller>(&self, canceller: &C) {
        let mut guard = self.0.lock();
        let drained: Vec<(String, StreamEntry)> = guard.drain().collect();
        let mut running = Vec::new();
        for (req_id, entry) in drained {
            match entry {
                StreamEntry::Running(job_id) => running.push(job_id),
                // Exhaustive on purpose: BOTH still-pending AND
                // already-cancelled-early entries must be reinserted as
                // `CancelledEarly`. Dropping the latter (the original bug)
                // loses the guard marker on a cancel-then-disconnect during
                // the pre-compose window — the entry vanishes from the map,
                // so the later `register` call for that `req_id` finds
                // nothing, returns `true`, and starts a full billable
                // generation for a request the user already cancelled.
                StreamEntry::Pending | StreamEntry::CancelledEarly => {
                    guard.insert(req_id, StreamEntry::CancelledEarly);
                }
            }
        }
        drop(guard);
        for job_id in running {
            canceller.cancel_job(&job_id);
        }
    }
}

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
/// early), drives [`Completer::stream_complete`], forwards every
/// visible-text delta through `sink` as a cap-clamped `assist.chunk` frame
/// (see [`forward_chunk`]), sends a terminal `assist.done` frame once the
/// stream ends, and returns the full accumulated text (or the provider's
/// error) for the caller to shape into the (unchanged) `answer.assist.result`
/// terminal reply.
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
        let mut stream_fut = Box::pin(completer.stream_complete(
            &job_id,
            super::answer_assist::ANSWER_ASSIST_SYSTEM,
            user,
            Some(0.5),
            Some(super::answer_assist::ANSWER_ASSIST_MAX_TOKENS),
        ));
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
    registry.unregister(req_id);
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
    use super::*;

    fn as_text(m: Message) -> String {
        match m {
            Message::Text(t) => t.to_string(),
            other => panic!("expected a text frame, got {other:?}"),
        }
    }

    // ── AssistStreamRegistry: register / take / unregister ─────────────────

    #[test]
    fn register_then_take_returns_and_forgets_it() {
        let r = AssistStreamRegistry::default();
        assert!(r.register("req-1", "job-1"));
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
        assert!(r.register("req-1", "job-1"));
        assert!(r.register("req-1", "job-2"));
        assert_eq!(
            r.take("req-1"),
            Some("job-2".to_string()),
            "a re-registration under the same reqId must replace, not duplicate"
        );
    }

    #[test]
    fn unregister_then_register_again_reflects_the_new_mapping() {
        let r = AssistStreamRegistry::default();
        assert!(r.register("req-1", "job-1"));
        r.unregister("req-1");
        assert!(r.register("req-1", "job-2"));
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

    // ── AssistStreamRegistry::cancel / cancel_all (JobCanceller-generic —
    // testable without a live AppHandle; this crate has no tauri::test
    // mock-app harness) ─────────────────────────────────────────────────────

    #[derive(Default)]
    struct RecordingCanceller {
        cancelled: std::cell::RefCell<Vec<String>>,
    }

    impl JobCanceller for RecordingCanceller {
        fn cancel_job(&self, job_id: &str) {
            self.cancelled.borrow_mut().push(job_id.to_string());
        }
    }

    #[test]
    fn cancel_on_an_unknown_req_id_never_touches_the_canceller() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.cancel(&canceller, "never-registered");
        assert!(canceller.cancelled.borrow().is_empty());
    }

    #[test]
    fn cancel_on_a_running_req_id_cancels_its_job_and_forgets_the_mapping() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.register("req-1", "job-1");
        r.cancel(&canceller, "req-1");
        assert_eq!(canceller.cancelled.into_inner(), vec!["job-1".to_string()]);
        assert_eq!(r.take("req-1"), None, "cancel also forgets the mapping");
    }

    #[test]
    fn cancel_all_cancels_every_running_stream_and_leaves_pending_alone_besides_marking_it() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.register("req-1", "job-1");
        r.register("req-2", "job-2");
        r.begin("req-3"); // still pending — no job to cancel
        r.cancel_all(&canceller);

        let mut got = canceller.cancelled.into_inner();
        got.sort();
        assert_eq!(
            got,
            vec!["job-1".to_string(), "job-2".to_string()],
            "only RUNNING entries are ever job-cancelled"
        );
        // The pending entry is now cancelled-early — a still in-flight
        // pre-compose caller must never be allowed to register a job for it.
        assert!(!r.register("req-3", "job-3"));
    }

    #[test]
    fn cancel_all_on_an_empty_registry_is_a_no_op() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.cancel_all(&canceller);
        assert!(canceller.cancelled.into_inner().is_empty());
    }

    #[test]
    fn cancel_all_preserves_an_already_cancelled_early_entry() {
        // HIGH regression: cancel-then-disconnect during the pre-compose
        // window. `begin` + `cancel` leaves `req-1` as `CancelledEarly`
        // BEFORE `cancel_all` ever runs; `cancel_all` must reinsert it
        // (not drop it on the floor), or the later `register` call for the
        // same `req_id` finds nothing, returns `true`, and starts a full
        // billable generation for a request the user already cancelled.
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.begin("req-1");
        r.cancel(&canceller, "req-1"); // -> CancelledEarly, no job existed yet
        r.cancel_all(&canceller);

        assert!(
            canceller.cancelled.borrow().is_empty(),
            "no Running job existed at any point — nothing to job_cancel"
        );
        assert!(
            !r.register("req-1", "job-1"),
            "the CancelledEarly guard must survive cancel_all's drain-and-reinsert"
        );
    }

    // ── Pre-registration cancel race (LOW fix): begin() + register()'s bool ─

    #[test]
    fn cancel_during_the_pending_window_prevents_the_later_register_call() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.begin("req-1"); // pre-compose work has started — no job yet
        r.cancel(&canceller, "req-1"); // assist.cancel races the awaits

        assert!(
            canceller.cancelled.borrow().is_empty(),
            "no job exists yet — there is nothing to job_cancel"
        );
        assert!(
            !r.register("req-1", "job-1"),
            "the compose call must never start a billable job for an early-cancelled reqId"
        );
        assert_eq!(
            r.take("req-1"),
            None,
            "the cancelled-early marker must never surface as a real job"
        );
    }

    #[test]
    fn register_without_a_prior_cancel_succeeds_normally_after_begin() {
        let r = AssistStreamRegistry::default();
        r.begin("req-1");
        assert!(r.register("req-1", "job-1"));
        assert_eq!(r.take("req-1"), Some("job-1".to_string()));
    }

    // ── Duplicate reqId rejection (MEDIUM fix): begin() on an already-active
    // entry must never orphan the original job ─────────────────────────────

    #[test]
    fn begin_on_a_fresh_req_id_succeeds() {
        let r = AssistStreamRegistry::default();
        assert!(r.begin("req-1"));
    }

    #[test]
    fn begin_on_an_already_pending_req_id_is_rejected() {
        let r = AssistStreamRegistry::default();
        r.begin("req-1"); // first request's pre-compose window is in flight

        assert!(
            !r.begin("req-1"),
            "a second begin for the same still-Pending reqId must be rejected"
        );
    }

    #[test]
    fn begin_on_an_already_running_req_id_is_rejected_and_the_original_stays_cancellable() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        assert!(r.register("req-1", "job-1")); // the original request is now Running

        // A client reusing the SAME reqId while the original is still
        // running must be rejected — never silently overwrite the Running
        // entry with a fresh Pending, which would orphan job-1 (still
        // running server-side, but no longer reachable to cancel).
        assert!(!r.begin("req-1"));

        r.cancel(&canceller, "req-1");
        assert_eq!(
            canceller.cancelled.into_inner(),
            vec!["job-1".to_string()],
            "the original job must still be there and cancellable after the rejected begin"
        );
    }

    #[test]
    fn begin_on_a_cancelled_early_req_id_is_rejected() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.begin("req-1");
        r.cancel(&canceller, "req-1"); // -> CancelledEarly, no job existed yet

        assert!(
            !r.begin("req-1"),
            "a CancelledEarly marker is not settled — reuse must be rejected \
             until register (or unregister/cancel_all) consumes it"
        );
    }

    #[test]
    fn begin_on_a_cancelled_early_req_id_is_rejected_until_register_consumes_it() {
        // Full spend/cancel-integrity guarantee: a run that reused req-1
        // before the CancelledEarly marker was consumed used to be able to
        // slip a fresh Pending in, which let the FIRST (already-cancelled)
        // run's later `register` call see that Pending instead of its own
        // marker and start a billable job anyway — the exact hole this fix
        // closes.
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.begin("req-1"); // run A's pre-compose window opens
        r.cancel(&canceller, "req-1"); // -> CancelledEarly, run A has no job yet

        assert!(
            !r.begin("req-1"),
            "a second run reusing req-1 must be rejected while the marker is un-consumed"
        );
        assert!(
            !r.register("req-1", "job-a"),
            "register consumes the CancelledEarly marker and reports false — \
             run A never starts a billable job"
        );
        assert!(
            r.begin("req-1"),
            "once consumed, req-1 names no entry at all — reuse is allowed again"
        );
    }

    // ── begin_or_reject_duplicate (HIGH fix: pre-begin cancel-drop —
    // `begin` must run synchronously, on the read loop's own thread, BEFORE
    // `tokio::spawn` ever schedules the streaming task) ────────────────────

    #[test]
    fn begin_or_reject_duplicate_marks_pending_synchronously_before_any_task_runs() {
        // This whole test has no `.await` at all — `begin_or_reject_duplicate`
        // is a plain, non-async fn — so a `true` return, and `contains`
        // reporting `true` immediately after, already proves `begin` ran on
        // the CALLER's thread, not deferred into whatever thread a spawned
        // task eventually runs on.
        let registry = AssistStreamRegistry::default();
        let (out_tx, _out_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        assert!(begin_or_reject_duplicate(&registry, "req-1", &out_tx));
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

        assert!(!begin_or_reject_duplicate(&registry, "req-1", &out_tx));

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

    // ── start_and_register (HIGH fix: job_start-before-register TOCTOU —
    // starting the job before registering it means a cancel racing the gap
    // finds Pending, not a not-yet-existing Running job) ───────────────────

    #[derive(Default)]
    struct RecordingStarterCanceller {
        started: std::cell::RefCell<Vec<String>>,
        cancelled: std::cell::RefCell<Vec<String>>,
    }

    impl JobStarter for RecordingStarterCanceller {
        fn start_job(&self, job_id: &str) {
            self.started.borrow_mut().push(job_id.to_string());
        }
    }

    impl JobCanceller for RecordingStarterCanceller {
        fn cancel_job(&self, job_id: &str) {
            self.cancelled.borrow_mut().push(job_id.to_string());
        }
    }

    #[test]
    fn start_and_register_starts_the_job_then_registers_it_on_the_happy_path() {
        let registry = AssistStreamRegistry::default();
        let recorder = RecordingStarterCanceller::default();

        let job_id = start_and_register(&recorder, &registry, "req-1")
            .expect("a fresh reqId with no prior cancel must register successfully");

        assert_eq!(
            recorder.started.into_inner(),
            vec![job_id.clone()],
            "job_start must have run, unconditionally, before register was ever consulted"
        );
        assert!(
            recorder.cancelled.borrow().is_empty(),
            "a successful register must never cancel the job it just started"
        );
        assert_eq!(
            registry.take("req-1"),
            Some(job_id),
            "register must have recorded the Running entry"
        );
    }

    #[test]
    fn start_and_register_cancels_the_just_started_job_when_a_cancel_already_raced_ahead() {
        // The exact TOCTOU this reorder closes: an `assist.cancel` that
        // arrived during the pre-compose window (captured here as a
        // pre-seeded `CancelledEarly` marker — see `AssistStreamRegistry::
        // begin`/`cancel`) must never leave a job that's Running but neither
        // cancelled nor cancellable. `job_start` still runs — unconditionally,
        // BEFORE `register` is ever consulted, proving the new order — but
        // `register` then reports the race, and this function must cancel
        // the very job it just started rather than leaving it orphaned.
        let registry = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        registry.begin("req-1");
        registry.cancel(&canceller, "req-1"); // -> CancelledEarly, no job existed yet

        let recorder = RecordingStarterCanceller::default();
        let result = start_and_register(&recorder, &registry, "req-1");

        assert!(
            result.is_none(),
            "a raced-ahead cancel must make start_and_register report failure"
        );
        assert_eq!(
            recorder.started.borrow().len(),
            1,
            "job_start must still have run — it happens BEFORE register is ever consulted"
        );
        let started_id = recorder.started.borrow()[0].clone();
        assert_eq!(
            recorder.cancelled.into_inner(),
            vec![started_id],
            "the job just started must be job-cancelled immediately — no leaked Running job"
        );
        assert!(
            !registry.contains("req-1"),
            "the CancelledEarly marker must be consumed, not left behind"
        );
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

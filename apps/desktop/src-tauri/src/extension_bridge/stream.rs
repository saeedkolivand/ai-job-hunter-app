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
//!
//! ## Split (R8 relief)
//! This file now holds only the [`FrameSink`] abstraction + [`ChannelFrameSink`], re-exporting
//! everything else so every existing `stream::…` reference (`mod.rs`, `answer_assist` and its
//! siblings, tests) keeps resolving unchanged: [`run_writer`]/[`next_step`]/[`NextStep`] live in
//! `stream_writer`; the three `spawn_*` task-launchers in `stream_spawn`; [`ComposeStream`]/
//! [`compose_draft_stream`] in `stream_compose`; and [`forward_chunk`]/[`ForwardOutcome`] in
//! `stream_forward`.

use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;

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

// ── Split siblings (R8 relief), declared in `mod.rs` — re-exported so every existing
// external caller keeps resolving these through `stream::…` unchanged. ─────────────────
pub(super) use super::stream_compose::{compose_draft_stream, ComposeStream};
// `forward_chunk`/`assist_done_frame` have no PRODUCTION caller through this path any more
// (`stream_compose` imports them directly from `stream_forward`) — only
// `answer_assist::tests::support` still reaches them as `stream::forward_chunk`/
// `stream::assist_done_frame`, so the re-export is test-only.
#[cfg(test)]
pub(super) use super::stream_forward::{assist_done_frame, forward_chunk};
pub(super) use super::stream_spawn::{spawn_agent_call, spawn_agent_query, spawn_answer_assist};
pub(super) use super::stream_writer::{next_step, run_writer, NextStep};

// ── Per-connection stream registry — the state machine itself now lives in
// `assist_registry` (R8 split); re-exported here so every existing
// `stream::AssistStreamRegistry` reference (mod.rs, answer_assist.rs, and
// their tests) keeps resolving unchanged. ───────────────────────────────────
pub(super) use super::assist_registry::AssistStreamRegistry;

#[cfg(test)]
mod tests;

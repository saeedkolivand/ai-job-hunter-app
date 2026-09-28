//! The streaming compose core for `answer.assist`/rewrite — split from `stream.rs` (R8 relief;
//! moved there from `answer_assist` originally, see that history in `stream`'s own module doc).
//! [`ComposeStream`] bundles one request's fixed inputs + the state that must span every retry
//! attempt; [`compose_draft_stream`] drives ONE attempt end to end. See `stream`'s module doc's
//! "Four ways a stream ends early" section for the cancellation/error-path contract this keeps.

use parking_lot::Mutex;
use tauri::{AppHandle, Listener, Manager};

use super::assist_registry::{start_and_register, AssistStreamRegistry};
use super::stream::FrameSink;
use super::stream_forward::{assist_done_frame, forward_chunk, ForwardOutcome};
use crate::error::{AppError, AppResult};
use crate::events::{AiStreamChunk, AI_STREAM};
use crate::pipeline::Completer;

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

/// Everything ONE `answer.assist` request's streaming compose works on: the
/// already-resolved inputs that never change between attempts, plus the three
/// pieces of state that must SPAN them (the registry entry's generation, the
/// sink, and the answer budget already forwarded).
///
/// A retry is not a second request. `compose_with_length_retry` (in
/// [`super::answer_assist`]) can drive [`compose_draft_stream`] twice for ONE
/// `answer.assist` frame, and everything in here belongs to that frame rather
/// than to an attempt — which is why the retry rebinds the SAME registry
/// entry (see [`start_and_register`]), appends to the SAME draft buffer
/// ([`Self::forwarded`] — though each attempt is capped against its OWN
/// window inside it), and emits ONE terminal `assist.done`
/// ([`Self::send_done`]). Bundling also takes `compose_draft_stream` from nine
/// positional parameters to four, so it no longer needs a
/// `clippy::too_many_arguments` exemption.
pub(super) struct ComposeStream<'a> {
    pub(super) app: &'a AppHandle,
    pub(super) completer: &'a Completer,
    pub(super) req_id: &'a str,
    /// The generation [`AssistStreamRegistry::begin`] minted for THIS request,
    /// threaded down from `handle_answer_assist` — every attempt binds its
    /// fresh job to that one entry, and the request's single
    /// `unregister_gen` then still owns it.
    pub(super) r#gen: u64,
    pub(super) registry: &'a AssistStreamRegistry,
    pub(super) system: &'a str,
    pub(super) user: &'a str,
    pub(super) sink: &'a mut dyn FrameSink,
    /// Every answer char forwarded to the client for this REQUEST, across
    /// every attempt. Append-only, which is what lets
    /// [`super::answer_assist::attempt_text`] read back one attempt's own
    /// tail: the draft sent back is the SUCCESSFUL attempt's text alone,
    /// never a failed attempt's prose riding along into what "Accept" pastes.
    ///
    /// It is NOT a shared cap window — each attempt is forwarded against its
    /// own `DRAFT_CAP`, based at where that attempt's text starts (see
    /// [`compose_draft_stream`]'s `cap_base`).
    pub(super) forwarded: String,
}

impl ComposeStream<'_> {
    /// Whether this request still owns its registry entry — the spend guard
    /// `compose_with_length_retry` checks before paying for a retry. Purely a
    /// registry read — it never probes the transport, so a dropped connection
    /// shows up here only once the read loop's `cancel_all` drained the entry.
    /// See [`AssistStreamRegistry::holds_running_gen`].
    pub(super) fn still_registered(&self) -> bool {
        self.registry.holds_running_gen(self.req_id, self.r#gen)
    }

    /// Send the ONE terminal `assist.done` frame this request owes its client
    /// — at `compose_with_length_retry`'s single exit, never per attempt. The
    /// popup's frame handler DELETES its `assist.chunk` listener for this
    /// `reqId` the moment it sees this frame, so a `done` emitted after a
    /// failed first attempt would drop every chunk of the retry (and leave
    /// the client's stall timer unable to re-arm) while the desktop streamed
    /// on into a sink nobody was reading.
    pub(super) async fn send_done(&mut self) {
        self.sink.send_frame(assist_done_frame(self.req_id)).await;
    }
}

/// Stream ONE compose attempt for `answer.assist` — see the module doc's
/// "Four ways a stream ends early" section for the full picture. Starts a
/// fresh job and binds it to `ctx`'s ALREADY-BEGUN registry entry (THIS
/// connection's own [`AssistStreamRegistry`] — so a client `assist.cancel`
/// can stop it early; does NOT `unregister` it on any path out of this
/// function — `handle_answer_assist` (in `answer_assist`) is the SOLE
/// unregister owner, once per request, at its single return point, so
/// `req_id` can never be clobbered by two cleanup sites racing over the same
/// key. See its doc), drives [`Completer::stream_complete`], and forwards
/// every visible-text delta through `ctx.sink` as a cap-clamped
/// `assist.chunk` frame (see [`forward_chunk`]), accumulating into
/// `ctx.forwarded` — from which the caller reads back THIS attempt's own
/// tail for the `answer.assist.result` terminal reply (see
/// [`super::answer_assist::attempt_text`]).
///
/// It does NOT send the terminal `assist.done` frame: that is
/// [`ComposeStream::send_done`], called once per REQUEST at the retry's
/// single exit (see its doc for the client-side listener this ordering
/// protects). Binding is generation-scoped
/// ([`AssistStreamRegistry::register`]), so a second attempt rebinds the
/// first attempt's entry and an attempt whose entry a cancel already removed
/// refuses to start at all.
///
/// `system`/`max_tokens` are CALLER-supplied (PR 11) rather than hardcoded to
/// [`super::answer_assist::ANSWER_ASSIST_SYSTEM`]/`ANSWER_ASSIST_MAX_TOKENS`
/// so a second prompt (rewrite mode's
/// [`super::answer_rewrite::REWRITE_SYSTEM`]) can reuse this SAME streaming
/// compose path instead of a parallel one — the draft caller passes the
/// draft system/cap unchanged, the rewrite caller passes its own.
///
/// `effort` is caller-supplied for the same reason, and for a second one: the
/// caller runs this function TWICE on one specific failure (see
/// `answer_assist::compose_with_length_retry`), and both attempts must be
/// driven by the SAME resolved tier. It is normally
/// [`Completer::low_effort`] — on a reasoning model the thinking tokens are
/// billed against `max_tokens`, so this path buys the cheapest reasoning the
/// provider offers; `None` (no cheap tier for this model) leaves the request
/// exactly as it was before this parameter existed.
///
/// `cap_base` is where THIS attempt's [`super::answer_assist::DRAFT_CAP`]
/// window starts in `ctx.forwarded` — the caller's own `drafted().len()`
/// snapshot for the attempt (see `answer_assist::compose_attempts`), so the
/// live cap counts only what this attempt forwards and a retry is never
/// clamped by prose a failed attempt already spent. One value, two uses: the
/// same snapshot is what the caller slices that attempt's text back out with.
///
/// Mechanism: `chat_stream` emits `ai:stream` Tauri events as it drives the
/// HTTP stream — the SAME channel the renderer's own provider hook listens
/// to. This registers a SECOND, Rust-side listener for this exact `job_id`
/// (`tauri::Listener`) and forwards each piece through `ctx.sink` instead of
/// into a webview. A synchronous listener callback can't itself `.await` a
/// socket write, so it pushes each event onto an unbounded channel that this
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
    ctx: &mut ComposeStream<'_>,
    max_tokens: u32,
    effort: Option<&str>,
    cap_base: usize,
) -> AppResult<()> {
    let app = ctx.app;
    let completer = ctx.completer;
    let req_id = ctx.req_id;
    let system = ctx.system;
    let user = ctx.user;

    // `start_and_register` starts the job BEFORE registering it — see its own
    // doc for the TOCTOU this order closes. `None` means this request no
    // longer owns its registry entry: an `assist.cancel` raced ahead during
    // the pre-compose window, or (between two attempts) cancelled this
    // request / dropped the whole connection. Either way the job
    // `start_and_register` itself just started has already been cancelled —
    // the client gave up, so never proceed into the stream loop.
    let Some(job_id) = start_and_register(app, ctx.registry, req_id, ctx.r#gen) else {
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

    let mut cap_reached = false;
    let mut sink_gone = false;
    let result: AppResult<()>;
    {
        let mut stream_fut = Box::pin(completer.stream_complete(
            &job_id,
            system,
            user,
            Some(0.5),
            Some(max_tokens),
            effort,
        ));
        loop {
            tokio::select! {
                maybe = rx.recv() => {
                    if let Some(chunk) = maybe {
                        let buf = &mut ctx.forwarded;
                        let out = forward_chunk(&chunk, req_id, ctx.sink, buf, cap_base).await;
                        match out {
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
            Ok(chunk) => {
                match forward_chunk(&chunk, req_id, ctx.sink, &mut ctx.forwarded, cap_base).await {
                    ForwardOutcome::Continue => {}
                    ForwardOutcome::CapReached => cap_reached = true,
                    ForwardOutcome::SinkGone => sink_gone = true,
                }
            }
            Err(_) => break,
        }
    }

    app.unlisten(listener_id);
    // No `registry.unregister(req_id)` here — see this function's doc:
    // `handle_answer_assist` is the SOLE unregister owner (one call, at its
    // single return point, covering every outcome of this function too). No
    // `assist.done` here either: that frame is per REQUEST, not per attempt
    // (see `ComposeStream::send_done`).

    if cap_reached {
        return Ok(());
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
    Ok(())
}

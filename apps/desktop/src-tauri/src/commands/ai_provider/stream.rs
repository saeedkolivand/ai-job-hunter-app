//! Shared streaming loop for the cloud chat adapters.
//!
//! Every cloud provider (`openai`, `anthropic`, `gemini`, `ollama`) repeated the
//! same `chat_stream` scaffold: lock the [`JobTracker`] to check cancellation at
//! the top of each chunk-read, drive the `response.chunk()` read loop
//! (`Ok(Some)`/`Ok(None)`/`Err`), emit one `ai:stream` event per delta, and call
//! [`job_complete`](crate::commands::jobs::job_complete) + [`RequestTrace::end`]
//! exactly once on completion/error. That control flow now lives **here, once**.
//!
//! What stays per-provider is *only* the wire framing: each adapter passes a
//! `parse` closure that drains its own accumulated byte buffer and yields
//! [`StreamPiece`]s. OpenAI/Anthropic are `data:`-prefixed SSE lines, Gemini is a
//! streamed JSON array, Ollama is newline-delimited JSON — the framing lives in
//! the closure, never here.
//!
//! Split into topic modules (R8 line-budget split): [`piece`] (the wire-agnostic
//! [`StreamPiece`] vocabulary), [`text`] (think-block stripping + UTF-8-safe
//! buffering), [`finish`] (the empty/truncated-completion messaging and the
//! one-shot terminal step) — this file keeps the loop itself: cancellation,
//! chunk reads, and [`stream_response`].

use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AiStreamChunk, AI_STREAM};
use crate::jobs::JobTracker;

use super::{ProviderId, RequestTrace, StopReason, Usage};

mod finish;
mod idle;
mod piece;
mod text;

#[cfg(test)]
pub(crate) use finish::empty_answer_error_for_test;
pub(crate) use finish::is_empty_answer_length_cut;
pub(super) use finish::EMPTY_ANSWER_MESSAGE;
#[cfg(test)]
pub(super) use idle::collect_canned;
pub(super) use idle::{collect, open, StreamLimits};
pub(super) use piece::StreamPiece;
pub(in crate::commands::ai_provider) use text::{push_utf8, strip_think_blocks};

use idle::IdleGuard;

/// Emit a single `ai:stream` delta for `job_id`.
fn emit_delta(app: &AppHandle, job_id: &str, delta: &str, thinking: bool) {
    emit_event(
        app,
        AI_STREAM,
        AiStreamChunk {
            job_id: job_id.to_string(),
            delta: delta.to_string(),
            done: false,
            error: None,
            thinking: if thinking { Some(true) } else { None },
        },
    );
}

/// Whether `job_id` has been cancelled.
fn is_cancelled(app: &AppHandle, job_id: &str) -> bool {
    app.state::<Mutex<JobTracker>>()
        .lock()
        .get(job_id)
        .map(|j| j.status == crate::jobs::JobStatus::Cancelled)
        .unwrap_or(false)
}

/// What the core stream loop should do after observing the current state.
/// Decoupled from `reqwest` and the `AppHandle` so the control flow
/// (cancel / emit / done / complete-on-end / error) is unit-testable with a fake
/// chunk source. See [`drive_stream`]. Test-only — production runs the inlined
/// loop in [`stream_response`].
#[cfg(test)]
enum StreamSink {
    /// Forward a decoded delta to the renderer.
    Emit { delta: String, thinking: bool },
    /// Provider's end-of-stream sentinel — emit terminal event + complete +
    /// record spend. Carries the LATEST [`Usage`] seen across the whole
    /// stream (mirroring `stream_response`/`finish`'s "last write wins" +
    /// "record once, at completion" behavior) AND the accumulated answer text
    /// (only non-thinking deltas — exactly what the renderer buffers) that
    /// [`finish`] think-strips via [`strip_think_blocks`] and persists into the
    /// job result as `result.text`.
    Complete(Usage, String),
    /// Cancelled mid-stream — fail with `"Job cancelled"`, no terminal event
    /// (mirroring production: no `job_complete`), but STILL carrying
    /// whatever [`Usage`] was last seen before the cancel, so it can be
    /// recorded — mirrors `stream_response`'s own cancellation branch
    /// (never estimated, zero when none was ever seen).
    Cancelled(Usage),
    /// Transport read error — no terminal event, but STILL carrying whatever
    /// [`Usage`] was last seen before the read failed (mirrors
    /// [`Self::Cancelled`]) — `stream_response`'s error branch is where the
    /// actual `record_usage` call for this sink lives, so a transport
    /// failure mid-stream no longer undercounts real, already-reported spend.
    Error(AppError, Usage),
}

/// Pure control-flow core, factored out so the loop (cancel / emit / done /
/// complete-on-end / error) is unit-testable with a fake chunk source — see the
/// tests. Pulls one chunk at a time from `next_chunk`, checks `cancelled` *before*
/// each pull, feeds bytes through `parse` (which must drain what it consumes), and
/// yields the resulting [`StreamSink`] actions via `on`. Returns once a sentinel
/// piece, cancellation, an error, or end-of-body is reached. End-of-body without a
/// sentinel still yields a trailing [`StreamSink::Complete`] (graceful close).
/// Tracks the LATEST [`StreamPiece::usage`] seen (mirroring `stream_response`'s
/// "last write wins") and carries it on [`StreamSink::Complete`],
/// [`StreamSink::Cancelled`], AND [`StreamSink::Error`] alike (mirroring
/// production: a cancellation OR a transport error still records whatever
/// REAL usage was already seen before it happened — never fabricated, never
/// silently dropped). [`stream_response`] mirrors this loop against a real
/// `reqwest::Response`.
#[cfg(test)]
async fn drive_stream<Cancel, Next, Fut, B, P>(
    mut cancelled: Cancel,
    mut next_chunk: Next,
    mut parse: P,
    mut on: impl FnMut(StreamSink),
) where
    Cancel: FnMut() -> bool,
    Next: FnMut() -> Fut,
    Fut: std::future::Future<Output = AppResult<Option<B>>>,
    B: AsRef<[u8]>,
    P: FnMut(&mut String) -> Vec<StreamPiece>,
{
    let mut buf = String::new();
    // Bytes from a read that ended mid-UTF-8-sequence — see `push_utf8`.
    let mut carry: Vec<u8> = Vec::new();
    let mut usage = Usage::default();
    let mut answer = String::new();
    loop {
        if cancelled() {
            on(StreamSink::Cancelled(usage));
            return;
        }
        match next_chunk().await {
            Ok(Some(bytes)) => {
                push_utf8(&mut buf, &mut carry, bytes.as_ref());
                for piece in parse(&mut buf) {
                    if let Some(u) = piece.usage {
                        usage = u;
                    }
                    if !piece.delta.is_empty() {
                        if !piece.thinking {
                            answer.push_str(&piece.delta);
                        }
                        on(StreamSink::Emit {
                            delta: piece.delta,
                            thinking: piece.thinking,
                        });
                    }
                    if piece.done {
                        on(StreamSink::Complete(usage, std::mem::take(&mut answer)));
                        return;
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                on(StreamSink::Error(e, usage));
                return;
            }
        }
    }
    on(StreamSink::Complete(usage, answer));
}

/// Drive a provider's streaming response to completion.
///
/// Owns the cancellation check, the `response.chunk()` read loop, byte buffering,
/// per-delta emission, and the one-shot complete/trace-end on done or end-of-body.
/// `parse` is the provider's only contribution: it is handed the accumulated
/// byte buffer (as a `&mut String`) and **must drain the bytes it consumes**,
/// leaving any partial trailing frame for the next call. It returns the pieces
/// decoded from the bytes it consumed this call.
///
/// On cancellation the response is dropped and the job fails with `"Job cancelled"`
/// (no terminal completion/`job_complete` is emitted) — but whatever REAL usage
/// the provider had already reported (e.g. the extension bridge's `answer.assist`
/// live `DRAFT_CAP`, or a user-initiated cancel mid-stream) is still recorded
/// against today's spend before returning, so a cost-capped or cancelled stream
/// is never invisible to spend tracking (never estimated, never fabricated —
/// zero when none was ever seen). A transport read error records that SAME
/// accumulated usage too, before the trace is closed and a [`AppError::Network`]
/// is returned — a provider that reports usage incrementally (Anthropic's
/// `message_delta`, Gemini's `usageMetadata`) may hold real, billable usage
/// even though the read itself then failed, and that must not be discarded
/// either. The two branches can never double-record: the cancellation check
/// runs BEFORE each read, the transport error is only ever seen INSIDE one.
/// Either way a provider that passes a correct `parse` closure can never
/// forget the cancellation check.
///
/// The body mirrors [`drive_stream`] (the tested control-flow core), kept as a
/// direct loop here so the returned `Future` stays `Send` (an async-trait
/// requirement — nothing non-`Send` is held across the `await`).
///
/// A read that stays silent for `limits.idle` (or a stream still running at
/// `limits.ceiling`) fails with `AppError::Timeout` — the same usage-recording
/// error branch as a transport failure — instead of the whole request being
/// bounded by one wall-clock deadline (#1353).
///
/// `provider`/`model`/`base_url` identify the call for spend recording only —
/// every [`StreamPiece::usage`] seen is remembered (last write wins, since
/// Anthropic reports usage incrementally and Gemini/Ollama repeat a running
/// total). [`finish`] records whatever was last seen (zero if the provider
/// never reported any) against today's AI spend on the normal completion
/// path; the cancellation branch below records that SAME accumulated value
/// directly (never through `finish`, which would also wrongly emit a
/// terminal `job_complete`).
#[allow(clippy::too_many_arguments)]
pub(super) async fn stream_response<F>(
    app: &AppHandle,
    job_id: &str,
    trace: &RequestTrace,
    mut response: reqwest::Response,
    status: u16,
    provider: ProviderId,
    model: &str,
    base_url: &str,
    limits: StreamLimits,
    mut parse: F,
) -> AppResult<()>
where
    F: FnMut(&mut String) -> Vec<StreamPiece> + Send,
{
    log::info!(
        "[ai] stream start job={} provider={} model={}",
        job_id,
        provider.as_str(),
        model
    );
    let mut buf = String::new();
    // Bytes from a read that ended mid-UTF-8-sequence — see `push_utf8`.
    let mut carry: Vec<u8> = Vec::new();
    let mut usage = Usage::default();
    // The latest `finish_reason` seen (last write wins, mirroring `usage`) —
    // see `StreamPiece::stop_reason` and `finish`'s empty-answer branch.
    let mut stop_reason: Option<StopReason> = None;
    // The full completed answer, accumulated from non-thinking deltas only —
    // the same shape the renderer buffers — persisted by `finish` so a dropped
    // frame or missed `done` event can be recovered by polling. A cancel/error
    // never reaches `finish`, so the partial answer is intentionally discarded
    // on those paths (the renderer fails the run rather than persisting a
    // truncated result).
    let mut answer = String::new();
    // Char count of every thinking-flagged delta seen — diagnostic only (never
    // the text itself), logged by `finish` so a support bundle can see a
    // reasoning model was actively thinking even when it never reached an
    // answer.
    let mut thinking_len: usize = 0;
    let guard = IdleGuard::start(limits);
    loop {
        if is_cancelled(app, job_id) {
            drop(response);
            trace.end(Some(status), false);
            // Record whatever REAL usage the provider had already reported
            // before the cancel was observed — never estimated, zero when
            // none was ever seen. See the doc above: this is what makes a
            // cost-capped (or user-cancelled) generation visible to spend
            // tracking instead of silently recording nothing.
            //
            // This is only ever non-zero for providers that report usage
            // INCREMENTALLY mid-stream (Anthropic's `message_delta`,
            // Gemini's `usageMetadata`) — a cap/early cancel for
            // OpenAI/Ollama (which only attach usage to their end-of-stream
            // piece, never seen if cancelled first) legitimately records
            // zero here. That is the honest never-estimate behavior working
            // as intended, not a gap to "fix" by estimating from tokens
            // seen so far.
            //
            // Mirrored (and asserted) by
            // `cancellation_after_a_usage_piece_still_carries_the_partial_usage`
            // in `drive_stream`'s test-only core below — that test is where
            // the assertion for this exact call site lives.
            super::record_usage(app, provider.as_str(), model, usage, Some(base_url));
            return Err(AppError::Message("Job cancelled".to_string()));
        }

        match guard.read(response.chunk()).await {
            Ok(Some(bytes)) => {
                push_utf8(&mut buf, &mut carry, &bytes);
                for piece in parse(&mut buf) {
                    if let Some(raw) = piece.error {
                        // An in-band upstream error frame on a 200 stream: fail like
                        // the transport-error branch below instead of finishing a
                        // partial draft as a success.
                        trace.end(Some(status), false);
                        super::record_usage(app, provider.as_str(), model, usage, Some(base_url));
                        return Err(idle::frame_error(provider.as_str(), &raw));
                    }
                    if let Some(u) = piece.usage {
                        usage = u;
                    }
                    if let Some(r) = piece.stop_reason {
                        stop_reason = Some(r);
                    }
                    if !piece.delta.is_empty() && !piece.refusal {
                        if piece.thinking {
                            thinking_len += piece.delta.chars().count();
                        } else {
                            answer.push_str(&piece.delta);
                        }
                        emit_delta(app, job_id, &piece.delta, piece.thinking);
                    }
                    if piece.done {
                        return finish::finish(
                            app,
                            job_id,
                            trace,
                            status,
                            provider,
                            model,
                            base_url,
                            usage,
                            &answer,
                            stop_reason,
                            thinking_len,
                        );
                    }
                }
            }
            Ok(None) => break,
            Err(stop) => {
                trace.end(Some(status), false);
                // Record whatever REAL usage the provider had already
                // reported before the read failed — mirrors the
                // cancellation branch above; mutually exclusive with it
                // (this only runs INSIDE a read, that only runs BEFORE
                // one), so a single stream can never double-record. Never
                // estimated, zero when none was ever seen.
                super::record_usage(app, provider.as_str(), model, usage, Some(base_url));
                // Idle/ceiling are `Timeout`; a reqwest timer firing mid-body is
                // the ceiling too (it is set to it — see `idle::open`).
                return Err(stop.into_app(provider.as_str(), limits, |e| {
                    if e.is_timeout() {
                        idle::ceiling_error(provider.as_str(), limits.ceiling)
                    } else {
                        AppError::Network(format!("Stream error: {e}"))
                    }
                }));
            }
        }
    }

    finish::finish(
        app,
        job_id,
        trace,
        status,
        provider,
        model,
        base_url,
        usage,
        &answer,
        stop_reason,
        thinking_len,
    )
}

#[cfg(test)]
mod tests;

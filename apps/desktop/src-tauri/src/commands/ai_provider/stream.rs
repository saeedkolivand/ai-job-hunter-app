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

use parking_lot::Mutex;
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AiStreamChunk, AI_STREAM};
use crate::jobs::JobTracker;

use super::{ProviderId, RequestTrace, Usage};

/// One emittable piece pulled from a provider's stream by its `parse` closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamPiece {
    /// Text to forward to the renderer. Empty pieces are skipped (so a parser can
    /// signal `done` without text).
    pub delta: String,
    /// `true` for reasoning/thinking deltas, `false` for normal answer text.
    pub thinking: bool,
    /// `true` when this piece marks the provider's own end-of-stream sentinel
    /// (OpenAI `[DONE]`, Anthropic `message_stop`, Ollama `done:true`). The loop
    /// emits the terminal event and returns after processing this piece.
    pub done: bool,
    /// The provider's REAL token usage, when this piece happens to carry it
    /// (OpenAI's separate `stream_options.include_usage` chunk, Anthropic's
    /// `message_start`/`message_delta` events, Gemini's `usageMetadata`,
    /// Ollama's final `done:true` object). The shared loop remembers the
    /// LATEST non-`None` value it sees across the whole stream and records it
    /// at completion — never estimated, never fabricated when absent.
    pub usage: Option<Usage>,
}

impl StreamPiece {
    /// A normal answer-text delta.
    pub fn text(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            thinking: false,
            done: false,
            usage: None,
        }
    }

    /// A reasoning/thinking delta.
    pub fn thinking(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            thinking: true,
            done: false,
            usage: None,
        }
    }

    /// The provider's end-of-stream sentinel, optionally carrying a final delta.
    pub fn done(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            thinking: false,
            done: true,
            usage: None,
        }
    }

    /// A usage-only piece: no visible text, not a completion sentinel — just
    /// reports the provider's real token usage as it becomes known (mid-stream
    /// for OpenAI/Gemini, incrementally for Anthropic, or attached directly to
    /// the `done` piece for Ollama).
    pub fn usage(usage: Usage) -> Self {
        Self {
            delta: String::new(),
            thinking: false,
            done: false,
            usage: Some(usage),
        }
    }
}

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

/// Emit the terminal `ai:stream` event, mark the job complete, close the
/// trace, and record the stream's REAL token usage (zero when the provider
/// never reported any) against today's AI spend. `base_url` is passed through
/// to the free/paid cost gate — only meaningful for `openai-compatible`
/// (LM Studio/vLLM/OpenRouter/…), ignored for every other provider.
#[allow(clippy::too_many_arguments)]
fn finish(
    app: &AppHandle,
    job_id: &str,
    trace: &RequestTrace,
    status: u16,
    provider: ProviderId,
    model: &str,
    base_url: &str,
    usage: Usage,
) {
    emit_event(
        app,
        AI_STREAM,
        AiStreamChunk {
            job_id: job_id.to_string(),
            delta: String::new(),
            done: true,
            error: None,
            thinking: None,
        },
    );
    crate::commands::jobs::job_complete(app, job_id, json!({ "done": true }));
    trace.end(Some(status), true);
    super::record_usage(
        app,
        provider.as_str(),
        model,
        usage.input_tokens,
        usage.output_tokens,
        Some(base_url),
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
    /// "record once, at completion" behavior).
    Complete(Usage),
    /// Cancelled mid-stream — fail with `"Job cancelled"`, no terminal event
    /// (mirroring production: no `job_complete`), but STILL carrying
    /// whatever [`Usage`] was last seen before the cancel, so it can be
    /// recorded — mirrors `stream_response`'s own cancellation branch
    /// (never estimated, zero when none was ever seen).
    Cancelled(Usage),
    /// Transport read error — no terminal event and no spend recorded.
    Error(AppError),
}

/// Pure control-flow core, factored out so the loop (cancel / emit / done /
/// complete-on-end / error) is unit-testable with a fake chunk source — see the
/// tests. Pulls one chunk at a time from `next_chunk`, checks `cancelled` *before*
/// each pull, feeds bytes through `parse` (which must drain what it consumes), and
/// yields the resulting [`StreamSink`] actions via `on`. Returns once a sentinel
/// piece, cancellation, an error, or end-of-body is reached. End-of-body without a
/// sentinel still yields a trailing [`StreamSink::Complete`] (graceful close).
/// Tracks the LATEST [`StreamPiece::usage`] seen (mirroring `stream_response`'s
/// "last write wins") and carries it on both [`StreamSink::Complete`] AND
/// [`StreamSink::Cancelled`] (mirroring production: a cancellation still
/// records whatever was seen) — only a transport error never reaches either
/// branch, so (mirroring production) nothing is ever recorded on THAT path.
/// [`stream_response`] mirrors this loop against a real `reqwest::Response`.
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
    let mut usage = Usage::default();
    loop {
        if cancelled() {
            on(StreamSink::Cancelled(usage));
            return;
        }
        match next_chunk().await {
            Ok(Some(bytes)) => {
                buf.push_str(&String::from_utf8_lossy(bytes.as_ref()));
                for piece in parse(&mut buf) {
                    if let Some(u) = piece.usage {
                        usage = u;
                    }
                    if !piece.delta.is_empty() {
                        on(StreamSink::Emit {
                            delta: piece.delta,
                            thinking: piece.thinking,
                        });
                    }
                    if piece.done {
                        on(StreamSink::Complete(usage));
                        return;
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                on(StreamSink::Error(e));
                return;
            }
        }
    }
    on(StreamSink::Complete(usage));
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
/// zero when none was ever seen). On a transport read error the trace is
/// closed and a [`AppError::Network`] is returned; usage recording is left
/// untouched on that path (out of scope for this fix — a read failure is a
/// separate, rarer case than the cancellation this was written for). Either
/// way a provider that passes a correct `parse` closure can never forget the
/// cancellation check.
///
/// The body mirrors [`drive_stream`] (the tested control-flow core), kept as a
/// direct loop here so the returned `Future` stays `Send` (an async-trait
/// requirement — nothing non-`Send` is held across the `await`).
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
pub async fn stream_response<F>(
    app: &AppHandle,
    job_id: &str,
    trace: &RequestTrace,
    mut response: reqwest::Response,
    status: u16,
    provider: ProviderId,
    model: &str,
    base_url: &str,
    mut parse: F,
) -> AppResult<()>
where
    F: FnMut(&mut String) -> Vec<StreamPiece> + Send,
{
    let mut buf = String::new();
    let mut usage = Usage::default();
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
            super::record_usage(
                app,
                provider.as_str(),
                model,
                usage.input_tokens,
                usage.output_tokens,
                Some(base_url),
            );
            return Err(AppError::Message("Job cancelled".to_string()));
        }

        match response.chunk().await {
            Ok(Some(bytes)) => {
                buf.push_str(&String::from_utf8_lossy(&bytes));
                for piece in parse(&mut buf) {
                    if let Some(u) = piece.usage {
                        usage = u;
                    }
                    if !piece.delta.is_empty() {
                        emit_delta(app, job_id, &piece.delta, piece.thinking);
                    }
                    if piece.done {
                        finish(app, job_id, trace, status, provider, model, base_url, usage);
                        return Ok(());
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                trace.end(Some(status), false);
                return Err(AppError::Network(format!("Stream error: {e}")));
            }
        }
    }

    finish(app, job_id, trace, status, provider, model, base_url, usage);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn stream_piece_constructors_set_flags() {
        let t = StreamPiece::text("hi");
        assert_eq!(t.delta, "hi");
        assert!(!t.thinking);
        assert!(!t.done);

        let r = StreamPiece::thinking("reasoning");
        assert!(r.thinking);
        assert!(!r.done);

        let d = StreamPiece::done("");
        assert!(d.done);
        assert!(d.delta.is_empty());
    }

    /// Collect the sink actions `drive_stream` produces for a canned chunk list.
    /// Each piece is identified by `(emit:delta/thinking, complete, cancelled, error)`.
    /// `Complete` carries the final [`Usage`] — see the usage-tracking tests below.
    #[derive(Debug, PartialEq)]
    enum Act {
        Emit(String, bool),
        Complete(Usage),
        Cancelled(Usage),
        Error(String),
    }

    fn run(
        chunks: Vec<AppResult<Option<Vec<u8>>>>,
        cancel_after: Option<usize>,
        parse: impl FnMut(&mut String) -> Vec<StreamPiece>,
    ) -> Vec<Act> {
        let acts = std::cell::RefCell::new(Vec::new());
        let idx = Cell::new(0usize);
        let mut chunks = chunks.into_iter();
        let cancel_calls = Cell::new(0usize);

        let mut cancelled = || {
            let n = cancel_calls.get();
            cancel_calls.set(n + 1);
            cancel_after.map(|after| n >= after).unwrap_or(false)
        };

        let fut = drive_stream(
            &mut cancelled,
            || {
                let _ = idx.get();
                let next = chunks.next().unwrap_or(Ok(None));
                async move { next }
            },
            parse,
            |sink| {
                let act = match sink {
                    StreamSink::Emit { delta, thinking } => Act::Emit(delta, thinking),
                    StreamSink::Complete(usage) => Act::Complete(usage),
                    StreamSink::Cancelled(usage) => Act::Cancelled(usage),
                    StreamSink::Error(e) => Act::Error(e.to_string()),
                };
                acts.borrow_mut().push(act);
            },
        );
        // The future is synchronous (the fake chunk source resolves immediately).
        futures::executor::block_on(fut);
        acts.into_inner()
    }

    /// A trivial newline-delimited parser: each complete line becomes a text piece;
    /// a line equal to `END` is the sentinel.
    fn line_parser(buf: &mut String) -> Vec<StreamPiece> {
        let mut out = Vec::new();
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            *buf = buf[nl + 1..].to_string();
            if line == "END" {
                out.push(StreamPiece::done(""));
            } else if !line.is_empty() {
                out.push(StreamPiece::text(line));
            }
        }
        out
    }

    #[test]
    fn emits_pieces_then_completes_on_sentinel() {
        let acts = run(
            vec![
                Ok(Some(b"hello\nwor".to_vec())),
                Ok(Some(b"ld\nEND\n".to_vec())),
            ],
            None,
            line_parser,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("hello".to_string(), false),
                Act::Emit("world".to_string(), false),
                Act::Complete(Usage::default()),
            ]
        );
    }

    #[test]
    fn completes_once_on_end_of_body_without_sentinel() {
        // No `END` line — the loop still completes exactly once when the body ends.
        let acts = run(
            vec![Ok(Some(b"a\nb\n".to_vec())), Ok(None)],
            None,
            line_parser,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("a".to_string(), false),
                Act::Emit("b".to_string(), false),
                Act::Complete(Usage::default()),
            ]
        );
    }

    #[test]
    fn cancellation_short_circuits_before_reading() {
        // Cancelled on the first check → no chunk is read, no completion
        // emitted, and no usage was ever seen (zero, not fabricated).
        let acts = run(
            vec![Ok(Some(b"hello\nEND\n".to_vec()))],
            Some(0),
            line_parser,
        );
        assert_eq!(acts, vec![Act::Cancelled(Usage::default())]);
    }

    #[test]
    fn cancellation_mid_stream_stops_without_complete() {
        // First check passes (reads + emits), second check cancels before the next read.
        let acts = run(
            vec![Ok(Some(b"hello\n".to_vec())), Ok(Some(b"world\n".to_vec()))],
            Some(1),
            line_parser,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("hello".to_string(), false),
                Act::Cancelled(Usage::default())
            ]
        );
    }

    #[test]
    fn read_error_surfaces_and_stops() {
        let acts = run(
            vec![
                Ok(Some(b"a\n".to_vec())),
                Err(AppError::Message("boom".to_string())),
            ],
            None,
            line_parser,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("a".to_string(), false),
                Act::Error("boom".to_string()),
            ]
        );
    }

    #[test]
    fn final_delta_on_sentinel_is_emitted_before_complete() {
        // A sentinel piece that also carries text emits the text, then completes.
        let parser = |buf: &mut String| -> Vec<StreamPiece> {
            let s = std::mem::take(buf);
            if s.is_empty() {
                vec![]
            } else {
                vec![StreamPiece::done(s)]
            }
        };
        let acts = run(vec![Ok(Some(b"tail".to_vec()))], None, parser);
        assert_eq!(
            acts,
            vec![
                Act::Emit("tail".to_string(), false),
                Act::Complete(Usage::default())
            ]
        );
    }

    // ── Usage tracking: latest-wins + record-once-at-completion ────────────────

    #[test]
    fn a_later_usage_piece_overwrites_an_earlier_one_at_completion() {
        // Two usage-only pieces (no delta) arrive across two chunks, then the
        // sentinel — `Complete` must carry only the LAST usage seen, mirroring
        // Anthropic's incremental `message_start`/`message_delta` reporting and
        // Gemini/Ollama repeating a running total.
        let parser = |buf: &mut String| -> Vec<StreamPiece> {
            let mut out = Vec::new();
            while let Some(nl) = buf.find('\n') {
                let line = buf[..nl].trim().to_string();
                *buf = buf[nl + 1..].to_string();
                match line.as_str() {
                    "USAGE1" => out.push(StreamPiece::usage(Usage {
                        input_tokens: 10,
                        output_tokens: 1,
                    })),
                    "USAGE2" => out.push(StreamPiece::usage(Usage {
                        input_tokens: 10,
                        output_tokens: 99,
                    })),
                    "END" => out.push(StreamPiece::done("")),
                    _ => {}
                }
            }
            out
        };
        let acts = run(
            vec![
                Ok(Some(b"USAGE1\n".to_vec())),
                Ok(Some(b"USAGE2\nEND\n".to_vec())),
            ],
            None,
            parser,
        );
        assert_eq!(
            acts,
            vec![Act::Complete(Usage {
                input_tokens: 10,
                output_tokens: 99,
            })],
            "only the LAST usage piece must be recorded, not the first or a sum"
        );
    }

    #[test]
    fn cancellation_after_a_usage_piece_still_carries_the_partial_usage() {
        // A usage piece arrives, then cancellation (e.g. `answer.assist`'s
        // live DRAFT_CAP calling `job_cancel`) — production now records
        // whatever REAL usage was already seen even on the `Cancelled` sink
        // (never through `Complete`/`finish`, which would also wrongly emit
        // a terminal `job_complete`), so a cost-capped generation is never
        // invisible to spend tracking.
        let parser = |buf: &mut String| -> Vec<StreamPiece> {
            let mut out = Vec::new();
            while let Some(nl) = buf.find('\n') {
                *buf = buf[nl + 1..].to_string();
                out.push(StreamPiece::usage(Usage {
                    input_tokens: 50,
                    output_tokens: 50,
                }));
            }
            out
        };
        let acts = run(
            vec![Ok(Some(b"USAGE\n".to_vec())), Ok(Some(b"USAGE\n".to_vec()))],
            Some(1),
            parser,
        );
        assert_eq!(
            acts,
            vec![Act::Cancelled(Usage {
                input_tokens: 50,
                output_tokens: 50,
            })],
            "cancellation must still carry the REAL usage already seen, never fabricated but never silently dropped either"
        );
    }

    #[test]
    fn transport_error_after_a_usage_piece_records_nothing() {
        // Same shape, but the stream fails with a read error instead of a
        // cancellation — production only records spend on the `Complete` sink,
        // which a transport error never reaches either.
        let parser = |buf: &mut String| -> Vec<StreamPiece> {
            let mut out = Vec::new();
            while let Some(nl) = buf.find('\n') {
                *buf = buf[nl + 1..].to_string();
                out.push(StreamPiece::usage(Usage {
                    input_tokens: 50,
                    output_tokens: 50,
                }));
            }
            out
        };
        let acts = run(
            vec![
                Ok(Some(b"USAGE\n".to_vec())),
                Err(AppError::Message("boom".to_string())),
            ],
            None,
            parser,
        );
        assert_eq!(
            acts,
            vec![Act::Error("boom".to_string())],
            "a transport error must never emit Complete, so no spend is ever recorded"
        );
    }
}

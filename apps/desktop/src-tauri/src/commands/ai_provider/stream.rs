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
/// never reported any) against today's AI spend.
#[allow(clippy::too_many_arguments)]
fn finish(
    app: &AppHandle,
    job_id: &str,
    trace: &RequestTrace,
    status: u16,
    provider: ProviderId,
    model: &str,
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
    crate::spend::record_usage(
        app,
        provider.as_str(),
        model,
        usage.input_tokens,
        usage.output_tokens,
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
    /// Provider's end-of-stream sentinel — emit terminal event + complete.
    Complete,
    /// Cancelled mid-stream — fail with `"Job cancelled"`, no terminal event.
    Cancelled,
    /// Transport read error.
    Error(AppError),
}

/// Pure control-flow core, factored out so the loop (cancel / emit / done /
/// complete-on-end / error) is unit-testable with a fake chunk source — see the
/// tests. Pulls one chunk at a time from `next_chunk`, checks `cancelled` *before*
/// each pull, feeds bytes through `parse` (which must drain what it consumes), and
/// yields the resulting [`StreamSink`] actions via `on`. Returns once a sentinel
/// piece, cancellation, an error, or end-of-body is reached. End-of-body without a
/// sentinel still yields a trailing [`StreamSink::Complete`] (graceful close).
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
    loop {
        if cancelled() {
            on(StreamSink::Cancelled);
            return;
        }
        match next_chunk().await {
            Ok(Some(bytes)) => {
                buf.push_str(&String::from_utf8_lossy(bytes.as_ref()));
                for piece in parse(&mut buf) {
                    if !piece.delta.is_empty() {
                        on(StreamSink::Emit {
                            delta: piece.delta,
                            thinking: piece.thinking,
                        });
                    }
                    if piece.done {
                        on(StreamSink::Complete);
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
    on(StreamSink::Complete);
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
/// (no terminal completion is emitted). On a transport read error the trace is
/// closed and a [`AppError::Network`] is returned. Either way a provider that
/// passes a correct `parse` closure can never forget the cancellation check.
///
/// The body mirrors [`drive_stream`] (the tested control-flow core), kept as a
/// direct loop here so the returned `Future` stays `Send` (an async-trait
/// requirement — nothing non-`Send` is held across the `await`).
///
/// `provider`/`model` identify the call for spend recording only — every
/// [`StreamPiece::usage`] seen is remembered (last write wins, since Anthropic
/// reports usage incrementally and Gemini/Ollama repeat a running total), and
/// [`finish`] records whatever was last seen (zero if the provider never
/// reported any) against today's AI spend.
#[allow(clippy::too_many_arguments)]
pub async fn stream_response<F>(
    app: &AppHandle,
    job_id: &str,
    trace: &RequestTrace,
    mut response: reqwest::Response,
    status: u16,
    provider: ProviderId,
    model: &str,
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
                        finish(app, job_id, trace, status, provider, model, usage);
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

    finish(app, job_id, trace, status, provider, model, usage);
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
    #[derive(Debug, PartialEq)]
    enum Act {
        Emit(String, bool),
        Complete,
        Cancelled,
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
                    StreamSink::Complete => Act::Complete,
                    StreamSink::Cancelled => Act::Cancelled,
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
                Act::Complete,
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
                Act::Complete,
            ]
        );
    }

    #[test]
    fn cancellation_short_circuits_before_reading() {
        // Cancelled on the first check → no chunk is read, no completion emitted.
        let acts = run(
            vec![Ok(Some(b"hello\nEND\n".to_vec()))],
            Some(0),
            line_parser,
        );
        assert_eq!(acts, vec![Act::Cancelled]);
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
            vec![Act::Emit("hello".to_string(), false), Act::Cancelled]
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
            vec![Act::Emit("tail".to_string(), false), Act::Complete]
        );
    }
}

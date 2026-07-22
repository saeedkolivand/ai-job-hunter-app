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
///
/// `answer` is the full completed text accumulated from this stream's
/// **non-thinking** deltas (see `stream_response`'s loop) — exactly what the
/// renderer feeds its `<think>` splitter. It is persisted into the job result
/// as `result.text` so a renderer that missed stream frames (or the terminal
/// `done` event) can recover the finished document by polling `jobs_get`
/// instead of resolving a truncated stream buffer. This is provider-agnostic:
/// every provider routes through here, so a new adapter inherits the behavior
/// for free. Before persisting, inline `<think>…</think>` reasoning is stripped
/// via [`strip_think_blocks`] so the persisted text is the SAME think-stripped
/// shape the renderer assembles — critical because the renderer's poll fallback
/// prefers the LONGER of {persisted, streamed buffer}, and its streamed buffer
/// is already think-stripped. Persisting raw `<think>` markup would make the
/// persisted side spuriously longer AND leak reasoning markup into the final
/// document; stripping here keeps both sides of that length comparison in the
/// same shape.
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
    answer: &str,
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
    crate::commands::jobs::job_complete(
        app,
        job_id,
        json!({ "done": true, "text": strip_think_blocks(answer) }),
    );
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

/// Remove inline `<think>…</think>` reasoning blocks, mirroring the renderer's
/// `createThinkSplitter` (`renderer/lib/generate/think-split.ts`) so the
/// persisted answer text is byte-for-byte the shape the renderer assembles from
/// the live stream. Local reasoning models (DeepSeek-R1, Qwen3, …) embed the
/// tags directly in their answer content; cloud providers flag reasoning
/// structurally (those deltas never reach `answer` in the first place, since the
/// loop only accumulates non-thinking deltas), so for them this is a no-op.
///
/// Semantics match the splitter's final output exactly: text outside a block is
/// kept, text inside a `<think>…</think>` pair is dropped, and an UNTERMINATED
/// `<think>` (no closing tag) discards everything from that tag onward — the
/// splitter drops an unterminated block at `flush()`. Because the whole answer
/// is stripped in one pass here (not incrementally across deltas), a `</think>`
/// split across two stream frames — which the renderer's streaming splitter can
/// mis-handle — is resolved correctly, so the persisted text can only ever be
/// equal-or-more-correct than the buffer, and never contains reasoning markup.
///
/// Shared with the CLI-agent streaming path (`cli_agent::run_stream`), which has
/// its own subprocess transport but persists the completed answer the SAME way,
/// so a CLI-agent generation's poll fallback works too and no provider path can
/// leak `<think>` markup.
pub(super) fn strip_think_blocks(text: &str) -> String {
    const OPEN: &str = "<think>";
    const CLOSE: &str = "</think>";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        match rest.find(OPEN) {
            Some(open) => {
                out.push_str(&rest[..open]);
                let after = &rest[open + OPEN.len()..];
                match after.find(CLOSE) {
                    Some(close) => rest = &after[close + CLOSE.len()..],
                    // Unterminated block — the renderer discards it at flush; drop the rest.
                    None => break,
                }
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    out
}

/// Append one transport read to `buf` as UTF-8, holding back an incomplete
/// trailing sequence in `carry` for the next read.
///
/// `reqwest::Response::chunk` splits the body at arbitrary byte offsets — a
/// chunked-transfer body surfaces as the socket reads land, and h2 DATA frames
/// are cut wherever the server flushed. So one multi-byte character (an em dash,
/// a curly quote, an accented letter, an emoji) routinely straddles two reads.
/// Decoding each read on its own with `String::from_utf8_lossy` replaced BOTH
/// halves with `U+FFFD`, and since a replacement char is legal inside a JSON
/// string the frame still parsed — the mojibake was forwarded to the renderer
/// and persisted as the finished document, with no error anywhere.
///
/// Genuinely invalid bytes (a corrupt transfer, never a provider) still collapse
/// to a single `U+FFFD` and are skipped, so a malformed stream can never stall
/// the loop.
pub(super) fn push_utf8(buf: &mut String, carry: &mut Vec<u8>, bytes: &[u8]) {
    carry.extend_from_slice(bytes);
    loop {
        match std::str::from_utf8(carry) {
            Ok(text) => {
                buf.push_str(text);
                carry.clear();
                return;
            }
            Err(e) => {
                let valid = e.valid_up_to();
                // `valid_up_to()` is by definition a valid UTF-8 prefix.
                buf.push_str(std::str::from_utf8(&carry[..valid]).unwrap_or_default());
                match e.error_len() {
                    // A truly invalid sequence: emit one replacement char, skip
                    // it, and keep decoding the rest of this read.
                    Some(n) => {
                        buf.push(char::REPLACEMENT_CHARACTER);
                        carry.drain(..valid + n);
                    }
                    // An incomplete tail: hold it back for the next read.
                    None => {
                        carry.drain(..valid);
                        return;
                    }
                }
            }
        }
    }
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
    // Bytes from a read that ended mid-UTF-8-sequence — see `push_utf8`.
    let mut carry: Vec<u8> = Vec::new();
    let mut usage = Usage::default();
    // The full completed answer, accumulated from non-thinking deltas only —
    // the same shape the renderer buffers — persisted by `finish` so a dropped
    // frame or missed `done` event can be recovered by polling. A cancel/error
    // never reaches `finish`, so the partial answer is intentionally discarded
    // on those paths (the renderer fails the run rather than persisting a
    // truncated result).
    let mut answer = String::new();
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
                push_utf8(&mut buf, &mut carry, &bytes);
                for piece in parse(&mut buf) {
                    if let Some(u) = piece.usage {
                        usage = u;
                    }
                    if !piece.delta.is_empty() {
                        if !piece.thinking {
                            answer.push_str(&piece.delta);
                        }
                        emit_delta(app, job_id, &piece.delta, piece.thinking);
                    }
                    if piece.done {
                        finish(
                            app, job_id, trace, status, provider, model, base_url, usage, &answer,
                        );
                        return Ok(());
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                trace.end(Some(status), false);
                // Record whatever REAL usage the provider had already
                // reported before the read failed — mirrors the
                // cancellation branch above; mutually exclusive with it
                // (this only runs INSIDE a read, that only runs BEFORE
                // one), so a single stream can never double-record. Never
                // estimated, zero when none was ever seen.
                super::record_usage(
                    app,
                    provider.as_str(),
                    model,
                    usage.input_tokens,
                    usage.output_tokens,
                    Some(base_url),
                );
                return Err(AppError::Network(format!("Stream error: {e}")));
            }
        }
    }

    finish(
        app, job_id, trace, status, provider, model, base_url, usage, &answer,
    );
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
    /// `Complete` carries the final [`Usage`] AND the accumulated answer text
    /// (non-thinking deltas only) that `finish` persists — see the usage- and
    /// answer-tracking tests below.
    #[derive(Debug, PartialEq)]
    enum Act {
        Emit(String, bool),
        Complete(Usage, String),
        Cancelled(Usage),
        Error(String, Usage),
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
                    StreamSink::Complete(usage, answer) => Act::Complete(usage, answer),
                    StreamSink::Cancelled(usage) => Act::Cancelled(usage),
                    StreamSink::Error(e, usage) => Act::Error(e.to_string(), usage),
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
                Act::Complete(Usage::default(), "helloworld".to_string()),
            ]
        );
    }

    #[test]
    fn a_multibyte_char_split_across_reads_is_not_corrupted() {
        // `response.chunk()` cuts the body at arbitrary byte offsets, so one
        // multi-byte char routinely straddles two reads. Decoding each read on
        // its own turned BOTH halves into U+FFFD, and the mojibake was persisted
        // as the finished document. The em dash here is E2 80 94, cut 1|2.
        let acts = run(
            vec![
                Ok(Some(vec![b'a', 0xE2])),
                Ok(Some(vec![0x80, 0x94, b'b', b'\n'])),
                Ok(None),
            ],
            None,
            line_parser,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("a\u{2014}b".to_string(), false),
                Act::Complete(Usage::default(), "a\u{2014}b".to_string()),
            ]
        );
    }

    #[test]
    fn a_multibyte_char_split_2_1_and_across_three_reads_is_not_corrupted() {
        // Same char cut 2|1, plus a 4-byte emoji (F0 9F 9A 80) dribbled one byte
        // per read — the carry must survive an arbitrary number of empty-yield
        // reads, not just one.
        let acts = run(
            vec![
                Ok(Some(vec![0xE2, 0x80])),
                Ok(Some(vec![0x94])),
                Ok(Some(vec![0xF0])),
                Ok(Some(vec![0x9F])),
                Ok(Some(vec![0x9A])),
                Ok(Some(vec![0x80, b'\n'])),
                Ok(None),
            ],
            None,
            line_parser,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("\u{2014}\u{1F680}".to_string(), false),
                Act::Complete(Usage::default(), "\u{2014}\u{1F680}".to_string()),
            ]
        );
    }

    #[test]
    fn genuinely_invalid_bytes_still_collapse_to_one_replacement_char() {
        // A corrupt transfer (never a provider) must not stall the loop: an
        // invalid sequence becomes exactly one U+FFFD and decoding continues.
        let acts = run(
            vec![Ok(Some(vec![b'a', 0xFF, b'b', b'\n'])), Ok(None)],
            None,
            line_parser,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("a\u{FFFD}b".to_string(), false),
                Act::Complete(Usage::default(), "a\u{FFFD}b".to_string()),
            ]
        );
    }

    #[test]
    fn an_incomplete_trailing_sequence_at_end_of_body_does_not_hang() {
        // The body ends mid-character: the held-back bytes are simply dropped and
        // the loop still completes exactly once.
        let acts = run(
            vec![Ok(Some(vec![b'a', b'\n', 0xE2])), Ok(None)],
            None,
            line_parser,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("a".to_string(), false),
                Act::Complete(Usage::default(), "a".to_string()),
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
                Act::Complete(Usage::default(), "ab".to_string()),
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
                Act::Error("boom".to_string(), Usage::default()),
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
                Act::Complete(Usage::default(), "tail".to_string())
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
            vec![Act::Complete(
                Usage {
                    input_tokens: 10,
                    output_tokens: 99,
                },
                // Usage-only pieces carry no visible delta, so the persisted
                // answer is empty here.
                String::new(),
            )],
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
    fn transport_error_after_a_usage_piece_still_carries_the_partial_usage() {
        // Same shape as the cancellation test above, but the stream fails
        // with a read error instead — production now records whatever REAL
        // usage was already seen on this path too (see `stream_response`'s
        // error branch), so a transport failure mid-stream no longer
        // undercounts spend the provider already reported.
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
            vec![Act::Error(
                "boom".to_string(),
                Usage {
                    input_tokens: 50,
                    output_tokens: 50,
                }
            )],
            "a transport error must still carry the REAL usage already seen, never fabricated \
             but never silently dropped either"
        );
    }

    // ── Persisted answer text (the poll-fallback contract) ─────────────────────
    //
    // `finish` persists the accumulated answer as `result.text` so a renderer
    // that missed stream frames or the terminal `done` event recovers the
    // finished document by polling `jobs_get`. These tests pin the two
    // properties that make that safe: (1) only NON-thinking deltas contribute
    // (reasoning is never persisted), and (2) inline `<think>…</think>` markup
    // is stripped, so the poll fallback's longer-wins branch can never resolve
    // reasoning markup into the final document.

    #[test]
    fn complete_carries_only_non_thinking_answer_text() {
        // A parser marking `T:`-prefixed lines as reasoning; everything else is
        // answer. The `Complete` sink (what `finish` persists) must carry ONLY
        // the answer deltas — reasoning is excluded, exactly as the renderer
        // routes provider-flagged `thinking` chunks away from its answer buffer.
        let parser = |buf: &mut String| -> Vec<StreamPiece> {
            let mut out = Vec::new();
            while let Some(nl) = buf.find('\n') {
                let line = buf[..nl].trim().to_string();
                *buf = buf[nl + 1..].to_string();
                if line == "END" {
                    out.push(StreamPiece::done(""));
                } else if let Some(reason) = line.strip_prefix("T:") {
                    out.push(StreamPiece::thinking(reason.to_string()));
                } else if !line.is_empty() {
                    out.push(StreamPiece::text(line));
                }
            }
            out
        };
        let acts = run(
            vec![Ok(Some(
                b"Dear team,\nT:they want speed\nI apply.\nEND\n".to_vec(),
            ))],
            None,
            parser,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("Dear team,".to_string(), false),
                Act::Emit("they want speed".to_string(), true),
                Act::Emit("I apply.".to_string(), false),
                Act::Complete(Usage::default(), "Dear team,I apply.".to_string()),
            ],
            "the persisted answer must exclude provider-flagged reasoning deltas"
        );
    }

    #[test]
    fn persisted_answer_strips_inline_think_markup_it_never_leaks() {
        // A local reasoning model embeds <think>…</think> inline in a single
        // answer delta (thinking:false — the renderer's splitter, not the
        // provider, separates it). The loop accumulates the RAW delta...
        let parser = |buf: &mut String| -> Vec<StreamPiece> {
            let s = std::mem::take(buf);
            if s.is_empty() {
                vec![]
            } else {
                vec![StreamPiece::done(s)]
            }
        };
        let raw = "Dear team,<think>they want speed, be brief</think> I apply now.";
        let acts = run(vec![Ok(Some(raw.as_bytes().to_vec()))], None, parser);
        assert_eq!(
            acts,
            vec![
                Act::Emit(raw.to_string(), false),
                Act::Complete(Usage::default(), raw.to_string()),
            ],
            "the accumulated answer is the raw stream; stripping happens in `finish`"
        );
        // ...but `finish` persists the THINK-STRIPPED text, so reasoning markup
        // can never reach the final document via the poll fallback.
        let persisted = strip_think_blocks(raw);
        assert_eq!(persisted, "Dear team, I apply now.");
        assert!(
            !persisted.contains("<think>") && !persisted.contains("</think>"),
            "persisted text must never contain reasoning markup"
        );
    }

    #[test]
    fn a_close_tag_split_across_frames_still_strips_clean() {
        // `</think>` arrives split across two frames. The renderer's STREAMING
        // splitter can mis-handle this, but the persisted answer accumulates the
        // whole stream first and strips in one pass — so the persisted text is
        // strictly equal-or-more-correct and never leaks markup.
        let passthrough = |buf: &mut String| -> Vec<StreamPiece> {
            let s = std::mem::take(buf);
            if s.is_empty() {
                vec![]
            } else {
                vec![StreamPiece::text(s)]
            }
        };
        let acts = run(
            vec![
                Ok(Some(b"a<think>b</thi".to_vec())),
                Ok(Some(b"nk>c".to_vec())),
                Ok(None),
            ],
            None,
            passthrough,
        );
        assert_eq!(
            acts,
            vec![
                Act::Emit("a<think>b</thi".to_string(), false),
                Act::Emit("nk>c".to_string(), false),
                Act::Complete(Usage::default(), "a<think>b</think>c".to_string()),
            ]
        );
        assert_eq!(
            strip_think_blocks("a<think>b</think>c"),
            "ac",
            "a </think> split across two stream frames still strips clean once the full \
             answer is accumulated"
        );
    }

    #[test]
    fn strip_think_blocks_matches_the_renderer_splitter() {
        // Plain text is untouched.
        assert_eq!(strip_think_blocks("hello world"), "hello world");
        // A single block is removed, surrounding text kept.
        assert_eq!(
            strip_think_blocks("answer<think>reasoning</think>more"),
            "answermore"
        );
        // Multiple blocks.
        assert_eq!(
            strip_think_blocks("a<think>x</think>b<think>y</think>c"),
            "abc"
        );
        // A leading block.
        assert_eq!(strip_think_blocks("<think>r</think>visible"), "visible");
        // An empty block.
        assert_eq!(strip_think_blocks("a<think></think>b"), "ab");
        // An UNTERMINATED block discards everything from the tag onward — the
        // renderer's splitter drops an unterminated block at flush().
        assert_eq!(strip_think_blocks("keep<think>dropped forever"), "keep");
        // Whatever the input, the output can never contain reasoning markup.
        for s in [
            "answer<think>reasoning</think>more",
            "<think>r</think>visible",
            "keep<think>dropped forever",
        ] {
            let out = strip_think_blocks(s);
            assert!(
                !out.contains("<think>") && !out.contains("</think>"),
                "{s:?} leaked markup"
            );
        }
    }
}

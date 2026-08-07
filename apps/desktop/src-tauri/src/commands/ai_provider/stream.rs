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

use super::{ProviderId, RequestTrace, StopReason, Usage};

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
    /// The provider's own end-of-turn reason, when this piece happens to carry
    /// it: OpenAI/Ollama Cloud's `finish_reason` on a streamed chunk (see
    /// `openai::parse_openai_finish_reason`) and local Ollama's `done_reason` on
    /// its final object (see `ollama::ollama_done_reason`). `None` before one has
    /// been reported, and always `None` for Anthropic/Gemini — NOT because those
    /// have no streamed equivalent (they do: Anthropic's
    /// `message_delta.stop_reason`, Gemini's `candidates[].finishReason`) but
    /// because their frame parsers do not map it yet. Mapping either is
    /// additive: parse the field into [`StopReason`] and the length diagnosis
    /// below starts working there too. Mirrors `usage`'s "latest non-`None`
    /// wins" handling.
    /// What this exists for: telling a `finish_reason: length` empty answer
    /// (the model ran out of budget mid-reasoning, never reached its final
    /// channel) apart from a provider that just silently returned nothing —
    /// see `finish`'s empty-answer branch.
    pub stop_reason: Option<StopReason>,
}

impl StreamPiece {
    /// A normal answer-text delta.
    pub fn text(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            thinking: false,
            done: false,
            usage: None,
            stop_reason: None,
        }
    }

    /// A reasoning/thinking delta.
    pub fn thinking(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            thinking: true,
            done: false,
            usage: None,
            stop_reason: None,
        }
    }

    /// The provider's end-of-stream sentinel, optionally carrying a final delta.
    pub fn done(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            thinking: false,
            done: true,
            usage: None,
            stop_reason: None,
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
            stop_reason: None,
        }
    }

    /// A stop-reason-only piece: no visible text, not a completion sentinel —
    /// OpenAI/Ollama Cloud report `finish_reason` on a regular content chunk
    /// (typically the one right before the `[DONE]` sentinel line), not on the
    /// sentinel itself.
    pub fn stop_reason(reason: StopReason) -> Self {
        Self {
            delta: String::new(),
            thinking: false,
            done: false,
            usage: None,
            stop_reason: Some(reason),
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

/// Generic empty-completion message — the stream ended with no usable answer
/// text and no `finish_reason: length` signal to explain why (or the provider
/// never reports one). See [`EMPTY_ANSWER_LENGTH_MESSAGE`] for the distinct,
/// more actionable case.
pub(super) const EMPTY_ANSWER_MESSAGE: &str = "The model produced no answer content.";

/// Empty-completion message for the specific, diagnosable case: the provider's
/// own `finish_reason` was `length` (OpenAI/Ollama Cloud) — the model ran out
/// of its output-token budget, most often while a reasoning model was still
/// inside its thinking channel and never reached a final answer. Distinct from
/// [`EMPTY_ANSWER_MESSAGE`] so a diagnostics bundle can tell "the provider
/// truncated us before any answer" apart from "the provider silently returned
/// nothing" — this is exactly the ambiguity that took two investigations to
/// pin down for a real report (Ollama Cloud `gpt-oss:120b`).
/// Deliberately does NOT point at a Settings control: the only max-output-tokens
/// field in the UI is `LocalModelLimits`, rendered solely for the LOCAL `ollama`
/// provider — so for every OTHER provider that reaches this branch there is
/// nothing to adjust, and sending them to a field they cannot see is worse than
/// no advice. Local Ollama gets [`EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE`] instead.
pub(super) const EMPTY_ANSWER_LENGTH_MESSAGE: &str = "The model ran out of output budget \
    before producing any answer text (finish_reason: length) — it was most likely still \
    reasoning when it hit the limit. Try again, or pick a model with a larger output budget.";

/// Same case as [`EMPTY_ANSWER_LENGTH_MESSAGE`], but for LOCAL Ollama, which is
/// the one provider whose output budget the user can actually raise in-app:
/// `LocalModelLimits` ("Generation limits" → "Max output tokens") renders under
/// the Ollama card in Settings → AI.
///
/// This message only became reachable when `ollama::parse_ollama_frames` started
/// mapping `done_reason` onto the streamed sentinel. Before that local Ollama
/// reported no stop reason at all and always fell through to
/// [`EMPTY_ANSWER_MESSAGE`] — which is why the constant above was originally
/// written with no Settings pointer. If a future change makes another provider's
/// cap adjustable in-app, it needs its own arm here rather than a reworded shared
/// string.
pub(super) const EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE: &str = "The model ran out of output budget \
    before producing any answer text — it was most likely still reasoning when it hit the \
    limit. Raise \"Max output tokens\" under Generation limits in Settings → AI, or try again.";

/// Pick the right empty-completion message for `stop_reason` — see the three
/// constants' docs for what each one means.
///
/// `provider` is needed because the remedy differs: only local Ollama exposes an
/// adjustable output cap in the UI.
pub(super) fn empty_answer_message(
    stop_reason: Option<StopReason>,
    provider: ProviderId,
) -> &'static str {
    match (stop_reason, provider) {
        (Some(StopReason::Length), ProviderId::Ollama) => EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE,
        (Some(StopReason::Length), _) => EMPTY_ANSWER_LENGTH_MESSAGE,
        _ => EMPTY_ANSWER_MESSAGE,
    }
}

/// Warning body for a **non-empty** completion that still hit `finish_reason:
/// length` — real text WAS produced, but the model was cut off by its
/// output-token budget before reaching a natural end, so the saved/exported
/// document is likely missing its tail. Distinct from [`EMPTY_ANSWER_LENGTH_MESSAGE`]
/// (the "zero text at all" case) — this generation still succeeds (the partial
/// text is persisted, same as any other completion); the warning exists so the
/// user learns to review it, not to imply the job failed. See
/// [`truncation_notification`] / `finish`'s non-empty branch.
const TRUNCATED_ANSWER_MESSAGE: &str = "The model ran out of output budget partway through \
    (finish_reason: length) — the saved text is likely cut off before the end. Review it \
    before exporting, or try again with a model that has a larger output budget.";

/// Local-Ollama sibling of [`TRUNCATED_ANSWER_MESSAGE`] — same "Max output
/// tokens" remedy [`EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE`] points to, for the same
/// reason (only local Ollama exposes an adjustable output cap in Settings).
const TRUNCATED_ANSWER_LOCAL_MESSAGE: &str = "The model ran out of output budget partway \
    through — the saved text is likely cut off before the end. Raise \"Max output tokens\" \
    under Generation limits in Settings → AI, or try again.";

/// Pick the right truncated-but-non-empty warning body for `provider` — mirrors
/// [`empty_answer_message`]'s per-provider split (only local Ollama gets the
/// Settings pointer).
fn truncated_answer_message(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::Ollama => TRUNCATED_ANSWER_LOCAL_MESSAGE,
        _ => TRUNCATED_ANSWER_MESSAGE,
    }
}

/// Notification `kind`/title for [`truncation_notification`]. `kind` is an
/// open string (mirroring every other source, e.g. `"autopilot.new_jobs"`,
/// `"email.match"`) — a new notification kind needs no codebase change beyond
/// picking one.
const TRUNCATED_NOTIFICATION_KIND: &str = "ai.generation_truncated";
const TRUNCATED_NOTIFICATION_TITLE: &str = "Generation may be cut off";

/// Whether a just-finished, **non-empty** completion should warn the user their
/// document is likely truncated, and if so, the [`crate::notifications::NewNotification`]
/// to push. Pure — no `AppHandle` — so it's directly testable without this
/// crate's (nonexistent) `tauri::test` mock-app harness; `finish` calls this via
/// [`finish_outcome`] and, when `Some`, hands it to `push_and_notify` (the
/// actual side effect, not exercised by this function's own tests).
/// `None` for every stop reason except [`StopReason::Length`] — a normal
/// completion (`End`/`ToolUse`/`Other`/unknown) never warns.
fn truncation_notification(
    provider: ProviderId,
    stop_reason: Option<StopReason>,
) -> Option<crate::notifications::NewNotification> {
    if stop_reason != Some(StopReason::Length) {
        return None;
    }
    Some(crate::notifications::NewNotification {
        kind: TRUNCATED_NOTIFICATION_KIND.to_string(),
        title: TRUNCATED_NOTIFICATION_TITLE.to_string(),
        body: truncated_answer_message(provider).to_string(),
        route: None,
    })
}

/// Pure result of [`finish_outcome`] — see that function's doc for why this
/// split exists. `Complete.warning` is advisory: the job is marked complete
/// either way, `Some` only adds a truncation notice on top.
enum FinishOutcome {
    /// No usable text at all — `finish` records spend and closes the trace but
    /// never marks the job complete; `message` becomes the returned `Err`.
    Empty { message: &'static str },
    /// Real text to persist as the finished job result, plus an optional
    /// truncation warning (see [`truncation_notification`]).
    Complete {
        text: String,
        warning: Option<crate::notifications::NewNotification>,
    },
}

/// Pure decision core of [`finish`]: given the already think-stripped answer,
/// the provider's own `stop_reason`, and which provider this is, decide
/// whether the completion is empty (and with what error message) or complete
/// (and with what text + optional truncation warning) — with NO `AppHandle`,
/// so it's directly unit-testable. Mirrors the pure-decision/thin-IO-shell
/// split already used elsewhere in this crate (e.g.
/// `net::http::is_allowed_redirect_target`, `profile_import::github::map_status`).
fn finish_outcome(
    stripped: &str,
    stop_reason: Option<StopReason>,
    provider: ProviderId,
) -> FinishOutcome {
    if stripped.trim().is_empty() {
        FinishOutcome::Empty {
            message: empty_answer_message(stop_reason, provider),
        }
    } else {
        FinishOutcome::Complete {
            text: stripped.to_string(),
            warning: truncation_notification(provider, stop_reason),
        }
    }
}

/// On a normal (non-empty) completion: emit the terminal `ai:stream` event,
/// mark the job complete, close the trace, and record the stream's REAL token
/// usage (zero when the provider never reported any) against today's AI
/// spend. `base_url` is passed through to the free/paid cost gate — only
/// meaningful for `openai-compatible` (LM Studio/vLLM/OpenRouter/…), ignored
/// for every other provider.
///
/// On an EMPTY completion (the accumulated answer is whitespace-only once
/// think-stripped — see `stream_response`'s loop, which only accumulates
/// non-thinking deltas) this does the opposite on purpose: no `job_complete`,
/// no plain `done` event — persisting a job as "completed" with `text: ""`
/// let an empty document sail silently through export/notifications with no
/// error anywhere (the bug this closes). Instead it returns `Err`, which every
/// caller of `chat_stream`/`Completer::stream`/`stream_complete` already
/// turns into `job_fail` + `emit_stream_error` on its own generic error path
/// (`ai_generate`, `generate_pipeline`, `answer.assist`'s `compose_draft_stream`)
/// — so this needs no caller-specific wiring, the same way a transport or HTTP
/// error already doesn't. `record_usage`/`trace.end` still run: the provider
/// may have spent real (billable) output tokens reasoning before giving up,
/// and that spend must never go unrecorded just because nothing usable came
/// of it.
///
/// `answer` is the full completed text accumulated from this stream's
/// **non-thinking** deltas (see `stream_response`'s loop) — exactly what the
/// renderer feeds its `<think>` splitter. On success it is persisted into the
/// job result as `result.text` so a renderer that missed stream frames (or the
/// terminal `done` event) can recover the finished document by polling
/// `jobs_get` instead of resolving a truncated stream buffer. This is
/// provider-agnostic: every provider routes through here, so a new adapter
/// inherits the behavior for free. Before persisting, inline
/// `<think>…</think>` reasoning is stripped via [`strip_think_blocks`] so the
/// persisted text is the SAME think-stripped shape the renderer assembles —
/// critical because the renderer's poll fallback prefers the LONGER of
/// {persisted, streamed buffer}, and its streamed buffer is already
/// think-stripped. Persisting raw `<think>` markup would make the persisted
/// side spuriously longer AND leak reasoning markup into the final document;
/// stripping here keeps both sides of that length comparison in the same
/// shape.
///
/// `thinking_len` (char count of every thinking-flagged delta seen, never the
/// text itself) is diagnostic-only. `stop_reason` (the provider's own
/// `finish_reason`, see [`StreamPiece::stop_reason`]) is diagnostic on the
/// EMPTY path (telling an empty-because-truncated stream apart from an
/// empty-because-the-provider-said-nothing one) but NOT on the success path:
/// a non-empty answer with `stop_reason: Some(Length)` still completes
/// normally (terminal event, `job_complete`, persisted result — same as any
/// other completion) but ALSO pushes a non-fatal [`truncation_notification`]
/// through the Notification Center, so the user learns their saved document
/// may be cut off instead of it silently sailing through export as if
/// finished. See [`finish_outcome`] for the pure decision this delegates to.
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
    stop_reason: Option<StopReason>,
    thinking_len: usize,
) -> AppResult<()> {
    let stripped = strip_think_blocks(answer);
    let outcome = finish_outcome(&stripped, stop_reason, provider);
    let is_empty = matches!(outcome, FinishOutcome::Empty { .. });
    log::info!(
        "[ai] stream end provider={} model={} answerLen={} thinkingLen={} finishReason={:?} \
         usageIn={} usageOut={} empty={}",
        provider.as_str(),
        model,
        stripped.chars().count(),
        thinking_len,
        stop_reason,
        usage.input_tokens,
        usage.output_tokens,
        is_empty,
    );

    match outcome {
        FinishOutcome::Empty { message } => {
            log::warn!(
                "[ai] stream produced no answer content provider={} model={} thinkingLen={} \
                 finishReason={:?}",
                provider.as_str(),
                model,
                thinking_len,
                stop_reason,
            );
            trace.end(Some(status), false);
            super::record_usage(
                app,
                provider.as_str(),
                model,
                usage.input_tokens,
                usage.output_tokens,
                Some(base_url),
            );
            Err(AppError::Provider(message.to_string()))
        }
        FinishOutcome::Complete { text, warning } => {
            if let Some(notification) = warning {
                log::warn!(
                    "[ai] stream truncated (finish_reason: length), surfacing a non-fatal \
                     warning provider={} model={} answerLen={}",
                    provider.as_str(),
                    model,
                    text.chars().count(),
                );
                crate::commands::notifications::push_and_notify(
                    app,
                    notification,
                    crate::commands::notifications::OsBanner::WhenUnfocused,
                );
            }
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
            crate::commands::jobs::job_complete(app, job_id, json!({ "done": true, "text": text }));
            trace.end(Some(status), true);
            super::record_usage(
                app,
                provider.as_str(),
                model,
                usage.input_tokens,
                usage.output_tokens,
                Some(base_url),
            );
            Ok(())
        }
    }
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
    log::info!(
        "[ai] stream start provider={} model={}",
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
                    if let Some(r) = piece.stop_reason {
                        stop_reason = Some(r);
                    }
                    if !piece.delta.is_empty() {
                        if piece.thinking {
                            thinking_len += piece.delta.chars().count();
                        } else {
                            answer.push_str(&piece.delta);
                        }
                        emit_delta(app, job_id, &piece.delta, piece.thinking);
                    }
                    if piece.done {
                        return finish(
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
#[path = "stream_tests.rs"]
mod tests;

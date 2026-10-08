//! The streaming engine: spawn, drain stdout line-by-line against a cancel
//! poll + deadline, emit `ai:stream` chunks, and report the job complete.

use std::time::Duration;

use parking_lot::Mutex;
use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use crate::commands::ai_provider::{record_usage, stream as cloud_stream, Usage};
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AiStreamChunk, AI_STREAM};
use crate::jobs::JobTracker;

use super::{CliAgentBackend, CliEvent};

/// How often the stream loop re-polls the JobTracker for cancellation while
/// waiting on the next output line, so a cancel mid-line (or on a stalled stream)
/// is observed promptly instead of blocking until the next newline arrives.
pub(super) const CANCEL_POLL: Duration = Duration::from_millis(200);

/// Outcome of one race between the next-line read and the cancel poll. A distinct
/// `Cancelled` variant keeps a natural EOF (`Eof`) from being misreported as a
/// cancel when cancellation and stream-end coincide in the same poll window —
/// overloading `Ok(None)` for both would surface a false "Job cancelled" error.
pub(super) enum ReadOutcome {
    /// A complete line of agent output.
    Line(String),
    /// The stream ended cleanly (normal completion).
    Eof,
    /// Cancellation observed before the next line arrived.
    Cancelled,
    /// The underlying read failed.
    Err(std::io::Error),
}

pub(super) async fn run_stream(
    app: &AppHandle,
    job_id: &str,
    backend: &dyn CliAgentBackend,
    model: &str,
    system: &str,
    prompt: &str,
    effort: Option<&str>,
) -> AppResult<()> {
    let binary = backend.binary();
    let label = backend.id().as_str();
    let inv = backend.stream_invocation(model, system, effort);
    let prompt = super::effective_prompt(backend, system, prompt);
    let trace = super::RequestTrace::begin(backend.id(), model, "cli:stream", &binary, true);

    // `_workspace` (not `_`) keeps the per-spawn config dir alive until this
    // function returns, i.e. until the child has exited.
    let (mut child, _workspace) = match super::spawn(&binary, &inv, &prompt, backend) {
        Ok(spawned) => spawned,
        Err(e) => {
            trace.end(None, false);
            return Err(super::spawn_error(label, &binary, e));
        }
    };

    // Feed stdin on a detached task so the stdout loop below drains concurrently.
    // Awaiting the whole write first can DEADLOCK on a prompt larger than the OS
    // pipe buffer (~64 KB) if the child interleaves stdout while still reading stdin
    // (see `write_prompt_stdin`) — realistic for cover-letter prompts that inline
    // the full JD + résumé + research brief.
    let stdin_writer = super::write_prompt_stdin(inv.prompt, child.stdin.take(), prompt, label);

    // Drain stderr concurrently so a chatty agent can't deadlock on a full pipe.
    let stderr = child.stderr.take();
    let stderr_handle = tokio::spawn(async move {
        let mut buf = String::new();
        if let Some(mut e) = stderr {
            let _ = e.read_to_string(&mut buf).await;
        }
        buf
    });

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Provider("Failed to capture CLI stdout".to_string()))?;
    let mut lines = BufReader::new(stdout).lines();

    let deadline = tokio::time::Instant::now() + super::TIMEOUT;
    let mut emitted_done = false;
    let mut any_delta = false;
    // Whether a non-blank content delta has streamed yet — gates leading-blank
    // suppression so plain-text agents don't emit a stray newline before the answer.
    let mut seen_content = false;
    // The full completed answer, accumulated from the exact non-thinking deltas
    // we stream (post leading-blank suppression) — the same shape the renderer
    // buffers. Persisted by `emit_done` so a renderer that missed frames or the
    // terminal `done` event recovers it by polling, exactly like the cloud
    // `stream::finish` path.
    let mut answer = String::new();

    loop {
        // Race the next line read against a cancel poll so a cancellation mid-line
        // (or on a stalled stream that never emits another newline) is observed
        // within `CANCEL_POLL` instead of hanging until the next line — or forever.
        // The poll branch loops back to re-check `is_cancelled`; the deadline still
        // bounds the whole read via `timeout_at`. A distinct `ReadOutcome` keeps a
        // real EOF from being misreported as a cancel when both happen to be true in
        // the same poll window (overloading `Ok(None)` for both would).
        let line = match tokio::time::timeout_at(deadline, async {
            loop {
                tokio::select! {
                    biased;
                    next = lines.next_line() => {
                        // Biased: a ready line always wins over the cancel poll.
                        break match next {
                            Ok(Some(l)) => ReadOutcome::Line(l),
                            Ok(None) => ReadOutcome::Eof,
                            Err(e) => ReadOutcome::Err(e),
                        };
                    }
                    _ = tokio::time::sleep(CANCEL_POLL) => {
                        if is_cancelled(app, job_id) {
                            break ReadOutcome::Cancelled;
                        }
                    }
                }
            }
        })
        .await
        {
            Err(_) => {
                let _ = child.start_kill();
                trace.end(None, false);
                return Err(AppError::Network(format!(
                    "{label} timed out after 5 minutes."
                )));
            }
            Ok(ReadOutcome::Cancelled) => {
                let _ = child.start_kill();
                trace.end(None, false);
                return Err(AppError::Message("Job cancelled".to_string()));
            }
            Ok(ReadOutcome::Line(line)) => line,
            Ok(ReadOutcome::Eof) => break, // clean EOF (never a cancel)
            Ok(ReadOutcome::Err(e)) => {
                let _ = child.start_kill();
                trace.end(None, false);
                return Err(AppError::Provider(format!("{label}: read error: {e}")));
            }
        };

        // Don't pre-skip blank lines: JSON parsers return None for them anyway,
        // while plain-text agents (Gemini) rely on them for paragraph breaks.
        match backend.parse_stream_line(&line) {
            Some(CliEvent::Delta(text)) if !text.is_empty() => {
                any_delta = true;
                // Suppress leading blank/whitespace-only lines so streamed output
                // starts at the first real content line — matching the trimmed
                // one-shot (`parse_complete`) output. A blank line left after a
                // stripped credential notice (Gemini/Antigravity) would otherwise
                // stream a stray leading newline. `any_delta` is set above regardless,
                // so the success heuristic is unchanged.
                if !seen_content {
                    if text.trim().is_empty() {
                        continue;
                    }
                    seen_content = true;
                }
                answer.push_str(&text);
                emit_event(
                    app,
                    AI_STREAM,
                    AiStreamChunk {
                        job_id: job_id.to_string(),
                        delta: text,
                        done: false,
                        error: None,
                        thinking: None,
                    },
                );
            }
            Some(CliEvent::Thinking(text)) if !text.is_empty() => {
                emit_event(
                    app,
                    AI_STREAM,
                    AiStreamChunk {
                        job_id: job_id.to_string(),
                        delta: text,
                        done: false,
                        error: None,
                        thinking: Some(true),
                    },
                );
            }
            Some(CliEvent::Done) => {
                emitted_done = true;
                break;
            }
            Some(CliEvent::Error(msg)) => {
                let _ = child.start_kill();
                trace.end(None, false);
                return Err(AppError::Provider(msg));
            }
            _ => {}
        }
    }

    let status = child.wait().await.ok();
    let success = status.map(|s| s.success()).unwrap_or(false);
    let stderr_text = stderr_handle.await.unwrap_or_default();
    // Reap the stdin writer — the child has exited, so it has completed; any write
    // error was already logged inside the task and is never fatal.
    let _ = stdin_writer.await;

    // No explicit terminal event (e.g. plain-text agents): a clean exit or any
    // streamed text means success; otherwise surface the failure.
    if !emitted_done && !success && !any_delta {
        trace.end(status.and_then(|s| s.code()).map(|c| c as u16), false);
        return Err(super::friendly_cli_error(
            label,
            status.and_then(|s| s.code()),
            &stderr_text,
        ));
    }

    // CLI agents run headless via their own tool's login (no API response to
    // read a `usage` field from) and stream over plain stdout lines, so they
    // never pass through the shared `commands::ai_provider::stream` loop that
    // records spend for the cloud adapters. Record zero tokens/cost here
    // (honest — never fabricate an estimate) so the AI-spend summary still
    // reflects that a call happened, at $0 real cost. The non-streaming
    // `complete`/`agent_run` path needs no equivalent call: it goes through
    // `AiProvider::complete_with_usage`'s DEFAULT impl, which already reports
    // zero usage for any provider (like this one) that doesn't override it.
    // A CLI agent reports no usage at all — `Usage::default()` is zeros on
    // the counted fields and `None` on the thinking one, which says
    // "nothing reported" rather than "no reasoning happened".
    record_usage(app, backend.id().as_str(), model, Usage::default(), None);
    // An agent that emitted a `Done` sentinel (or a whitespace-only delta) and
    // THEN exited non-zero skips the `!emitted_done && !success` guard above,
    // so without this it would report the generic "produced no answer content"
    // and throw away the stderr that says why — the not-logged-in / quota /
    // bad-flag cases `friendly_cli_error` already recognises. The captured
    // stderr is strictly more informative than the empty-answer message, so
    // prefer it whenever the process itself failed.
    let result = emit_done(app, job_id, label, model, &answer).map_err(|empty| {
        super::terminal_error(
            empty,
            success,
            label,
            status.and_then(|s| s.code()),
            &stderr_text,
        )
    });
    trace.end(
        status.and_then(|s| s.code()).map(|c| c as u16),
        result.is_ok(),
    );
    result
}

fn is_cancelled(app: &AppHandle, job_id: &str) -> bool {
    app.state::<Mutex<JobTracker>>().lock().is_cancelled(job_id)
}

/// On a normal (non-empty) completion: emit the terminal `ai:stream` event and
/// mark the job complete, persisting the completed `answer` as `result.text`
/// (think-stripped via the shared [`super::super::stream::strip_think_blocks`])
/// so a CLI-agent generation's poll fallback recovers the finished document
/// just like the cloud `stream::finish` path — the poll contract is
/// provider-agnostic.
///
/// On an EMPTY completion (a CLI agent that streamed only `Thinking` events —
/// or none at all — before a clean exit/`Done` sentinel, so `run_stream`'s own
/// `!emitted_done && !success && !any_delta` guard above never fired) this
/// does the opposite on purpose: no `job_complete`, no plain `done` event —
/// returns `Err` instead, exactly like `super::super::stream::finish`'s same
/// branch. `run_stream`'s caller (`chat_stream`, then
/// `Completer::stream`/`stream_complete`) already turns that into `job_fail` +
/// `emit_stream_error` on its generic error path, so this needs no
/// caller-specific wiring. CLI agents have no `finish_reason` signal of their
/// own (no HTTP response to carry one), so this always reports the generic
/// empty message — `run_stream` replaces it with `friendly_cli_error`'s
/// stderr-derived diagnosis when the process itself also exited non-zero.
fn emit_done(
    app: &AppHandle,
    job_id: &str,
    provider: &str,
    model: &str,
    answer: &str,
) -> AppResult<()> {
    let stripped = cloud_stream::strip_think_blocks(answer);
    if stripped.trim().is_empty() {
        log::warn!(
            "[ai] cli-agent stream produced no answer content provider={provider} model={model}"
        );
        return Err(AppError::Provider(
            cloud_stream::EMPTY_ANSWER_MESSAGE.to_string(),
        ));
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
    crate::commands::jobs::job_complete(app, job_id, json!({ "done": true, "text": stripped }));
    Ok(())
}

//! Terminal stream handling: the empty/truncated-completion messaging and
//! [`finish`] — the one-shot "emit terminal event + persist + record spend"
//! step the shared loop reaches on a sentinel or end-of-body. Split out of
//! `stream.rs` (R8 line-budget split): a self-contained decision + one IO
//! shell around it.

use serde_json::json;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AiStreamChunk, AI_STREAM};

use super::super::{ProviderId, RequestTrace, StopReason, Usage};
use super::text::strip_think_blocks;

/// Generic empty-completion message — the stream ended with no usable answer
/// text and no `finish_reason: length` signal to explain why (or the provider
/// never reports one). See [`EMPTY_ANSWER_LENGTH_MESSAGE`] for the distinct,
/// more actionable case.
pub(in crate::commands::ai_provider) const EMPTY_ANSWER_MESSAGE: &str =
    "The model produced no answer content.";

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

/// Whether `e` is the empty-answer-because-`finish_reason: length` failure
/// [`finish`]'s `FinishOutcome::Empty` branch returns — EITHER of the two
/// messages [`empty_answer_message`] can pick for it
/// ([`EMPTY_ANSWER_LENGTH_MESSAGE`] and its local-Ollama sibling
/// [`EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE`]), never the generic
/// [`EMPTY_ANSWER_MESSAGE`] and never any other provider error.
///
/// The classification is as typed as [`AppError`] allows: the variant is
/// matched structurally ([`AppError::Provider`] — the ONLY variant `finish`
/// builds this from, so the same text arriving as a `Validation`/`Network`
/// error is correctly NOT this failure), and the payload is compared against
/// the two constants above rather than a re-typed string literal, so
/// rewording either one can never silently un-classify it. A dedicated
/// `AppError` variant would be strictly better, but that enum is the app-wide
/// error taxonomy consumed by every IPC surface — a new variant for one
/// provider outcome is a far larger change than this predicate.
///
/// Exists for ONE caller: the extension bridge's `answer.assist` compose
/// (`extension_bridge::answer_assist`), which retries exactly this failure
/// once at a larger output budget because the model's own reasoning tokens
/// are what consumed the budget. Nothing else may retry on it — see that
/// call site's doc for the spend discipline.
pub(crate) fn is_empty_answer_length_cut(e: &AppError) -> bool {
    matches!(
        e,
        AppError::Provider(message)
            if message == EMPTY_ANSWER_LENGTH_MESSAGE
                || message == EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE
    )
}

/// The empty-completion error [`finish`] returns for `stop_reason`/`provider`,
/// built through THE SAME [`empty_answer_message`] picker and the SAME
/// `AppError` variant `finish` wraps it in — for a cross-module test that has
/// to drive a CALLER's handling of that failure without a live stream
/// (`extension_bridge::answer_assist`'s compose retry). Test-only: production
/// code never constructs this error anywhere but `finish`.
#[cfg(test)]
pub(crate) fn empty_answer_error_for_test(
    stop_reason: Option<StopReason>,
    provider: ProviderId,
) -> AppError {
    AppError::Provider(empty_answer_message(stop_reason, provider).to_string())
}

/// Warning body for a **non-empty** completion that still hit `finish_reason:
/// length` — real text WAS produced, but the model was cut off by its
/// output-token budget before reaching a natural end, so the saved/exported
/// document is likely missing its tail. Distinct from [`EMPTY_ANSWER_LENGTH_MESSAGE`]
/// (the "zero text at all" case) — this generation still succeeds (the partial
/// text is persisted, same as any other completion); the warning exists so the
/// user learns to review it, not to imply the job failed. See
/// [`truncation_notification`] / `finish`'s non-empty branch.
pub(super) const TRUNCATED_ANSWER_MESSAGE: &str =
    "The model ran out of output budget partway through \
    (finish_reason: length) — the saved text is likely cut off before the end. Review it \
    before exporting, or try again with a model that has a larger output budget.";

/// Local-Ollama sibling of [`TRUNCATED_ANSWER_MESSAGE`] — same "Max output
/// tokens" remedy [`EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE`] points to, for the same
/// reason (only local Ollama exposes an adjustable output cap in Settings).
pub(super) const TRUNCATED_ANSWER_LOCAL_MESSAGE: &str =
    "The model ran out of output budget partway \
    through — the saved text is likely cut off before the end. Raise \"Max output tokens\" \
    under Generation limits in Settings → AI, or try again.";

/// Pick the right truncated-but-non-empty warning body for `provider` — mirrors
/// [`empty_answer_message`]'s per-provider split (only local Ollama gets the
/// Settings pointer).
pub(super) fn truncated_answer_message(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::Ollama => TRUNCATED_ANSWER_LOCAL_MESSAGE,
        _ => TRUNCATED_ANSWER_MESSAGE,
    }
}

/// Notification `kind`/title for [`truncation_notification`]. `kind` is an
/// open string (mirroring every other source, e.g. `"autopilot.new_jobs"`,
/// `"email.match"`) — a new notification kind needs no codebase change beyond
/// picking one.
pub(super) const TRUNCATED_NOTIFICATION_KIND: &str = "ai.generation_truncated";
pub(super) const TRUNCATED_NOTIFICATION_TITLE: &str = "Generation may be cut off";

/// Whether a just-finished, **non-empty** completion should warn the user their
/// document is likely truncated, and if so, the [`crate::notifications::NewNotification`]
/// to push. Pure — no `AppHandle` — so it's directly testable without this
/// crate's (nonexistent) `tauri::test` mock-app harness; `finish` calls this via
/// [`finish_outcome`] and, when `Some`, hands it to `push_and_notify` (the
/// actual side effect, not exercised by this function's own tests).
/// `None` for every stop reason except [`StopReason::Length`] — a normal
/// completion (`End`/`ToolUse`/`Other`/unknown) never warns.
pub(super) fn truncation_notification(
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
pub(super) enum FinishOutcome {
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
pub(super) fn finish_outcome(
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
/// `finish_reason`, see [`super::piece::StreamPiece::stop_reason`]) is
/// diagnostic on the EMPTY path (telling an empty-because-truncated stream
/// apart from an empty-because-the-provider-said-nothing one) but NOT on the
/// success path: a non-empty answer with `stop_reason: Some(Length)` still
/// completes normally (terminal event, `job_complete`, persisted result —
/// same as any other completion) but ALSO pushes a non-fatal
/// [`truncation_notification`] through the Notification Center, so the user
/// learns their saved document may be cut off instead of it silently sailing
/// through export as if finished. See [`finish_outcome`] for the pure
/// decision this delegates to.
#[allow(clippy::too_many_arguments)]
pub(super) fn finish(
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
            super::super::record_usage(app, provider.as_str(), model, usage, Some(base_url));
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
            super::super::record_usage(app, provider.as_str(), model, usage, Some(base_url));
            Ok(())
        }
    }
}

//! Idle-timeout reads for every provider call that arrives as a stream (#1353).
//!
//! A whole-request `.timeout()` kills a healthy generation that is merely slow
//! (a thinking model at ~7 tok/s writing a long draft). The bound here is an
//! IDLE one instead: a call fails only when no bytes arrive for
//! [`StreamLimits::idle`], with [`StreamLimits::ceiling`] as the absolute
//! backstop for a call that keeps trickling forever. Both the `chat_stream`
//! loop (`stream_response`) and the structured/plain completions
//! ([`collect`]) read through the one [`IdleGuard`], so the two cannot drift.

use std::future::Future;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{RequestBuilder, Response};
use tokio::time::Instant;

use crate::error::{AppError, AppResult};

use super::{push_utf8, StreamPiece};
use crate::commands::ai_provider::{redact_upstream_text, timeouts, StopReason, Usage};
use crate::net::http::DEFAULT_MAX_BODY_BYTES;

/// How long a provider call may stay silent, and how long it may run at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::commands::ai_provider) struct StreamLimits {
    pub idle: Duration,
    pub ceiling: Duration,
}

impl StreamLimits {
    /// `idle` is the call's old whole-request deadline (so nothing that worked
    /// before can fail sooner); the ceiling derives from it.
    pub fn new(idle: Duration) -> Self {
        Self {
            idle,
            ceiling: timeouts::stream_ceiling(idle),
        }
    }
}

/// Why a guarded read stopped.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::commands::ai_provider) enum Stop<E> {
    Idle,
    Ceiling,
    Source(E),
}

impl<E> Stop<E> {
    /// The `AppError` for this stop. `Timeout` (not `Network`) for both clocks:
    /// `pipeline/stage.rs` keys the `StoppedReason::Timeout` mapping on it.
    pub fn into_app(
        self,
        label: &str,
        limits: StreamLimits,
        source: impl FnOnce(E) -> AppError,
    ) -> AppError {
        match self {
            Stop::Idle => idle_error(label, limits.idle),
            Stop::Ceiling => ceiling_error(label, limits.ceiling),
            Stop::Source(e) => source(e),
        }
    }
}

pub(in crate::commands::ai_provider) fn idle_error(label: &str, idle: Duration) -> AppError {
    AppError::Timeout(format!("{label}: no data received for {}s", idle.as_secs()))
}

pub(in crate::commands::ai_provider) fn ceiling_error(label: &str, ceiling: Duration) -> AppError {
    AppError::Timeout(format!(
        "{label}: generation still running after the {}s limit",
        ceiling.as_secs()
    ))
}

/// Applies [`StreamLimits`] to successive reads of one call. `tokio::time`, so
/// the tests drive it with paused time.
pub(in crate::commands::ai_provider) struct IdleGuard {
    started: Instant,
    limits: StreamLimits,
}

impl IdleGuard {
    pub fn start(limits: StreamLimits) -> Self {
        Self {
            started: Instant::now(),
            limits,
        }
    }

    /// Await one read. Every call restarts the idle clock (that is the whole
    /// point); the ceiling clock never restarts.
    pub async fn read<T, E>(&self, read: impl Future<Output = Result<T, E>>) -> Result<T, Stop<E>> {
        let left = self.limits.ceiling.saturating_sub(self.started.elapsed());
        if left.is_zero() {
            return Err(Stop::Ceiling);
        }
        let (wait, expired) = if left <= self.limits.idle {
            (left, Stop::Ceiling)
        } else {
            (self.limits.idle, Stop::Idle)
        };
        match tokio::time::timeout(wait, read).await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(Stop::Source(e)),
            Err(_) => Err(expired),
        }
    }
}

/// Send a stream request (retrying a transient 429/5xx handshake) and fail if
/// the response headers do not arrive within the idle bound. reqwest's own
/// `.timeout()` (set by the retry helper) is given the CEILING, so it bounds the
/// body read too without being the thing that fires first on a healthy stream.
pub(in crate::commands::ai_provider) async fn open(
    build: impl FnMut() -> RequestBuilder,
    limits: StreamLimits,
    label: &str,
    map_err: impl FnOnce(reqwest::Error) -> AppError,
) -> AppResult<Response> {
    let sent = super::super::retry::send_stream_with_retry(build, limits.ceiling);
    match tokio::time::timeout(limits.idle, sent).await {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(e)) => Err(map_err(e)),
        Err(_) => Err(idle_error(label, limits.idle)),
    }
}

/// A failed chunk read, already classified for [`Stop::into_app`].
#[derive(Debug, PartialEq, Eq)]
pub(in crate::commands::ai_provider) enum SourceError {
    /// reqwest's own ceiling timer fired mid-body.
    Timeout,
    Other(String),
}

/// Where [`collect`] reads its bytes from — a `reqwest::Response` in
/// production, a scripted fake in the tests.
#[async_trait]
pub(in crate::commands::ai_provider) trait ChunkSource:
    Send
{
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, SourceError>;
}

#[async_trait]
impl ChunkSource for Response {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, SourceError> {
        self.chunk()
            .await
            .map(|c| c.map(|b| b.to_vec()))
            .map_err(|e| {
                if e.is_timeout() {
                    SourceError::Timeout
                } else {
                    SourceError::Other(e.to_string())
                }
            })
    }
}

/// The in-band upstream error text as an `AppError`, redacted like a non-2xx body
/// ([`redact_upstream_text`]). A busy/rate-limited frame is `Network` (what the
/// retry classifier keys on for a 429/5xx); everything else is `Provider`, with
/// the upstream wording kept so `is_context_length_error` still matches it.
pub(in crate::commands::ai_provider) fn frame_error(label: &str, raw: &str) -> AppError {
    let detail = redact_upstream_text(raw);
    let lower = detail.to_ascii_lowercase();
    let busy = [
        "overload",
        "rate limit",
        "rate_limit",
        "quota",
        "unavailable",
        "capacity",
    ];
    if busy.iter().any(|k| lower.contains(k)) {
        AppError::Network(format!(
            "{label}: service busy or rate limited ({detail}). Try again shortly."
        ))
    } else {
        AppError::Provider(format!("{label}: {detail}"))
    }
}

/// Read a provider's stream to its end and return the concatenated ANSWER text
/// (reasoning deltas excluded) plus the last usage it reported. The server-side
/// twin of `stream_response`: no job, no `ai:stream` events, no spend recording
/// (the `Completer` records the returned usage), but the same per-provider
/// `parse` closure and the same [`IdleGuard`].
///
/// Fails, like the one-shot JSON path it replaces, instead of returning a
/// partial answer as a success: an in-band error frame, an EOF with no terminal
/// piece (`done` or a stop reason — Gemini's only proof of completion), an empty
/// answer (quoting a refusal when there was one), more than
/// [`DEFAULT_MAX_BODY_BYTES`] of answer or of one unframed line (the old
/// `read_json_capped` memory cap), and — when `reject_truncated` — an output-limit
/// stop. `reject_truncated` is set only for JSON-mode calls: truncated JSON must
/// not reach `repair_json`, while a plain-text caller keeps accepting the text.
pub(in crate::commands::ai_provider) async fn collect<S, P>(
    source: &mut S,
    mut parse: P,
    limits: StreamLimits,
    label: &str,
    reject_truncated: bool,
) -> AppResult<(String, Usage)>
where
    S: ChunkSource,
    P: FnMut(&mut String) -> Vec<StreamPiece> + Send,
{
    let too_big = || AppError::Provider(format!("{label}: response exceeded the size limit"));
    let guard = IdleGuard::start(limits);
    let mut buf = String::new();
    let mut carry: Vec<u8> = Vec::new();
    let mut usage = Usage::default();
    let mut answer = String::new();
    let mut refusal = String::new();
    let mut stop: Option<StopReason> = None;
    let mut done = false;
    'read: loop {
        let chunk = guard
            .read(source.next_chunk())
            .await
            .map_err(|s| s.into_app(label, limits, |e| source_error(e, label, limits)))?;
        let Some(bytes) = chunk else { break };
        push_utf8(&mut buf, &mut carry, &bytes);
        for piece in parse(&mut buf) {
            if let Some(raw) = piece.error {
                return Err(frame_error(label, &raw));
            }
            if let Some(u) = piece.usage {
                usage = u;
            }
            stop = piece.stop_reason.or(stop);
            if piece.refusal {
                refusal.push_str(&piece.delta);
            } else if !piece.thinking {
                answer.push_str(&piece.delta);
            }
            if answer.len() + refusal.len() > DEFAULT_MAX_BODY_BYTES {
                return Err(too_big());
            }
            if piece.done {
                done = true;
                break 'read;
            }
        }
        // What `parse` left behind is a line it could not frame yet.
        if buf.len() > DEFAULT_MAX_BODY_BYTES {
            return Err(too_big());
        }
    }
    if !done && stop.is_none() {
        return Err(AppError::Network(format!(
            "{label}: the stream ended before the model finished"
        )));
    }
    if reject_truncated && stop == Some(StopReason::Length) {
        return Err(AppError::Provider(format!(
            "{label}: the response hit the output limit and was cut off"
        )));
    }
    if answer.trim().is_empty() {
        return Err(AppError::Provider(if refusal.trim().is_empty() {
            format!("{label}: unexpected response shape")
        } else {
            format!(
                "{label}: the model refused — {}",
                redact_upstream_text(&refusal)
            )
        }));
    }
    Ok((answer, usage))
}

fn source_error(e: SourceError, label: &str, limits: StreamLimits) -> AppError {
    match e {
        SourceError::Timeout => ceiling_error(label, limits.ceiling),
        SourceError::Other(m) => AppError::Network(format!("Stream error: {m}")),
    }
}

/// Test seam for the per-provider parser tests: run [`collect`] over canned
/// chunks (delivered back to back, then EOF) with the production limits.
#[cfg(test)]
pub(in crate::commands::ai_provider) async fn collect_canned(
    chunks: &[&str],
    parse: impl FnMut(&mut String) -> Vec<StreamPiece> + Send,
    reject_truncated: bool,
) -> AppResult<(String, Usage)> {
    struct Canned(std::collections::VecDeque<Vec<u8>>);
    #[async_trait]
    impl ChunkSource for Canned {
        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, SourceError> {
            Ok(self.0.pop_front())
        }
    }
    let mut src = Canned(chunks.iter().map(|c| c.as_bytes().to_vec()).collect());
    let limits = StreamLimits::new(Duration::from_secs(300));
    collect(&mut src, parse, limits, "T", reject_truncated).await
}

#[cfg(test)]
mod tests;

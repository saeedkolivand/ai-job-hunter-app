//! Local-chat / embed contention (Ollama serialises the daemon).
//!
//! A plain child module of `ollama` (`mod local_chat;`) — split out purely to
//! keep the parent module under the R8 LOC cap, not because this slice
//! belongs to a different responsibility. A pure move plus the new logic
//! itself; nothing about any OTHER Ollama call shape changed with it.
//!
//! Ollama serves one daemon per host and serialises requests to it by
//! default: an `/api/embeddings` call that starts while a `/api/chat`
//! completion is still running just queues behind it, and (before this) the
//! embed's own per-attempt `.timeout()` clock ran the WHOLE time it sat in
//! that queue — so all `EMBED_BUDGET_ATTEMPTS` attempts could expire without
//! one of them ever being serviced (see `timeouts::OLLAMA_EMBED`'s doc for
//! the field incident: three 30s attempts, 45s apart, none a real request).
//! [`ChatInFlight`] lets `ollama::embed_with` notice a chat is running and
//! wait a short, bounded amount for it to clear before dispatching, so the
//! request that follows gets a genuinely full timeout window instead.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;
use std::time::Duration;

use tokio::sync::Notify;

use crate::error::AppError;

/// Count of LOCAL `/api/chat` calls (streamed or not) currently in flight —
/// `ollama::stream_chat`, `ollama::complete_impl`, and the native branch of
/// `OllamaClient::chat_with_tools`. Never touched by `ollama_cloud.rs`
/// (routes through the OpenAI-compatible client, a different HTTP path
/// entirely) or by any cloud provider.
///
/// Chat NEVER reads or waits on this — only [`ChatInFlight::begin`]/`Drop`
/// touch it, both a single atomic op with no `.await` in between, so
/// wrapping a chat call in the guard cannot add latency, reordering, or
/// contention to chat itself. That asymmetry is deliberate: a
/// `tokio::sync::RwLock` (chat=read, embed=write) was considered and
/// rejected — tokio's own docs describe its write lock as
/// FAIR/write-preferring, using "a first-in, first-out queue for the tasks
/// waiting" so that "a read lock ... will not be granted until prior write
/// locks [complete], to prevent starvation" — which means a QUEUED embed (a
/// writer) would delay the next chat (a reader) behind it, the one outcome
/// this fix must not produce.
static LOCAL_CHAT_INFLIGHT: AtomicUsize = AtomicUsize::new(0);

/// Wakes [`wait_for_quiet`] the instant [`LOCAL_CHAT_INFLIGHT`] returns to
/// zero, so a quiet moment is noticed immediately rather than only at the
/// next poll.
static LOCAL_CHAT_QUIET: LazyLock<Notify> = LazyLock::new(Notify::new);

/// RAII marker held for the duration of one local `/api/chat` call. `Drop`
/// releases it on every exit path (success, an early `?`, or a panic unwind)
/// — the same discipline `RunGuard` (`commands/autopilot.rs`) uses for a
/// whole autopilot run.
pub(super) struct ChatInFlight;

impl ChatInFlight {
    pub(super) fn begin() -> Self {
        LOCAL_CHAT_INFLIGHT.fetch_add(1, Ordering::AcqRel);
        Self
    }
}

impl Drop for ChatInFlight {
    fn drop(&mut self) {
        if LOCAL_CHAT_INFLIGHT.fetch_sub(1, Ordering::AcqRel) == 1 {
            // Count just went 1 -> 0: wake anyone waiting for quiet.
            LOCAL_CHAT_QUIET.notify_waiters();
        }
    }
}

/// Whether a local chat completion is currently in flight — read AFTER
/// [`wait_for_quiet`] gives up, so callers can tell "busy" apart from a
/// genuinely unreachable/slow-but-idle daemon.
pub(super) fn is_chat_in_flight() -> bool {
    LOCAL_CHAT_INFLIGHT.load(Ordering::Acquire) > 0
}

/// Wait up to `budget` for [`LOCAL_CHAT_INFLIGHT`] to reach zero.
///
/// Returns immediately if it is already zero — the overwhelmingly common
/// case, so a healthy embed pays nothing extra. Otherwise waits for either a
/// wake from [`ChatInFlight::drop`] or `budget` to elapse, whichever comes
/// first; a chat that outlasts `budget` just means the caller proceeds
/// anyway, busy or not.
///
/// The `notified()` future is created BEFORE the length check on every loop
/// iteration — `tokio::sync::Notify`'s documented pattern for avoiding a
/// missed wakeup, where a `notify_waiters()` call that lands between the
/// check and the `.await` would otherwise never be observed.
pub(super) async fn wait_for_quiet(budget: Duration) {
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        let notified = LOCAL_CHAT_QUIET.notified();
        if LOCAL_CHAT_INFLIGHT.load(Ordering::Acquire) == 0 {
            return;
        }
        let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now()) else {
            return;
        };
        tokio::select! {
            _ = notified => {}
            _ = tokio::time::sleep(remaining) => return,
        }
    }
}

/// [`super::map_completion_transport_error`] plus one more distinction: a
/// timeout that follows [`wait_for_quiet`] giving up while a local chat was
/// STILL running is legibly "busy" — a materially different diagnosis from a
/// genuinely unreachable or slow-but-idle daemon, and a follow-up change
/// surfaces it in the UI (mirrors the Timeout-vs-Network distinction
/// `map_completion_transport_error` itself added for PR #1051).
pub(super) fn map_embed_transport_error(
    e: reqwest::Error,
    deadline: Duration,
    was_busy: bool,
) -> AppError {
    if was_busy && e.is_timeout() {
        AppError::Timeout(format!(
            "Ollama busy: a local chat completion was still running; no response within {}s",
            deadline.as_secs()
        ))
    } else {
        super::map_completion_transport_error(e, "Ollama", deadline)
    }
}

#[cfg(test)]
mod tests;

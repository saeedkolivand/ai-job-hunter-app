//! Local-chat / embed contention (Ollama serialises the daemon).
//!
//! Split out of `ollama.rs` (R8 LOC cap) purely to keep the parent module
//! under the hard cap — mirrors the `ollama_tests.rs` precedent (see that
//! file's own header) of moving a self-contained slice out to a sibling file
//! rather than growing the module that already owns every other Ollama
//! concern. A pure move plus the new logic itself; nothing about any OTHER
//! Ollama call shape changed with it.
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
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use serial_test::serial;

    use super::*;

    // `LOCAL_CHAT_INFLIGHT`/`LOCAL_CHAT_QUIET` are process-global statics, so
    // every test below that touches the gauge is `#[serial]` (a named key —
    // scoped to this file, not the crate-wide default `#[serial]` key other
    // modules use) to avoid cross-test interference under the default
    // parallel test runner. The two error-mapping tests at the bottom don't
    // touch the gauge and stay unserialized.

    /// No chat in flight: the wait must return effectively immediately, not
    /// spend any of `budget`. Generous 100ms upper bound (CI-safe) against a
    /// 2s budget that would fail this if the "already zero" fast path were
    /// ever removed.
    #[tokio::test]
    #[serial(ollama_local_chat_gauge)]
    async fn quiet_wait_returns_immediately_when_no_chat_is_in_flight() {
        assert!(!is_chat_in_flight());
        let started = tokio::time::Instant::now();
        wait_for_quiet(Duration::from_secs(2)).await;
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "must not wait at all when the gauge is already zero; took {:?}",
            started.elapsed()
        );
    }

    /// **The core property under test in the parent task**: an embed must not
    /// burn its whole retry budget on queue wait. Here a "chat" holds the
    /// gauge for ~40ms against a 2s budget; `wait_for_quiet` must wake as
    /// soon as it clears, not sit out the full budget.
    ///
    /// Mutation check: replace the `tokio::select!` body with a bare
    /// `tokio::time::sleep(remaining).await` (i.e. ignore the notify and
    /// always wait out the full remainder) — this fails, taking ~2s instead
    /// of ~40ms.
    #[tokio::test]
    #[serial(ollama_local_chat_gauge)]
    async fn quiet_wait_wakes_promptly_once_chat_clears_well_before_the_budget() {
        let guard = ChatInFlight::begin();
        assert!(is_chat_in_flight());
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            drop(guard);
        });
        let started = tokio::time::Instant::now();
        wait_for_quiet(Duration::from_secs(2)).await;
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_millis(500),
            "must wake promptly once chat clears, not wait out the budget; took {elapsed:?}"
        );
        assert!(
            elapsed >= Duration::from_millis(30),
            "must actually have waited for the guard to drop, not returned instantly; took {elapsed:?}"
        );
        assert!(!is_chat_in_flight());
    }

    /// A chat that never quiets down within `budget`: the wait must give up
    /// at the budget, not hang indefinitely — this is what lets the caller
    /// "degrade fast and honestly" instead of hanging on a busy daemon.
    ///
    /// Mutation check: drop the `tokio::time::sleep` arm from the `select!`
    /// (wait only on `notified`) — this test then hangs instead of returning
    /// around 80ms.
    #[tokio::test]
    #[serial(ollama_local_chat_gauge)]
    async fn quiet_wait_is_bounded_when_chat_never_clears() {
        let _guard = ChatInFlight::begin();
        let started = tokio::time::Instant::now();
        wait_for_quiet(Duration::from_millis(80)).await;
        let elapsed = started.elapsed();
        assert!(
            elapsed >= Duration::from_millis(70) && elapsed < Duration::from_millis(400),
            "must give up close to the budget, not hang or return early; took {elapsed:?}"
        );
        assert!(
            is_chat_in_flight(),
            "the still-in-flight chat guard must not have been affected by the wait"
        );
    }

    /// **Chat concurrency must be identical before and after.** The gauge is
    /// bookkeeping only — it must never itself serialise concurrent chat
    /// calls. Two overlapping guards must both be observably in flight at
    /// once (the counter reaches 2), proving `ChatInFlight::begin` never
    /// blocks on another instance.
    #[tokio::test]
    #[serial(ollama_local_chat_gauge)]
    async fn concurrent_chat_guards_do_not_serialize_each_other() {
        let peak = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..3 {
            let peak = peak.clone();
            handles.push(tokio::spawn(async move {
                let _g = ChatInFlight::begin();
                // Give the other tasks a chance to have also started theirs.
                tokio::time::sleep(Duration::from_millis(20)).await;
                peak.fetch_max(
                    LOCAL_CHAT_INFLIGHT.load(Ordering::Acquire),
                    Ordering::AcqRel,
                );
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        assert_eq!(
            peak.load(Ordering::Acquire),
            3,
            "three concurrent chat calls must all be observably in flight at once"
        );
        assert!(!is_chat_in_flight(), "every guard must release on drop");
    }

    /// A REAL `reqwest::Error` timeout (a wiremock server that answers slower
    /// than the client's own `.timeout()`, exactly like `retry.rs`'s own
    /// integration tests), mapped with `was_busy = true`, must be
    /// distinguishable from the plain-timeout wording.
    ///
    /// Mutation check: hardcode `was_busy` to `false` inside
    /// `map_embed_transport_error` and this fails — the message falls
    /// through to the generic `Ollama: no response...` wording instead of
    /// naming "busy".
    #[tokio::test]
    async fn a_timeout_while_chat_is_busy_gets_a_distinguishable_message() {
        let err = timed_out_request().await;
        let mapped = map_embed_transport_error(err, Duration::from_secs(30), true);
        let msg = mapped.to_string();
        assert!(
            msg.contains("busy"),
            "expected a busy-specific message, got: {msg}"
        );
        assert!(
            !msg.to_ascii_lowercase().contains("unreachable"),
            "a busy daemon must not be reported as unreachable; got: {msg}"
        );
    }

    /// The SAME timeout with `was_busy = false` must fall through to the
    /// existing plain-timeout wording (`map_completion_transport_error`'s own
    /// contract) — proving the two branches are genuinely distinguishable in
    /// both directions, not just that "busy" can appear.
    #[tokio::test]
    async fn a_timeout_while_chat_is_idle_gets_the_plain_timeout_message() {
        let err = timed_out_request().await;
        let mapped = map_embed_transport_error(err, Duration::from_secs(30), false);
        let msg = mapped.to_string();
        assert!(
            !msg.contains("busy"),
            "an idle-daemon timeout must not claim the daemon was busy; got: {msg}"
        );
        assert!(msg.contains("no response within 30s"), "got: {msg}");
    }

    /// Drive a real request against a wiremock server that answers slower
    /// than the client's own timeout, so `.is_timeout()` is genuinely `true`
    /// — reqwest has no public constructor for a timeout error outside its
    /// own crate. Mirrors `retry.rs`'s own integration-test pattern.
    async fn timed_out_request() -> reqwest::Error {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(300)))
            .mount(&server)
            .await;
        crate::net::http::shared()
            .get(server.uri())
            .timeout(Duration::from_millis(20))
            .send()
            .await
            .expect_err("a 20ms timeout against a 300ms-delayed response must time out")
    }
}

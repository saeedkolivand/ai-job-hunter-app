//! Bounded exponential backoff for the **non-streaming** provider paths
//! (`complete` / `embed`).
//!
//! Cloud providers occasionally return transient 429 (rate limit) or 5xx
//! (service) errors that succeed on a quick retry. This module retries those —
//! and transport-level send failures — a small, bounded number of times with
//! exponential backoff, honoring a `Retry-After` header when present. Streaming
//! is intentionally **not** retried (a mid-stream restart would duplicate already
//! emitted deltas), so this is only wired into the one-shot `complete`/`embed`
//! calls.
//!
//! The retry *decision* ([`should_retry`], [`backoff_delay`]) is pure and
//! unit-tested; [`send_with_retry`] is the thin async wrapper that rebuilds and
//! re-sends the request each attempt (a `RequestBuilder` is consumed by `send`,
//! so the caller supplies a builder factory).

use std::time::Duration;

use reqwest::{RequestBuilder, Response, StatusCode};

/// Maximum number of attempts (initial try + retries) for a transient failure.
pub const MAX_ATTEMPTS: u32 = 3;
/// Base delay for the exponential schedule (attempt 1 → BASE, attempt 2 → 2·BASE…).
const BASE_DELAY_MS: u64 = 500;
/// Never wait longer than this between attempts, even if `Retry-After` is huge —
/// a one-shot completion shouldn't stall the UI for minutes.
const MAX_DELAY_MS: u64 = 8_000;

/// Whether a response status is worth retrying. 429 (rate limit / quota) and 5xx
/// (service errors) are transient; everything else (success, 4xx client errors)
/// is terminal and returned to the caller as-is.
pub fn is_retryable_status(status: StatusCode) -> bool {
    let code = status.as_u16();
    code == 429 || (500..=599).contains(&code)
}

/// Whether to make another attempt given the attempt number (1-based) and the
/// outcome. `attempt` is the attempt that just finished; we retry while there are
/// attempts left and the failure is transient (a transport error, or a retryable
/// status).
pub fn should_retry(attempt: u32, transient: bool) -> bool {
    transient && attempt < MAX_ATTEMPTS
}

/// Backoff delay before the *next* attempt. Prefers the server's `Retry-After`
/// (seconds) when present and sane, otherwise an exponential schedule. Always
/// clamped to `[0, MAX_DELAY_MS]`. `attempt` is the 1-based number of the attempt
/// that just failed.
pub fn backoff_delay(attempt: u32, retry_after_secs: Option<u64>) -> Duration {
    let ms = match retry_after_secs {
        Some(secs) => secs.saturating_mul(1000),
        None => {
            // attempt 1 → BASE, attempt 2 → 2·BASE, attempt 3 → 4·BASE …
            let factor = 1u64 << (attempt.saturating_sub(1)).min(16);
            BASE_DELAY_MS.saturating_mul(factor)
        }
    };
    Duration::from_millis(ms.min(MAX_DELAY_MS))
}

/// Parse a `Retry-After` header value (RFC 7231) as whole seconds. Only the
/// delta-seconds form is honored (the HTTP-date form is rare for these APIs and
/// the exponential fallback covers it).
fn parse_retry_after(resp: &Response) -> Option<u64> {
    resp.headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
}

/// Send a request with bounded exponential backoff on transient failures.
///
/// `build` is called once per attempt to produce a fresh [`RequestBuilder`]
/// (since `send` consumes it). Returns the first success, the first terminal
/// (non-retryable) response, or — when every attempt was transient — the last
/// outcome (response or transport error). Never retries beyond [`MAX_ATTEMPTS`].
pub async fn send_with_retry<F>(mut build: F) -> reqwest::Result<Response>
where
    F: FnMut() -> RequestBuilder,
{
    let mut attempt = 1u32;
    loop {
        let outcome = build().send().await;
        let (transient, retry_after) = match &outcome {
            Ok(resp) if is_retryable_status(resp.status()) => (true, parse_retry_after(resp)),
            Ok(_) => (false, None),
            Err(_) => (true, None), // transport-level failure (connect/timeout) is transient
        };

        if !should_retry(attempt, transient) {
            return outcome;
        }

        let delay = backoff_delay(attempt, retry_after);
        tracing::debug!(
            "ai retry: attempt {attempt}/{MAX_ATTEMPTS} transient, backing off {:?}",
            delay
        );
        tokio::time::sleep(delay).await;
        attempt += 1;
    }
}

// ── send_with_retry integration tests ────────────────────────────────────────
//
// These tests exercise the full retry *loop* (build → send → check → backoff →
// rebuild → send …) against a real wiremock server rather than the helper
// predicates in isolation.
//
// `send_with_retry` uses `tokio::time::sleep` for backoff, which requires
// `tokio`'s `test-util` feature to pause.  That feature is NOT enabled in this
// crate's Cargo.toml, so we let the real backoff run.  With MAX_ATTEMPTS=3 and
// BASE_DELAY_MS=500 the worst case is ~1.5 s of wall time — acceptable for an
// integration test that exercises code no unit test can reach.
//
// Wiremock's `up_to_n_times(1)` mocks serve responses in FIFO registration
// order so the sequence [429, 429, 200] is faithfully replayed.

#[cfg(test)]
mod retry_loop_tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{send_with_retry, MAX_ATTEMPTS};

    /// Spin up a wiremock server that serves the given status codes in FIFO order
    /// and drive `send_with_retry` once.  Returns (call_count, is_ok).
    async fn run_retry(status_codes: Vec<u16>) -> (u32, bool) {
        let server = MockServer::start().await;

        // Register one mock per expected response, consumed in FIFO order.
        for code in &status_codes {
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(*code))
                .up_to_n_times(1)
                .mount(&server)
                .await;
        }

        let url = server.uri();
        let client = crate::net::http::shared();
        let call_count = Arc::new(AtomicU32::new(0));
        let counter = call_count.clone();

        let result = send_with_retry(|| {
            counter.fetch_add(1, Ordering::SeqCst);
            client.get(&url)
        })
        .await;

        (call_count.load(Ordering::SeqCst), result.is_ok())
    }

    #[tokio::test]
    async fn retry_loop_succeeds_after_two_transient_429s() {
        // R4: 429 → 429 → 200.  Build closure invoked 3×; final result is Ok.
        let (calls, is_ok) = run_retry(vec![429, 429, 200]).await;
        assert_eq!(
            calls, 3,
            "build closure must be invoked 3× (initial + 2 retries); got {calls}"
        );
        assert!(is_ok, "the eventual 200 response must be returned as Ok");
    }

    #[tokio::test]
    async fn retry_loop_stops_at_max_attempts_on_persistent_429() {
        // R4: MAX_ATTEMPTS consecutive 429s → loop stops exactly at the budget.
        // The final return is Ok(resp with status 429) because HTTP 4xx are not
        // reqwest transport errors; what matters is the call count stays bounded.
        let statuses = vec![429u16; MAX_ATTEMPTS as usize];
        let (calls, _) = run_retry(statuses).await;
        assert_eq!(
            calls, MAX_ATTEMPTS,
            "loop must stop after exactly MAX_ATTEMPTS ({MAX_ATTEMPTS}) calls; got {calls}"
        );
    }

    #[tokio::test]
    async fn retry_loop_does_not_retry_terminal_4xx() {
        // A 400 is terminal; one call, no retry.
        let (calls, is_ok) = run_retry(vec![400]).await;
        assert_eq!(
            calls, 1,
            "terminal 400 must not be retried; got {calls} calls"
        );
        assert!(
            is_ok,
            "400 response must be returned as Ok (not a transport Err)"
        );
    }

    #[tokio::test]
    async fn retry_loop_returns_immediately_on_200() {
        let (calls, is_ok) = run_retry(vec![200]).await;
        assert_eq!(calls, 1, "200 must not trigger a retry; got {calls} calls");
        assert!(is_ok, "200 response must be Ok");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_statuses_are_429_and_5xx_only() {
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::GATEWAY_TIMEOUT));

        // Terminal — never retried.
        assert!(!is_retryable_status(StatusCode::OK));
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(StatusCode::NOT_FOUND));
        assert!(!is_retryable_status(StatusCode::UNPROCESSABLE_ENTITY));
    }

    #[test]
    fn should_retry_respects_attempt_budget_and_transience() {
        // Transient failures retry until the last attempt.
        assert!(should_retry(1, true));
        assert!(should_retry(2, true));
        // The final attempt never retries.
        assert!(!should_retry(MAX_ATTEMPTS, true));
        assert!(!should_retry(MAX_ATTEMPTS + 1, true));
        // Non-transient outcomes never retry.
        assert!(!should_retry(1, false));
    }

    #[test]
    fn backoff_is_exponential_without_retry_after() {
        assert_eq!(backoff_delay(1, None), Duration::from_millis(500));
        assert_eq!(backoff_delay(2, None), Duration::from_millis(1000));
        assert_eq!(backoff_delay(3, None), Duration::from_millis(2000));
    }

    #[test]
    fn backoff_honors_retry_after_over_exponential() {
        // 2s Retry-After wins over the ~500ms exponential value.
        assert_eq!(backoff_delay(1, Some(2)), Duration::from_millis(2000));
        // Sub-exponential Retry-After is honored exactly (the server knows best).
        assert_eq!(backoff_delay(3, Some(1)), Duration::from_millis(1000));
    }

    #[test]
    fn backoff_is_clamped_to_the_ceiling() {
        // A huge Retry-After is clamped so the UI never stalls for minutes.
        assert_eq!(
            backoff_delay(1, Some(600)),
            Duration::from_millis(MAX_DELAY_MS)
        );
        // The exponential schedule is clamped too at high attempt counts.
        assert!(backoff_delay(20, None) <= Duration::from_millis(MAX_DELAY_MS));
    }
}

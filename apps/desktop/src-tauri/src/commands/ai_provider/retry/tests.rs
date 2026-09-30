//! Unit tests for `retry.rs`: the pure retry-decision predicates
//! (`is_retryable_status`/`should_retry`/`backoff_delay`) plus the full
//! retry *loop* (build → send → check → backoff → rebuild → send …) driven
//! against a real wiremock server for the one-shot, embed, and streaming
//! entry points.
//!
//! `send_with_retry` uses `tokio::time::sleep` for backoff, which requires
//! `tokio`'s `test-util` feature to pause.  That feature is NOT enabled in
//! this crate's Cargo.toml, so we let the real backoff run.  With
//! MAX_ATTEMPTS=3 and BASE_DELAY_MS=500 the worst case is ~1.5 s of wall
//! time — acceptable for an integration test that exercises code no unit
//! test can reach.
//!
//! Wiremock's `up_to_n_times(1)` mocks serve responses in FIFO registration
//! order so the sequence [429, 429, 200] is faithfully replayed.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

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

// ── one-shot loop (send_with_retry) ─────────────────────────────────────

/// Spin up a wiremock server that serves the given status codes in FIFO order
/// and drive `send_with_retry` once.  Returns (call_count, is_ok).
async fn run_retry(status_codes: Vec<u16>) -> (u32, bool) {
    // Generous timeout: these tests are about the ATTEMPT COUNT. The budget
    // itself has its own test below.
    run_retry_with(
        status_codes,
        std::time::Duration::ZERO,
        Duration::from_secs(60),
    )
    .await
}

/// [`run_retry`] with an explicit per-response `delay` and per-call
/// `timeout` (which is also the whole sequence's budget).
async fn run_retry_with(status_codes: Vec<u16>, delay: Duration, timeout: Duration) -> (u32, bool) {
    let server = MockServer::start().await;

    // Register one mock per expected response, consumed in FIFO order.
    for code in &status_codes {
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(*code).set_delay(delay))
            .up_to_n_times(1)
            .mount(&server)
            .await;
    }

    let url = server.uri();
    let client = crate::net::http::shared();
    let call_count = Arc::new(AtomicU32::new(0));
    let counter = call_count.clone();

    let result = send_with_retry(
        || {
            counter.fetch_add(1, Ordering::SeqCst);
            client.get(&url)
        },
        timeout,
    )
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

/// **The one-shot path's total budget** — the bound every derived deadline
/// (`timeouts::quality_run_deadline`, the renderer's client timeouts) counts
/// on when it counts ONE per-call deadline per provider call.
///
/// Without it each attempt rebuilt its own full `.timeout()`, so a call
/// documented as 300 s-bounded really cost up to `MAX_ATTEMPTS × 300 s` plus
/// backoff — 901 s — and a 14-call quality run was bounded by ~12 600 s
/// against an advertised 4 500 s.
///
/// Shape rather than a tight wall-clock: the response is delayed 300 ms and
/// the budget is 700 ms, so the first backoff (500 ms) provably cannot fit
/// (300 + 500 > 700) on ANY machine — a slow host only makes `elapsed`
/// larger, never smaller. Mutation check: drop the budget (pass `None`
/// through to the loop, i.e. the pre-fix behaviour) and this becomes 3 calls.
#[tokio::test]
async fn a_one_shot_call_stops_once_its_own_timeout_is_spent() {
    let (calls, _) = run_retry_with(
        vec![429, 200],
        Duration::from_millis(300),
        Duration::from_millis(700),
    )
    .await;
    assert_eq!(
        calls, 1,
        "the retry sequence must stay inside the caller's per-call timeout; got {calls} calls"
    );
}

/// The same budget, unspent: a provider that rejects PROMPTLY is still
/// retried inside it. Together with the test above this pins the trade —
/// the bound is on wall time, not on retrying.
#[tokio::test]
async fn a_prompt_rejection_is_still_retried_inside_the_budget() {
    let (calls, is_ok) =
        run_retry_with(vec![429, 200], Duration::ZERO, Duration::from_secs(30)).await;
    assert_eq!(calls, 2, "a fast 429 leaves budget for the retry");
    assert!(is_ok, "the eventual 200 must be returned as Ok");
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

// ── the sub-floor-remainder refusal + send_embed_with_retry's split budget ──

/// Mount one mock per `(status, delay)`, consumed in FIFO order — the
/// per-response variant of [`run_retry_with`]'s uniform delay, needed by the
/// two tests below where the point is that attempt 2 behaves DIFFERENTLY
/// from attempt 1.
async fn mount_sequence(server: &MockServer, responses: &[(u16, Duration)]) {
    for (code, delay) in responses {
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(*code).set_delay(*delay))
            .up_to_n_times(1)
            .mount(server)
            .await;
    }
}

/// Drive one call and report `(call_count, the status of the RESPONSE that
/// came back)` — `None` when the loop returned a transport error instead of a
/// response, which is the whole distinction the floor test turns on.
async fn run_sequenced(
    responses: Vec<(u16, Duration)>,
    per_attempt: Duration,
    embed: bool,
) -> (u32, Option<u16>) {
    let server = MockServer::start().await;
    mount_sequence(&server, &responses).await;
    let url = server.uri();
    let client = crate::net::http::shared();
    let call_count = Arc::new(AtomicU32::new(0));
    let counter = call_count.clone();
    let build = || {
        counter.fetch_add(1, Ordering::SeqCst);
        client.get(&url)
    };
    let result = if embed {
        send_embed_with_retry(build, per_attempt).await
    } else {
        send_with_retry(build, per_attempt).await
    };
    (
        call_count.load(Ordering::SeqCst),
        result.ok().map(|resp| resp.status().as_u16()),
    )
}

/// **A remainder too small to finish a request must not be spent** — because
/// the attempt it buys can only end in a timeout, and that timeout REPLACES
/// the actionable outcome the loop already has.
///
/// `!left.is_zero()` admitted any positive sliver: after a 429 at ~100 ms plus
/// the 500 ms backoff, ~1.3 s of a 1.9 s budget was left, so a second attempt
/// started against a provider that needs 5 s — and the 429 (which the caller
/// maps to a rate-limit error naming `Retry-After`) came back to the user as
/// a generic transport timeout instead.
///
/// Shape rather than a tight wall clock, and one-sided: the remainder is at
/// MOST 1.3 s (a slower host only shrinks it), always under the 2 s floor, so
/// the refusal is provable on any machine. Mutation check: drop the floor
/// (`filter(|left| !left.is_zero())`) and this becomes 2 calls returning
/// `None`.
#[tokio::test]
async fn a_remainder_too_small_to_finish_keeps_the_last_real_outcome() {
    let (calls, status) = run_sequenced(
        vec![
            (429, Duration::from_millis(100)),
            (200, Duration::from_secs(5)),
        ],
        Duration::from_millis(1_900),
        false,
    )
    .await;
    assert_eq!(
        calls, 1,
        "a sub-floor remainder must not buy a doomed attempt; got {calls} calls"
    );
    assert_eq!(
        status,
        Some(429),
        "the caller must get the actionable 429, not the doomed attempt's timeout"
    );
}

/// **An embed whose first attempt TIMES OUT still gets a second one.**
///
/// Collapsing the per-attempt timeout into the sequence budget made
/// retry-after-timeout structurally unreachable at every call site — correct
/// for a 300 s completion, wrong for a 30 s embed, where the first request of
/// an indexing run times out while Ollama cold-loads the embedding model and
/// a fresh attempt then succeeds at once. `send_embed_with_retry` separates
/// the two values so that recovery exists again.
///
/// The 2 s per-attempt bound is scaled down from `OLLAMA_EMBED`'s 30 s but the
/// arithmetic is the real one: attempt 1 burns the full per-attempt timeout,
/// the 500 ms backoff follows, and the 3× sequence budget still leaves 3.5 s —
/// comfortably over the 2 s floor, so a slow host cannot flip it. Mutation
/// check: route this call through `send_with_retry` (the collapsed shape) and
/// it becomes 1 call returning `None`.
#[tokio::test]
async fn an_embed_that_times_out_cold_still_gets_a_second_attempt() {
    let (calls, status) = run_sequenced(
        vec![
            (200, Duration::from_secs(5)),
            (200, Duration::from_millis(0)),
        ],
        Duration::from_secs(2),
        true,
    )
    .await;
    assert_eq!(
        calls, 2,
        "a timed-out first attempt must be retried inside the embed budget; got {calls}"
    );
    assert_eq!(
        status,
        Some(200),
        "the second attempt's success is what the caller sees"
    );
}

// ── send_stream_with_retry (the STREAMING entry point's initial send) ──────
//
// The response STATUS is known before any delta is read, so a retry here
// re-sends a request that emitted nothing.

/// [`run_stream_retry_with_budget`] driven through the STREAMING entry point
/// instead. `budget` is the caller's `stream_deadline`; generous here so the
/// attempt count is what is under test, except in the budget test below.
async fn run_stream_retry(status_codes: Vec<u16>) -> (u32, bool) {
    run_stream_retry_with_budget(status_codes, Duration::ZERO, Duration::from_secs(60)).await
}

async fn run_stream_retry_with_budget(
    status_codes: Vec<u16>,
    delay: Duration,
    budget: Duration,
) -> (u32, bool) {
    let server = MockServer::start().await;
    for code in &status_codes {
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(*code).set_delay(delay))
            .up_to_n_times(1)
            .mount(&server)
            .await;
    }
    let url = server.uri();
    let client = crate::net::http::shared();
    let call_count = Arc::new(AtomicU32::new(0));
    let counter = call_count.clone();
    let result = send_stream_with_retry(
        || {
            counter.fetch_add(1, Ordering::SeqCst);
            client.get(&url)
        },
        budget,
    )
    .await;
    (call_count.load(Ordering::SeqCst), result.is_ok())
}

// A stream's INITIAL send is retried like any other request. This used to be
// terminal on the reasoning that "a mid-stream restart would duplicate
// deltas" — but the response status is known before a single delta has been
// read, so there is nothing to duplicate. Treating it as terminal is what
// turned one provider 429 into a discarded nine-minute generation.
#[tokio::test]
async fn stream_handshake_recovers_from_a_transient_429() {
    let (calls, is_ok) = run_stream_retry(vec![429, 200]).await;
    assert_eq!(
        calls, 2,
        "a 429 on the stream handshake must be retried; got {calls} calls"
    );
    assert!(is_ok, "the eventual 200 must be returned as Ok");
}

#[tokio::test]
async fn stream_handshake_stays_bounded_on_a_persistent_429() {
    // Bounded exactly like the one-shot path — a rate-limited account must
    // not turn into an unbounded retry storm.
    let (calls, _) = run_stream_retry(vec![429u16; MAX_ATTEMPTS as usize]).await;
    assert_eq!(
        calls, MAX_ATTEMPTS,
        "stream retries must stop at the budget"
    );
}

/// The deadline bounds the WHOLE retry sequence, not each attempt.
///
/// Without it three attempts could each get a full `stream_deadline` and run
/// to 3x it — past the renderer's own timeout, which would replace the
/// actionable provider error with a generic "Generation timed out" and
/// invert the relationship `computeStreamTimeoutMs`'s test pins.
///
/// Same shape as the one-shot twin above (a 300 ms response against a 700 ms
/// budget, so the 500 ms first backoff provably cannot fit) and the same
/// reported case: that 429 came back only after the deadline had elapsed, so
/// there was never budget for a retry and behaviour is unchanged. Retries
/// only help a provider that rejects promptly, which is the normal shape of
/// a rate limit.
#[tokio::test]
async fn stream_handshake_does_not_retry_once_the_budget_is_spent() {
    let (calls, _) = run_stream_retry_with_budget(
        vec![429, 200],
        Duration::from_millis(300),
        Duration::from_millis(700),
    )
    .await;
    assert_eq!(
        calls, 1,
        "no budget left means no retry, even though the 429 is transient"
    );
}

#[tokio::test]
async fn stream_handshake_does_not_retry_a_terminal_4xx() {
    // e.g. a bad model name: retrying cannot help and would triple the wait.
    let (calls, _) = run_stream_retry(vec![400]).await;
    assert_eq!(calls, 1, "a terminal 400 must not be retried");
}

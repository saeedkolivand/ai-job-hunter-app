//! Bounded exponential backoff for the **non-streaming** provider paths
//! (`complete` / `embed`).
//!
//! Cloud providers occasionally return transient 429 (rate limit) or 5xx
//! (service) errors that succeed on a quick retry. This module retries those —
//! and transport-level send failures — a small, bounded number of times with
//! exponential backoff, honoring a `Retry-After` header when present.
//!
//! A stream is never restarted MID-stream (that would duplicate already-emitted
//! deltas), but the initial `send()` on a streaming request is retried like any
//! other: the response STATUS arrives before a single delta has been read, so a
//! 429/5xx there is exactly the transient one-shot failure this module exists
//! for. Treating it as terminal is what turned a provider rate-limit into a lost
//! nine-minute generation in a reported session.
//!
//! **Both entry points own the caller's per-request timeout and bound the WHOLE
//! retry sequence by it** — the caller passes the operation's timeout instead of
//! setting `.timeout()` on the builder itself. Retries are free wall-clock
//! otherwise: each attempt would rebuild its own full `.timeout()`, so an
//! operation documented as "bounded by `OLLAMA_COMPLETION_BASELINE` (300 s)" really cost
//! up to `MAX_ATTEMPTS × 300 s` + backoff, and every outer deadline derived from
//! those per-call bounds (`timeouts::quality_run_deadline`, the renderer's own
//! client timeouts) was short by that factor. The streaming path had this budget
//! from the start; the one-shot path did not, which is the bug this shape closes.
//!
//! One consequence is structural and worth naming: when the per-attempt timeout
//! IS the whole budget, a timed-out attempt can never be retried. That is the
//! intended trade for a 120 s/300 s completion and the wrong one for a 15 s
//! embedding, so [`send_embed_with_retry`] separates the two values for that one
//! call shape (see its doc for the cold-model-load case it exists to recover).
//!
//! The retry *decision* ([`should_retry`], [`backoff_delay`]) is pure and
//! unit-tested; [`send_with_retry`] is the thin async wrapper that rebuilds and
//! re-sends the request each attempt (a `RequestBuilder` is consumed by `send`,
//! so the caller supplies a builder factory).

use std::time::{Duration, Instant};

use reqwest::{RequestBuilder, Response, StatusCode};

/// Maximum number of attempts (initial try + retries) for a transient failure.
///
/// **Not a multiplier on the caller's timeout.** Both entry points bound the
/// whole sequence by the operation's own timeout (see the module doc), so
/// raising this changes how many PROMPT rejections are retried inside that one
/// bound, never how long the operation can take. That is what lets
/// `timeouts::quality_run_deadline` count one call's own deadline per call
/// rather than three.
pub const MAX_ATTEMPTS: u32 = 3;
/// Base delay for the exponential schedule (attempt 1 → BASE, attempt 2 → 2·BASE…).
const BASE_DELAY_MS: u64 = 500;
/// Never wait longer than this between attempts, even if `Retry-After` is huge —
/// a one-shot completion shouldn't stall the UI for minutes.
const MAX_DELAY_MS: u64 = 8_000;

/// The same ceiling for a STREAM's initial send. Higher than [`MAX_DELAY_MS`]
/// because the trade is different: the alternative to waiting is discarding a
/// generation the user has already been waiting minutes for, and the renderer
/// shows the job as running throughout. Still bounded, and still capped by the
/// request's own `stream_deadline`.
const MAX_STREAM_DELAY_MS: u64 = 30_000;

/// The smallest remainder worth starting another attempt with.
///
/// A retry needs enough time for a WHOLE round trip — DNS, TCP connect, the TLS
/// handshake, the request, and the provider's response. Below that it cannot
/// possibly finish, and starting it anyway is strictly harmful, not merely
/// wasteful: the doomed attempt ends in a transport TIMEOUT, and that timeout
/// becomes the loop's return value, REPLACING the actionable outcome the
/// previous attempt already had. A 429 with a `Retry-After` (the caller maps it
/// to a rate-limit error the UI can explain) came back to the user as a generic
/// "request timed out" — the last real answer thrown away by an attempt that
/// never had a chance. Executed, not theorised.
///
/// 2 s is a deliberate small value: a cloud 429 rejection round-trips in a few
/// hundred milliseconds, so this is roughly 4× the observed floor plus
/// handshake headroom, while staying far below the SMALLEST per-attempt bound
/// that reaches this loop (`timeouts::EMBED`, 30 s — it took that title from
/// `OLLAMA_EMBED` when the latter widened to 60 s) — so it can only ever refuse
/// an attempt that was already doomed, never one that had a real chance.
const MIN_RETRY_ATTEMPT_FLOOR: Duration = Duration::from_secs(2);

/// How many per-attempt timeouts one EMBED call may spend in total (see
/// [`send_embed_with_retry`]).
const EMBED_BUDGET_ATTEMPTS: u32 = 3;

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

/// Backoff delay at the default (one-shot) ceiling. Test-only entry point for
/// the pure schedule — production always goes through [`backoff_delay_capped`],
/// since the streaming path needs a different ceiling.
///
/// Backoff delay before the *next* attempt. Prefers the server's `Retry-After`
/// (seconds) when present and sane, otherwise an exponential schedule. Always
/// clamped to `[0, MAX_DELAY_MS]`. `attempt` is the 1-based number of the attempt
/// that just failed.
#[cfg(test)]
pub fn backoff_delay(attempt: u32, retry_after_secs: Option<u64>) -> Duration {
    backoff_delay_capped(attempt, retry_after_secs, MAX_DELAY_MS)
}

/// [`backoff_delay`] with an explicit ceiling, so the streaming path can afford
/// a longer wait than a one-shot completion. Pure + unit-tested.
pub fn backoff_delay_capped(attempt: u32, retry_after_secs: Option<u64>, max_ms: u64) -> Duration {
    let ms = match retry_after_secs {
        Some(secs) => secs.saturating_mul(1000),
        None => {
            // attempt 1 → BASE, attempt 2 → 2·BASE, attempt 3 → 4·BASE …
            let factor = 1u64 << (attempt.saturating_sub(1)).min(16);
            BASE_DELAY_MS.saturating_mul(factor)
        }
    };
    Duration::from_millis(ms.min(max_ms))
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
///
/// **`timeout` is the operation's own per-request bound (`timeouts::COMPLETION`,
/// `timeouts::ollama_completion_deadline(effort)`, `timeouts::EMBED`, …) and
/// the caller must not
/// set one on the builder** — this function applies it, and it bounds the WHOLE
/// sequence rather than each attempt (see [`send_with_retry_capped`]). One
/// argument rather than a builder `.timeout()` plus a budget parameter, because
/// the two have to be the SAME value: a call site that set 300 s and passed 60 s
/// would silently truncate, and one that passed 900 s would re-open the 3×
/// overrun this bound exists to close.
pub async fn send_with_retry<F>(build: F, timeout: Duration) -> reqwest::Result<Response>
where
    F: FnMut() -> RequestBuilder,
{
    send_with_retry_capped(build, MAX_DELAY_MS, timeout, timeout).await
}

/// [`send_with_retry`] for an EMBEDDINGS call, where the per-attempt timeout and
/// the sequence budget are deliberately NOT the same value: each attempt is
/// bounded by `per_attempt` (`timeouts::EMBED` / `timeouts::OLLAMA_EMBED`) and
/// the sequence by [`EMBED_BUDGET_ATTEMPTS`] × that.
///
/// **Why this one call shape keeps a real retry.** Collapsing the two into one
/// argument made "retry after a TIMEOUT" structurally unreachable everywhere:
/// attempt 1 IS the whole budget, so `Err(timeout) => transient` can never lead
/// to a second attempt. That is the intended trade for a 120 s/300 s completion
/// — an attempt that spent five minutes is not worth repeating, and the outer
/// `quality_run_deadline` counts exactly one of them per call. It is the WRONG
/// trade here: `OLLAMA_EMBED` is 60 s, and the case that needs a second attempt
/// is the first embed of an indexing run, where Ollama is COLD-LOADING the
/// embedding model and the first request times out while the load completes. A
/// fresh attempt then succeeds immediately; without one, the first document of
/// an indexing run fails for a reason that has already gone away.
///
/// The worst case is unchanged from before that collapse (`MAX_ATTEMPTS` × the
/// per-attempt timeout + backoff), it is bounded, and no outer deadline is
/// derived from the embed constants — indexing has no run-level deadline that
/// counts them.
pub async fn send_embed_with_retry<F>(build: F, per_attempt: Duration) -> reqwest::Result<Response>
where
    F: FnMut() -> RequestBuilder,
{
    let budget = per_attempt.saturating_mul(EMBED_BUDGET_ATTEMPTS);
    send_with_retry_capped(build, MAX_DELAY_MS, per_attempt, budget).await
}

/// [`send_with_retry`] for a STREAM's initial send.
///
/// Safe on the streaming path because this covers only the request/response
/// handshake: the response STATUS is known before any delta has been read, so a
/// retry here re-sends a request that emitted nothing. Nothing restarts a stream
/// that has already produced output.
///
/// `deadline` is the request's own `stream_deadline` — applied to each attempt
/// and bounding the whole sequence, exactly like [`send_with_retry`]'s
/// `timeout`. The only difference is the backoff ceiling
/// ([`MAX_STREAM_DELAY_MS`]): the alternative to waiting here is discarding a
/// generation the user has already waited minutes for.
///
/// The practical effect matches the reported failure: that 429 came back after
/// the full deadline had already elapsed, so it retries zero times and behaves
/// exactly as before. Retries only help when a provider rejects promptly, which
/// is the normal shape of a rate limit.
pub async fn send_stream_with_retry<F>(build: F, deadline: Duration) -> reqwest::Result<Response>
where
    F: FnMut() -> RequestBuilder,
{
    send_with_retry_capped(build, MAX_STREAM_DELAY_MS, deadline, deadline).await
}

/// The shared loop. Every attempt is bounded by `per_attempt` and the WHOLE
/// sequence by `budget`:
///
/// * the first attempt gets `per_attempt` — for the completion/stream entry
///   points the two arguments are the same value, so an unretried call behaves
///   exactly as it did when the call site set its own `.timeout()`;
/// * a retry is only started when the backoff AND a usable slice of request time
///   ([`MIN_RETRY_ATTEMPT_FLOOR`]) still fit inside what is left, and it is given
///   `min(per_attempt, remainder)`, so the sequence cannot outlive `budget` no
///   matter how many attempts it makes.
///
/// The consequence that matters when `per_attempt == budget`: an attempt that
/// spends its whole timeout is never retried. A prompt rejection (a 429 that
/// comes back in milliseconds — the normal shape of a rate limit) still is, which
/// is the case retries were added for. [`send_embed_with_retry`] is the one
/// caller that separates the two, and its doc says why.
async fn send_with_retry_capped<F>(
    mut build: F,
    max_delay_ms: u64,
    per_attempt: Duration,
    budget: Duration,
) -> reqwest::Result<Response>
where
    F: FnMut() -> RequestBuilder,
{
    let started = Instant::now();
    let mut attempt = 1u32;
    let mut attempt_timeout = per_attempt.min(budget);
    loop {
        let outcome = build().timeout(attempt_timeout).send().await;
        let (transient, retry_after) = match &outcome {
            Ok(resp) if is_retryable_status(resp.status()) => (true, parse_retry_after(resp)),
            Ok(_) => (false, None),
            Err(_) => (true, None), // transport-level failure (connect/timeout) is transient
        };

        if !should_retry(attempt, transient) {
            return outcome;
        }

        let delay = backoff_delay_capped(attempt, retry_after, max_delay_ms);

        // Only start another attempt if the budget can still pay for the backoff
        // AND leave a USABLE slice of request time. `spent` is projected past the
        // sleep so the remainder handed to the next attempt is what will
        // actually be left when it starts.
        //
        // The floor is what makes this a real refusal rather than a formality: a
        // remainder of a few milliseconds is not zero, so it used to admit an
        // attempt that could only end in a transport timeout — and that timeout
        // then REPLACED the actionable outcome this loop already held (a 429 with
        // its `Retry-After`). Refusing it returns the last REAL outcome instead.
        let spent = started.elapsed() + delay;
        let Some(remaining) = budget
            .checked_sub(spent)
            .filter(|left| *left >= MIN_RETRY_ATTEMPT_FLOOR)
        else {
            tracing::warn!(
                "ai retry: budget spent after attempt {attempt}/{MAX_ATTEMPTS} \
                 ({spent:?} of {budget:?}, less than {MIN_RETRY_ATTEMPT_FLOOR:?} left), \
                 returning the last outcome"
            );
            return outcome;
        };

        tracing::warn!(
            // WARN, not DEBUG: a retry means the provider pushed back, which is
            // the context you want when a generation later fails outright.
            "ai retry: attempt {attempt}/{MAX_ATTEMPTS} transient, backing off {:?}",
            delay
        );
        tokio::time::sleep(delay).await;
        attempt_timeout = per_attempt.min(remaining);
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

    use super::{send_stream_with_retry, send_with_retry, Duration, MAX_ATTEMPTS};

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
    async fn run_retry_with(
        status_codes: Vec<u16>,
        delay: Duration,
        timeout: Duration,
    ) -> (u32, bool) {
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
            super::send_embed_with_retry(build, per_attempt).await
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
    /// for a 300 s completion, wrong for a 60 s embed, where the first request of
    /// an indexing run times out while Ollama cold-loads the embedding model and
    /// a fresh attempt then succeeds at once. `send_embed_with_retry` separates
    /// the two values so that recovery exists again.
    ///
    /// The 2 s per-attempt bound is scaled down from `OLLAMA_EMBED`'s 60 s but the
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

    /// [`run_retry`] driven through the STREAMING entry point instead. `budget`
    /// is the caller's `stream_deadline`; generous here so the attempt count is
    /// what is under test, except in the budget test below.
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

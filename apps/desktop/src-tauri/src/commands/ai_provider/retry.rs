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
/// that reaches this loop (`timeouts::EMBED` and `timeouts::OLLAMA_EMBED`, both
/// 30 s) — so it can only ever refuse an attempt that was already doomed, never
/// one that had a real chance.
const MIN_RETRY_ATTEMPT_FLOOR: Duration = Duration::from_secs(2);

/// How many per-attempt timeouts one EMBED call may spend in total (see
/// [`send_embed_with_retry`]).
pub(crate) const EMBED_BUDGET_ATTEMPTS: u32 = 3;

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
/// trade here: `OLLAMA_EMBED` is 30 s, and the case that needs a second attempt
/// is the first embed of an indexing run, where Ollama is COLD-LOADING the
/// embedding model and the first request times out while the load completes. A
/// fresh attempt then succeeds immediately; without one, the first document of
/// an indexing run fails for a reason that has already gone away.
///
/// The worst case is unchanged from before that collapse and it is bounded — and
/// the bound is the WHOLE sequence, not just its request time: `budget` below is
/// wall clock measured from the first attempt, and `send_with_retry_capped` pays
/// each backoff OUT of it (projecting `spent` past the sleep before deciding
/// whether another attempt still fits). So one call costs at most
/// `per_attempt` × [`EMBED_BUDGET_ATTEMPTS`] in total, backoff included — never
/// that plus backoff. Indexing has no run-level
/// deadline that counts it — but the autopilot re-rank phase DOES: this budget
/// (`per_attempt` × [`EMBED_BUDGET_ATTEMPTS`]) is what one degraded job costs
/// there, and `RERANK_DEGRADE_BREAKER` of them has to fit inside
/// `RERANK_STEP_TIMEOUT` or the breaker never fires. `rerank.rs` asserts that
/// at compile time; do not widen `OLLAMA_EMBED` or this constant without
/// reading it.
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

#[cfg(test)]
mod tests;

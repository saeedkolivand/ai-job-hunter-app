/// Shared rate-limiter primitives — used by the LinkedIn client (via re-export
/// in `scraping/linkedin/rate_limiter`) and by the generic HTTP fetch path.
///
/// Each struct is identical to what was previously in `linkedin/rate_limiter/mod.rs`
/// so that module keeps working with a single `use super::...` import swap.
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

#[derive(Clone)]
pub struct RateLimiterOptions {
    pub max_requests: usize,
    pub window_ms: u64,
    pub max_retries: usize,
    pub initial_delay: u64,
    pub max_delay: u64,
}

impl Default for RateLimiterOptions {
    fn default() -> Self {
        Self {
            max_requests: 10,
            window_ms: 60000,
            max_retries: 5,
            initial_delay: 1000,
            max_delay: 30000,
        }
    }
}

pub struct RateLimiter {
    requests: Arc<Mutex<Vec<Instant>>>,
    options: RateLimiterOptions,
}

impl RateLimiter {
    pub fn new(options: RateLimiterOptions) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            options,
        }
    }

    /// Wait if necessary to respect rate limits.
    ///
    /// Re-validates the window under the lock after each sleep so that multiple
    /// concurrent waiters (thundering herd) cannot all pass simultaneously.
    ///
    /// Timestamps are `Instant`, not wall-clock `SystemTime`. `Instant` is
    /// monotonic — it can never step backwards (NTP correction, DST, a clock
    /// set by the user) — so a stored timestamp can never read as "ahead of
    /// now" the way a `SystemTime` could. That removes the underflow panic
    /// this used to hit (`now - t` on a `u64`) at the root, with no clamping
    /// or saturating arithmetic needed to reason about clock steps.
    pub async fn wait_for_slot(&self) {
        loop {
            let mut requests = self.requests.lock().await;
            let now = Instant::now();
            let window = Duration::from_millis(self.options.window_ms);

            // Remove requests outside the current window.
            requests.retain(|&t| now.saturating_duration_since(t) < window);

            match Self::wait_for_full_window(&requests, now, window, self.options.max_requests) {
                None => return, // slot is free — caller will record after the request
                Some(wait) => {
                    drop(requests);
                    sleep(wait).await;
                    // Loop and re-check under the lock — another waiter may
                    // have filled the slot while we were sleeping.
                }
            }
        }
    }

    /// Given the already-window-filtered `requests` (oldest first — insertion
    /// order matches chronological order because `Instant` is monotonic),
    /// decide whether the caller must wait for a slot, and for how long.
    ///
    /// Returns `None` when a slot is free. The returned wait is always capped
    /// at one `window`: even a request timestamp that somehow reads as ahead
    /// of `now` (not reachable via the normal record path, since every write
    /// goes through this same lock — kept as an explicit invariant here
    /// rather than an assumption) cannot inflate the wait past one window.
    fn wait_for_full_window(
        requests: &[Instant],
        now: Instant,
        window: Duration,
        max_requests: usize,
    ) -> Option<Duration> {
        if requests.len() < max_requests {
            return None;
        }
        let oldest = *requests.first()?;
        Some((oldest + window).saturating_duration_since(now).min(window))
    }

    /// Record a request was made.
    pub async fn record_request(&self) {
        let mut requests = self.requests.lock().await;
        requests.push(Instant::now());
    }

    /// Clear all recorded request timestamps, resetting the window.
    ///
    /// # Panics
    ///
    /// `blocking_lock` panics if called from inside a Tokio async context
    /// (e.g. inside `async fn` or a spawned task). Only call this from a
    /// synchronous context such as a `#[test]` function.
    pub fn reset(&self) {
        let mut requests = self.requests.blocking_lock();
        requests.clear();
    }
}

// ── Host-keyed registry ───────────────────────────────────────────────────────

/// Process-wide per-host rate limiters. Get-or-create via [`for_host`].
static HOST_LIMITERS: std::sync::LazyLock<
    Mutex<std::collections::HashMap<String, Arc<RateLimiter>>>,
> = std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Return the shared rate limiter for `host`, creating one on first use.
pub async fn for_host(host: &str) -> Arc<RateLimiter> {
    let mut map = HOST_LIMITERS.lock().await;
    if let Some(rl) = map.get(host) {
        return rl.clone();
    }
    let rl = Arc::new(RateLimiter::new(options_for_host(host)));
    map.insert(host.to_string(), rl.clone());
    rl
}

/// Rate-limiter options tuned per host.
///
/// Default is generous (30 req / 60 s) so public HTTP boards are not unduly
/// throttled.
///
/// `pub(crate)` so tests can assert the per-host configuration without going
/// through a full HTTP round-trip.
pub(crate) fn options_for_host(_host: &str) -> RateLimiterOptions {
    RateLimiterOptions {
        max_requests: 30,
        window_ms: 60_000,
        ..RateLimiterOptions::default()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// A timestamp AHEAD of `now` must not panic the window sweep.
    ///
    /// This used to be reachable two ways with `SystemTime`: sampling `now`
    /// BEFORE awaiting the lock let a concurrent `record_request` land a
    /// later timestamp while this task waited, and a backward wall-clock
    /// step (NTP) could do it even with correct lock ordering — both hit
    /// `attempt to subtract with overflow`. `Instant` closes off the second
    /// cause structurally; this test proves the sweep also can't panic if,
    /// against that invariant, a stored value is still ahead of `now`.
    #[tokio::test]
    async fn a_future_instant_does_not_underflow_the_window() {
        let limiter = RateLimiter::new(RateLimiterOptions {
            max_requests: 30,
            window_ms: 60_000,
            ..RateLimiterOptions::default()
        });
        let future = Instant::now() + Duration::from_secs(5);
        limiter.requests.lock().await.push(future);

        // Must return (slot free: 1 of 30) rather than panic.
        limiter.wait_for_slot().await;
    }

    /// The MAJOR this fixed: a full window plus one pathological entry ahead
    /// of `now` must not add its skew on top of the wait. Before capping the
    /// result, the formula was `oldest + window - now`, so an `oldest` far
    /// ahead of `now` inflated the wait to `window + skew` instead of at
    /// most one `window` — a live-traffic stall, not just a panic.
    #[test]
    fn full_window_with_a_skewed_entry_waits_at_most_one_window() {
        let now = Instant::now();
        let window = Duration::from_millis(60_000);
        let skewed = now + Duration::from_secs(3_600); // pathological: 1h ahead
        let requests = [skewed];

        let wait = RateLimiter::wait_for_full_window(&requests, now, window, 1)
            .expect("window is full (1 of 1) so a wait must be returned");

        assert!(
            wait <= window,
            "a skewed entry must not push the wait beyond one window, got {wait:?}"
        );
    }

    /// Baseline: normal (non-skewed) gating still works — the first
    /// `max_requests` calls get a slot immediately, and once the window is
    /// full the next caller actually waits for roughly one window rather
    /// than returning immediately or stalling far longer than the window.
    #[tokio::test]
    async fn slots_are_gated_and_freed_after_the_window_elapses() {
        let limiter = RateLimiter::new(RateLimiterOptions {
            max_requests: 2,
            window_ms: 80,
            ..RateLimiterOptions::default()
        });

        for _ in 0..2 {
            let start = Instant::now();
            limiter.wait_for_slot().await;
            limiter.record_request().await;
            assert!(
                start.elapsed() < Duration::from_millis(40),
                "a free slot must not wait"
            );
        }

        let start = Instant::now();
        limiter.wait_for_slot().await;
        let waited = start.elapsed();
        assert!(
            waited >= Duration::from_millis(40),
            "the third request must wait for a slot to free up, waited {waited:?}"
        );
        assert!(
            waited <= Duration::from_millis(500),
            "the wait must stay close to one window, waited {waited:?}"
        );
    }

    /// All hosts get the uniform 30-req/60-s default (per-board overrides were
    /// removed when the anti-bot scraper boards were retired).
    #[test]
    fn options_for_host_uniform_default() {
        for host in &[
            "www.linkedin.com",
            "greenhouse.io",
            "jobs.lever.co",
            "api.ashbyhq.com",
        ] {
            let opts = options_for_host(host);
            assert_eq!(
                opts.max_requests, 30,
                "host '{host}' must have max_requests=30 (uniform default)"
            );
            assert_eq!(
                opts.window_ms, 60_000,
                "host '{host}' must have window_ms=60 000"
            );
        }
    }
}

/// Shared rate-limiter primitives — used by the LinkedIn client (via re-export
/// in `scraping/linkedin/rate_limiter`) and by the generic HTTP fetch path.
///
/// Each struct is identical to what was previously in `linkedin/rate_limiter/mod.rs`
/// so that module keeps working with a single `use super::...` import swap.
use std::sync::Arc;
use std::time::Duration;
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
    requests: Arc<Mutex<Vec<u64>>>,
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
    pub async fn wait_for_slot(&self) {
        loop {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let mut requests = self.requests.lock().await;

            // Remove requests outside the current window.
            requests.retain(|&t| now - t < self.options.window_ms);

            if requests.len() < self.options.max_requests {
                // Slot is free — return; caller will record after the request.
                return;
            }

            // Window is full: compute how long until the oldest slot expires,
            // then release the lock and sleep before re-checking.
            let wait_ms = match requests.first() {
                Some(&oldest) => oldest + self.options.window_ms - now,
                None => return, // empty after retain — should not happen, but be safe
            };
            drop(requests);
            sleep(Duration::from_millis(wait_ms)).await;
            // Loop and re-check under the lock — another waiter may have filled
            // the slot while we were sleeping.
        }
    }

    /// Record a request was made.
    pub async fn record_request(&self) {
        let mut requests = self.requests.lock().await;
        requests.push(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        );
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

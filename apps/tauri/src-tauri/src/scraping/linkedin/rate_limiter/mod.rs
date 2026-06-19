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

    pub fn reset(&self) {
        let mut requests = self.requests.blocking_lock();
        requests.clear();
    }
}

/// Process-wide LinkedIn rate limiter — shared across all concurrent scrapes.
static LINKEDIN_RATE_LIMITER: std::sync::LazyLock<RateLimiter> =
    std::sync::LazyLock::new(|| RateLimiter::new(RateLimiterOptions::default()));

/// Returns a reference to the process-wide LinkedIn rate limiter.
pub fn linkedin_rate_limiter() -> &'static RateLimiter {
    &LINKEDIN_RATE_LIMITER
}

#[cfg(test)]
mod test;

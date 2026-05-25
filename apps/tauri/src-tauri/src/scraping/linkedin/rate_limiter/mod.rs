#![allow(dead_code)]
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
    pub async fn wait_for_slot(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let mut requests = self.requests.lock().await;
        
        // Remove requests outside the current window
        requests.retain(|&t| now - t < self.options.window_ms);

        if requests.len() >= self.options.max_requests {
            if let Some(&oldest) = requests.first() {
                let wait_time = oldest + self.options.window_ms - now;
                if wait_time > 0 {
                    drop(requests);
                    sleep(Duration::from_millis(wait_time)).await;
                }
            }
        }
    }

    /// Record a request was made.
    pub async fn record_request(&self) {
        let mut requests = self.requests.lock().await;
        requests.push(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64);
    }

    pub fn reset(&self) {
        let mut requests = self.requests.blocking_lock();
        requests.clear();
    }
}

/// Global rate limiter instance for LinkedIn requests.
pub fn linkedin_rate_limiter() -> RateLimiter {
    RateLimiter::new(RateLimiterOptions::default())
}

#[cfg(test)]
mod test;

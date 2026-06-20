// Re-export the shared types so existing `super::rate_limiter::RateLimiter` and
// `super::rate_limiter::RateLimiterOptions` paths in linkedin/client/mod.rs keep
// resolving without change.
pub use crate::scraping::rate_limiter::{RateLimiter, RateLimiterOptions};

/// Process-wide LinkedIn rate limiter — shared across all concurrent scrapes.
static LINKEDIN_RATE_LIMITER: std::sync::LazyLock<RateLimiter> =
    std::sync::LazyLock::new(|| RateLimiter::new(RateLimiterOptions::default()));

/// Returns a reference to the process-wide LinkedIn rate limiter.
pub fn linkedin_rate_limiter() -> &'static RateLimiter {
    &LINKEDIN_RATE_LIMITER
}

#[cfg(test)]
mod test;

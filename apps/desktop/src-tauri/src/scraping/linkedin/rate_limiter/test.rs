use super::*;

#[test]
fn test_rate_limiter_options_default() {
    let opts = RateLimiterOptions::default();
    assert_eq!(opts.max_requests, 10);
    assert_eq!(opts.window_ms, 60000);
    assert_eq!(opts.max_retries, 5);
    assert_eq!(opts.initial_delay, 1000);
    assert_eq!(opts.max_delay, 30000);
}

#[test]
fn test_rate_limiter_new() {
    let opts = RateLimiterOptions::default();
    let limiter = RateLimiter::new(opts);
    // Just verify it constructs without panicking
    let _ = limiter;
}

#[test]
fn test_rate_limiter_reset() {
    let opts = RateLimiterOptions::default();
    let limiter = RateLimiter::new(opts);
    limiter.reset();
    // Just verify it doesn't panic
}

#[test]
fn test_linkedin_rate_limiter() {
    let limiter = linkedin_rate_limiter();
    // Just verify it constructs without panicking
    let _ = limiter;
}

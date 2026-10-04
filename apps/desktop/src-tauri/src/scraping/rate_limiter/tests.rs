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

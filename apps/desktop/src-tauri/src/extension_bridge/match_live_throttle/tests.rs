use super::*;

// ── MatchLiveThrottle (per-connection compute-verb throttle) ─────────────

#[test]
fn throttle_allows_a_burst_then_refuses() {
    let mut t = MatchLiveThrottle::new();
    let now = std::time::Instant::now();
    assert!(t.try_acquire_at(now), "1st request in the burst");
    assert!(t.try_acquire_at(now), "2nd request in the burst");
    assert!(t.try_acquire_at(now), "3rd request in the burst");
    assert!(
        !t.try_acquire_at(now),
        "a 4th immediate request must be throttled (burst exhausted)"
    );
}

#[test]
fn throttle_refills_one_token_per_interval_not_a_second_burst() {
    let mut t = MatchLiveThrottle::new();
    let t0 = std::time::Instant::now();
    for _ in 0..3 {
        assert!(t.try_acquire_at(t0));
    }
    assert!(!t.try_acquire_at(t0), "bucket is empty");

    let t1 = t0 + std::time::Duration::from_secs_f64(MATCH_LIVE_REFILL_SECS);
    assert!(
        t.try_acquire_at(t1),
        "exactly one interval later, one token must have refilled"
    );
    assert!(
        !t.try_acquire_at(t1),
        "only ONE token refilled — this must not re-open the full burst"
    );
}

#[test]
fn throttle_state_is_isolated_per_instance() {
    // Two throttle INSTANCES never share tokens — this is what would
    // guarantee a future distinct-throttle verb stays unaffected by this
    // one's exhaustion. In production there is exactly ONE instance
    // (owned by `BridgeState`, shared across every connection for a
    // pairing — see this struct's doc); this test only pins that the
    // struct itself carries no hidden global state.
    let mut a = MatchLiveThrottle::new();
    let mut b = MatchLiveThrottle::new();
    let now = std::time::Instant::now();
    for _ in 0..3 {
        assert!(a.try_acquire_at(now));
    }
    assert!(!a.try_acquire_at(now), "a is exhausted");
    assert!(
        b.try_acquire_at(now),
        "b must be entirely unaffected by a's exhaustion — no shared state"
    );
}

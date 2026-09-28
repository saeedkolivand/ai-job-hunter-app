use super::*;

#[test]
fn throttle_allows_a_burst_then_refuses() {
    let mut t = DocumentExportThrottle::new();
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
    let mut t = DocumentExportThrottle::new();
    let t0 = std::time::Instant::now();
    for _ in 0..3 {
        assert!(t.try_acquire_at(t0));
    }
    assert!(!t.try_acquire_at(t0), "bucket is empty");
    assert!(
        t.retry_after_ms() > 0,
        "an exhausted bucket must report a positive wait"
    );

    let t1 = t0 + std::time::Duration::from_secs_f64(DOCUMENT_EXPORT_REFILL_SECS);
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
    let mut a = DocumentExportThrottle::new();
    let mut b = DocumentExportThrottle::new();
    let now = std::time::Instant::now();
    for _ in 0..3 {
        assert!(a.try_acquire_at(now));
    }
    assert!(!a.try_acquire_at(now));
    assert!(
        b.try_acquire_at(now),
        "a second instance's bucket is untouched by the first's"
    );
}

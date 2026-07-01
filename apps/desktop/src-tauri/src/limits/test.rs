//! Unit tests for the in-memory anti-abuse limiter: window rollover, RAII
//! concurrency acquire/release, and the per-provider daily-ceiling trip.

use std::sync::Arc;
use std::time::Duration;

use super::*;

const CMD: &str = "test.cmd";

#[test]
fn rate_window_rejects_over_cap_then_recovers_after_rollover() {
    let limiter = Arc::new(Limiter::new());
    let base = std::time::Instant::now();
    // High concurrency so only the RATE cap is under test here.
    let max_requests = 3;
    let max_concurrent = 100;

    // Fill the window at `base`. Drop each guard immediately so concurrency
    // never gates — we are exercising the sliding-window count only.
    for _ in 0..max_requests {
        let g = limiter
            .acquire_at(CMD, max_requests, max_concurrent, base)
            .expect("within rate cap");
        drop(g);
    }

    // The next call in the SAME window is rejected by the rate cap.
    let over = limiter.acquire_at(CMD, max_requests, max_concurrent, base);
    assert!(
        over.is_err(),
        "request over the window cap must be rejected"
    );
    let err = over.err().unwrap();
    assert_eq!(err.code(), "RATE_LIMITED");
    assert!(err.retriable(), "rate-limit errors must be retriable");

    // A rejected call must NOT consume a window slot: still exactly `max_requests`
    // recorded, so one rollover frees the whole window.
    let after_window = base + RATE_WINDOW + Duration::from_secs(1);
    for _ in 0..max_requests {
        let g = limiter
            .acquire_at(CMD, max_requests, max_concurrent, after_window)
            .expect("window rolled over → cap available again");
        drop(g);
    }
}

#[test]
fn concurrency_guard_releases_slot_on_drop() {
    let limiter = Arc::new(Limiter::new());
    let now = std::time::Instant::now();
    // High rate cap so only CONCURRENCY is under test.
    let max_requests = 1000;
    let max_concurrent = 2;

    let g1 = limiter
        .acquire_at(CMD, max_requests, max_concurrent, now)
        .expect("1st slot");
    let g2 = limiter
        .acquire_at(CMD, max_requests, max_concurrent, now)
        .expect("2nd slot");

    // Third concurrent acquire exceeds the concurrency cap.
    assert!(
        limiter
            .acquire_at(CMD, max_requests, max_concurrent, now)
            .is_err(),
        "third concurrent call must exceed the concurrency cap"
    );

    // Releasing one guard (RAII drop) frees exactly one slot.
    drop(g1);
    let g3 = limiter
        .acquire_at(CMD, max_requests, max_concurrent, now)
        .expect("a freed slot is reusable");

    // Two are held again → next is rejected.
    assert!(
        limiter
            .acquire_at(CMD, max_requests, max_concurrent, now)
            .is_err(),
        "two slots held again → rejected"
    );

    drop(g2);
    drop(g3);

    // All released → full concurrency available again.
    let _g = limiter
        .acquire_at(CMD, max_requests, max_concurrent, now)
        .expect("all slots released → acquire succeeds");
}

#[test]
fn provider_daily_ceiling_trips_at_the_cap() {
    let limiter = Arc::new(Limiter::new());
    let max_per_day = 3;

    for i in 0..max_per_day {
        limiter
            .charge_provider_daily("openai", max_per_day)
            .unwrap_or_else(|_| panic!("charge {i} within ceiling"));
    }

    // The (max+1)-th charge for the SAME provider trips.
    let over = limiter.charge_provider_daily("openai", max_per_day);
    assert!(over.is_err(), "daily ceiling must trip at the cap");
    let err = over.err().unwrap();
    assert_eq!(err.code(), "RATE_LIMITED");
    assert!(err.retriable());

    // A DIFFERENT provider has its own independent budget.
    limiter
        .charge_provider_daily("anthropic", max_per_day)
        .expect("separate provider has its own daily budget");
}

#[test]
fn rate_and_concurrency_caps_are_per_command_independent() {
    let limiter = Arc::new(Limiter::new());
    let now = std::time::Instant::now();

    // Saturate command A's single concurrency slot.
    let _a = limiter.acquire_at("cmd.a", 100, 1, now).expect("A slot");
    assert!(
        limiter.acquire_at("cmd.a", 100, 1, now).is_err(),
        "A is saturated"
    );

    // Command B is unaffected — distinct key.
    let _b = limiter
        .acquire_at("cmd.b", 100, 1, now)
        .expect("B has its own independent slot");
}

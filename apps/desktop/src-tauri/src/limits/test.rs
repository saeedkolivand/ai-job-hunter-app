//! Unit tests for the in-memory anti-abuse limiter: window rollover, RAII
//! concurrency acquire/release, and the per-provider daily-ceiling trip.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

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

// ── acquire_queued: park instead of reject ───────────────────────────────────
//
// The behaviour these pin is why `generate_pipeline` uses this path: a user
// working through imported applications clicks Generate several times in a row,
// and throwing the 4th click away is worse than making it wait.

#[tokio::test]
async fn queued_acquire_parks_the_over_cap_caller_until_a_slot_frees() {
    let limiter = Arc::new(Limiter::new());
    let max_requests = 1000;
    let max_concurrent = 1;
    let max_queued = 10;

    let (first, parked) = limiter
        .acquire_queued(CMD, max_requests, max_concurrent, max_queued, |_| {
            panic!("an empty gate must not park")
        })
        .await
        .expect("1st slot is free");
    assert!(!parked, "the first caller starts immediately");

    // The second caller must PARK, not fail — the whole point of this path.
    let ahead = Arc::new(Mutex::new(None));
    let seen = Arc::clone(&ahead);
    let limiter2 = Arc::clone(&limiter);
    let waiter = tokio::spawn(async move {
        limiter2
            .acquire_queued(CMD, max_requests, max_concurrent, max_queued, |n| {
                *seen.lock() = Some(n)
            })
            .await
    });

    // It is still parked: it cannot have completed while the only slot is held.
    tokio::task::yield_now().await;
    assert!(!waiter.is_finished(), "second caller must still be waiting");
    assert_eq!(
        *ahead.lock(),
        Some(0),
        "on_park reports 0 callers ahead of it, and fires BEFORE the wait"
    );

    // Releasing the first guard hands the permit to the waiter.
    drop(first);
    let (_second, parked) = waiter
        .await
        .expect("waiter task")
        .expect("a freed slot wakes the parked caller");
    assert!(parked, "the second caller reports that it waited");
}

#[tokio::test]
async fn queued_acquire_rejects_past_the_queue_ceiling() {
    // The queue must not become the unbounded resource: a looping renderer that
    // can't be rejected on concurrency has to be rejected on depth.
    let limiter = Arc::new(Limiter::new());
    let max_queued = 2;

    let _held = limiter
        .acquire_queued(CMD, 1000, 1, max_queued, |_| ())
        .await
        .expect("1st slot");

    let mut waiters = Vec::new();
    for _ in 0..max_queued {
        let l = Arc::clone(&limiter);
        waiters.push(tokio::spawn(async move {
            l.acquire_queued(CMD, 1000, 1, max_queued, |_| ()).await
        }));
    }
    // Let both waiters park before testing the ceiling.
    while limiter.queue_depth(CMD) < max_queued {
        tokio::task::yield_now().await;
    }

    let over = limiter.acquire_queued(CMD, 1000, 1, max_queued, |_| ()).await;
    assert!(
        over.is_err(),
        "a caller arriving at a full queue must be rejected, not parked"
    );
    assert_eq!(over.err().unwrap().code(), "RATE_LIMITED");

    for w in waiters {
        w.abort();
    }
}

#[tokio::test]
async fn dropping_a_parked_future_frees_its_queue_slot() {
    // `.await` is a cancellation point: a user who closes the window or cancels
    // while queued must not leave the counter high forever, or the queue wedges
    // shut for the rest of the session.
    let limiter = Arc::new(Limiter::new());
    let _held = limiter
        .acquire_queued(CMD, 1000, 1, 4, |_| ())
        .await
        .expect("1st slot");

    {
        let fut = limiter.acquire_queued(CMD, 1000, 1, 4, |_| ());
        tokio::pin!(fut);
        // Poll once so it registers in the queue, then drop it unfinished.
        tokio::select! {
            biased;
            _ = &mut fut => panic!("cannot complete while the only slot is held"),
            _ = tokio::task::yield_now() => {}
        }
        assert_eq!(limiter.queue_depth(CMD), 1, "the parked caller is counted");
    }

    assert_eq!(
        limiter.queue_depth(CMD),
        0,
        "dropping the future released its queue slot"
    );
}

#[tokio::test]
async fn queued_and_rejecting_acquires_share_one_concurrency_budget() {
    // `ai_generate` (rejecting) and `generate_pipeline` (queueing) name the same
    // command key on purpose. Two independent mechanisms would mean two budgets
    // — which is exactly the bug that let 7 streams run against a cap of 3.
    let limiter = Arc::new(Limiter::new());

    let _queued_holder = limiter
        .acquire_queued(CMD, 1000, 1, 4, |_| ())
        .await
        .expect("queued path takes the only slot");

    assert!(
        limiter.acquire(CMD, 1000, 1).is_err(),
        "the rejecting path must see the slot the queueing path took"
    );
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

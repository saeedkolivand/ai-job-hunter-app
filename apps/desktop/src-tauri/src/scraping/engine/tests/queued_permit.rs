//! The permit wait: parks are reported before the await, granted after, the
//! waiter counter is exact, and a Stop while parked ends the wait.

use std::sync::Mutex;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::super::*;
use super::support::*;

fn recorder() -> (QueueHook, Arc<Mutex<Vec<QueueEvent>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = log.clone();
    let hook: QueueHook = Arc::new(move |e| sink.lock().unwrap().push(e));
    (hook, log)
}

async fn until(cond: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !cond() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("condition not reached in time");
}

fn one_permit_engine() -> Arc<ScraperEngine> {
    let engine = Arc::new(ScraperEngine::new());
    engine.set_concurrency(1);
    engine
}

#[tokio::test]
async fn free_permit_never_fires_the_hook() {
    let engine = one_permit_engine();
    let (hook, log) = recorder();
    let _p = engine
        .acquire_permit(&CancellationToken::new(), Some(&hook))
        .await
        .unwrap();
    assert!(log.lock().unwrap().is_empty());
    assert_eq!(engine.waiting_count(), 0);
}

#[tokio::test]
async fn second_caller_parks_then_is_granted_and_counter_returns_to_zero() {
    let engine = one_permit_engine();
    let first = engine
        .acquire_permit(&CancellationToken::new(), None)
        .await
        .unwrap();
    let (hook, log) = recorder();
    let e = engine.clone();
    let waiter = tokio::spawn(async move {
        e.acquire_permit(&CancellationToken::new(), Some(&hook))
            .await
            .map(|_| ())
    });

    until(|| engine.waiting_count() == 1).await;
    assert_eq!(*log.lock().unwrap(), vec![QueueEvent::Parked { ahead: 0 }]);

    drop(first);
    waiter.await.unwrap().unwrap();
    assert_eq!(
        *log.lock().unwrap(),
        vec![QueueEvent::Parked { ahead: 0 }, QueueEvent::Granted]
    );
    assert_eq!(engine.waiting_count(), 0);
}

#[tokio::test]
async fn ahead_counts_callers_already_waiting() {
    let engine = one_permit_engine();
    let _first = engine
        .acquire_permit(&CancellationToken::new(), None)
        .await
        .unwrap();
    let (hook, log) = recorder();
    let mut waiters = Vec::new();
    for n in 1..=2 {
        let (e, h) = (engine.clone(), hook.clone());
        waiters.push(tokio::spawn(async move {
            e.acquire_permit(&CancellationToken::new(), Some(&h))
                .await
                .map(|_| ())
        }));
        until(|| engine.waiting_count() == n).await;
    }
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            QueueEvent::Parked { ahead: 0 },
            QueueEvent::Parked { ahead: 1 }
        ]
    );
    for w in waiters {
        w.abort();
    }
}

#[tokio::test]
async fn dropped_waiter_decrements_the_counter() {
    let engine = one_permit_engine();
    let _first = engine
        .acquire_permit(&CancellationToken::new(), None)
        .await
        .unwrap();
    let e = engine.clone();
    let waiter = tokio::spawn(async move {
        e.acquire_permit(&CancellationToken::new(), None)
            .await
            .map(|_| ())
    });
    until(|| engine.waiting_count() == 1).await;
    waiter.abort();
    let _ = waiter.await;
    assert_eq!(engine.waiting_count(), 0);
}

#[tokio::test]
async fn stop_while_parked_ends_the_wait_without_granting() {
    let engine = one_permit_engine();
    let _first = engine
        .acquire_permit(&CancellationToken::new(), None)
        .await
        .unwrap();
    let (hook, log) = recorder();
    let token = CancellationToken::new();
    let (e, t) = (engine.clone(), token.clone());
    let waiter = tokio::spawn(async move { e.acquire_permit(&t, Some(&hook)).await.map(|_| ()) });
    until(|| engine.waiting_count() == 1).await;

    token.cancel();
    let err = waiter.await.unwrap().unwrap_err();
    assert!(err.to_string().contains("cancelled"), "{err}");
    assert_eq!(*log.lock().unwrap(), vec![QueueEvent::Parked { ahead: 0 }]);
    assert_eq!(engine.waiting_count(), 0);
}

/// End to end through `scrape_boards`: a Stop on a pre-registered token while
/// the engine is saturated returns Err (the job ends cancelled, not hung).
#[tokio::test]
async fn scrape_boards_cancelled_while_queued_returns_err() {
    let engine = one_permit_engine();
    let _held = engine
        .acquire_permit(&CancellationToken::new(), None)
        .await
        .unwrap();
    engine
        .register_token("job-queued", CancellationToken::new())
        .await;
    let (hook, log) = recorder();

    let e = engine.clone();
    let run = tokio::spawn(async move {
        e.scrape_boards(
            &["fake".to_string()],
            fake_input(1),
            "job-queued".to_string(),
            None,
            None,
            Some(hook),
        )
        .await
        .map(|_| ())
    });
    until(|| engine.waiting_count() == 1).await;
    engine.cancel("job-queued").await;

    let err = tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .expect("a Stop while queued must not hang")
        .unwrap()
        .unwrap_err();
    assert!(err.to_string().contains("cancelled"), "{err}");
    assert_eq!(*log.lock().unwrap(), vec![QueueEvent::Parked { ahead: 0 }]);
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::time::{sleep, Instant};

use super::*;

/// Sets a flag when dropped: proof the lookup future was dropped, not parked.
struct DropFlag(Arc<AtomicBool>);
impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// The lookup future a run drives: waits for the role, then runs a fake
/// provider call of `latency` that records when it started.
fn lookup(
    role_rx: oneshot::Receiver<String>,
    brief_tx: oneshot::Sender<String>,
    started: Arc<parking_lot::Mutex<Option<Instant>>>,
    latency: Duration,
    answer: &'static str,
) -> impl Future<Output = ()> {
    drive(role_rx, brief_tx, move |role| async move {
        *started.lock() = Some(Instant::now());
        sleep(latency).await;
        format!("{answer}:{role}")
    })
}

/// The pipeline half of the fake: analyze (publishes the role), a strategy +
/// draft stretch of `middle`, then the letter stage awaiting the brief.
async fn pipeline(mut early: EarlyResearch, middle: Duration) -> (String, Instant) {
    early.publish_role("Engineer");
    sleep(middle).await;
    let middle_ended = Instant::now();
    let brief = early.take_brief().expect("armed").await.unwrap_or_default();
    (brief, middle_ended)
}

/// Mutation check: make `race_background` poll `background` only after `main`
/// (or run `research` inline in `pipeline`) and the "starts before strategy
/// ends" and "no extra wait" asserts fail.
#[tokio::test(start_paused = true)]
async fn research_starts_before_strategy_ends_and_adds_no_wait() {
    let (early, role_rx, brief_tx) = EarlyResearch::channel();
    let started = Arc::new(parking_lot::Mutex::new(None));
    let t0 = Instant::now();
    let bg = lookup(
        role_rx,
        brief_tx,
        started.clone(),
        Duration::from_millis(50),
        "brief",
    );

    let (brief, middle_ended) =
        race_background(pipeline(early, Duration::from_millis(100)), Some(bg)).await;

    let research_started = started.lock().expect("research ran");
    assert!(
        research_started < middle_ended,
        "research must overlap strategy"
    );
    assert_eq!(brief, "brief:Engineer");
    // Serial would be 100 + 50; overlapped is the longer of the two.
    assert_eq!(t0.elapsed(), Duration::from_millis(100));
}

#[tokio::test(start_paused = true)]
async fn a_slow_lookup_costs_only_its_remainder() {
    let (early, role_rx, brief_tx) = EarlyResearch::channel();
    let started = Arc::new(parking_lot::Mutex::new(None));
    let t0 = Instant::now();
    let bg = lookup(role_rx, brief_tx, started, Duration::from_millis(300), "b");

    let (brief, _) = race_background(pipeline(early, Duration::from_millis(100)), Some(bg)).await;

    assert_eq!(brief, "b:Engineer");
    assert_eq!(t0.elapsed(), Duration::from_millis(300));
}

/// A failed lookup yields `""`, and so does a lookup dropped before it answers
/// (analyze failed, so the role never arrived): the letter proceeds, no brief.
#[tokio::test(start_paused = true)]
async fn a_failed_or_dropped_lookup_reads_as_no_brief() {
    let (early, role_rx, brief_tx) = EarlyResearch::channel();
    let bg = drive(role_rx, brief_tx, |_| async { String::new() });
    let (brief, _) = race_background(pipeline(early, Duration::ZERO), Some(bg)).await;
    assert_eq!(brief, "");

    // Analyze failed: the ctx (and its role sender) is dropped unpublished.
    let (mut early, role_rx, brief_tx) = EarlyResearch::channel();
    let rx = early.take_brief().expect("armed");
    drop(early);
    drive(role_rx, brief_tx, |_| async { "never".to_string() }).await;
    assert_eq!(rx.await.unwrap_or_default(), "");
}

/// The pipeline finishing (error, success or cancel) drops a lookup that is
/// still waiting on the provider.
#[tokio::test(start_paused = true)]
async fn the_lookup_is_dropped_when_the_pipeline_ends() {
    let dropped = Arc::new(AtomicBool::new(false));
    let (mut early, role_rx, brief_tx) = EarlyResearch::channel();
    let flag = dropped.clone();
    let bg = drive(role_rx, brief_tx, move |_| async move {
        let _guard = DropFlag(flag);
        sleep(Duration::from_secs(90)).await;
        "late".to_string()
    });
    let main = async {
        early.publish_role("Engineer");
        sleep(Duration::from_millis(10)).await;
        "run ended early"
    };

    assert_eq!(race_background(main, Some(bg)).await, "run ended early");
    assert!(
        dropped.load(Ordering::SeqCst),
        "research must not be orphaned"
    );
}

#[tokio::test(start_paused = true)]
async fn cancel_drops_the_provider_call_and_yields_no_brief() {
    let dropped = Arc::new(AtomicBool::new(false));
    let flag = dropped.clone();
    let cancel = CancellationToken::new();
    let trip = cancel.clone();
    let slow = async move {
        let _guard = DropFlag(flag);
        sleep(Duration::from_secs(90)).await;
        "late".to_string()
    };
    tokio::spawn(async move {
        sleep(Duration::from_millis(5)).await;
        trip.cancel();
    });

    assert_eq!(cancellable(&cancel, slow).await, "");
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn only_a_letter_run_that_asked_for_research_arms_it() {
    assert!(should_research(true, true));
    assert!(!should_research(false, true), "no letter, no research");
    assert!(!should_research(true, false));
    assert!(!should_research(false, false));
}

#[tokio::test(start_paused = true)]
async fn an_unarmed_run_is_just_the_pipeline() {
    let none: Option<std::future::Pending<()>> = None;
    assert_eq!(race_background(async { 7 }, none).await, 7);
}

/// A role that is never published (analyze skipped or reordered) must fail
/// soft: taking the brief drops the role sender, so the lookup ends and the
/// brief reads empty instead of hanging. The timeout turns a regression into a
/// failure rather than a hang.
///
/// Mutation check: remove `self.role_tx.take();` from `take_brief`.
#[tokio::test(start_paused = true)]
async fn an_unpublished_role_fails_soft_instead_of_hanging() {
    let (mut early, role_rx, brief_tx) = EarlyResearch::channel();
    let bg = drive(role_rx, brief_tx, |_| async { "never".to_string() });
    let rx = early.take_brief().expect("armed");

    let brief = tokio::time::timeout(Duration::from_secs(5), async {
        let (_, brief) = tokio::join!(bg, rx);
        brief.unwrap_or_default()
    })
    .await
    .expect("must not hang");
    assert_eq!(brief, "");
}

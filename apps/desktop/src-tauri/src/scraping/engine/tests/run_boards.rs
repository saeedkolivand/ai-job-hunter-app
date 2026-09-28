//! `ScraperEngine::run_boards` fan-out tests: input-order preservation,
//! concurrency, and per-board callback wiring.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

// ── run_boards tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn run_boards_collects_all_boards_up_to_amount() {
    // Three boards each producing 50 items; amount=10 caps each to 10.
    let a = FakeScraper::http(50);
    let b = FakeScraper::http(50);
    let c = FakeScraper::http(50);

    let resolved: Vec<(String, anyhow::Result<&dyn Scraper>)> = vec![
        ("a".into(), Ok(&a as &dyn Scraper)),
        ("b".into(), Ok(&b as &dyn Scraper)),
        ("c".into(), Ok(&c as &dyn Scraper)),
    ];

    let received = Arc::new(AtomicUsize::new(0));
    let received_cb = received.clone();
    let on_item: Arc<dyn Fn(JobPosting) + Send + Sync> = Arc::new(move |_| {
        received_cb.fetch_add(1, Ordering::SeqCst);
    });

    let parent = CancellationToken::new();
    let results = ScraperEngine::run_boards(
        resolved,
        fake_input(10),
        parent,
        None,
        Some(on_item),
        None,
        None,
        None,
        test_browser_sem(),
        &HashMap::new(),
    )
    .await;

    assert_eq!(results.len(), 3, "one result per board");
    for (_, res) in &results {
        let items = res.as_ref().expect("board should succeed");
        assert_eq!(items.len(), 10, "each board capped to amount=10");
    }
    assert_eq!(
        received.load(Ordering::SeqCst),
        30,
        "30 items streamed total (3 × 10)"
    );
}

#[tokio::test]
async fn run_boards_one_error_does_not_kill_others() {
    let good_a = FakeScraper::http(5);
    let failing = FailingScraper;
    let good_b = FakeScraper::http(5);

    let resolved: Vec<(String, anyhow::Result<&dyn Scraper>)> = vec![
        ("good_a".into(), Ok(&good_a as &dyn Scraper)),
        ("fail".into(), Ok(&failing as &dyn Scraper)),
        ("good_b".into(), Ok(&good_b as &dyn Scraper)),
    ];

    let parent = CancellationToken::new();
    let results = ScraperEngine::run_boards(
        resolved,
        fake_input(10),
        parent,
        None,
        None,
        None,
        None,
        None,
        test_browser_sem(),
        &HashMap::new(),
    )
    .await;

    assert_eq!(results.len(), 3);
    let map: HashMap<String, _> = results.into_iter().collect();

    assert!(map["good_a"].is_ok(), "good_a should succeed");
    assert!(map["fail"].is_err(), "fail should produce an error");
    assert!(map["good_b"].is_ok(), "good_b should succeed");

    // The error board's summary would carry an error message.
    assert!(map["fail"]
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains("board error"));
}

#[tokio::test]
async fn run_boards_parent_cancel_stops_all() {
    // Boards that produce 1000 items — the parent cancel should cut them short.
    let a = FakeScraper::http(1000);
    let b = FakeScraper::http(1000);

    let resolved: Vec<(String, anyhow::Result<&dyn Scraper>)> = vec![
        ("a".into(), Ok(&a as &dyn Scraper)),
        ("b".into(), Ok(&b as &dyn Scraper)),
    ];

    let parent = CancellationToken::new();
    parent.cancel(); // cancel before boards start

    let results = ScraperEngine::run_boards(
        resolved,
        fake_input(1000),
        parent,
        None,
        None,
        None,
        None,
        None,
        test_browser_sem(),
        &HashMap::new(),
    )
    .await;

    // Both boards observed the cancelled signal immediately and streamed 0 items.
    for (_, res) in &results {
        let items = res.as_ref().expect("cancelled scrape returns Ok([])");
        assert_eq!(items.len(), 0, "cancelled board streams nothing");
    }
}

#[tokio::test]
async fn run_boards_child_cap_stops_only_its_board() {
    // Board A capped to 3; Board B produces 20 and should complete fully.
    let a = FakeScraper::http(20);
    let b = FakeScraper::http(20);

    let resolved: Vec<(String, anyhow::Result<&dyn Scraper>)> = vec![
        ("a".into(), Ok(&a as &dyn Scraper)),
        ("b".into(), Ok(&b as &dyn Scraper)),
    ];

    let parent = CancellationToken::new();
    let results = ScraperEngine::run_boards(
        resolved,
        fake_input(3),
        parent.clone(),
        None,
        None,
        None,
        None,
        None,
        test_browser_sem(),
        &HashMap::new(),
    )
    .await;

    assert!(
        !parent.is_cancelled(),
        "child cap must not cancel parent token"
    );

    let map: HashMap<String, _> = results.into_iter().collect();
    assert_eq!(map["a"].as_ref().unwrap().len(), 3, "board a capped to 3");
    assert_eq!(
        map["b"].as_ref().unwrap().len(),
        3,
        "board b also capped to 3 (same amount applied per-board)"
    );
}

#[tokio::test]
async fn run_boards_browser_board_collects_items() {
    // Verify that a browser-mode FakeScraper collects items correctly —
    // the browser-semaphore path must not swallow results.
    let browser_a = FakeScraper::browser(5);
    let http_b = FakeScraper::http(5);

    let resolved: Vec<(String, anyhow::Result<&dyn Scraper>)> = vec![
        ("browser_a".into(), Ok(&browser_a as &dyn Scraper)),
        ("http_b".into(), Ok(&http_b as &dyn Scraper)),
    ];

    let parent = CancellationToken::new();
    let results = ScraperEngine::run_boards(
        resolved,
        fake_input(10),
        parent,
        None,
        None,
        None,
        None,
        None,
        test_browser_sem(),
        &HashMap::new(),
    )
    .await;

    assert_eq!(results.len(), 2, "one result per board");
    let map: HashMap<String, _> = results.into_iter().collect();
    assert_eq!(
        map["browser_a"].as_ref().unwrap().len(),
        5,
        "browser-mode board returns all items when under the cap"
    );
    assert_eq!(
        map["http_b"].as_ref().unwrap().len(),
        5,
        "http-mode board returns all items when under the cap"
    );
}

#[tokio::test]
async fn run_boards_browser_boards_serialized() {
    // Two browser-mode boards. We track peak concurrency with a shared atomic
    // counter incremented on entry and decremented on exit of `search`.
    use std::sync::atomic::{AtomicI32, Ordering as Ord};
    use std::time::Duration;

    struct ConcurrencyProbeScraper {
        active: Arc<AtomicI32>,
        peak: Arc<AtomicI32>,
    }

    #[async_trait::async_trait]
    impl Scraper for ConcurrencyProbeScraper {
        fn id(&self) -> &'static str {
            "probe"
        }
        fn display_name(&self) -> &'static str {
            "Probe"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Browser
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            let now = self.active.fetch_add(1, Ord::SeqCst) + 1;
            // Update peak.
            let mut cur_peak = self.peak.load(Ord::SeqCst);
            while now > cur_peak {
                match self
                    .peak
                    .compare_exchange(cur_peak, now, Ord::SeqCst, Ord::SeqCst)
                {
                    Ok(_) => break,
                    Err(p) => cur_peak = p,
                }
            }
            // Simulate work — enough for the concurrent board to try to start.
            tokio::time::sleep(Duration::from_millis(20)).await;
            self.active.fetch_sub(1, Ord::SeqCst);
            Ok(vec![])
        }
    }

    let active = Arc::new(AtomicI32::new(0));
    let peak = Arc::new(AtomicI32::new(0));
    let p1 = ConcurrencyProbeScraper {
        active: active.clone(),
        peak: peak.clone(),
    };
    let p2 = ConcurrencyProbeScraper {
        active: active.clone(),
        peak: peak.clone(),
    };

    let resolved: Vec<(String, anyhow::Result<&dyn Scraper>)> = vec![
        ("p1".into(), Ok(&p1 as &dyn Scraper)),
        ("p2".into(), Ok(&p2 as &dyn Scraper)),
    ];

    let parent = CancellationToken::new();
    ScraperEngine::run_boards(
        resolved,
        fake_input(5),
        parent,
        None,
        None,
        None,
        None,
        None,
        test_browser_sem(),
        &HashMap::new(),
    )
    .await;

    assert_eq!(
        peak.load(Ordering::SeqCst),
        1,
        "browser boards must run one at a time (peak concurrency must be 1)"
    );
}

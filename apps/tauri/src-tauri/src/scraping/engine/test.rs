use super::*;

#[test]
fn test_catalog() {
    let engine = ScraperEngine::new();
    let catalog = engine.catalog();
    assert_eq!(catalog.len(), 20);

    // Check specific scrapers
    assert!(catalog.iter().any(|s| s.id == "linkedin"));
    assert!(catalog.iter().any(|s| s.id == "indeed"));
    assert!(catalog.iter().any(|s| s.id == "ycombinator"));
}

#[test]
fn test_catalog_auth_tiers() {
    use crate::scraping::types::AuthRequirement;

    let engine = ScraperEngine::new();
    let catalog = engine.catalog();

    let entry = |id: &str| {
        catalog
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing board: {id}"))
    };

    // Required login — guest returns ~nothing
    assert_eq!(
        entry("indeed").auth,
        AuthRequirement::Required,
        "indeed must be Required"
    );
    assert_eq!(
        entry("xing").auth,
        AuthRequirement::Required,
        "xing must be Required"
    );

    // Optional — guest works; login enriches
    assert_eq!(
        entry("linkedin").auth,
        AuthRequirement::Optional,
        "linkedin must be Optional"
    );

    // Guest default — no override needed
    assert_eq!(
        entry("greenhouse").auth,
        AuthRequirement::Guest,
        "greenhouse must be Guest (default)"
    );
    assert_eq!(
        entry("ycombinator").auth,
        AuthRequirement::Guest,
        "ycombinator must be Guest (default)"
    );
}

#[test]
fn test_catalog_listed_flags() {
    let engine = ScraperEngine::new();
    let catalog = engine.catalog();

    let entry = |id: &str| {
        catalog
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing board: {id}"))
    };

    // glassdoor is registered but hidden from the picker
    assert!(!entry("glassdoor").listed, "glassdoor must not be listed");

    // A representative guest board is listed
    assert!(entry("greenhouse").listed, "greenhouse must be listed");
    assert!(entry("linkedin").listed, "linkedin must be listed");
    assert!(entry("indeed").listed, "indeed must be listed");

    // 19 of the 20 boards are listed (glassdoor is the only hidden one)
    let listed_count = catalog.iter().filter(|e| e.listed).count();
    assert_eq!(listed_count, 19, "exactly 19 boards should be listed");
}

#[test]
fn test_health() {
    let engine = ScraperEngine::new();
    let health = engine.health();
    assert_eq!(health.mode, "in-process");
    assert!(health.ready);
    assert_eq!(health.scrapers.len(), 20);
}

#[test]
fn test_set_concurrency() {
    let engine = ScraperEngine::new();
    engine.set_concurrency(1); // low-memory tier
    engine.set_concurrency(2); // balanced
    engine.set_concurrency(4); // performance
    engine.set_concurrency(0); // clamps to >= 1, must not panic
}

#[tokio::test]
async fn test_token_registration() {
    let engine = ScraperEngine::new();
    let token = tokio_util::sync::CancellationToken::new();

    engine.register_token("job-1", token.clone()).await;
    engine.unregister_token("job-1").await;
}

#[tokio::test]
async fn test_cancel_nonexistent_job() {
    let engine = ScraperEngine::new();
    // Should not panic for nonexistent job
    engine.cancel("nonexistent").await;
}

// ── Fake scrapers ─────────────────────────────────────────────────────────────

/// Fake board that streams `count` items through `ctx.on_item`, stopping early
/// when `ctx.signal` is cancelled (mimicking a real pagination loop), and returns
/// the same Vec it streamed. Used to drive the engine's central `amount` cap.
struct FakeScraper {
    count: usize,
    /// Overrides the scraper mode so we can fake a browser board.
    mode: ScraperMode,
}

impl FakeScraper {
    fn http(count: usize) -> Self {
        Self {
            count,
            mode: ScraperMode::Http,
        }
    }

    fn browser(count: usize) -> Self {
        Self {
            count,
            mode: ScraperMode::Browser,
        }
    }
}

#[async_trait::async_trait]
impl super::super::types::Scraper for FakeScraper {
    fn id(&self) -> &'static str {
        "fake"
    }
    fn display_name(&self) -> &'static str {
        "Fake"
    }
    fn mode(&self) -> ScraperMode {
        self.mode
    }
    async fn search(
        &self,
        _input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let mut out = Vec::new();
        for i in 0..self.count {
            if ctx.signal.is_cancelled() {
                break;
            }
            let job = JobPosting {
                id: format!("fake:{i}"),
                external_id: Some(i.to_string()),
                title: format!("Job {i}"),
                company: "Fake Co".to_string(),
                location: None,
                url: format!("https://example.com/{i}"),
                source: "fake".to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra: std::collections::HashMap::new(),
            };
            if let Some(ref on_item) = ctx.on_item {
                on_item(job.clone());
            }
            out.push(job);
        }
        Ok(out)
    }
}

/// A fake scraper that always returns an error.
struct FailingScraper;

#[async_trait::async_trait]
impl super::super::types::Scraper for FailingScraper {
    fn id(&self) -> &'static str {
        "failing"
    }
    fn display_name(&self) -> &'static str {
        "Failing"
    }
    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }
    async fn search(
        &self,
        _input: BoardSearchInput,
        _ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        Err(anyhow::anyhow!("board error"))
    }
}

fn fake_input(amount: u32) -> BoardSearchInput {
    BoardSearchInput {
        query: "q".to_string(),
        location: None,
        amount,
        pages: 10,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        locale: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
    }
}

// ── run_one tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn central_amount_cap_truncates_stream_and_return() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let fake = FakeScraper::http(50);

    // Count how many items the renderer-facing callback actually receives.
    let streamed = std::sync::Arc::new(AtomicUsize::new(0));
    let streamed_cb = streamed.clone();
    let on_item: Box<dyn Fn(JobPosting) + Send> = Box::new(move |_item| {
        streamed_cb.fetch_add(1, Ordering::SeqCst);
    });

    let token = CancellationToken::new();
    let items = ScraperEngine::run_one(
        "fake",
        Ok(&fake as &dyn super::super::types::Scraper),
        fake_input(20),
        token,
        None,
        Some(on_item),
    )
    .await
    .expect("capped scrape recovers as success");

    assert_eq!(
        streamed.load(Ordering::SeqCst),
        20,
        "exactly `amount` items forwarded to the renderer"
    );
    assert_eq!(items.len(), 20, "returned Vec truncated to `amount`");
}

#[tokio::test]
async fn cancel_reaches_a_pre_registered_token() {
    let engine = ScraperEngine::new();
    let token = tokio_util::sync::CancellationToken::new();

    // Autopilot registers its own token for the whole run, then scrape_boards
    // reuses (does not overwrite) that slot — so the clone the run keeps for
    // its post-scrape phase must flip when a tray/UI cancel hits the job.
    engine.register_token("run-1", token.clone()).await;
    assert!(!token.is_cancelled());
    engine.cancel("run-1").await;
    assert!(token.is_cancelled());
}

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
    let on_item: Arc<dyn Fn(JobPosting) + Send + Sync> =
        Arc::new(move |_| { received_cb.fetch_add(1, Ordering::SeqCst); });

    let parent = CancellationToken::new();
    let results = ScraperEngine::run_boards(
        resolved,
        fake_input(10),
        parent,
        None,
        Some(on_item),
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
    let results = ScraperEngine::run_boards(resolved, fake_input(10), parent, None, None).await;

    assert_eq!(results.len(), 3);
    let map: HashMap<String, _> = results.into_iter().collect();

    assert!(map["good_a"].is_ok(), "good_a should succeed");
    assert!(map["fail"].is_err(), "fail should produce an error");
    assert!(map["good_b"].is_ok(), "good_b should succeed");

    // The error board's summary would carry an error message.
    assert!(map["fail"].as_ref().unwrap_err().to_string().contains("board error"));
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

    let results = ScraperEngine::run_boards(resolved, fake_input(1000), parent, None, None).await;

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
    let results =
        ScraperEngine::run_boards(resolved, fake_input(3), parent.clone(), None, None).await;

    assert!(!parent.is_cancelled(), "child cap must not cancel parent token");

    let map: HashMap<String, _> = results.into_iter().collect();
    assert_eq!(
        map["a"].as_ref().unwrap().len(),
        3,
        "board a capped to 3"
    );
    assert_eq!(
        map["b"].as_ref().unwrap().len(),
        3,
        "board b also capped to 3 (same amount applied per-board)"
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
        fn id(&self) -> &'static str { "probe" }
        fn display_name(&self) -> &'static str { "Probe" }
        fn mode(&self) -> ScraperMode { ScraperMode::Browser }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            let now = self.active.fetch_add(1, Ord::SeqCst) + 1;
            // Update peak.
            let mut cur_peak = self.peak.load(Ord::SeqCst);
            while now > cur_peak {
                match self.peak.compare_exchange(cur_peak, now, Ord::SeqCst, Ord::SeqCst) {
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
    ScraperEngine::run_boards(resolved, fake_input(5), parent, None, None).await;

    assert_eq!(
        peak.load(Ordering::SeqCst),
        1,
        "browser boards must run one at a time (peak concurrency must be 1)"
    );
}

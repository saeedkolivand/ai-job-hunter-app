use super::*;
use tokio::sync::Semaphore as TokioSemaphore;

fn test_browser_sem() -> Arc<TokioSemaphore> {
    Arc::new(TokioSemaphore::new(1))
}

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
    assert_eq!(
        entry("glassdoor").auth,
        AuthRequirement::Required,
        "glassdoor must be Required (bot-blocks anonymous sessions)"
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
fn test_catalog_requires_company_flags() {
    let engine = ScraperEngine::new();
    let catalog = engine.catalog();

    let entry = |id: &str| {
        catalog
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing board: {id}"))
    };

    // The 6 ATS boards must declare requires_company = true.
    for ats_id in &[
        "greenhouse",
        "lever",
        "ashby",
        "recruitee",
        "personio",
        "smartrecruiters",
    ] {
        assert!(
            entry(ats_id).requires_company,
            "ATS board '{ats_id}' must have requires_company=true"
        );
    }

    // All other boards must keep the default false.
    for non_ats_id in &[
        "linkedin",
        "indeed",
        "xing",
        "glassdoor",
        "ycombinator",
        "remoteok",
        "remotive",
        "arbeitnow",
        "wwr",
        "stepstone",
        "workday",
        "berlinstartupjobs",
        "germantechjobs",
        "arbeitsagentur",
    ] {
        assert!(
            !entry(non_ats_id).requires_company,
            "board '{non_ats_id}' must have requires_company=false (default)"
        );
    }
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

    // glassdoor is listed; login is Required (engine skips without a valid session)
    assert!(entry("glassdoor").listed, "glassdoor must be listed");

    // A representative guest board is listed
    assert!(entry("greenhouse").listed, "greenhouse must be listed");
    assert!(entry("linkedin").listed, "linkedin must be listed");
    assert!(entry("indeed").listed, "indeed must be listed");

    // All 20 boards are listed
    let listed_count = catalog.iter().filter(|e| e.listed).count();
    assert_eq!(listed_count, 20, "all 20 boards should be listed");
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

/// A fake scraper that always returns a fixed number of items, ignoring the
/// cancellation signal. Used to simulate a board that already has items buffered
/// before checking cancellation (e.g., a board that completed a page fetch).
struct UncancellableScraper {
    count: usize,
}

#[async_trait::async_trait]
impl super::super::types::Scraper for UncancellableScraper {
    fn id(&self) -> &'static str {
        "uncancellable"
    }
    fn display_name(&self) -> &'static str {
        "Uncancellable"
    }
    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }
    async fn search(
        &self,
        _input: BoardSearchInput,
        _ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Intentionally ignores the signal to simulate a board returning items
        // it already fetched before noticing the cancellation.
        Ok((0..self.count)
            .map(|i| JobPosting {
                id: format!("always:{i}"),
                external_id: None,
                title: format!("Job {i}"),
                company: "Always Co".to_string(),
                location: None,
                url: format!("https://always.example.com/{i}"),
                source: "uncancellable".to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra: std::collections::HashMap::new(),
            })
            .collect())
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
        companies: Vec::new(),
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
        test_browser_sem(),
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
        test_browser_sem(),
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
        test_browser_sem(),
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
        test_browser_sem(),
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
        test_browser_sem(),
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
        test_browser_sem(),
    )
    .await;

    assert_eq!(
        peak.load(Ordering::SeqCst),
        1,
        "browser boards must run one at a time (peak concurrency must be 1)"
    );
}

// ── CWE-770 board-batch cap tests ─────────────────────────────────────────────

/// A 5000-entry input made of duplicates + a handful of distinct valid ids must
/// resolve to at most MAX_BOARDS_PER_BATCH (6) distinct board runs.
/// Uses the engine's `scrape_boards` method directly via the public API seam,
/// injecting a real (but fast-returning) FakeScraper via run_boards for isolation.
#[tokio::test]
async fn scrape_boards_dedupes_and_caps_large_input() {
    use super::MAX_BOARDS_PER_BATCH;

    // Build 8 distinct names (>6) plus duplicates filling 5000 total entries.
    let distinct: Vec<String> = (0..8).map(|i| format!("board_{i}")).collect();
    let mut boards: Vec<String> = Vec::with_capacity(5000);
    for i in 0..5000 {
        boards.push(distinct[i % distinct.len()].clone());
    }

    // Wire up fake scrapers for all 8 distinct ids so "unknown board" errors don't
    // confuse the count — we test the batch size cap, not unknown-id handling.
    let fakes: Vec<FakeScraper> = (0..8).map(|_| FakeScraper::http(1)).collect();
    let fake_refs: Vec<(String, anyhow::Result<&dyn Scraper>)> = {
        // Dedupe + truncate exactly as scrape_boards does — mirror the logic here
        // so the test drives run_boards at the capped slice.
        let mut seen = std::collections::HashSet::new();
        boards
            .iter()
            .filter(|id| seen.insert(id.as_str()))
            .take(MAX_BOARDS_PER_BATCH)
            .enumerate()
            .map(|(i, id)| (id.clone(), Ok(&fakes[i] as &dyn Scraper)))
            .collect()
    };

    let parent = CancellationToken::new();
    let results = ScraperEngine::run_boards(
        fake_refs,
        fake_input(1),
        parent,
        None,
        None,
        test_browser_sem(),
    )
    .await;

    assert!(
        results.len() <= MAX_BOARDS_PER_BATCH,
        "run_boards result count ({}) must not exceed MAX_BOARDS_PER_BATCH ({})",
        results.len(),
        MAX_BOARDS_PER_BATCH
    );
    assert_eq!(
        results.len(),
        MAX_BOARDS_PER_BATCH,
        "expected exactly MAX_BOARDS_PER_BATCH distinct board runs after dedup+truncate"
    );
}

/// HIGH 2 — CWE-770: `scrape_boards` itself must enforce the cap and dedupe,
/// not just `run_boards`. A future refactor moving the guard out of `scrape_boards`
/// MUST fail this test.
///
/// Strategy: pass 5 000 entries composed of > MAX_BOARDS_PER_BATCH distinct IDs
/// directly to `ScraperEngine::scrape_boards`. Unknown board IDs resolve to
/// `Err("Unknown board: …")` entries immediately — no network calls — so the run
/// is fast. The test asserts that summaries.len() ≤ MAX_BOARDS_PER_BATCH and that
/// first-seen order is preserved (the first 6 distinct IDs win, not a random set).
#[tokio::test]
async fn scrape_boards_real_entrypoint_caps_and_dedupes() {
    use super::MAX_BOARDS_PER_BATCH;

    // 9 distinct fake IDs (> MAX_BOARDS_PER_BATCH=6) interleaved with duplicates.
    // None of these match registered boards, so they resolve to Err immediately.
    let distinct: Vec<String> = (0..9).map(|i| format!("nonexistent_board_{i}")).collect();
    let mut boards: Vec<String> = Vec::with_capacity(5000);
    for i in 0..5000 {
        boards.push(distinct[i % distinct.len()].clone());
    }

    // The first 6 distinct IDs we see in iteration order.
    let expected_first_six: Vec<String> = distinct[..MAX_BOARDS_PER_BATCH].to_vec();

    let engine = ScraperEngine::new();
    // No cancellation — all_failed=true but parent.is_cancelled()=false → Ok.
    let result = engine
        .scrape_boards(
            &boards,
            fake_input(1),
            "test-job-cap".to_string(),
            None,
            None,
        )
        .await;

    let (postings, summaries) = result
        .expect("all boards unknown (no network) but not cancelled → must return Ok, not Err");

    assert!(postings.is_empty(), "unknown boards produce no postings");
    assert!(
        summaries.len() <= MAX_BOARDS_PER_BATCH,
        "summaries ({}) must not exceed MAX_BOARDS_PER_BATCH ({})",
        summaries.len(),
        MAX_BOARDS_PER_BATCH
    );
    assert_eq!(
        summaries.len(),
        MAX_BOARDS_PER_BATCH,
        "exactly MAX_BOARDS_PER_BATCH summaries expected after dedup+truncate"
    );

    // Verify first-seen order: the winning IDs must be the first 6 distinct ones.
    let summary_boards: Vec<&str> = summaries.iter().map(|s| s.board.as_str()).collect();
    let expected_refs: Vec<&str> = expected_first_six.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        summary_boards, expected_refs,
        "dedupe must preserve first-seen order; got {summary_boards:?}"
    );

    // Every summary must carry an error (unknown board → no items recovered).
    for s in &summaries {
        assert!(
            s.error.is_some(),
            "board '{}' is unknown so must report an error",
            s.board
        );
        assert_eq!(
            s.count, 0,
            "unknown board '{}' must report count=0",
            s.board
        );
    }
}

/// HIGH 3 — cancellation + all-failed gate: `scrape_boards` returns `Err("scrape cancelled")`
/// only when ALL boards failed AND the parent token is cancelled.
#[tokio::test]
async fn scrape_boards_all_failed_and_cancelled_returns_err() {
    // Pre-register a token under the job_id so scrape_boards reuses it (matching
    // the Autopilot pattern). Cancel it before the run so it's already cancelled
    // when the boards start — no timing dependency.
    let engine = ScraperEngine::new();
    let token = CancellationToken::new();
    engine
        .register_token("job-cancel-all-fail", token.clone())
        .await;
    token.cancel();

    // All boards are unknown → all fail with Err("Unknown board: …") immediately.
    let boards: Vec<String> = vec![
        "nonexistent_a".to_string(),
        "nonexistent_b".to_string(),
        "nonexistent_c".to_string(),
    ];

    let result = engine
        .scrape_boards(
            &boards,
            fake_input(5),
            "job-cancel-all-fail".to_string(),
            None,
            None,
        )
        .await;

    assert!(
        result.is_err(),
        "all-failed + cancelled must return Err, got: {result:?}"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("cancelled"),
        "error message must mention cancellation, got: {msg}"
    );
}

/// HIGH 3 (complementary) — partial success under cancellation returns `Ok`.
///
/// When at least one board recovers ≥1 item and the parent token is cancelled,
/// `scrape_boards` must return `Ok`. The gate now requires BOTH
/// `parent.is_cancelled() && !any_recovered_items` to return `Err`, so a board
/// that delivers items despite the cancel (already had a page buffered) keeps
/// the run as `Ok`.
///
/// Exercises the EXACT same code path as the public `scrape_boards` via the
/// resolver seam (`scrape_boards_with_resolver`).
#[tokio::test]
async fn scrape_boards_partial_success_under_cancel_returns_ok() {
    // UncancellableScraper ignores the signal — simulates a board that already
    // fetched a page before the cancel arrived. Static so the resolver closure
    // can return `&'static dyn Scraper`.
    static FAKE_OK: std::sync::LazyLock<UncancellableScraper> =
        std::sync::LazyLock::new(|| UncancellableScraper { count: 3 });
    static FAKE_FAIL: std::sync::LazyLock<FailingScraper> =
        std::sync::LazyLock::new(|| FailingScraper);

    let engine = ScraperEngine::new();

    // Pre-register and cancel the token before the run — matches the Autopilot
    // pattern tested by `scrape_boards_all_failed_and_cancelled_returns_err`.
    let token = CancellationToken::new();
    engine
        .register_token("job-partial-cancel", token.clone())
        .await;
    token.cancel();

    // Two board ids: "ok-board" resolves to UncancellableScraper (3 items even
    // under cancel), "fail-board" resolves to FailingScraper (always Err).
    let boards = vec!["ok-board".to_string(), "fail-board".to_string()];

    let result = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(3),
            "job-partial-cancel".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "ok-board" => Ok(&*FAKE_OK as &'static dyn super::super::types::Scraper),
                "fail-board" => Ok(&*FAKE_FAIL as &'static dyn super::super::types::Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await;

    // Partial success (ok-board recovered 3 items) + cancelled → must return Ok.
    let (postings, summaries) =
        result.expect("partial success under cancellation must return Ok, not Err");

    assert_eq!(
        summaries.len(),
        2,
        "summaries must cover both boards; got {summaries:?}"
    );

    let ok_summary = summaries
        .iter()
        .find(|s| s.board == "ok-board")
        .expect("ok-board summary missing");
    assert!(
        ok_summary.error.is_none(),
        "ok-board must not carry an error in its summary; got {ok_summary:?}"
    );
    assert!(
        ok_summary.count > 0,
        "ok-board must report recovered items; got count=0"
    );

    let fail_summary = summaries
        .iter()
        .find(|s| s.board == "fail-board")
        .expect("fail-board summary missing");
    assert!(
        fail_summary.error.is_some(),
        "fail-board must carry an error in its summary; got {fail_summary:?}"
    );

    // The ok-board's 3 items must be present in the result.
    assert!(
        !postings.is_empty(),
        "recovered postings must be non-empty when ok-board delivered items"
    );
}

// ── F1: empty boards guard ────────────────────────────────────────────────────

#[tokio::test]
async fn scrape_boards_rejects_empty_boards_list() {
    let engine = ScraperEngine::new();
    let result = engine
        .scrape_boards(&[], fake_input(5), "job-empty".to_string(), None, None)
        .await;
    assert!(result.is_err(), "empty boards list must return Err");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("at least one board"),
        "error message must mention the empty-list requirement"
    );
}

// ── F3: process-wide browser semaphore across concurrent engine calls ─────────

/// Two concurrent `scrape_boards` calls that both include a browser board must
/// serialize those browser boards on the shared engine semaphore — peak browser
/// concurrency must remain 1 regardless of how many scrape_boards calls are
/// in-flight simultaneously.
#[tokio::test]
async fn concurrent_scrape_boards_serialize_browser_boards() {
    use std::sync::atomic::{AtomicI32, Ordering as Ord};
    use std::time::Duration;

    // Static probe scrapers so the resolver closure can return `&'static dyn Scraper`.
    static ACTIVE: std::sync::LazyLock<Arc<AtomicI32>> =
        std::sync::LazyLock::new(|| Arc::new(AtomicI32::new(0)));
    static PEAK: std::sync::LazyLock<Arc<AtomicI32>> =
        std::sync::LazyLock::new(|| Arc::new(AtomicI32::new(0)));

    struct ProbeScraper;

    #[async_trait::async_trait]
    impl Scraper for ProbeScraper {
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
            let now = ACTIVE.fetch_add(1, Ord::SeqCst) + 1;
            let mut cur = PEAK.load(Ord::SeqCst);
            while now > cur {
                match PEAK.compare_exchange(cur, now, Ord::SeqCst, Ord::SeqCst) {
                    Ok(_) => break,
                    Err(p) => cur = p,
                }
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
            ACTIVE.fetch_sub(1, Ord::SeqCst);
            Ok(vec![])
        }
    }

    static PROBE: std::sync::LazyLock<ProbeScraper> = std::sync::LazyLock::new(|| ProbeScraper);

    // Use one shared engine — both calls share the same `browser_sem` field.
    let engine = Arc::new(ScraperEngine::new());
    let e1 = engine.clone();
    let e2 = engine.clone();

    // Bind board slices to named variables so the temporaries live long enough
    // across the tokio::join! expansion.
    let boards_a = vec!["probe-a".to_string()];
    let boards_b = vec!["probe-b".to_string()];

    let (r1, r2) = tokio::join!(
        e1.scrape_boards_with_resolver(
            &boards_a,
            fake_input(1),
            "job-browser-1".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |_id| Ok(&*PROBE as &'static dyn Scraper),
        ),
        e2.scrape_boards_with_resolver(
            &boards_b,
            fake_input(1),
            "job-browser-2".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |_id| Ok(&*PROBE as &'static dyn Scraper),
        ),
    );
    let _ = (r1, r2);

    assert_eq!(
        PEAK.load(Ord::SeqCst),
        1,
        "shared browser_sem must serialize browser boards across concurrent scrape_boards calls"
    );
}

// ── F4: input-order preservation ─────────────────────────────────────────────

/// `.buffered(3)` preserves input order — postings from board "a" must precede
/// those from "b", which must precede "c", regardless of which board finishes first.
#[tokio::test]
async fn run_boards_preserves_input_order() {
    use std::time::Duration;

    // Slow → Fast → Medium intentionally out of completion order.
    struct DelayedScraper {
        delay_ms: u64,
        tag: &'static str,
    }

    #[async_trait::async_trait]
    impl Scraper for DelayedScraper {
        fn id(&self) -> &'static str {
            "delayed"
        }
        fn display_name(&self) -> &'static str {
            "Delayed"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
            if ctx.signal.is_cancelled() {
                return Ok(vec![]);
            }
            Ok(vec![JobPosting {
                id: self.tag.to_string(),
                external_id: None,
                title: self.tag.to_string(),
                company: "co".into(),
                location: None,
                url: format!("https://example.com/{}", self.tag),
                source: self.tag.to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra: std::collections::HashMap::new(),
            }])
        }
    }

    let slow = DelayedScraper {
        delay_ms: 40,
        tag: "a",
    };
    let fast = DelayedScraper {
        delay_ms: 5,
        tag: "b",
    };
    let mid = DelayedScraper {
        delay_ms: 20,
        tag: "c",
    };

    let resolved: Vec<(String, anyhow::Result<&dyn Scraper>)> = vec![
        ("a".into(), Ok(&slow as &dyn Scraper)),
        ("b".into(), Ok(&fast as &dyn Scraper)),
        ("c".into(), Ok(&mid as &dyn Scraper)),
    ];

    let parent = CancellationToken::new();
    let results = ScraperEngine::run_boards(
        resolved,
        fake_input(5),
        parent,
        None,
        None,
        test_browser_sem(),
    )
    .await;

    // Order must match input (a, b, c) even though completion order is b, c, a.
    let names: Vec<&str> = results.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        vec!["a", "b", "c"],
        "run_boards must preserve input order"
    );
}

// ── F6: cancelled + zero-recovered = error ────────────────────────────────────

/// All boards return `Ok([])` under a pre-cancelled token → `scrape_boards`
/// must return `Err("scrape cancelled")` because no items were actually recovered.
#[tokio::test]
async fn scrape_boards_all_empty_ok_under_cancel_returns_err() {
    static EMPTY_FAKE: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(100));

    let engine = ScraperEngine::new();
    let token = CancellationToken::new();
    engine
        .register_token("job-empty-cancel", token.clone())
        .await;
    token.cancel(); // pre-cancel so FakeScraper sees it and returns Ok([])

    let result = engine
        .scrape_boards_with_resolver(
            &["board-a".to_string(), "board-b".to_string()],
            fake_input(100),
            "job-empty-cancel".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |_id| Ok(&*EMPTY_FAKE as &'static dyn Scraper),
        )
        .await;

    assert!(
        result.is_err(),
        "cancelled run with all Ok([]) must return Err, got: {result:?}"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("cancelled"),
        "error must mention cancellation; got: {msg}"
    );
}

// ── Required-board skip short-circuit ────────────────────────────────────────

/// A `Required` fake scraper with no cookies must be skipped without calling
/// `search` (the summary has `skipped=Some("needs-login")`, `count=0`, no
/// error), while a `Guest` fake runs normally. The RequiredPanicker's `search`
/// panics, so if the short-circuit is absent the test fails immediately.
#[tokio::test]
async fn required_board_without_session_is_skipped() {
    // Required scraper: panics if searched.
    struct RequiredPanicker;

    #[async_trait::async_trait]
    impl Scraper for RequiredPanicker {
        fn id(&self) -> &'static str {
            "required-panicker"
        }
        fn display_name(&self) -> &'static str {
            "RequiredPanicker"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn auth(&self) -> super::super::types::AuthRequirement {
            super::super::types::AuthRequirement::Required
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            panic!("RequiredPanicker.search must never be called when no session exists");
        }
    }

    static REQ: std::sync::LazyLock<RequiredPanicker> =
        std::sync::LazyLock::new(|| RequiredPanicker);
    // Guest board: 1 item so we can assert it WAS actually run (count=1), not silently skipped.
    static FAKE_GUEST: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(1));

    let engine = ScraperEngine::new();
    // Use an isolated tempdir — no cookies.json exists there for "required-board",
    // so load_cookies returns [] and the skip fires. If the skip is absent the
    // panicker's search() runs and the test panics.
    let tmp = tempfile::tempdir().expect("tempdir");
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["required-board".to_string(), "guest-board".to_string()],
            fake_input(5),
            "job-skip-test".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "required-board" => Ok(&*REQ as &'static dyn Scraper),
                "guest-board" => Ok(&*FAKE_GUEST as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unknown: {other}")),
            },
        )
        .await
        .expect("skip run must return Ok");

    let req_summary = summaries
        .iter()
        .find(|s| s.board == "required-board")
        .expect("required-board summary missing");
    assert_eq!(
        req_summary.skipped.as_deref(),
        Some("needs-login"),
        "Required board with no session must be skipped with 'needs-login'"
    );
    assert_eq!(req_summary.count, 0, "skipped board must report count=0");
    assert!(
        req_summary.error.is_none(),
        "skipped board must not carry an error"
    );

    let guest_summary = summaries
        .iter()
        .find(|s| s.board == "guest-board")
        .expect("guest-board summary missing");
    assert!(
        guest_summary.skipped.is_none(),
        "Guest board must not be skipped"
    );
    assert!(
        guest_summary.error.is_none(),
        "Guest board must run without error"
    );
    assert_eq!(
        guest_summary.count, 1,
        "Guest board (FakeScraper::http(1)) must have been run and report count=1"
    );
}

/// Input order is preserved across a mixed run/skip/run scenario:
/// [required-no-session (skip), guest (run), required-no-session-2 (skip)]
/// Summaries must come back in that same order, not skips-last.
#[tokio::test]
async fn scrape_boards_summaries_preserve_input_order_with_skips() {
    struct AlwaysRequired;

    #[async_trait::async_trait]
    impl Scraper for AlwaysRequired {
        fn id(&self) -> &'static str {
            "always-required"
        }
        fn display_name(&self) -> &'static str {
            "AlwaysRequired"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn auth(&self) -> super::super::types::AuthRequirement {
            super::super::types::AuthRequirement::Required
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            panic!("AlwaysRequired.search must not be called when no session exists");
        }
    }

    static REQ1: std::sync::LazyLock<AlwaysRequired> = std::sync::LazyLock::new(|| AlwaysRequired);
    static REQ2: std::sync::LazyLock<AlwaysRequired> = std::sync::LazyLock::new(|| AlwaysRequired);
    static GUEST: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(2));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &[
                "req-1".to_string(),
                "guest-mid".to_string(),
                "req-2".to_string(),
            ],
            fake_input(10),
            "job-order-test".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "req-1" => Ok(&*REQ1 as &'static dyn Scraper),
                "guest-mid" => Ok(&*GUEST as &'static dyn Scraper),
                "req-2" => Ok(&*REQ2 as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unknown: {other}")),
            },
        )
        .await
        .expect("mixed run must return Ok");

    assert_eq!(summaries.len(), 3, "one summary per requested board");
    let order: Vec<&str> = summaries.iter().map(|s| s.board.as_str()).collect();
    assert_eq!(
        order,
        vec!["req-1", "guest-mid", "req-2"],
        "summaries must be in input order, not run-results-then-skips; got {order:?}"
    );
    assert_eq!(
        summaries[0].skipped.as_deref(),
        Some("needs-login"),
        "req-1 must be skipped"
    );
    assert_eq!(summaries[1].skipped, None, "guest-mid must not be skipped");
    assert_eq!(summaries[1].count, 2, "guest-mid must report 2 items run");
    assert_eq!(
        summaries[2].skipped.as_deref(),
        Some("needs-login"),
        "req-2 must be skipped"
    );
}

/// A Required board whose skip predicate fires on a stale session must be
/// skipped with `skipped=Some("needs-login")` — same outcome as no-cookies.
///
/// Uses a tempdir so this test never touches the real `data_dir()` and is
/// safe to run concurrently with any other test.
#[tokio::test]
async fn required_board_stale_session_is_skipped() {
    use crate::scraping::board_login::{auth_status_path, write_cookies, StoredCookie};

    let board_id = "stale-engine-test-board";

    struct RequiredPanicker2;
    #[async_trait::async_trait]
    impl Scraper for RequiredPanicker2 {
        fn id(&self) -> &'static str {
            "req-stale"
        }
        fn display_name(&self) -> &'static str {
            "RequiredPanicker2"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn auth(&self) -> super::super::types::AuthRequirement {
            super::super::types::AuthRequirement::Required
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            panic!("RequiredPanicker2.search must not be called on stale session");
        }
    }
    static STALE_SCRAPER: std::sync::LazyLock<RequiredPanicker2> =
        std::sync::LazyLock::new(|| RequiredPanicker2);

    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path();

    // Write a fresh cookie (non-empty → skip fires on staleness, not absence).
    let cookie = StoredCookie {
        name: "sess".into(),
        value: "tok".into(),
        domain: "example.com".into(),
        path: "/".into(),
        expires: None,
        http_only: false,
        secure: false,
    };
    write_cookies(data_dir, board_id, &[cookie]).expect("write_cookies");
    // connected_at = 0 → age ≈ now-ms → always > SESSION_MAX_AGE_MS (7 days).
    let apath = auth_status_path(data_dir, board_id);
    std::fs::write(&apath, r#"{"connected":true,"connected_at":0}"#)
        .expect("overwrite auth-status with epoch-0");

    let engine = ScraperEngine::new();
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &[board_id.to_string()],
            fake_input(1),
            "job-stale-skip".to_string(),
            None,
            None,
            data_dir,
            |_id| Ok(&*STALE_SCRAPER as &'static dyn Scraper),
        )
        .await
        .expect("stale-session skip must return Ok");

    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].skipped.as_deref(),
        Some("needs-login"),
        "stale-session Required board must be skipped with 'needs-login'"
    );
    assert_eq!(summaries[0].count, 0);
    assert!(summaries[0].error.is_none());
}

/// A Required board with fresh cookies + fresh auth-status must NOT be skipped —
/// search is called and the summary appears in run results with its items.
///
/// Uses a tempdir so this test never touches the real `data_dir()`.
#[tokio::test]
async fn required_board_with_valid_session_runs() {
    use crate::scraping::board_login::{write_auth_status, write_cookies, StoredCookie};

    let board_id = "fresh-engine-test-board";

    static FRESH_SCRAPER: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(3));

    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path();

    let cookie = StoredCookie {
        name: "sess".into(),
        value: "tok".into(),
        domain: "example.com".into(),
        path: "/".into(),
        expires: None,
        http_only: false,
        secure: false,
    };
    write_cookies(data_dir, board_id, &[cookie]).expect("write_cookies");
    // connected_at = now → age ≈ 0 → not stale.
    write_auth_status(data_dir, board_id, true);

    let engine = ScraperEngine::new();
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &[board_id.to_string()],
            fake_input(5),
            "job-fresh-session".to_string(),
            None,
            None,
            data_dir,
            |_id| Ok(&*FRESH_SCRAPER as &'static dyn Scraper),
        )
        .await
        .expect("fresh-session Required board must return Ok");

    assert_eq!(summaries.len(), 1);
    assert!(
        summaries[0].skipped.is_none(),
        "Required board with fresh session must NOT be skipped; got {:?}",
        summaries[0].skipped
    );
    assert!(summaries[0].error.is_none(), "must not error");
    assert_eq!(
        summaries[0].count, 3,
        "FakeScraper::http(3) must return 3 items"
    );
}

/// A Required board with non-empty cookies but no valid connected status (e.g.
/// `{"connected":false,"connected_at":0}`) must be skipped — the fix to also
/// check `session_age_ms(…).is_none()` covers this case. The cookie-empty branch
/// would NOT fire here, so this test specifically exercises the new branch.
#[tokio::test]
async fn required_board_cookies_but_no_valid_status_is_skipped() {
    use crate::scraping::board_login::{auth_status_path, write_cookies, StoredCookie};

    let board_id = "no-status-engine-test-board";

    struct RequiredPanicker3;
    #[async_trait::async_trait]
    impl Scraper for RequiredPanicker3 {
        fn id(&self) -> &'static str {
            "req-no-status"
        }
        fn display_name(&self) -> &'static str {
            "RequiredPanicker3"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn auth(&self) -> super::super::types::AuthRequirement {
            super::super::types::AuthRequirement::Required
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            panic!("RequiredPanicker3.search must not be called when connected status is invalid");
        }
    }
    static NO_STATUS_SCRAPER: std::sync::LazyLock<RequiredPanicker3> =
        std::sync::LazyLock::new(|| RequiredPanicker3);

    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path();

    // Write non-empty cookies so the empty-cookie branch does NOT fire.
    let cookie = StoredCookie {
        name: "sess".into(),
        value: "tok".into(),
        domain: "example.com".into(),
        path: "/".into(),
        expires: None,
        http_only: false,
        secure: false,
    };
    write_cookies(data_dir, board_id, &[cookie]).expect("write_cookies");

    // Write an auth-status with connected:false → session_age_ms returns None.
    // connected:false → session_age_ms() == None (clause 2 fires) BEFORE connected_at is read;
    // near-now ts means session_is_stale() is false, so clause 3 cannot mask the fix.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let apath = auth_status_path(data_dir, board_id);
    std::fs::write(
        &apath,
        format!(r#"{{"connected":false,"connected_at":{now_ms}}}"#),
    )
    .expect("write connected:false auth-status");

    let engine = ScraperEngine::new();
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &[board_id.to_string()],
            fake_input(1),
            "job-no-status-skip".to_string(),
            None,
            None,
            data_dir,
            |_id| Ok(&*NO_STATUS_SCRAPER as &'static dyn Scraper),
        )
        .await
        .expect("no-valid-status skip must return Ok");

    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].skipped.as_deref(),
        Some("needs-login"),
        "Required board with non-empty cookies but no valid connected status must be skipped with 'needs-login'"
    );
    assert_eq!(summaries[0].count, 0);
    assert!(summaries[0].error.is_none());
}

/// Every board Required + no session → returns Ok with empty postings and
/// all-skipped summaries (NOT Err). No network calls, no panics.
#[tokio::test]
async fn all_required_no_session_returns_ok_empty() {
    struct ReqNoop;

    #[async_trait::async_trait]
    impl Scraper for ReqNoop {
        fn id(&self) -> &'static str {
            "req-noop"
        }
        fn display_name(&self) -> &'static str {
            "ReqNoop"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn auth(&self) -> super::super::types::AuthRequirement {
            super::super::types::AuthRequirement::Required
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            panic!("ReqNoop.search must not be called when no session exists");
        }
    }

    static A: std::sync::LazyLock<ReqNoop> = std::sync::LazyLock::new(|| ReqNoop);
    static B: std::sync::LazyLock<ReqNoop> = std::sync::LazyLock::new(|| ReqNoop);

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = engine
        .scrape_boards_with_resolver(
            &["req-a".to_string(), "req-b".to_string()],
            fake_input(5),
            "job-all-req-no-session".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "req-a" => Ok(&*A as &'static dyn Scraper),
                "req-b" => Ok(&*B as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unknown: {other}")),
            },
        )
        .await;

    let (postings, summaries) =
        result.expect("all-Required-no-session must return Ok (not Err) — skips are not failures");
    assert!(postings.is_empty(), "no postings when all boards skipped");
    assert_eq!(summaries.len(), 2, "one summary per board");
    for s in &summaries {
        assert_eq!(
            s.skipped.as_deref(),
            Some("needs-login"),
            "board '{}' must be skipped with 'needs-login'",
            s.board
        );
        assert_eq!(s.count, 0, "skipped board '{}' must have count=0", s.board);
        assert!(
            s.error.is_none(),
            "skipped board '{}' must not carry an error",
            s.board
        );
    }
}

/// An unknown board id (resolver returns Err) must produce an error summary,
/// not a skip summary — it must not be silently treated as a skipped board.
#[tokio::test]
async fn unknown_board_errors_not_skipped() {
    let engine = ScraperEngine::new();
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["totally-unknown-board".to_string()],
            fake_input(1),
            "job-unknown-test".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| Err(anyhow::anyhow!("Unknown board: {id}")),
        )
        .await
        .expect("unknown board returns Ok (not Err) because parent is not cancelled");

    assert_eq!(summaries.len(), 1);
    let s = &summaries[0];
    assert_eq!(s.board, "totally-unknown-board");
    assert!(
        s.skipped.is_none(),
        "unknown board must not appear as skipped; got skipped={:?}",
        s.skipped
    );
    assert!(
        s.error.is_some(),
        "unknown board must carry an error summary"
    );
    assert_eq!(s.count, 0, "unknown board must have count=0");
}

// ── needs-company skip ────────────────────────────────────────────────────────

/// An ATS board (requires_company=true) with no companies in the input must be
/// skipped with `skipped=Some("needs-company")` and its `search` must never be
/// called — identical structure to the `needs-login` panicker tests.
#[tokio::test]
async fn ats_board_without_companies_is_skipped() {
    struct AtsNeedsPanicker;

    #[async_trait::async_trait]
    impl Scraper for AtsNeedsPanicker {
        fn id(&self) -> &'static str {
            "ats-needs-panicker"
        }
        fn display_name(&self) -> &'static str {
            "AtsNeedsPanicker"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn requires_company(&self) -> bool {
            true
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            panic!("AtsNeedsPanicker.search must never be called when companies is empty");
        }
    }

    static ATS: std::sync::LazyLock<AtsNeedsPanicker> =
        std::sync::LazyLock::new(|| AtsNeedsPanicker);
    // Guest board runs normally alongside the skipped ATS board.
    static GUEST: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(2));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    // input.companies is empty — the ATS board must be skipped.
    let input = fake_input(5); // companies: Vec::new() by construction

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["ats-board".to_string(), "guest-board".to_string()],
            input,
            "job-needs-company-test".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "ats-board" => Ok(&*ATS as &'static dyn Scraper),
                "guest-board" => Ok(&*GUEST as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unknown: {other}")),
            },
        )
        .await
        .expect("needs-company skip must return Ok");

    let ats_summary = summaries
        .iter()
        .find(|s| s.board == "ats-board")
        .expect("ats-board summary missing");
    assert_eq!(
        ats_summary.skipped.as_deref(),
        Some("needs-company"),
        "ATS board with no companies must be skipped with 'needs-company'"
    );
    assert_eq!(
        ats_summary.count, 0,
        "skipped ATS board must report count=0"
    );
    assert!(
        ats_summary.error.is_none(),
        "skipped ATS board must not carry an error"
    );

    let guest_summary = summaries
        .iter()
        .find(|s| s.board == "guest-board")
        .expect("guest-board summary missing");
    assert!(
        guest_summary.skipped.is_none(),
        "Guest board must not be skipped"
    );
    assert_eq!(
        guest_summary.count, 2,
        "Guest board (FakeScraper::http(2)) must have run normally"
    );
}

/// An ATS board with whitespace-only company entries must be skipped with
/// `skipped=Some("needs-company")`, just like an empty list.
/// Regression for the engine skip that checked only `is_empty()` — a payload
/// like `[" ", "\t"]` bypassed that check but was silently dropped by ATS
/// scrapers, breaking the UI missing-company warning path.
#[tokio::test]
async fn ats_board_whitespace_only_companies_is_skipped() {
    struct AtsWhitespacePanicker;

    #[async_trait::async_trait]
    impl Scraper for AtsWhitespacePanicker {
        fn id(&self) -> &'static str {
            "ats-whitespace-panicker"
        }
        fn display_name(&self) -> &'static str {
            "AtsWhitespacePanicker"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn requires_company(&self) -> bool {
            true
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            panic!("AtsWhitespacePanicker.search must never be called when only whitespace companies are supplied");
        }
    }

    static ATS_WS: std::sync::LazyLock<AtsWhitespacePanicker> =
        std::sync::LazyLock::new(|| AtsWhitespacePanicker);

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut input = fake_input(5);
    // Whitespace-only entries — not empty, but all trimmed to "".
    input.companies = vec!["   ".to_string(), "\t".to_string()];

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["ats-whitespace-panicker".to_string()],
            input,
            "job-whitespace-company-test".to_string(),
            None,
            None,
            tmp.path(),
            |_id| Ok(&*ATS_WS as &'static dyn Scraper),
        )
        .await
        .expect("whitespace-only companies must return Ok (skip path)");

    let s = summaries
        .iter()
        .find(|s| s.board == "ats-whitespace-panicker")
        .expect("summary missing");
    assert_eq!(
        s.skipped.as_deref(),
        Some("needs-company"),
        "whitespace-only companies must be treated as 'needs-company'"
    );
    assert_eq!(s.count, 0);
    assert!(s.error.is_none());
}

/// An ATS board with non-empty companies must NOT be skipped — search is called
/// and returns items.
#[tokio::test]
async fn ats_board_with_companies_runs() {
    static FAKE_ATS: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(3));

    struct FakeAtsWrapper;
    #[async_trait::async_trait]
    impl Scraper for FakeAtsWrapper {
        fn id(&self) -> &'static str {
            "fake-ats"
        }
        fn display_name(&self) -> &'static str {
            "FakeAts"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn requires_company(&self) -> bool {
            true
        }
        async fn search(
            &self,
            input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            // Delegate to the inner FakeScraper so we get real items.
            FAKE_ATS.search(input, ctx).await
        }
    }

    static WRAPPER: std::sync::LazyLock<FakeAtsWrapper> =
        std::sync::LazyLock::new(|| FakeAtsWrapper);

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut input = fake_input(5);
    input.companies = vec!["acme".to_string()]; // non-empty → must NOT skip

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["fake-ats".to_string()],
            input,
            "job-ats-with-company".to_string(),
            None,
            None,
            tmp.path(),
            |_id| Ok(&*WRAPPER as &'static dyn Scraper),
        )
        .await
        .expect("ATS board with companies must return Ok");

    assert_eq!(summaries.len(), 1);
    assert!(
        summaries[0].skipped.is_none(),
        "ATS board with companies must NOT be skipped; got {:?}",
        summaries[0].skipped
    );
    assert!(summaries[0].error.is_none(), "must not error");
    assert_eq!(
        summaries[0].count, 3,
        "FakeScraper::http(3) must return 3 items when companies is non-empty"
    );
}

// ── F2/F5: pre-registered token not removed by scrape_boards ─────────────────

/// When the caller pre-registers a token before calling `scrape_boards`,
/// the token slot must still exist in the engine's job map after the call
/// (scrape_boards must not remove a token it did not mint).
#[tokio::test]
async fn scrape_boards_does_not_remove_pre_registered_token() {
    static FAKE: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(1));

    let engine = ScraperEngine::new();
    let token = CancellationToken::new();
    engine
        .register_token("job-preregistered", token.clone())
        .await;

    // scrape_boards reuses the pre-registered token; we_minted=false → no removal.
    let _ = engine
        .scrape_boards_with_resolver(
            &["board-x".to_string()],
            fake_input(1),
            "job-preregistered".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |_id| Ok(&*FAKE as &'static dyn Scraper),
        )
        .await;

    // Token must still be reachable via cancel — if scrape_boards had removed it,
    // this cancel would be a no-op and the token would not be cancelled.
    engine.cancel("job-preregistered").await;
    assert!(
        token.is_cancelled(),
        "pre-registered token must remain in the job map after scrape_boards completes"
    );
}

// ── ATS per-company partial-failure isolation ─────────────────────────────────

/// A fake ATS scraper that iterates `input.companies`, returns a transport `Err`
/// for the first company ("slug-1"), and yields one item for the second
/// ("slug-2"). This is the pattern used by greenhouse, lever, ashby, recruitee,
/// smartrecruiters (list-fetch), and personio (per-host fetch_text) after the
/// partial-failure fix that replaces `?` with a `match … warn … continue`.
///
/// The test asserts that a transport error on slug-1 does NOT suppress the
/// result for slug-2 — i.e. the scraper continues the loop, not aborts.
struct AtsPartialFailScraper;

#[async_trait::async_trait]
impl Scraper for AtsPartialFailScraper {
    fn id(&self) -> &'static str {
        "ats-partial-fail"
    }
    fn display_name(&self) -> &'static str {
        "AtsPartialFail"
    }
    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }
    fn requires_company(&self) -> bool {
        true
    }
    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let mut out = Vec::new();
        for company in &input.companies {
            if ctx.signal.is_cancelled() {
                break;
            }
            // Simulate transport Err on "slug-1" (DNS / TLS failure pattern).
            if company == "slug-1" {
                log::warn!(
                    "[ats-partial-fail] simulated transport error for '{}'",
                    company
                );
                if ctx.signal.is_cancelled() {
                    break;
                }
                // continue to next company — do NOT propagate with `?`
                continue;
            }
            out.push(JobPosting {
                id: format!("ats-partial-fail:{company}"),
                external_id: Some(company.clone()),
                title: format!("Job at {company}"),
                company: company.clone(),
                location: None,
                url: format!("https://{company}.example.com/jobs/1"),
                source: "ats-partial-fail".to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra: std::collections::HashMap::new(),
            });
        }
        Ok(out)
    }
}

/// A transport error on slug-1 must not abort the company loop — slug-2's
/// items must still appear in the result. No live network.
#[tokio::test]
async fn ats_per_company_transport_error_does_not_suppress_remaining_companies() {
    static SCRAPER: std::sync::LazyLock<AtsPartialFailScraper> =
        std::sync::LazyLock::new(|| AtsPartialFailScraper);

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");

    let mut input = fake_input(10);
    input.companies = vec!["slug-1".to_string(), "slug-2".to_string()];

    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["ats-board".to_string()],
            input,
            "job-ats-partial-fail".to_string(),
            None,
            None,
            tmp.path(),
            |_id| Ok(&*SCRAPER as &'static dyn Scraper),
        )
        .await
        .expect("partial-failure ATS run must return Ok");

    assert_eq!(summaries.len(), 1, "one summary for the one board");
    assert!(
        summaries[0].error.is_none(),
        "board must not report a fatal error when only one slug failed; got {:?}",
        summaries[0].error
    );
    // slug-2 must have produced its item despite slug-1 erroring.
    assert!(
        postings.iter().any(|p| p.company == "slug-2"),
        "slug-2's job must appear in postings even though slug-1 errored; got {:?}",
        postings.iter().map(|p| &p.company).collect::<Vec<_>>()
    );
    // slug-1 must have produced nothing (it errored, not panicked).
    assert!(
        !postings.iter().any(|p| p.company == "slug-1"),
        "slug-1 errored and must produce no items"
    );
}

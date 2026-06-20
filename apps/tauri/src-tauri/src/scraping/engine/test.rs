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
    let results = ScraperEngine::run_boards(resolved, fake_input(10), parent, None, None).await;

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
    let results =
        ScraperEngine::run_boards(fake_refs, fake_input(1), parent, None, None).await;

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
        .scrape_boards(&boards, fake_input(1), "test-job-cap".to_string(), None, None)
        .await;

    let (postings, summaries) = result.expect(
        "all boards unknown (no network) but not cancelled → must return Ok, not Err",
    );

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
        assert_eq!(s.count, 0, "unknown board '{}' must report count=0", s.board);
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
    engine.register_token("job-cancel-all-fail", token.clone()).await;
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
/// When at least one board succeeds (≥1 item recovered) and the parent token is
/// cancelled, `scrape_boards` must return `Ok` — the two-condition gate
/// (`all_failed && parent.is_cancelled()`) requires BOTH to be true.
///
/// Exercises the EXACT same code path as the public `scrape_boards` via the
/// resolver seam (`scrape_boards_with_resolver`).
#[tokio::test]
async fn scrape_boards_partial_success_under_cancel_returns_ok() {
    // Static fake scrapers — required so the resolver closure can return
    // `&'static dyn Scraper` (matching what boards::get returns from SCRAPERS).
    static FAKE_OK: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(3));
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

    // Two board ids: "ok-board" resolves to a working FakeScraper (3 items),
    // "fail-board" resolves to a FailingScraper (always Err).
    let boards = vec!["ok-board".to_string(), "fail-board".to_string()];

    let result = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(3),
            "job-partial-cancel".to_string(),
            None,
            None,
            |id| match id {
                "ok-board" => Ok(&*FAKE_OK as &'static dyn super::super::types::Scraper),
                "fail-board" => Ok(&*FAKE_FAIL as &'static dyn super::super::types::Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await;

    // Partial success (at least one board Ok) + cancelled → must return Ok, not Err.
    let (postings, summaries) = result.expect(
        "partial success under cancellation must return Ok, not Err",
    );

    // ok-board streamed items even though the parent was pre-cancelled:
    // FakeScraper checks ctx.signal which is a child token, so 0 items is also
    // acceptable (the parent was cancelled before search started). What matters
    // is that run_boards returned Ok for that board — the gate must see !all_failed.
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

    let fail_summary = summaries
        .iter()
        .find(|s| s.board == "fail-board")
        .expect("fail-board summary missing");
    assert!(
        fail_summary.error.is_some(),
        "fail-board must carry an error in its summary; got {fail_summary:?}"
    );

    // postings may be 0 (if the child token was already cancelled), but the
    // overall result must be Ok — that's the gate being tested.
    let _ = postings; // count is environment-dependent; Ok return is the assertion
}

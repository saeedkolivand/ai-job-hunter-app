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

/// Fake board that streams `count` items through `ctx.on_item`, stopping early
/// when `ctx.signal` is cancelled (mimicking a real pagination loop), and returns
/// the same Vec it streamed. Used to drive the engine's central `amount` cap.
struct FakeScraper {
    count: usize,
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
        ScraperMode::Http
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

#[tokio::test]
async fn central_amount_cap_truncates_stream_and_return() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let engine = ScraperEngine::new();
    let fake = FakeScraper { count: 50 };

    // Count how many items the renderer-facing callback actually receives.
    let streamed = std::sync::Arc::new(AtomicUsize::new(0));
    let streamed_cb = streamed.clone();
    let on_item: Box<dyn Fn(JobPosting) + Send> = Box::new(move |_item| {
        streamed_cb.fetch_add(1, Ordering::SeqCst);
    });

    let items = engine
        .run_search(
            "fake",
            Ok(&fake as &dyn super::super::types::Scraper),
            fake_input(20),
            "job-cap".to_string(),
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

    // Autopilot registers its own token for the whole run, then scrape_board
    // reuses (does not overwrite) that slot — so the clone the run keeps for
    // its post-scrape phase must flip when a tray/UI cancel hits the job.
    engine.register_token("run-1", token.clone()).await;
    assert!(!token.is_cancelled());
    engine.cancel("run-1").await;
    assert!(token.is_cancelled());
}

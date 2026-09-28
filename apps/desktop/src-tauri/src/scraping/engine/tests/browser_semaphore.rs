//! F3: the process-wide browser semaphore serializes concurrent
//! `scrape_boards` calls that each include a browser-mode board.

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

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

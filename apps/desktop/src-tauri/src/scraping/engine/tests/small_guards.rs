//! Small engine invariant guards: F1 (empty board list errors), F4 (input-
//! order preservation), F6 (a cancelled run with zero recovered items is
//! `Err`), and F2/F5 (a pre-registered job token is not removed by
//! `scrape_boards` — the caller that registered it owns removal).

use std::collections::HashMap;

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

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
        None,
        None,
        None,
        test_browser_sem(),
        &HashMap::new(),
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

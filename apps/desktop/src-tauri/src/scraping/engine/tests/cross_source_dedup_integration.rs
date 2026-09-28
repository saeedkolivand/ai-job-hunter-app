//! Cross-source dedup, continued: `extra`-union edge cases and the
//! `scrape_boards`-level integration (an aggregator + a direct board
//! surfacing the same job collapse to one row).

use super::cross_source_dedup_field_policy::dedup_posting;
use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::dedup::dedup_cross_source;
use super::super::*;

#[test]
fn dedup_cross_source_equal_description_length_keeps_incumbent_unchanged() {
    // Equal-length descriptions (both None here) → no description upgrade; and
    // incumbent identity (source/url/id) is NEVER swapped by dedup regardless of
    // whether an upgrade happens.
    let input = vec![
        dedup_posting("first", "https://acme.example/jobs/42", "Eng", "Acme", None),
        dedup_posting(
            "second",
            "https://www.acme.example/jobs/42?ref=y",
            "Eng",
            "Acme",
            None,
        ),
    ];
    let out = dedup_cross_source(input);
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].source, "first",
        "incumbent identity (board/source) must never be swapped by dedup"
    );
    assert!(out[0].description.is_none(), "no description to upgrade to");
}

#[test]
fn dedup_cross_source_keeps_distinct_jobs_and_order() {
    // Distinct canonical keys must all survive, in first-seen order.
    let input = vec![
        dedup_posting("b", "https://acme.example/jobs/1", "A", "Acme", None),
        dedup_posting("b", "https://acme.example/jobs/2", "B", "Acme", None),
        dedup_posting("b", "https://acme.example/jobs/3", "C", "Acme", None),
    ];
    let urls: Vec<String> = dedup_cross_source(input)
        .iter()
        .map(|p| p.url.clone())
        .collect();
    assert_eq!(
        urls,
        vec![
            "https://acme.example/jobs/1",
            "https://acme.example/jobs/2",
            "https://acme.example/jobs/3",
        ],
        "distinct jobs must all be kept in first-seen order"
    );
}

#[test]
fn dedup_cross_source_collapses_urlless_by_title_and_company() {
    // URL-less postings fall back to title+company; same title+company (case/edge-
    // whitespace insensitive) collapses; description upgrades to the richer one,
    // but incumbent ("x") identity is kept.
    let input = vec![
        dedup_posting("x", "", " Staff Engineer ", "Acme", None),
        dedup_posting("y", "   ", "staff engineer", "  ACME", Some("desc")),
    ];
    let out = dedup_cross_source(input);
    assert_eq!(out.len(), 1, "same title+company with no URL must collapse");
    assert_eq!(
        out[0].source, "x",
        "incumbent identity is kept even when the challenger's description wins"
    );
    assert_eq!(
        out[0].description.as_deref(),
        Some("desc"),
        "description upgrades to the challenger's non-empty text"
    );
}

/// Engine seam — two boards return the SAME job (same canonical URL up to
/// www/tracking): the aggregated result collapses to ONE posting, keeping the
/// FIRST board's identity (id/url/source) while upgrading its description to
/// the richer, full-text one from its later duplicate — while each board's
/// `BoardScrapeSummary.count` stays as-fetched (1 each) — per-board counts
/// describe raw returns, not the deduped set.
#[tokio::test]
async fn scrape_boards_collapses_cross_source_duplicates_keeping_richer() {
    struct OneJobScraper {
        url: &'static str,
        description: &'static str,
        source: &'static str,
    }

    #[async_trait::async_trait]
    impl Scraper for OneJobScraper {
        fn id(&self) -> &'static str {
            "onejob"
        }
        fn display_name(&self) -> &'static str {
            "OneJob"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            let job = JobPosting {
                id: format!("{}:1", self.source),
                external_id: Some("1".to_string()),
                title: "Staff Engineer".to_string(),
                company: "Acme".to_string(),
                location: None,
                url: self.url.to_string(),
                source: self.source.to_string(),
                description: Some(self.description.to_string()),
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra: std::collections::HashMap::new(),
            };
            if let Some(ref on_item) = ctx.on_item {
                on_item(job.clone());
            }
            Ok(vec![job])
        }
    }

    // "agg" first with a truncated snippet + tracking params; "board" second with
    // the same canonical URL (www + no query) and the FULL description.
    static AGG: std::sync::LazyLock<OneJobScraper> = std::sync::LazyLock::new(|| OneJobScraper {
        url: "https://www.acme.example/jobs/42?utm_source=agg",
        description: "short snippet",
        source: "aggregator",
    });
    static BOARD: std::sync::LazyLock<OneJobScraper> = std::sync::LazyLock::new(|| OneJobScraper {
        url: "https://acme.example/jobs/42",
        description: "full description with many more characters than the snippet",
        source: "board",
    });

    let engine = ScraperEngine::new();
    let boards = vec!["agg-board".to_string(), "board-board".to_string()];
    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(10),
            "job-trust-e-dedup".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "agg-board" => Ok(&*AGG as &'static dyn Scraper),
                "board-board" => Ok(&*BOARD as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("cross-source dedup run must be Ok");

    // Per-board counts stay as-fetched: each board returned exactly 1.
    assert_eq!(summaries.len(), 2, "one summary per board");
    for s in &summaries {
        assert_eq!(
            s.count, 1,
            "per-board count must stay as-fetched (pre-dedup); got {s:?}"
        );
        assert!(s.error.is_none() && s.skipped.is_none());
    }

    // The aggregated result collapses the duplicate to ONE posting...
    assert_eq!(
        postings.len(),
        1,
        "the same job from two boards must collapse to one aggregated row"
    );
    // ...the FIRST board's identity is kept (never overwritten by a later
    // duplicate)...
    assert_eq!(
        postings[0].source, "aggregator",
        "incumbent (first-seen) board identity must be kept, not swapped for the challenger's"
    );
    // ...but its description is upgraded to the richer, full text.
    assert_eq!(
        postings[0].description.as_deref(),
        Some("full description with many more characters than the snippet"),
        "the surviving row's description must upgrade to the full text, not stay truncated"
    );
}

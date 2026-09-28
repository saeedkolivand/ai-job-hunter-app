//! `needs-company` skip: an ATS-requiring board with no usable company slug
//! is skipped before any fetch; whitespace-only slugs count as empty; a
//! non-ATS board is never skipped for this reason.

use super::panickers::AtsCompanyPanicker;
use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

// ── needs-company skip ────────────────────────────────────────────────────────

/// An ATS board (requires_company=true) with no companies in the input must be
/// skipped with `skipped=Some("needs-company")` and its `search` must never be
/// called — identical structure to the `needs-login` panicker tests.
#[tokio::test]
async fn ats_board_without_companies_is_skipped() {
    static ATS: std::sync::LazyLock<AtsCompanyPanicker> =
        std::sync::LazyLock::new(|| AtsCompanyPanicker::new("ats-needs-panicker"));
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
    static ATS_WS: std::sync::LazyLock<AtsCompanyPanicker> =
        std::sync::LazyLock::new(|| AtsCompanyPanicker::new("ats-whitespace-panicker"));

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

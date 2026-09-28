//! ATS per-company partial-failure isolation: one board's error does not
//! affect a sibling board's own seeded-company run.

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

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

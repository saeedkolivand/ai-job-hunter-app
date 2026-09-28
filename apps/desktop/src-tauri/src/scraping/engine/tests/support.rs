//! Shared fixtures for the engine test topics: fake `Scraper` impls
//! (streaming/uncancellable/failing/seed-capturing) plus the browser-
//! semaphore and `BoardSearchInput` builders every topic composes from.

use std::sync::Arc;

use tokio::sync::Semaphore as TokioSemaphore;

use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, ScraperMode};

pub(super) fn test_browser_sem() -> Arc<TokioSemaphore> {
    Arc::new(TokioSemaphore::new(1))
}

// ── Fake scrapers ─────────────────────────────────────────────────────────────

/// Fake board that streams `count` items through `ctx.on_item`, stopping early
/// when `ctx.signal` is cancelled (mimicking a real pagination loop), and returns
/// the same Vec it streamed. Used to drive the engine's central `amount` cap.
pub(super) struct FakeScraper {
    pub(super) count: usize,
    /// Overrides the scraper mode so we can fake a browser board.
    mode: ScraperMode,
}

impl FakeScraper {
    pub(super) fn http(count: usize) -> Self {
        Self {
            count,
            mode: ScraperMode::Http,
        }
    }

    pub(super) fn browser(count: usize) -> Self {
        Self {
            count,
            mode: ScraperMode::Browser,
        }
    }
}

#[async_trait::async_trait]
impl crate::scraping::types::Scraper for FakeScraper {
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
pub(super) struct UncancellableScraper {
    pub(super) count: usize,
}

#[async_trait::async_trait]
impl crate::scraping::types::Scraper for UncancellableScraper {
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
pub(super) struct FailingScraper;

#[async_trait::async_trait]
impl crate::scraping::types::Scraper for FailingScraper {
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

/// Fake board that captures the `input.companies` it actually receives at
/// `search()` time, with a configurable `id()`/`requires_company()` — used to
/// prove the `ats_seed` auto-population targets exactly ATS-requiring boards
/// by `Scraper::id()`, and leaves everything else untouched.
pub(super) struct SeedCapturingScraper {
    board_id: &'static str,
    ats_board: bool,
    pub(super) captured: std::sync::Mutex<Option<Vec<String>>>,
}

impl SeedCapturingScraper {
    pub(super) fn ats(board_id: &'static str) -> Self {
        Self {
            board_id,
            ats_board: true,
            captured: std::sync::Mutex::new(None),
        }
    }
    pub(super) fn non_ats(board_id: &'static str) -> Self {
        Self {
            board_id,
            ats_board: false,
            captured: std::sync::Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl crate::scraping::types::Scraper for SeedCapturingScraper {
    fn id(&self) -> &'static str {
        self.board_id
    }
    fn display_name(&self) -> &'static str {
        "SeedCapturing"
    }
    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }
    fn requires_company(&self) -> bool {
        self.ats_board
    }
    async fn search(
        &self,
        input: BoardSearchInput,
        _ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        *self.captured.lock().unwrap() = Some(input.companies);
        Ok(Vec::new())
    }
}

pub(super) fn fake_input(amount: u32) -> BoardSearchInput {
    BoardSearchInput {
        query: "q".to_string(),
        location: None,
        amount,
        pages: 10,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: Vec::new(),
    }
}

/// A non-work-type-supporting board (default `supports_work_type() == false`)
/// streaming a mix of declared-remote, declared-on-site, declared-hybrid and
/// undeclared rows. Reused by several tests below.
pub(super) struct WorkTypeFake;
#[async_trait::async_trait]
impl crate::scraping::types::Scraper for WorkTypeFake {
    fn id(&self) -> &'static str {
        "wtfake"
    }
    fn display_name(&self) -> &'static str {
        "WorkTypeFake"
    }
    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }
    async fn search(
        &self,
        _input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let rows: [(&str, Option<&str>); 4] = [
            ("keep-remote", Some("remote")),
            ("drop-onsite", Some("on-site")),
            ("keep-unknown", None), // undeclared → Unknown → always kept
            ("drop-hybrid", Some("hybrid")),
        ];
        let mut out = Vec::new();
        for (slug, work_type) in rows {
            let mut extra = std::collections::HashMap::new();
            if let Some(wt) = work_type {
                extra.insert("workType".to_string(), serde_json::json!(wt));
            }
            let job = JobPosting {
                id: format!("wtfake:{slug}"),
                external_id: Some(slug.to_string()),
                title: "Job".to_string(),
                company: "WT".to_string(),
                location: None,
                url: format!("https://wt.example/{slug}"),
                source: "wtfake".to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra,
            };
            if let Some(ref on_item) = ctx.on_item {
                on_item(job.clone());
            }
            out.push(job);
        }
        Ok(out)
    }
}

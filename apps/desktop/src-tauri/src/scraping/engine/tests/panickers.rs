//! Shared "must never be searched" panicker fixtures for the
//! `*_skip` engine test topics — a scraper whose `search()` panics,
//! proving a skip fires BEFORE any fetch. One struct per skip
//! predicate (`Required` auth / `needs_keys` / `requires_company`);
//! `id()` is caller-supplied so a test can run several distinct
//! instances.

use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, ScraperMode};

/// A `Required`-auth scraper whose `search()` panics — proves a skip
/// short-circuits BEFORE any fetch. `id()` is caller-supplied so a test can
/// run several distinct instances that must never be searched.
pub(super) struct RequiredPanicker {
    board_id: &'static str,
}

impl RequiredPanicker {
    pub(super) fn new(board_id: &'static str) -> Self {
        Self { board_id }
    }
}

#[async_trait::async_trait]
impl crate::scraping::types::Scraper for RequiredPanicker {
    fn id(&self) -> &'static str {
        self.board_id
    }
    fn display_name(&self) -> &'static str {
        "RequiredPanicker"
    }
    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }
    fn auth(&self) -> crate::scraping::types::AuthRequirement {
        crate::scraping::types::AuthRequirement::Required
    }
    async fn search(
        &self,
        _input: BoardSearchInput,
        _ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        panic!(
            "RequiredPanicker({}).search must never be called when skipped",
            self.board_id
        );
    }
}

/// Sibling of [`RequiredPanicker`] for the `needs-keys` skip predicate
/// (`needs_keys() == true`) instead of `Required` auth.
pub(super) struct NeedsKeysPanicker {
    board_id: &'static str,
}

impl NeedsKeysPanicker {
    pub(super) fn new(board_id: &'static str) -> Self {
        Self { board_id }
    }
}

#[async_trait::async_trait]
impl crate::scraping::types::Scraper for NeedsKeysPanicker {
    fn id(&self) -> &'static str {
        self.board_id
    }
    fn display_name(&self) -> &'static str {
        "NeedsKeysPanicker"
    }
    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }
    fn needs_keys(&self) -> bool {
        true
    }
    async fn search(
        &self,
        _input: BoardSearchInput,
        _ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        panic!(
            "NeedsKeysPanicker({}).search must never be called when skipped",
            self.board_id
        );
    }
}

/// Sibling of [`RequiredPanicker`] for the `needs-company` skip predicate
/// (`requires_company() == true`) instead of `Required` auth.
pub(super) struct AtsCompanyPanicker {
    board_id: &'static str,
}

impl AtsCompanyPanicker {
    pub(super) fn new(board_id: &'static str) -> Self {
        Self { board_id }
    }
}

#[async_trait::async_trait]
impl crate::scraping::types::Scraper for AtsCompanyPanicker {
    fn id(&self) -> &'static str {
        self.board_id
    }
    fn display_name(&self) -> &'static str {
        "AtsCompanyPanicker"
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
        panic!(
            "AtsCompanyPanicker({}).search must never be called when skipped",
            self.board_id
        );
    }
}

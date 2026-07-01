/// Shared types for scraping operations.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPosting {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "externalId")]
    pub external_id: Option<String>,
    pub title: String,
    pub company: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub url: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "postedAt")]
    pub posted_at: Option<i64>,
    #[serde(rename = "capturedAt")]
    pub captured_at: i64,
    /// Board-specific metadata (salary, remote status, etc.)
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct BoardSearchInput {
    pub query: String,
    pub location: Option<String>,
    /// Target number of postings to collect (the user's requested item count,
    /// clamped to 100). The engine caps streamed/returned items to this centrally.
    pub amount: u32,
    /// Per-board page request BUDGET. Each board clamps this down to its own max
    /// page count; it bounds how many requests we make, independent of `amount`.
    pub pages: u32,
    pub date_filter: Option<String>,
    pub job_type: Option<String>, // 'F' (Full-time), 'P' (Part-time), etc.
    pub work_type: Option<String>, // '1' (On-site), '2' (Remote), '3' (Hybrid)
    pub experience_level: Option<String>,
    pub easy_apply: Option<bool>,
    pub actively_hiring: Option<bool>,
    pub verified: Option<bool>,
    pub sort_by: Option<String>, // 'DD' (Date Descending), 'R' (Relevance)
    // Structured location from a picked geocode suggestion (#49/#40). Boards that
    // support precise geo filtering (e.g. LinkedIn geoId + distance) use these;
    // boards without geo filtering ignore them.
    pub country_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_km: Option<u32>,
    /// Company / board identifiers for ATS boards (greenhouse, lever, ashby,
    /// recruitee, personio, smartrecruiters) whose public APIs require a company
    /// slug instead of a global keyword search. Empty = no company filter; only
    /// those ATS boards read it, every other board ignores it.
    pub companies: Vec<String>,
}

pub struct ScrapeContext {
    pub signal: tokio_util::sync::CancellationToken,
    pub on_progress: Option<Box<dyn Fn(f32) + Send>>,
    pub on_item: Option<Box<dyn Fn(JobPosting) + Send>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScraperMode {
    Http,
    Browser,
}

/// Whether a board can be scraped without a logged-in session. Serializes to
/// `"guest" | "optional" | "required"` for the renderer's manual jobs picker.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthRequirement {
    /// No login needed — guest scraping returns full results.
    Guest,
    /// Guest scraping works; logging in enriches results.
    Optional,
    /// Guest scraping returns ~nothing — login is mandatory.
    Required,
}

#[allow(dead_code)]
#[async_trait]
pub trait Scraper: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn mode(&self) -> ScraperMode;

    /// Auth tier for this board. Defaults to [`AuthRequirement::Guest`] so a
    /// board only overrides this when it needs a session.
    fn auth(&self) -> AuthRequirement {
        AuthRequirement::Guest
    }

    /// Whether the board appears in the manual jobs picker. Defaults to `true`;
    /// a board overrides to `false` to stay registered (dispatchable) but hidden.
    fn listed(&self) -> bool {
        true
    }

    /// Whether this board requires a company slug to return any results.
    ///
    /// ATS platforms (Greenhouse, Lever, Ashby, Recruitee, Personio,
    /// SmartRecruiters) have no global keyword search — their public APIs only
    /// accept a per-company slug. Boards that return `true` here are skipped by
    /// the engine with reason `"needs-company"` when `input.companies` is empty,
    /// instead of making a wasted network call that would return nothing.
    ///
    /// Defaults to `false` so only the 6 ATS boards need to override this.
    fn requires_company(&self) -> bool {
        false
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> Result<Vec<JobPosting>, anyhow::Error>;

    /// Reads as "scrape *from* a URL" — not a constructor, so the `&self` receiver
    /// is intentional despite the `from_` prefix.
    #[allow(clippy::wrong_self_convention)]
    async fn from_url(
        &self,
        _url: &str,
        _ctx: ScrapeContext,
    ) -> Result<Option<JobPosting>, anyhow::Error> {
        Ok(None)
    }
}

#[cfg(test)]
mod test;

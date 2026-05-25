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
    pub pages: u32,
    pub date_filter: Option<String>,
    pub job_type: Option<String>, // 'F' (Full-time), 'P' (Part-time), etc.
    pub work_type: Option<String>, // '1' (On-site), '2' (Remote), '3' (Hybrid)
    pub experience_level: Option<String>,
    pub easy_apply: Option<bool>,
    pub actively_hiring: Option<bool>,
    pub verified: Option<bool>,
    pub sort_by: Option<String>, // 'DD' (Date Descending), 'R' (Relevance)
    pub locale: Option<String>,
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

#[allow(dead_code)]
#[async_trait]
pub trait Scraper: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn mode(&self) -> ScraperMode;

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> Result<Vec<JobPosting>, anyhow::Error>;

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

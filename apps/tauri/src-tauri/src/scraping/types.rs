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
mod tests {
    use super::*;

    #[test]
    fn test_job_posting_creation() {
        let posting = JobPosting {
            id: "test:123".to_string(),
            external_id: Some("123".to_string()),
            title: "Software Engineer".to_string(),
            company: "Test Corp".to_string(),
            location: Some("Berlin".to_string()),
            url: "https://example.com/job/123".to_string(),
            source: "test".to_string(),
            description: Some("Test description".to_string()),
            requirements: Some(vec!["Rust".to_string(), "TypeScript".to_string()]),
            posted_at: Some(1234567890),
            captured_at: 9876543210,
            extra: std::collections::HashMap::new(),
        };
        assert_eq!(posting.id, "test:123");
        assert_eq!(posting.title, "Software Engineer");
    }

    #[test]
    fn test_job_posting_defaults() {
        let posting = JobPosting {
            id: "test:123".to_string(),
            external_id: None,
            title: "Software Engineer".to_string(),
            company: "Test Corp".to_string(),
            location: None,
            url: "https://example.com/job/123".to_string(),
            source: "test".to_string(),
            description: None,
            requirements: None,
            posted_at: None,
            captured_at: 9876543210,
            extra: std::collections::HashMap::new(),
        };
        assert!(posting.external_id.is_none());
        assert!(posting.location.is_none());
    }

    #[test]
    fn test_job_posting_clone() {
        let posting = JobPosting {
            id: "test:123".to_string(),
            external_id: Some("123".to_string()),
            title: "Software Engineer".to_string(),
            company: "Test Corp".to_string(),
            location: Some("Berlin".to_string()),
            url: "https://example.com/job/123".to_string(),
            source: "test".to_string(),
            description: None,
            requirements: None,
            posted_at: None,
            captured_at: 9876543210,
            extra: std::collections::HashMap::new(),
        };
        let cloned = posting.clone();
        assert_eq!(posting.id, cloned.id);
        assert_eq!(posting.title, cloned.title);
    }

    #[test]
    fn test_board_search_input_creation() {
        let input = BoardSearchInput {
            query: "Software Engineer".to_string(),
            location: Some("Berlin".to_string()),
            pages: 5,
            date_filter: Some("7d".to_string()),
            job_type: Some("F".to_string()),
            work_type: Some("2".to_string()),
            experience_level: Some("2".to_string()),
            easy_apply: Some(true),
            actively_hiring: Some(true),
            verified: Some(true),
            sort_by: Some("DD".to_string()),
            locale: Some("de".to_string()),
        };
        assert_eq!(input.query, "Software Engineer");
        assert_eq!(input.pages, 5);
    }

    #[test]
    fn test_board_search_input_defaults() {
        let input = BoardSearchInput {
            query: "Software Engineer".to_string(),
            location: None,
            pages: 1,
            date_filter: None,
            job_type: None,
            work_type: None,
            experience_level: None,
            easy_apply: None,
            actively_hiring: None,
            verified: None,
            sort_by: None,
            locale: None,
        };
        assert!(input.location.is_none());
        assert_eq!(input.pages, 1);
    }

    #[test]
    fn test_scraper_mode_http() {
        let mode = ScraperMode::Http;
        assert_eq!(mode, ScraperMode::Http);
        assert_ne!(mode, ScraperMode::Browser);
    }

    #[test]
    fn test_scraper_mode_browser() {
        let mode = ScraperMode::Browser;
        assert_eq!(mode, ScraperMode::Browser);
        assert_ne!(mode, ScraperMode::Http);
    }

    #[test]
    fn test_scraper_mode_copy() {
        let mode = ScraperMode::Http;
        let copied = mode;
        assert_eq!(mode, copied);
    }

    #[test]
    fn test_scrape_context_creation() {
        let ctx = ScrapeContext {
            signal: tokio_util::sync::CancellationToken::new(),
            on_progress: None,
            on_item: None,
        };
        assert!(!ctx.signal.is_cancelled());
    }

    #[test]
    fn test_scrape_context_with_callbacks() {
        let ctx = ScrapeContext {
            signal: tokio_util::sync::CancellationToken::new(),
            on_progress: Some(Box::new(|_| {})),
            on_item: Some(Box::new(|_| {})),
        };
        assert!(ctx.on_progress.is_some());
        assert!(ctx.on_item.is_some());
    }
}

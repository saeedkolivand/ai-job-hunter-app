#![allow(dead_code)]

/// Greenhouse — public per-company JSON board API
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Location {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    name: String,
    value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct Job {
    id: i64,
    title: String,
    #[serde(rename = "absolute_url")]
    absolute_url: String,
    location: Location,
    content: Option<String>,
    #[serde(rename = "updated_at")]
    updated_at: Option<String>,
    metadata: Option<Vec<Metadata>>,
}

#[derive(Debug, Deserialize)]
struct GhJobsResponse {
    jobs: Vec<Job>,
}

pub struct GreenhouseScraper;

#[async_trait]
impl Scraper for GreenhouseScraper {
    fn id(&self) -> &'static str {
        "greenhouse"
    }

    fn display_name(&self) -> &'static str {
        "Greenhouse"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let company = input.query.trim();
        if company.is_empty() {
            return Ok(vec![]);
        }

        let url = format!(
            "https://boards-api.greenhouse.io/v1/boards/{}/jobs?content=true",
            urlencoding::encode(company)
        );

        let data = fetch_json::<GhJobsResponse>(&url, Default::default(), ctx.signal).await?;

        let jobs = match data {
            Some(d) => d.jobs,
            None => return Ok(vec![]),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for j in jobs {
            let description = j.content.map(|c| strip_html(&c));
            let posted_at = j.updated_at.and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok()).map(|dt| dt.timestamp_millis());

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), j.id),
                external_id: Some(j.id.to_string()),
                title: j.title,
                company: company.to_string(),
                location: Some(j.location.name),
                url: j.absolute_url,
                source: self.id().to_string(),
                description,
                requirements: None,
                posted_at,
                captured_at: now,
                extra: std::collections::HashMap::new(),
            };

            if let Some(ref on_item) = ctx.on_item {
                on_item(posting.clone());
            }

            out.push(posting);
        }

        if let Some(ref on_progress) = ctx.on_progress {
            on_progress(1.0);
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greenhouse_scraper_id() {
        let scraper = GreenhouseScraper;
        assert_eq!(scraper.id(), "greenhouse");
    }

    #[test]
    fn test_greenhouse_scraper_display_name() {
        let scraper = GreenhouseScraper;
        assert_eq!(scraper.display_name(), "Greenhouse");
    }

    #[test]
    fn test_greenhouse_scraper_mode() {
        let scraper = GreenhouseScraper;
        assert_eq!(scraper.mode(), ScraperMode::Http);
    }
}

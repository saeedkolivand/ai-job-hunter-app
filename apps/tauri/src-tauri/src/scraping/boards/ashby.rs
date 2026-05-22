#![allow(dead_code)]

/// Ashby — public posting API
use super::super::http::fetch_json;
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Job {
    id: String,
    title: String,
    #[serde(rename = "departmentName")]
    department_name: Option<String>,
    #[serde(rename = "teamName")]
    team_name: Option<String>,
    #[serde(rename = "locationName")]
    location_name: Option<String>,
    #[serde(rename = "isRemote")]
    is_remote: Option<bool>,
    #[serde(rename = "jobUrl")]
    job_url: String,
    #[serde(rename = "descriptionPlain")]
    description_plain: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AshbyResponse {
    #[serde(rename = "apiVersion")]
    api_version: String,
    jobs: Vec<Job>,
}

pub struct AshbyScraper;

#[async_trait]
impl Scraper for AshbyScraper {
    fn id(&self) -> &'static str {
        "ashby"
    }

    fn display_name(&self) -> &'static str {
        "Ashby"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let board = input.query.trim();
        if board.is_empty() {
            return Ok(vec![]);
        }

        let url = format!(
            "https://api.ashbyhq.com/posting-api/job-board/{}?includeCompensation=true",
            urlencoding::encode(board)
        );

        let data = fetch_json::<AshbyResponse>(&url, Default::default(), ctx.signal).await?;

        let jobs = match data {
            Some(d) => d.jobs,
            None => return Ok(vec![]),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for j in jobs {
            let posted_at = j.published_at.and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok()).map(|dt| dt.timestamp_millis());

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), j.id),
                external_id: Some(j.id.clone()),
                title: j.title,
                company: board.to_string(),
                location: j.location_name,
                url: j.job_url,
                source: self.id().to_string(),
                description: j.description_plain,
                requirements: None,
                posted_at,
                captured_at: now,
                extra: {
                    let mut map = std::collections::HashMap::new();
                    if let Some(remote) = j.is_remote {
                        map.insert("remote".to_string(), serde_json::json!(remote));
                    }
                    map
                },
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

/// Remotive — public JSON API
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Job {
    id: i64,
    url: String,
    title: String,
    #[serde(rename = "company_name")]
    company_name: String,
    #[serde(rename = "candidate_required_location")]
    candidate_required_location: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
    #[serde(rename = "publication_date")]
    publication_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Resp {
    jobs: Vec<Job>,
}

pub struct RemotiveScraper;

#[async_trait]
impl Scraper for RemotiveScraper {
    fn id(&self) -> &'static str {
        "remotive"
    }

    fn display_name(&self) -> &'static str {
        "Remotive"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let q = input.query.trim();
        let url = if q.is_empty() {
            "https://remotive.com/api/remote-jobs".to_string()
        } else {
            format!("https://remotive.com/api/remote-jobs?search={}", urlencoding::encode(q))
        };

        let data = fetch_json::<Resp>(&url, Default::default(), ctx.signal).await?;

        let jobs = match data {
            Some(d) => d.jobs,
            None => return Ok(vec![]),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for j in jobs {
            let posting = JobPosting {
                id: format!("{}:{}", self.id(), j.id),
                external_id: Some(j.id.to_string()),
                title: j.title,
                company: j.company_name,
                location: j.candidate_required_location,
                url: j.url,
                source: self.id().to_string(),
                description: j.description.map(|d| strip_html(&d)),
                requirements: j.tags,
                posted_at: j.publication_date.and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok()).map(|dt| dt.timestamp_millis()),
                captured_at: now,
                extra: {
                    let mut map = std::collections::HashMap::new();
                    map.insert("remote".to_string(), serde_json::json!(true));
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

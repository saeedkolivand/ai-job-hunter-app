/// Y Combinator Jobs (Work at a Startup) — public Algolia search.
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

const ALGOLIA_APP_ID: &str = "45BWZJ1SGC";
const ALGOLIA_API_KEY: &str =
    "NDYyYmNmMDU5OWVmNzNlNzMwMWQ5MDE4ZWY3M2NlNDU0NjA5MTRmZTdiNDAxYjE3MTUyYmU5OWZlNjVmZmUyZHRhZ0ZpbHRlcnM9JTVCJTIyam9icyUyMiU1RA==";

#[derive(Debug, Deserialize)]
struct Hit {
    #[serde(rename = "objectID")]
    object_id: String,
    title: Option<String>,
    description: Option<String>,
    #[serde(rename = "company_name")]
    company_name: Option<String>,
    location: Option<String>,
    remote: Option<bool>,
    #[serde(rename = "apply_url")]
    apply_url: Option<String>,
    #[serde(rename = "created_at_i")]
    created_at_i: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AlgoliaResp {
    hits: Vec<Hit>,
}

pub struct YCombinatorScraper;

#[async_trait]
impl Scraper for YCombinatorScraper {
    fn id(&self) -> &'static str {
        "ycombinator"
    }

    fn display_name(&self) -> &'static str {
        "Y Combinator"
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
        let url = format!(
            "https://{}-dsn.algolia.net/1/indexes/JobPosting/query",
            ALGOLIA_APP_ID.to_lowercase()
        );

        let data = fetch_json::<AlgoliaResp>(
            &url,
            super::super::http::FetchOptions {
                method: Some(reqwest::Method::POST),
                headers: Some(vec![
                    ("x-algolia-application-id".to_string(), ALGOLIA_APP_ID.to_string()),
                    ("x-algolia-api-key".to_string(), ALGOLIA_API_KEY.to_string()),
                    ("content-type".to_string(), "application/json".to_string()),
                ]),
                body: Some(serde_json::json!({ "query": q, "hitsPerPage": 50 }).to_string()),
                ..Default::default()
            },
            ctx.signal,
        )
        .await?;

        let hits = match data {
            Some(d) => d.hits,
            None => return Ok(vec![]),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for h in hits {
            if h.title.is_none() {
                continue;
            }

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), h.object_id),
                external_id: Some(h.object_id.clone()),
                title: h.title.unwrap_or_default(),
                company: h.company_name.unwrap_or_else(|| "Unknown".to_string()),
                location: h.location,
                url: h.apply_url.unwrap_or_else(|| {
                    format!("https://www.workatastartup.com/jobs/{}", h.object_id)
                }),
                source: self.id().to_string(),
                description: h.description.map(|d| strip_html(&d)),
                requirements: None,
                posted_at: h.created_at_i.map(|t| t * 1000),
                captured_at: now,
                extra: {
                    let mut map = std::collections::HashMap::new();
                    if let Some(remote) = h.remote {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ycombinator_scraper_id() {
        let scraper = YCombinatorScraper;
        assert_eq!(scraper.id(), "ycombinator");
    }

    #[test]
    fn test_ycombinator_scraper_display_name() {
        let scraper = YCombinatorScraper;
        assert_eq!(scraper.display_name(), "Y Combinator");
    }

    #[test]
    fn test_ycombinator_scraper_mode() {
        let scraper = YCombinatorScraper;
        assert_eq!(scraper.mode(), ScraperMode::Http);
    }
}

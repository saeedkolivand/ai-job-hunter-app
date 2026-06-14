/// Lever — public JSON board API
use super::super::http::fetch_json;
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Categories {
    location: Option<String>,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    team: Option<String>,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    commitment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LeverPosting {
    id: String,
    text: String,
    #[serde(rename = "hostedUrl")]
    hosted_url: String,
    categories: Option<Categories>,
    #[serde(rename = "descriptionPlain")]
    description_plain: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<i64>,
}

pub struct LeverScraper;

#[async_trait]
impl Scraper for LeverScraper {
    fn id(&self) -> &'static str {
        "lever"
    }

    fn display_name(&self) -> &'static str {
        "Lever"
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
            "https://api.lever.co/v0/postings/{}?mode=json",
            urlencoding::encode(company)
        );

        let data = fetch_json::<Vec<LeverPosting>>(&url, Default::default(), ctx.signal).await?;

        let postings = match data {
            Some(d) => d,
            None => return Ok(vec![]),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for p in postings {
            let posting = JobPosting {
                id: format!("{}:{}", self.id(), p.id),
                external_id: Some(p.id.clone()),
                title: p.text,
                company: company.to_string(),
                location: p.categories.as_ref().and_then(|c| c.location.clone()),
                url: p.hosted_url,
                source: self.id().to_string(),
                description: p.description_plain,
                requirements: None,
                posted_at: p.created_at.map(|t| t * 1000),
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
mod test;

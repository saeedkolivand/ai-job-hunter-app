#![allow(dead_code)]

/// Recruitee — public per-company offers API
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Offer {
    id: i64,
    slug: String,
    title: String,
    description: Option<String>,
    requirements: Option<String>,
    #[serde(rename = "careers_url")]
    careers_url: String,
    city: Option<String>,
    country: Option<String>,
    remote: Option<bool>,
    #[serde(rename = "created_at")]
    created_at: Option<String>,
    #[serde(rename = "company_name")]
    company_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Resp {
    offers: Vec<Offer>,
}

pub struct RecruiteeScraper;

#[async_trait]
impl Scraper for RecruiteeScraper {
    fn id(&self) -> &'static str {
        "recruitee"
    }

    fn display_name(&self) -> &'static str {
        "Recruitee"
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

        let url = format!("https://{}.recruitee.com/api/offers/", urlencoding::encode(company));
        let data = fetch_json::<Resp>(&url, Default::default(), ctx.signal).await?;

        let offers = match data {
            Some(d) => d.offers,
            None => return Ok(vec![]),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for o in offers {
            let description = vec![
                o.description.as_deref().map(strip_html),
                o.requirements.as_deref().map(strip_html),
            ]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

            let location = vec![o.city.as_deref(), o.country.as_deref()]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ");

            let posted_at = o.created_at.and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok()).map(|dt| dt.timestamp_millis());

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), o.id),
                external_id: Some(o.id.to_string()),
                title: o.title,
                company: o.company_name.unwrap_or_else(|| company.to_string()),
                location: if location.is_empty() { None } else { Some(location) },
                url: o.careers_url,
                source: self.id().to_string(),
                description: if description.is_empty() { None } else { Some(description) },
                requirements: None,
                posted_at,
                captured_at: now,
                extra: {
                    let mut map = std::collections::HashMap::new();
                    if let Some(remote) = o.remote {
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
mod test;

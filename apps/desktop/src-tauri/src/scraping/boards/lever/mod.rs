/// Lever — public JSON board API
///
/// Endpoint: `https://api.lever.co/v0/postings/{company}?mode=json`
/// No global keyword search — requires a company slug. The engine skips this
/// board with `"needs-company"` when `input.companies` is empty.
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

/// Map a Lever `createdAt` value (already epoch-milliseconds) to the
/// `posted_at` field.  Exposed for testing so the test exercises this exact
/// function rather than a local re-implementation that could drift.
#[inline]
pub(crate) fn map_posted_at(created_at: Option<i64>) -> Option<i64> {
    created_at // createdAt is already epoch-milliseconds — pass through verbatim
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

    fn requires_company(&self) -> bool {
        true
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Engine skips us when companies is empty; guard defensively anyway.
        if input.companies.is_empty() {
            return Ok(vec![]);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];
        let total = input.companies.len();

        for (i, company) in input.companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let company = company.trim();
            if company.is_empty() {
                continue;
            }

            let url = format!(
                "https://api.lever.co/v0/postings/{}?mode=json",
                urlencoding::encode(company)
            );

            let data =
                match fetch_json::<Vec<LeverPosting>>(&url, Default::default(), ctx.signal.clone())
                    .await
                {
                    Ok(d) => d,
                    Err(e) => {
                        log::warn!("[lever] fetch failed for '{}': {e}", company);
                        if ctx.signal.is_cancelled() {
                            break;
                        }
                        continue;
                    }
                };

            let postings = match data {
                Some(d) => d,
                None => continue,
            };

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
                    posted_at: map_posted_at(p.created_at), // createdAt is already epoch-milliseconds
                    captured_at: now,
                    extra: std::collections::HashMap::new(),
                };

                if let Some(ref on_item) = ctx.on_item {
                    on_item(posting.clone());
                }

                out.push(posting);
            }

            if let Some(ref on_progress) = ctx.on_progress {
                on_progress((i + 1) as f32 / total as f32);
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

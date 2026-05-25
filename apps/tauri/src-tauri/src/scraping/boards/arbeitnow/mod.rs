/// Arbeitnow — public JSON API
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Job {
    slug: String,
    #[serde(rename = "company_name")]
    company_name: String,
    title: String,
    description: Option<String>,
    remote: Option<bool>,
    url: String,
    tags: Option<Vec<String>>,
    location: Option<String>,
    #[serde(rename = "created_at")]
    created_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct Links {
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Resp {
    data: Vec<Job>,
    links: Option<Links>,
}

pub struct ArbeitnowScraper;

#[async_trait]
impl Scraper for ArbeitnowScraper {
    fn id(&self) -> &'static str {
        "arbeitnow"
    }

    fn display_name(&self) -> &'static str {
        "Arbeitnow"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let q = input.query.trim().to_lowercase();
        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];
        let max_pages = input.pages.min(5).max(1);

        for page in 1..=max_pages {
            if ctx.signal.is_cancelled() {
                break;
            }

            let data = fetch_json::<Resp>(
                &format!("https://www.arbeitnow.com/api/job-board-api?page={}", page),
                Default::default(),
                ctx.signal.clone(),
            )
            .await?;

            let (jobs, has_next) = match data {
                Some(d) => (d.data, d.links.and_then(|l| l.next).is_some()),
                None => break,
            };

            if jobs.is_empty() {
                break;
            }

            for j in jobs {
                let haystack = format!(
                    "{} {} {}",
                    j.title,
                    j.company_name,
                    j.tags.as_ref().map(|t| t.join(" ")).unwrap_or_else(|| "".to_string())
                ).to_lowercase();

                if !q.is_empty() && !haystack.contains(&q) {
                    continue;
                }

                let posting = JobPosting {
                    id: format!("{}:{}", self.id(), j.slug),
                    external_id: Some(j.slug.clone()),
                    title: j.title,
                    company: j.company_name,
                    location: j.location,
                    url: j.url,
                    source: self.id().to_string(),
                    description: j.description.map(|d| strip_html(&d)),
                    requirements: j.tags,
                    posted_at: j.created_at.map(|t| t * 1000),
                    captured_at: now,
                    extra: {
                        let mut map = std::collections::HashMap::new();
                        if let Some(remote) = j.remote {
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
                on_progress(page as f32 / max_pages as f32);
            }

            if !has_next {
                break;
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

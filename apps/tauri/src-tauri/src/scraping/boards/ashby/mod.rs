/// Ashby — public posting API
///
/// Endpoint: `https://api.ashbyhq.com/posting-api/job-board/{company}?includeCompensation=true`
/// No global keyword search — requires a company slug. The engine skips this
/// board with `"needs-company"` when `input.companies` is empty.
use super::super::http::fetch_json;
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Job {
    id: String,
    title: String,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    #[serde(rename = "departmentName")]
    department_name: Option<String>,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
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
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
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
                "https://api.ashbyhq.com/posting-api/job-board/{}?includeCompensation=true",
                urlencoding::encode(company)
            );

            let data =
                match fetch_json::<AshbyResponse>(&url, Default::default(), ctx.signal.clone())
                    .await
                {
                    Ok(d) => d,
                    Err(e) => {
                        log::warn!("[ashby] fetch failed for '{}': {e}", company);
                        if ctx.signal.is_cancelled() {
                            break;
                        }
                        continue;
                    }
                };

            let jobs = match data {
                Some(d) => d.jobs,
                None => continue,
            };

            for j in jobs {
                let posted_at = j
                    .published_at
                    .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
                    .map(|dt| dt.timestamp_millis());

                let posting = JobPosting {
                    id: format!("{}:{}", self.id(), j.id),
                    external_id: Some(j.id.clone()),
                    title: j.title,
                    company: company.to_string(),
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
                on_progress((i + 1) as f32 / total as f32);
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

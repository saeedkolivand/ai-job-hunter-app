#![allow(dead_code)]

/// Workday — per-tenant CXS API
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct WorkdayJobPosting {
    title: String,
    #[serde(rename = "externalPath")]
    external_path: String,
    #[serde(rename = "locationsText")]
    locations_text: Option<String>,
    #[serde(rename = "postedOn")]
    posted_on: Option<String>,
    #[serde(rename = "bulletFields")]
    bullet_fields: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct JobsResp {
    #[serde(rename = "jobPostings")]
    job_postings: Vec<WorkdayJobPosting>,
    total: i64,
}

#[derive(Debug, Deserialize)]
struct JobPostingInfo {
    #[serde(rename = "jobDescription")]
    job_description: Option<String>,
    #[serde(rename = "jobPostingId")]
    job_posting_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DetailResp {
    #[serde(rename = "jobPostingInfo")]
    job_posting_info: Option<JobPostingInfo>,
}

pub struct WorkdayScraper;

impl WorkdayScraper {
    fn parse_target(&self, q: &str) -> Option<(String, String, String)> {
        let trimmed = q.trim();
        if trimmed.is_empty() {
            return None;
        }

        if trimmed.contains(':') {
            let parts: Vec<&str> = trimmed.split(':').collect();
            let tenant = parts.get(0).map(|s| s.to_string())?;
            let site = parts.get(1).map(|s| s.to_string())?;
            let host = parts.get(2).map(|s| s.to_string()).unwrap_or_else(|| "wd1".to_string());
            return Some((tenant, site, host));
        }

        let re = regex::Regex::new(r"^https?://([^.]+)\.(wd\d+)\.myworkdayjobs\.com/(?:[a-z-]+/)?([^/?#]+)").unwrap();
        if let Some(caps) = re.captures(trimmed) {
            let tenant = caps.get(1).map(|m| m.as_str().to_string())?;
            let host = caps.get(2).map(|m| m.as_str().to_string())?;
            let site = caps.get(3).map(|m| m.as_str().to_string())?;
            return Some((tenant, site, host));
        }

        None
    }
}

#[async_trait]
impl Scraper for WorkdayScraper {
    fn id(&self) -> &'static str {
        "workday"
    }

    fn display_name(&self) -> &'static str {
        "Workday"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let (tenant, site, host) = match self.parse_target(&input.query) {
            Some(parsed) => parsed,
            None => return Ok(vec![]),
        };

        let base = format!("https://{}.{}.myworkdayjobs.com/wday/cxs/{}/{}", tenant, host, tenant, site);
        let max_pages = input.pages.min(5).max(1);
        let limit = 20;
        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for p in 0..max_pages {
            if ctx.signal.is_cancelled() {
                break;
            }

            let body = serde_json::json!({
                "appliedFacets": {},
                "searchText": "",
                "limit": limit,
                "offset": p * limit
            });

            let data = fetch_json::<JobsResp>(
                &format!("{}/jobs", base),
                super::super::http::FetchOptions {
                    method: Some(reqwest::Method::POST),
                    headers: Some(vec![("content-type".to_string(), "application/json".to_string())]),
                    body: Some(body.to_string()),
                    ..Default::default()
                },
                ctx.signal.clone(),
            )
            .await?;

            let job_postings = match data {
                Some(d) => d.job_postings,
                None => break,
            };

            if job_postings.is_empty() {
                break;
            }

            for j in &job_postings {
                let external_id = j.external_path.split('/').last().unwrap_or("").to_string();
                if external_id.is_empty() {
                    continue;
                }

                let detail = fetch_json::<DetailResp>(
                    &format!("{}{}", base, j.external_path),
                    Default::default(),
                    ctx.signal.clone(),
                )
                .await?;

                let description = detail
                    .and_then(|d| d.job_posting_info)
                    .and_then(|info| info.job_description)
                    .map(|d| strip_html(&d));

                let posted_at = j.posted_on
                    .as_ref()
                    .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                    .map(|dt| dt.timestamp_millis());

                let posting = super::super::types::JobPosting {
                    id: format!("{}:{}", self.id(), external_id),
                    external_id: Some(external_id),
                    title: j.title.clone(),
                    company: tenant.clone(),
                    location: j.locations_text.clone(),
                    url: format!("https://{}.{}.myworkdayjobs.com/en-US/{}{}", tenant, host, site, j.external_path),
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
                on_progress((p + 1) as f32 / max_pages as f32);
            }

            if job_postings.len() < limit as usize {
                break;
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

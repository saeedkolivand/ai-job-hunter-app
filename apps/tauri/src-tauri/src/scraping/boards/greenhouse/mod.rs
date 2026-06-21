/// Greenhouse — public per-company JSON board API
///
/// Endpoint: `https://boards-api.greenhouse.io/v1/boards/{company}/jobs?content=true`
/// No global keyword search — requires a company slug. The engine skips this
/// board with `"needs-company"` when `input.companies` is empty.
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Location {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    name: String,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
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
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    metadata: Option<Vec<Metadata>>,
}

#[derive(Debug, Deserialize)]
struct GhJobsResponse {
    jobs: Vec<Job>,
}

/// Maximum number of company slugs processed per scrape call.
/// Prevents an unbounded number of outbound requests from a large IPC payload.
const MAX_COMPANIES: usize = 50;

/// Trim, drop blanks, dedupe (first-seen order), and cap to `max`.
/// Extracted so the normalisation logic can be unit-tested without network.
pub(crate) fn normalize_companies(input: &[String], max: usize) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    input
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .take(max)
        .collect()
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

        // Dedupe (first-seen order), drop blanks, and cap to MAX_COMPANIES so a
        // large IPC payload cannot fan out unbounded requests to Greenhouse.
        let companies = normalize_companies(&input.companies, MAX_COMPANIES);
        let total = companies.len();

        let mut successful_fetches = 0usize;
        let mut first_fetch_error: Option<String> = None;

        for (i, company) in companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let url = format!(
                "https://boards-api.greenhouse.io/v1/boards/{}/jobs?content=true",
                urlencoding::encode(company)
            );

            let data =
                match fetch_json::<GhJobsResponse>(&url, Default::default(), ctx.signal.clone())
                    .await
                {
                    Ok(d) => d,
                    Err(e) => {
                        // Check cancellation first: a fetch that failed because
                        // the run was cancelled is not a real board-level error.
                        if ctx.signal.is_cancelled() {
                            break;
                        }
                        log::warn!("[greenhouse] fetch failed for '{}': {e}", company);
                        first_fetch_error.get_or_insert_with(|| e.to_string());
                        if let Some(ref on_progress) = ctx.on_progress {
                            on_progress((i + 1) as f32 / total as f32);
                        }
                        continue;
                    }
                };

            let jobs = match data {
                Some(d) => {
                    successful_fetches += 1;
                    d.jobs
                }
                None => {
                    if let Some(ref on_progress) = ctx.on_progress {
                        on_progress((i + 1) as f32 / total as f32);
                    }
                    continue;
                }
            };

            for j in jobs {
                let description = j.content.map(|c| strip_html(&c));
                let posted_at = j
                    .updated_at
                    .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
                    .map(|dt| dt.timestamp_millis());

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
                on_progress((i + 1) as f32 / total as f32);
            }
        }

        // Return Err only when every attempt failed — partial success is kept.
        if successful_fetches == 0 {
            if let Some(error) = first_fetch_error {
                return Err(anyhow::anyhow!(
                    "all greenhouse company fetches failed: {error}"
                ));
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

/// Rippling ATS — public per-company board JSON
///
/// Endpoint: `https://api.rippling.com/platform/api/ats/v1/board/{slug}/jobs`
/// No global keyword search — requires a company slug. The engine skips this
/// board with `"needs-company"` when `input.companies` is empty.
///
/// Endpoint reconnaissance ported from santifer/career-ops (MIT), `providers/rippling.mjs`.
use super::super::http::fetch_json;
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use serde::Deserialize;

const BOARD_ID: &str = "rippling";

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

/// Validate the Rippling board slug (a URL path segment, not a hostname
/// label — so mixed case is allowed, unlike the DNS-label boards). Rejects
/// anything that could carry a path-traversal or query-string payload; the
/// slug is also percent-encoded before use, defence in depth.
fn is_valid_rippling_slug(slug: &str) -> bool {
    if slug.is_empty() || slug.len() > 63 {
        return false;
    }
    let bytes = slug.as_bytes();
    bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'-')
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
}

/// Validate that a job URL from the response is `https://ats.rippling.com/…`.
/// A drifting or hostile response could inject arbitrary URLs into
/// `JobPosting.url`; constrain it to the one host Rippling actually serves
/// job pages from.
fn is_valid_rippling_job_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .map(|u| u.scheme() == "https" && u.host_str() == Some("ats.rippling.com"))
        .unwrap_or(false)
}

#[derive(Debug, Deserialize)]
struct RpWorkLocationObj {
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RpWorkLocation {
    Object(RpWorkLocationObj),
    Text(String),
}

#[derive(Debug, Deserialize)]
pub(crate) struct RpJob {
    uuid: String,
    name: Option<String>,
    url: Option<String>,
    #[serde(rename = "workLocation")]
    work_location: Option<RpWorkLocation>,
}

/// Deserialize each row independently, dropping (with a debug log) any row
/// that fails — e.g. a missing `uuid` or a non-object/non-string
/// `workLocation`. Without this, `Vec<RpJob>`'s atomic deserialize would fail
/// the whole company on a single malformed row (silent zero-jobs).
pub(crate) fn rows_to_jobs(values: Vec<serde_json::Value>) -> Vec<RpJob> {
    values
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<RpJob>(v) {
            Ok(job) => Some(job),
            Err(e) => {
                log::debug!("[rippling] skipping malformed row: {e}");
                None
            }
        })
        .collect()
}

/// Map a parsed Rippling response into postings for one company. Standalone
/// (no `&self`) so it is unit-testable against a JSON fixture.
pub(crate) fn parse_rippling_response(
    jobs: Vec<RpJob>,
    company: &str,
    now: i64,
) -> Vec<JobPosting> {
    let mut out = Vec::new();

    for j in jobs {
        let title = j.name.unwrap_or_default().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let url = match j.url.as_deref() {
            Some(u) if is_valid_rippling_job_url(u) => u.to_string(),
            _ => continue,
        };

        let location = match j.work_location {
            Some(RpWorkLocation::Object(o)) => o
                .label
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            Some(RpWorkLocation::Text(t)) => {
                let t = t.trim();
                (!t.is_empty()).then(|| t.to_string())
            }
            None => None,
        };

        out.push(JobPosting {
            id: format!("{BOARD_ID}:{}", j.uuid),
            external_id: Some(j.uuid.clone()),
            title,
            company: company.to_string(),
            location,
            url,
            source: BOARD_ID.to_string(),
            description: None,
            requirements: None,
            posted_at: None,
            captured_at: now,
            extra: std::collections::HashMap::new(),
        });
    }

    out
}

pub struct RipplingScraper;

#[async_trait]
impl Scraper for RipplingScraper {
    fn id(&self) -> &'static str {
        BOARD_ID
    }

    fn display_name(&self) -> &'static str {
        "Rippling"
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
        // large IPC payload cannot fan out unbounded requests to Rippling.
        let companies = normalize_companies(&input.companies, MAX_COMPANIES);
        let total = companies.len();

        let mut successful_fetches = 0usize;
        let mut first_fetch_error: Option<String> = None;

        for (i, company) in companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            if !is_valid_rippling_slug(company) {
                log::warn!("[rippling] skipping invalid company slug '{}'", company);
                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress((i + 1) as f32 / total as f32);
                }
                continue;
            }

            let url = format!(
                "https://api.rippling.com/platform/api/ats/v1/board/{}/jobs",
                urlencoding::encode(company)
            );

            let data = match fetch_json::<Vec<serde_json::Value>>(
                &url,
                Default::default(),
                ctx.signal.clone(),
            )
            .await
            {
                Ok(d) => d,
                Err(e) => {
                    // Check cancellation first: a fetch that failed because
                    // the run was cancelled is not a real board-level error.
                    if ctx.signal.is_cancelled() {
                        break;
                    }
                    log::warn!("[rippling] fetch failed for '{}': {e}", company);
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
                    rows_to_jobs(d)
                }
                None => {
                    if let Some(ref on_progress) = ctx.on_progress {
                        on_progress((i + 1) as f32 / total as f32);
                    }
                    continue;
                }
            };

            for posting in parse_rippling_response(jobs, company, now) {
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
                    "all rippling company fetches failed: {error}"
                ));
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

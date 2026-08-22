//! Ashby — public posting API
//!
//! Endpoint: `https://api.ashbyhq.com/posting-api/job-board/{company}?includeCompensation=true`
//! No global keyword search — requires a company slug. The engine skips this
//! board with `"needs-company"` when `input.companies` is empty.
use super::super::engine::work_type_filter::parse_work_type;
use super::super::http::{fetch_json, FetchOptions};
use super::super::types::{
    BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode, WorkType,
};
use super::common::{ats_all_fetches_failed, ats_failed_fetches_note, normalize_companies};
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
    /// **Not a remote signal on its own** — measured live (`api.ashbyhq.com/posting-api/job-board/Ramp`,
    /// 136 jobs): `isRemote == true` for 107 Hybrid rows as well as 16 Remote rows, so a bare `true`
    /// means "Hybrid OR Remote", not "Remote". Kept only for the pre-existing `extra["remote"]`
    /// badge fallback; [`Job::workplace_type`] is the field that actually distinguishes the three.
    #[serde(rename = "isRemote")]
    is_remote: Option<bool>,
    /// The declared workplace arrangement — `"OnSite" | "Remote" | "Hybrid"`. Added because
    /// `isRemote` alone conflates Hybrid and Remote (see its doc comment).
    #[serde(rename = "workplaceType")]
    workplace_type: Option<String>,
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

/// Maximum number of company slugs processed per scrape call.
/// Prevents an unbounded number of outbound requests from a large IPC payload.
const MAX_COMPANIES: usize = 50;

/// Production Ashby API host. Factored into a constant (rather than inlined)
/// purely so tests can drive [`AshbyScraper::search_with_base`] against a local
/// `wiremock` base instead — mirrors Lever's `LEVER_BASE_URL`.
const ASHBY_BASE_URL: &str = "https://api.ashbyhq.com";

/// Fetch ONE company's whole board. Split out of the search loop so the
/// per-company request — including the raised byte cap — is drivable against a
/// mock host, which is what makes the partial-failure note testable without the
/// live API.
async fn fetch_ashby_company(
    base_url: &str,
    company: &str,
    signal: tokio_util::sync::CancellationToken,
) -> crate::error::AppResult<AshbyResponse> {
    let url = format!(
        "{base_url}/posting-api/job-board/{}?includeCompensation=true",
        urlencoding::encode(company)
    );

    let opts = FetchOptions {
        // One Ashby call returns a company's WHOLE board, so a large employer's
        // payload can exceed the 8 MB default guard (observed: openai →
        // "Response too large", i.e. a silently dropped company). Raise the
        // per-request cap only here.
        max_bytes: Some(16 * 1024 * 1024),
        ..Default::default()
    };

    fetch_json::<AshbyResponse>(&url, opts, signal).await
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
        self.search_with_base(ASHBY_BASE_URL, input, ctx).await
    }
}

impl AshbyScraper {
    /// The real search loop, parameterized by API base URL so tests can point it
    /// at a mock server ([`fetch_ashby_company`]). `search` supplies
    /// [`ASHBY_BASE_URL`]; nothing else calls this.
    async fn search_with_base(
        &self,
        base_url: &str,
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
        // large IPC payload cannot fan out unbounded requests to Ashby.
        let companies = normalize_companies(&input.companies, MAX_COMPANIES);
        let total = companies.len();

        let mut successful_fetches = 0usize;
        let mut failed_fetches = 0usize;
        let mut first_fetch_error: Option<String> = None;

        for (i, company) in companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let data = match fetch_ashby_company(base_url, company, ctx.signal.clone()).await {
                Ok(d) => d,
                Err(e) => {
                    // Check cancellation first: a fetch that failed because
                    // the run was cancelled is not a real board-level error.
                    if ctx.signal.is_cancelled() {
                        break;
                    }
                    log::warn!("[ashby] fetch failed for '{}': {e}", company);
                    failed_fetches += 1;
                    first_fetch_error.get_or_insert_with(|| e.to_string());
                    if let Some(ref on_progress) = ctx.on_progress {
                        on_progress((i + 1) as f32 / total as f32);
                    }
                    continue;
                }
            };

            // A non-2xx / schema-drift response is now an `Err` above (which records
            // `first_fetch_error`), so reaching here means a real success — count it.
            successful_fetches += 1;
            let jobs = data.jobs;

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
                        let work_type = j.workplace_type.as_deref().and_then(parse_work_type);
                        // `isRemote` conflates Hybrid and Remote (see the field's doc
                        // comment above) — it is a location-filter/badge signal, not a
                        // work-type one, so it must AGREE with the declared
                        // `workplaceType` rather than compete with it. Once
                        // `workplaceType` is present, trust ONLY it: write `remote`
                        // when it resolved to Remote, write nothing when it resolved to
                        // Hybrid/OnSite (regardless of `isRemote`), and fall back to the
                        // raw `isRemote` only when `workplaceType` itself is absent
                        // (older/odd tenants with no other signal at all).
                        let remote = match work_type {
                            Some(WorkType::Remote) => true,
                            Some(_) => false,
                            None => j.is_remote.unwrap_or(false),
                        };
                        if remote {
                            map.insert("remote".to_string(), serde_json::json!(true));
                        }
                        if let Some(wt) = work_type {
                            map.insert("workType".to_string(), serde_json::json!(wt));
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

        // A PARTIAL run (some companies fetched, some failed) still returns Ok, so
        // the failures would otherwise be log-only — surface the count as ONE
        // informational note. NOT gated on cancellation, for the reason spelled
        // out in `lever`'s copy: the engine cancels `ctx.signal` as soon as the
        // central `amount` cap fills, and `failed_fetches` only counts
        // non-cancellation errors anyway.
        if let Some(note) = ats_failed_fetches_note(successful_fetches, failed_fetches) {
            ctx.report_note(note);
        }

        // Return Err only when every attempt failed — see `ats_all_fetches_failed`.
        if let Some(message) =
            ats_all_fetches_failed(self.id(), successful_fetches, &first_fetch_error)
        {
            return Err(anyhow::anyhow!(message));
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

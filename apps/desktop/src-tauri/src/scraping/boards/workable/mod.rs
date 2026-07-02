//! Workable — public per-company careers-widget JSON
//!
//! Endpoint: `https://apply.workable.com/api/v1/widget/accounts/{slug}?details=true`
//! No global keyword search — requires a company slug. The engine skips this
//! board with `"needs-company"` when `input.companies` is empty.
//!
//! Endpoint verified live (2026-07-02) against slug `careers-at-sleek` (55
//! jobs) — see `.claude/scratch/scraping-followups.md`. Unlike the
//! career-ops-ported boards (breezy/pinpoint/rippling/bamboohr/themuse), this
//! endpoint was confirmed by a real request, not reconnaissance-ported from a
//! doc/blog — so it isn't marked "unverified" like those.
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::normalize_companies;
use async_trait::async_trait;
use serde::Deserialize;

const BOARD_ID: &str = "workable";

/// Maximum number of company slugs processed per scrape call.
/// Prevents an unbounded number of outbound requests from a large IPC payload.
const MAX_COMPANIES: usize = 50;

/// Validate a company slug before it is interpolated into the widget API's
/// URL PATH segment (`/api/v1/widget/accounts/{slug}`). Workable uses the
/// slug as a path segment, not a subdomain, but the same DNS-label-shaped
/// character set (alphanumeric + hyphen only — no `.`/`/`/`:`/`?`) still
/// guards against a slug redirecting the request via path traversal or an
/// injected query string — same guard shape as Breezy/Pinpoint's subdomain
/// check, reused here for a path segment instead.
fn is_valid_workable_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 63
        && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

/// Lowercase every company entry BEFORE deduping. `search()` lowercases the
/// slug for the outbound request regardless (Workable slugs are
/// case-insensitive), so deduping on the raw casing first would let
/// case-only variants (e.g. `"Acme"` vs `"acme"`) survive `normalize_companies`
/// as two distinct entries and fire two identical fetches for the same
/// tenant. Lowercasing first collapses them to one before dedup runs.
fn normalize_workable_companies(input: &[String]) -> Vec<String> {
    let lowercased: Vec<String> = input.iter().map(|s| s.to_lowercase()).collect();
    normalize_companies(&lowercased, MAX_COMPANIES)
}

/// Validate that a job URL from the response is `https://apply.workable.com/…`.
/// A drifting or hostile response could inject arbitrary URLs into
/// `JobPosting.url`; constrain it to the one host the widget API actually
/// serves job pages from.
fn is_valid_workable_job_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .map(|u| u.scheme() == "https" && u.host_str() == Some("apply.workable.com"))
        .unwrap_or(false)
}

/// Parse a `published_on`/`created_at` value that may be a full RFC3339
/// timestamp or a bare `YYYY-MM-DD` date. Returns `None` on any unparseable
/// value — same fallback shape as Breezy's `parse_breezy_date`.
fn parse_workable_date(s: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp_millis())
}

#[derive(Debug, Deserialize)]
pub(crate) struct WkJob {
    title: Option<String>,
    shortcode: Option<String>,
    url: Option<String>,
    published_on: Option<String>,
    created_at: Option<String>,
    country: Option<String>,
    city: Option<String>,
    state: Option<String>,
    #[serde(default)]
    telecommuting: Option<bool>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WkAccountResponse {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    jobs: Vec<serde_json::Value>,
}

/// Deserialize each job row independently, dropping (with a debug log) any
/// row that fails to type-check — e.g. a `telecommuting` value shipped as
/// something other than a bool for one row. Without this, `Vec<WkJob>`'s
/// atomic deserialize would fail the WHOLE company on a single malformed
/// row (silent zero-jobs). Mirrors Rippling's `rows_to_jobs` resilience idiom.
pub(crate) fn rows_to_jobs(values: Vec<serde_json::Value>) -> Vec<WkJob> {
    let total = values.len();
    let jobs: Vec<WkJob> = values
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<WkJob>(v) {
            Ok(job) => Some(job),
            Err(e) => {
                log::debug!("[workable] skipping malformed row: {e}");
                None
            }
        })
        .collect();
    let skipped = total - jobs.len();
    if skipped > 0 {
        log::warn!("[workable] skipped {skipped}/{total} malformed rows");
    }
    jobs
}

/// Map a parsed Workable response into postings for one company. Standalone
/// (no `&self`) so it is unit-testable against a JSON fixture.
pub(crate) fn parse_workable_response(
    jobs: Vec<WkJob>,
    company: &str,
    slug: &str,
    now: i64,
) -> Vec<JobPosting> {
    let mut seen_urls = std::collections::HashSet::new();
    let mut out = Vec::new();

    for j in jobs {
        let title = j.title.unwrap_or_default().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let shortcode = match j
            .shortcode
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(s) => s.to_string(),
            None => continue,
        };

        // `url` is normally present and already on-host. When it's absent (or
        // blank), fall back to Workable's own canonical apply-page URL pattern
        // built from the shortcode — the same host-lock validation applies to
        // the built URL, so this can't be used to smuggle an off-host link.
        let url = match j.url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(u) => {
                if !is_valid_workable_job_url(u) {
                    continue;
                }
                u.to_string()
            }
            None => {
                let built = format!(
                    "https://apply.workable.com/j/{}",
                    urlencoding::encode(&shortcode)
                );
                if !is_valid_workable_job_url(&built) {
                    continue;
                }
                built
            }
        };
        if !seen_urls.insert(url.clone()) {
            continue;
        }

        let mut parts: Vec<String> = [j.city, j.state, j.country]
            .into_iter()
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if j.telecommuting.unwrap_or(false) {
            parts.push("Remote".to_string());
        }
        let location = (!parts.is_empty()).then(|| parts.join(", "));

        let posted_at = j
            .published_on
            .as_deref()
            .or(j.created_at.as_deref())
            .and_then(parse_workable_date);

        // Namespaced with the company slug: `shortcode` is only unique WITHIN
        // one Workable tenant, so two companies fetched in the same
        // `companies[]` batch could otherwise collide on the same id.
        out.push(JobPosting {
            id: format!("{BOARD_ID}:{slug}:{shortcode}"),
            external_id: Some(shortcode),
            title,
            company: company.to_string(),
            location,
            url,
            source: BOARD_ID.to_string(),
            description: j.description.map(|d| strip_html(&d)),
            requirements: None,
            posted_at,
            captured_at: now,
            extra: std::collections::HashMap::new(),
        });
    }

    out
}

pub struct WorkableScraper;

#[async_trait]
impl Scraper for WorkableScraper {
    fn id(&self) -> &'static str {
        BOARD_ID
    }

    fn display_name(&self) -> &'static str {
        "Workable"
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

        // Lowercase-then-dedupe (first-seen order), drop blanks, and cap to
        // MAX_COMPANIES so a large IPC payload cannot fan out unbounded
        // requests to Workable (and so case-only variants collapse to one).
        let companies = normalize_workable_companies(&input.companies);
        let total = companies.len();

        let mut successful_fetches = 0usize;
        let mut first_fetch_error: Option<String> = None;

        for (i, slug) in companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            // Guard: reject slugs that could redirect the request via path
            // traversal or an injected query string.
            if !is_valid_workable_slug(slug) {
                log::warn!("[workable] skipping invalid company slug '{}'", slug);
                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress((i + 1) as f32 / total as f32);
                }
                continue;
            }

            let url =
                format!("https://apply.workable.com/api/v1/widget/accounts/{slug}?details=true");

            let data =
                match fetch_json::<WkAccountResponse>(&url, Default::default(), ctx.signal.clone())
                    .await
                {
                    Ok(d) => d,
                    Err(e) => {
                        // Check cancellation first: a fetch that failed because
                        // the run was cancelled is not a real board-level error.
                        if ctx.signal.is_cancelled() {
                            break;
                        }
                        log::warn!("[workable] fetch failed for '{}': {e}", slug);
                        first_fetch_error.get_or_insert_with(|| e.to_string());
                        if let Some(ref on_progress) = ctx.on_progress {
                            on_progress((i + 1) as f32 / total as f32);
                        }
                        continue;
                    }
                };

            let resp = match data {
                Some(d) => {
                    successful_fetches += 1;
                    d
                }
                None => {
                    if let Some(ref on_progress) = ctx.on_progress {
                        on_progress((i + 1) as f32 / total as f32);
                    }
                    continue;
                }
            };

            let company = resp
                .name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(slug.as_str())
                .to_string();
            let jobs = rows_to_jobs(resp.jobs);

            for posting in parse_workable_response(jobs, &company, slug, now) {
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
                    "all workable company fetches failed: {error}"
                ));
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

//! Breezy HR — public per-company postings JSON
//!
//! Endpoint: `https://{slug}.breezy.hr/json`
//! No global keyword search — requires a company slug. The engine skips this
//! board with `"needs-company"` when `input.companies` is empty.
//!
//! Endpoint reconnaissance ported from santifer/career-ops (MIT), `providers/breezy.mjs`.
use super::super::http::fetch_json;
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::{
    ats_all_fetches_failed, is_https_url, is_valid_dns_label_slug, normalize_companies,
};
use async_trait::async_trait;
use serde::Deserialize;

const BOARD_ID: &str = "breezy";

/// Maximum number of company slugs processed per scrape call.
/// Prevents an unbounded number of outbound requests from a large IPC payload.
const MAX_COMPANIES: usize = 50;

/// Parse a `published_date` value that may be a full RFC3339 timestamp or a
/// bare `YYYY-MM-DD` date. Returns `None` on any unparseable value.
fn parse_breezy_date(s: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp_millis())
}

#[derive(Debug, Deserialize)]
struct BzCountry {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BzLocation {
    name: Option<String>,
    city: Option<String>,
    state: Option<String>,
    country: Option<BzCountry>,
    #[serde(rename = "is_remote")]
    is_remote: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BzPosting {
    name: Option<String>,
    url: Option<String>,
    #[serde(rename = "published_date")]
    published_date: Option<String>,
    location: Option<BzLocation>,
}

/// Map a parsed Breezy response into postings for one company. Standalone
/// (no `&self`) so it is unit-testable against a JSON fixture.
///
/// The response has no stable job id, so the (deduped) posting URL doubles as
/// the id — same precedent as the We Work Remotely RSS `<guid>` permalink.
pub(crate) fn parse_breezy_response(
    postings: Vec<BzPosting>,
    company: &str,
    now: i64,
) -> Vec<JobPosting> {
    let mut seen_urls = std::collections::HashSet::new();
    let mut out = Vec::new();

    for p in postings {
        let title = p.name.unwrap_or_default().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let url = match p.url.as_deref().map(str::trim) {
            Some(u) if is_https_url(u) => u.to_string(),
            _ => continue,
        };
        if !seen_urls.insert(url.clone()) {
            continue;
        }

        let posted_at = p.published_date.as_deref().and_then(parse_breezy_date);

        let location = p.location.and_then(|l| {
            let is_remote = l.is_remote.unwrap_or(false);
            let mut base = match l.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(name) => name.to_string(),
                None => {
                    let parts: Vec<String> = [l.city, l.state, l.country.and_then(|c| c.name)]
                        .into_iter()
                        .flatten()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    parts.join(", ")
                }
            };
            if is_remote && !base.to_lowercase().contains("remote") {
                if base.is_empty() {
                    base = "Remote".to_string();
                } else {
                    base.push_str(", Remote");
                }
            }
            (!base.is_empty()).then_some(base)
        });

        out.push(JobPosting {
            id: format!("{BOARD_ID}:{url}"),
            external_id: Some(url.clone()),
            title,
            company: company.to_string(),
            location,
            url,
            source: BOARD_ID.to_string(),
            description: None,
            requirements: None,
            posted_at,
            captured_at: now,
            extra: std::collections::HashMap::new(),
        });
    }

    out
}

pub struct BreezyScraper;

#[async_trait]
impl Scraper for BreezyScraper {
    fn id(&self) -> &'static str {
        BOARD_ID
    }

    fn display_name(&self) -> &'static str {
        "Breezy HR"
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
        // large IPC payload cannot fan out unbounded requests to Breezy.
        let companies = normalize_companies(&input.companies, MAX_COMPANIES);
        let total = companies.len();

        let mut successful_fetches = 0usize;
        let mut first_fetch_error: Option<String> = None;

        for (i, raw_company) in companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let company = raw_company.to_lowercase();

            // Guard: reject slugs that are not valid single DNS hostname labels.
            // A slug like `127.0.0.1:8443/foo` would change the URL authority
            // and redirect the fetch away from Breezy (SSRF).
            if !is_valid_dns_label_slug(&company) {
                log::warn!("[breezy] skipping invalid company slug '{}'", company);
                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress((i + 1) as f32 / total as f32);
                }
                continue;
            }

            let url = format!("https://{company}.breezy.hr/json");

            let data =
                match fetch_json::<Vec<BzPosting>>(&url, Default::default(), ctx.signal.clone())
                    .await
                {
                    Ok(d) => d,
                    Err(e) => {
                        // Check cancellation first: a fetch that failed because
                        // the run was cancelled is not a real board-level error.
                        if ctx.signal.is_cancelled() {
                            break;
                        }
                        log::warn!("[breezy] fetch failed for '{}': {e}", company);
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
            let postings = data;

            for posting in parse_breezy_response(postings, &company, now) {
                if let Some(ref on_item) = ctx.on_item {
                    on_item(posting.clone());
                }
                out.push(posting);
            }

            if let Some(ref on_progress) = ctx.on_progress {
                on_progress((i + 1) as f32 / total as f32);
            }
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

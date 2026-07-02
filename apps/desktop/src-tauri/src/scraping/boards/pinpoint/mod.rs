//! Pinpoint (PinpointHQ) — public per-company postings JSON
//!
//! Endpoint: `https://{slug}.pinpointhq.com/postings.json`
//! No global keyword search — requires a company slug. The engine skips this
//! board with `"needs-company"` when `input.companies` is empty.
//!
//! Endpoint reconnaissance ported from santifer/career-ops (MIT), `providers/pinpoint.mjs`.
use super::super::http::fetch_json;
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::{is_https_url, normalize_companies};
use async_trait::async_trait;
use serde::Deserialize;

const BOARD_ID: &str = "pinpoint";

/// Maximum number of company slugs processed per scrape call.
/// Prevents an unbounded number of outbound requests from a large IPC payload.
const MAX_COMPANIES: usize = 50;

/// Validate that a company slug is a single valid DNS hostname label.
/// Pinpoint uses the slug as a subdomain — a slug with dots, slashes, or
/// colons could change the URL authority and redirect the fetch away from
/// Pinpoint (SSRF).
fn is_valid_pinpoint_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 63
        && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

#[derive(Debug, Deserialize)]
struct PpLocation {
    name: Option<String>,
    city: Option<String>,
    province: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PpPosting {
    title: Option<String>,
    url: Option<String>,
    location: Option<PpLocation>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PpResponse {
    data: Vec<PpPosting>,
}

/// Map a parsed Pinpoint response into postings for one company. Standalone
/// (no `&self`) so it is unit-testable against a JSON fixture.
///
/// The response has no stable job id, so the (deduped) posting URL doubles as
/// the id — same precedent as the We Work Remotely RSS `<guid>` permalink.
pub(crate) fn parse_pinpoint_response(
    resp: PpResponse,
    company: &str,
    now: i64,
) -> Vec<JobPosting> {
    let mut seen_urls = std::collections::HashSet::new();
    let mut out = Vec::new();

    for p in resp.data {
        let title = p.title.unwrap_or_default().trim().to_string();
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

        let location = p.location.and_then(|l| {
            let name = l.name.as_deref().map(str::trim).filter(|s| !s.is_empty());
            if let Some(name) = name {
                return Some(name.to_string());
            }
            let parts: Vec<String> = [l.city, l.province]
                .into_iter()
                .flatten()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (!parts.is_empty()).then(|| parts.join(", "))
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
            posted_at: None,
            captured_at: now,
            extra: std::collections::HashMap::new(),
        });
    }

    out
}

pub struct PinpointScraper;

#[async_trait]
impl Scraper for PinpointScraper {
    fn id(&self) -> &'static str {
        BOARD_ID
    }

    fn display_name(&self) -> &'static str {
        "Pinpoint"
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
        // large IPC payload cannot fan out unbounded requests to Pinpoint.
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
            // and redirect the fetch away from Pinpoint (SSRF).
            if !is_valid_pinpoint_slug(&company) {
                log::warn!("[pinpoint] skipping invalid company slug '{}'", company);
                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress((i + 1) as f32 / total as f32);
                }
                continue;
            }

            let url = format!("https://{company}.pinpointhq.com/postings.json");

            let data = match fetch_json::<PpResponse>(&url, Default::default(), ctx.signal.clone())
                .await
            {
                Ok(d) => d,
                Err(e) => {
                    // Check cancellation first: a fetch that failed because
                    // the run was cancelled is not a real board-level error.
                    if ctx.signal.is_cancelled() {
                        break;
                    }
                    log::warn!("[pinpoint] fetch failed for '{}': {e}", company);
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

            for posting in parse_pinpoint_response(resp, &company, now) {
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
                    "all pinpoint company fetches failed: {error}"
                ));
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

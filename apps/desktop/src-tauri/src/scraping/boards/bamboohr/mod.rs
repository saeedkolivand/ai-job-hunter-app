//! BambooHR — public per-company careers list JSON
//!
//! Endpoint: `https://{slug}.bamboohr.com/careers/list`
//! No global keyword search — requires a company slug. The engine skips this
//! board with `"needs-company"` when `input.companies` is empty.
//!
//! Endpoint reconnaissance ported from santifer/career-ops (MIT), `providers/bamboohr.mjs`.
use super::super::http::fetch_json;
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::normalize_companies;
use async_trait::async_trait;
use serde::Deserialize;

const BOARD_ID: &str = "bamboohr";

/// Maximum number of company slugs processed per scrape call.
/// Prevents an unbounded number of outbound requests from a large IPC payload.
const MAX_COMPANIES: usize = 50;

/// Validate that a company slug is a single valid DNS hostname label.
/// BambooHR uses the slug as a subdomain — a slug with dots, slashes, or
/// colons could change the URL authority and redirect the fetch away from
/// BambooHR (SSRF).
fn is_valid_bamboohr_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 63
        && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

/// BambooHR job ids have been observed as both JSON numbers and strings
/// across tenants — accept either, trimmed, dropping empty values.
fn bamboohr_id_to_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => {
            let t = s.trim();
            (!t.is_empty()).then(|| t.to_string())
        }
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Build the namespaced job id for a BambooHR posting.
///
/// Format: `bamboohr:{company}:{id}` — the company prefix prevents job ids
/// from different tenants colliding in any deduplication layer. BambooHR ids
/// are small tenant-local integers (e.g. `1`, `2`, …), so without the prefix
/// two different companies in one `companies[]` fan-out could both emit
/// `id=1` and silently overwrite each other in `PostingsCache`. Mirrors
/// `personio::make_job_id`.
fn make_job_id(company: &str, id: &str) -> String {
    format!("{BOARD_ID}:{company}:{id}")
}

#[derive(Debug, Deserialize)]
struct BhLocation {
    city: Option<String>,
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BhPosting {
    id: Option<serde_json::Value>,
    #[serde(rename = "jobOpeningName")]
    job_opening_name: Option<String>,
    location: Option<BhLocation>,
    #[serde(rename = "isRemote")]
    is_remote: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BhResponse {
    result: Vec<BhPosting>,
}

/// Map a parsed BambooHR response into postings for one company. Standalone
/// (no `&self`) so it is unit-testable against a JSON fixture. Unlike
/// Pinpoint/Breezy, BambooHR's `id` is a stable per-tenant job identifier
/// (not the constructed URL), so it is the dedup key — but it is
/// company-namespaced via [`make_job_id`] since raw ids collide across tenants.
pub(crate) fn parse_bamboohr_response(
    resp: BhResponse,
    company: &str,
    now: i64,
) -> Vec<JobPosting> {
    let mut out = Vec::new();

    for p in resp.result {
        let title = match p.job_opening_name.as_deref().map(str::trim) {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => continue,
        };
        let id = match p.id.as_ref().and_then(bamboohr_id_to_string) {
            Some(id) => id,
            None => continue,
        };

        let mut location_parts = Vec::new();
        if let Some(l) = &p.location {
            if let Some(city) = l.city.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                location_parts.push(city.to_string());
            }
            if let Some(state) = l.state.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                location_parts.push(state.to_string());
            }
        }
        if p.is_remote.unwrap_or(false) {
            location_parts.push("Remote".to_string());
        }
        let location = (!location_parts.is_empty()).then(|| location_parts.join(", "));

        let url = format!(
            "https://{company}.bamboohr.com/careers/{}",
            urlencoding::encode(&id)
        );

        out.push(JobPosting {
            id: make_job_id(company, &id),
            external_id: Some(id),
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

pub struct BambooHrScraper;

#[async_trait]
impl Scraper for BambooHrScraper {
    fn id(&self) -> &'static str {
        BOARD_ID
    }

    fn display_name(&self) -> &'static str {
        "BambooHR"
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
        // large IPC payload cannot fan out unbounded requests to BambooHR.
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
            // and redirect the fetch away from BambooHR (SSRF).
            if !is_valid_bamboohr_slug(&company) {
                log::warn!("[bamboohr] skipping invalid company slug '{}'", company);
                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress((i + 1) as f32 / total as f32);
                }
                continue;
            }

            let url = format!("https://{company}.bamboohr.com/careers/list");

            let data = match fetch_json::<BhResponse>(&url, Default::default(), ctx.signal.clone())
                .await
            {
                Ok(d) => d,
                Err(e) => {
                    // Check cancellation first: a fetch that failed because
                    // the run was cancelled is not a real board-level error.
                    if ctx.signal.is_cancelled() {
                        break;
                    }
                    log::warn!("[bamboohr] fetch failed for '{}': {e}", company);
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

            for posting in parse_bamboohr_response(resp, &company, now) {
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
                    "all bamboohr company fetches failed: {error}"
                ));
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

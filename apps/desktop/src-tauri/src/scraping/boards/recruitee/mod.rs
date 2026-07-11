/// Recruitee — public per-company offers API
///
/// Endpoint: `https://{company}.recruitee.com/api/offers/`
/// No global keyword search — requires a company slug. The engine skips this
/// board with `"needs-company"` when `input.companies` is empty.
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::ats_board_failure;
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Offer {
    id: i64,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    slug: String,
    title: String,
    description: Option<String>,
    requirements: Option<String>,
    #[serde(rename = "careers_url")]
    careers_url: String,
    city: Option<String>,
    country: Option<String>,
    remote: Option<bool>,
    #[serde(rename = "created_at")]
    created_at: Option<String>,
    #[serde(rename = "company_name")]
    company_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Resp {
    offers: Vec<Offer>,
}

/// Validate that a slug is a valid DNS hostname label (RFC 1123).
/// Recruitee uses the slug as a subdomain so URL-encoding would produce a
/// malformed host; this guard rejects any slug that would break or redirect
/// the URL (e.g., dots, colons, spaces, percent signs, leading/trailing hyphens,
/// or labels exceeding the 63-character DNS limit).
pub(crate) fn is_valid_recruitee_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 63
        && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

pub struct RecruiteeScraper;

#[async_trait]
impl Scraper for RecruiteeScraper {
    fn id(&self) -> &'static str {
        "recruitee"
    }

    fn display_name(&self) -> &'static str {
        "Recruitee"
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

        let mut successful_fetches = 0usize;
        let mut rejected_slugs = 0usize;
        let mut first_fetch_error: Option<String> = None;

        for (i, company) in input.companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let company = company.trim();
            if company.is_empty() {
                continue;
            }

            // Recruitee uses the slug as a hostname label — URL-encoding would
            // percent-encode dots and produce a malformed host. Accept only
            // labels that are valid hostname components (alphanumeric + hyphen).
            if !is_valid_recruitee_slug(company) {
                rejected_slugs += 1;
                log::warn!("[recruitee] skipping invalid hostname slug '{}'", company);
                continue;
            }

            // Use the raw slug as the subdomain; do NOT percent-encode it.
            let url = format!("https://{}.recruitee.com/api/offers/", company);
            let data = match fetch_json::<Resp>(&url, Default::default(), ctx.signal.clone()).await
            {
                Ok(d) => d,
                Err(e) => {
                    // A fetch that failed because the run was cancelled is not
                    // a real board-level error.
                    if ctx.signal.is_cancelled() {
                        break;
                    }
                    log::warn!("[recruitee] fetch failed for '{}': {e}", company);
                    first_fetch_error.get_or_insert_with(|| e.to_string());
                    continue;
                }
            };
            successful_fetches += 1;
            let offers = data.offers;

            for o in offers {
                let description = vec![
                    o.description.as_deref().map(strip_html),
                    o.requirements.as_deref().map(strip_html),
                ]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");

                let location = vec![o.city.as_deref(), o.country.as_deref()]
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ");

                let posted_at = o
                    .created_at
                    .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
                    .map(|dt| dt.timestamp_millis());

                let posting = JobPosting {
                    id: format!("{}:{}", self.id(), o.id),
                    external_id: Some(o.id.to_string()),
                    title: o.title,
                    company: o.company_name.unwrap_or_else(|| company.to_string()),
                    location: if location.is_empty() {
                        None
                    } else {
                        Some(location)
                    },
                    url: o.careers_url,
                    source: self.id().to_string(),
                    description: if description.is_empty() {
                        None
                    } else {
                        Some(description)
                    },
                    requirements: None,
                    posted_at,
                    captured_at: now,
                    extra: {
                        let mut map = std::collections::HashMap::new();
                        if let Some(remote) = o.remote {
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

        // A cancel that fired after an invalid slug was rejected (but before a
        // later valid slug was reached) must not be misattributed as "all slugs
        // invalid" — the run was interrupted, not misconfigured.
        if ctx.signal.is_cancelled() {
            return Ok(out);
        }

        // Distinguishes an all-slug 403/parse-drift run OR an all-slug-rejected
        // run from a genuine zero result; see `ats_board_failure` for the decision.
        if let Some(message) = ats_board_failure(
            self.id(),
            successful_fetches,
            rejected_slugs,
            &first_fetch_error,
        ) {
            return Err(anyhow::anyhow!(message));
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

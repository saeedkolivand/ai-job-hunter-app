/// Personio — public XML feed per company
///
/// Endpoint: `https://{company}.jobs.personio.de/xml` (falls back to `.com`)
/// No global keyword search — requires a company slug. The engine skips this
/// board with `"needs-company"` when `input.companies` is empty.
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;

const HOSTS: &[&str] = &["jobs.personio.de", "jobs.personio.com"];

// Personio XML feed parsing (shared). The public feed is a flat <position>
// list; the regex set + capture loop is identical for the board scraper and the
// single-URL resolver (scrape_url::try_personio), so parsing lives here once.
// Each caller still builds its own JobPosting (different id/url/posted_at shape).
static POSITION_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<position>(.*?)</position>").unwrap());
static ID_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<id>(.*?)</id>").unwrap());
static NAME_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<name>(.*?)</name>").unwrap());
static OFFICE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<office>(.*?)</office>").unwrap());
// 2025+: Personio XML uses <jobDescriptions> wrapping <jobDescription> blocks each with
// <name> + <value>. First extract the <jobDescriptions>…</jobDescriptions> block, then
// capture every <value> within it — prevents matching <value> nodes in sibling blocks
// like <customAttributes> or <department>.
static JOBDESC_BLOCK_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?s)<jobDescriptions>(.*?)</jobDescriptions>").unwrap()
});
// Legacy (pre-2025) Personio feeds use a singular <jobDescription> block.
// When the plural wrapper is absent, scope the fallback to this block only —
// avoids leaking <value> fields from sibling blocks like <customAttributes>.
static JOBDESC_SINGULAR_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?s)<jobDescription>(.*?)</jobDescription>").unwrap()
});
static DESC_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<value>(.*?)</value>").unwrap());
static CREATED_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<createdAt>(.*?)</createdAt>").unwrap());

/// One parsed Personio position. Description is already run through strip_html.
pub(crate) struct PersonioPosition {
    pub id: String,
    pub title: String,
    pub office: String,
    pub description: String,
    pub created: String,
}

/// Parse a Personio XML feed into its positions, skipping empty-id entries.
pub(crate) fn parse_xml_feed(xml: &str) -> Vec<PersonioPosition> {
    let mut out = Vec::new();
    for position_cap in POSITION_RE.captures_iter(xml) {
        let Some(position_content) = position_cap.get(1) else {
            continue;
        };
        let position_str = position_content.as_str();
        let cap = |re: &regex::Regex| {
            re.captures(position_str)
                .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
                .unwrap_or_default()
        };
        let id = cap(&ID_RE);
        if id.is_empty() {
            continue;
        }
        // Extract <value> nodes only from within <jobDescriptions>…</jobDescriptions>
        // to avoid picking up <value> tags in sibling blocks (customAttributes, etc.).
        // Fall back to the singular <jobDescription> block for legacy tenants/fixtures;
        // never fall back to the whole position string (leaks <value> from other blocks).
        let desc_scope_owned;
        let desc_scope = if let Some(c) = JOBDESC_BLOCK_RE.captures(position_str) {
            c.get(1).map(|m| m.as_str()).unwrap_or("")
        } else {
            desc_scope_owned = JOBDESC_SINGULAR_RE
                .captures(position_str)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
                .unwrap_or_default();
            desc_scope_owned.as_str()
        };
        let description = DESC_RE
            .captures_iter(desc_scope)
            .filter_map(|c| c.get(1).map(|m| strip_html(m.as_str().trim())))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        out.push(PersonioPosition {
            id,
            title: cap(&NAME_RE),
            office: cap(&OFFICE_RE),
            description,
            created: cap(&CREATED_RE),
        });
    }
    out
}

/// Validate that a company slug is a single valid DNS hostname label.
/// Rejects anything with colons, slashes, dots, or other characters that could
/// alter the URL authority and redirect the fetch away from Personio (SSRF).
fn is_valid_personio_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 63
        && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

pub struct PersonioScraper;

#[async_trait]
impl Scraper for PersonioScraper {
    fn id(&self) -> &'static str {
        "personio"
    }

    fn display_name(&self) -> &'static str {
        "Personio"
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

        for (i, raw_company) in input.companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let company = raw_company.trim().to_lowercase();
            if company.is_empty() {
                continue;
            }

            // Guard: reject slugs that are not valid single DNS hostname labels.
            // A slug like `127.0.0.1:8443/foo` would change the URL authority and
            // redirect the fetch away from Personio (SSRF).
            if !is_valid_personio_slug(&company) {
                log::warn!("[personio] skipping invalid company slug '{}'", company);
                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress((i + 1) as f32 / total as f32);
                }
                continue;
            }

            let mut xml_and_host: Option<(String, String)> = None;
            for &host in HOSTS {
                if ctx.signal.is_cancelled() {
                    break;
                }
                // 2025+: Personio migrated career sites to Next.js; root URL returns HTML.
                // The XML feed now lives at /xml on the same subdomain.
                let url = format!("https://{}.{}/xml", company, host);
                let res = match fetch_text(&url, Default::default(), ctx.signal.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        log::warn!(
                            "[personio] fetch failed for '{}' via {}: {e}",
                            company,
                            host
                        );
                        if ctx.signal.is_cancelled() {
                            break;
                        }
                        continue;
                    }
                };

                if res.status_code == 200 && res.text.contains("<position") {
                    xml_and_host = Some((res.text, host.to_string()));
                    break;
                }
            }

            let (xml, serving_host) = match xml_and_host {
                Some(x) => x,
                None => {
                    if let Some(ref on_progress) = ctx.on_progress {
                        on_progress((i + 1) as f32 / total as f32);
                    }
                    continue;
                }
            };

            for pos in parse_xml_feed(&xml) {
                let posted_at = if pos.created.is_empty() {
                    None
                } else {
                    chrono::DateTime::parse_from_rfc3339(&pos.created)
                        .ok()
                        .map(|dt| dt.timestamp_millis())
                };

                let posting = JobPosting {
                    // Namespace by company so position IDs from different Personio
                    // tenants never collide during a multi-company scrape.
                    id: format!("{}:{}:{}", self.id(), company, pos.id),
                    external_id: Some(pos.id.clone()),
                    title: pos.title,
                    company: company.clone(),
                    location: if pos.office.is_empty() {
                        None
                    } else {
                        Some(pos.office)
                    },
                    // Use the host that actually served the XML so .com fallback
                    // produces correct job URLs instead of hardcoding .de.
                    url: format!("https://{}.{}/job/{}", company, serving_host, pos.id),
                    source: self.id().to_string(),
                    description: if pos.description.is_empty() {
                        None
                    } else {
                        Some(pos.description)
                    },
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

        Ok(out)
    }
}

#[cfg(test)]
mod test;

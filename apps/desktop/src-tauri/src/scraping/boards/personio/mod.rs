/// Personio — public XML feed per company
///
/// Endpoint: `https://{company}.jobs.personio.de/xml` (falls back to `.com`)
/// No global keyword search — requires a company slug. The engine skips this
/// board with `"needs-company"` when `input.companies` is empty.
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::{ats_finish_search, ats_partial_note};
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

/// Build the namespaced job id for a Personio posting.
///
/// Format: `personio:{company}:{pos_id}` — the company prefix prevents
/// position IDs from different tenants colliding in any deduplication layer.
pub(crate) fn make_job_id(company: &str, pos_id: &str) -> String {
    format!("personio:{company}:{pos_id}")
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
        // #597 + trust-H: track slugs rejected by the SSRF guard, per-company
        // fetch success, and the first fetch error so BOTH an all-invalid-slug
        // run AND an all-hosts-all-companies-fetch-fail run surface a distinct
        // board error instead of a silent zero (see `ats_finish_search` after
        // the loop). Personio is a `fetch_text` (XML feed) board, so PR A's
        // representable-failure semantics never applied here until now.
        let mut rejected_slugs = 0usize;
        let mut successful_fetches = 0usize;
        let mut first_fetch_error: Option<String> = None;

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
                rejected_slugs += 1;
                log::warn!("[personio] skipping invalid company slug '{}'", company);
                if let Some(ref on_progress) = ctx.on_progress {
                    on_progress((i + 1) as f32 / total as f32);
                }
                continue;
            }

            // Try each host. A company counts as REACHED once any host returns a
            // 200 — even a 200 with no `<position>` (a genuinely empty feed is a
            // reached, zero-job company, NOT a fetch failure). Only a 404/5xx on
            // every host, or a network error on every host, is a fetch failure
            // recorded into `first_fetch_error` (unchanged 200-with-`<position>`
            // success heuristic drives what we actually parse). CAVEAT (unverified,
            // unlike LinkedIn's live-verified soft-block claim): this assumes
            // Personio never serves a 200 bot-block/interstitial page in place of
            // the feed — if it does, a blocked company would count as
            // reached-empty rather than a fetch failure.
            let mut company_reached = false;
            let mut company_error: Option<String> = None;
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
                        company_error.get_or_insert_with(|| e.to_string());
                        continue;
                    }
                };

                if res.status_code == 200 {
                    company_reached = true;
                    if res.text.contains("<position") {
                        xml_and_host = Some((res.text, host.to_string()));
                        break;
                    }
                } else {
                    company_error
                        .get_or_insert_with(|| format!("HTTP {} from {host}", res.status_code));
                }
            }

            // Any 200 = a successful fetch (reached the endpoint); otherwise the
            // first 404/5xx/network error is recorded so an all-companies-fail
            // run returns Err via `ats_finish_search`.
            if company_reached {
                successful_fetches += 1;
            } else if let Some(e) = company_error {
                first_fetch_error.get_or_insert(e);
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
                    id: make_job_id(&company, &pos.id),
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

        // trust-H item 3: surface a partial-visibility anomaly (some slugs
        // rejected while others fetched) as ONE informational note (PR D
        // grammar). Personio has no per-row parse drop count, so only
        // `slugs-invalid` can fire here. Gated on non-cancellation: a benign
        // interruption reports nothing (mirrors `ats_finish_search`).
        if !ctx.signal.is_cancelled() {
            if let Some(note) = ats_partial_note(successful_fetches, rejected_slugs, 0) {
                ctx.report_note(note);
            }
        }

        // See `ats_finish_search`: cancellation wins over a synthesized
        // all-fetches-failed/all-slugs-invalid board error. This is the same
        // finish shape the other 6 ATS boards use — extended to Personio now
        // that it tracks `successful_fetches`/`first_fetch_error`.
        ats_finish_search(
            &ctx.signal,
            out,
            self.id(),
            successful_fetches,
            rejected_slugs,
            &first_fetch_error,
        )
    }
}

#[cfg(test)]
mod test;

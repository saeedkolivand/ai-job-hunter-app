/// Jobicy — public JSON API (`https://jobicy.com/api/v2/remote-jobs`), keyless,
/// remote jobs only. Unlike most feeds here, Jobicy returns the FULL job
/// description HTML inline (`jobDescription`) — no truncated snippet, no
/// detail-fetch wall — so this board never needs `scrape_url`'s resolve-on-open
/// path.
///
/// ATTRIBUTION (Jobicy ToS — `friendlyNotice` on every API response): Jobicy
/// requires visible credit and that every posting link back to the ORIGINAL
/// jobicy.com job URL. `map_job` below passes `url` through unmodified (it is
/// already jobicy.com's own posting page, and `is_valid_jobicy_url` below
/// REJECTS anything else) and `source`/`display_name()` surface as "Jobicy" in
/// the jobs UI — never replace either with a third-party/apply redirect link.
///
/// `from_url()` is intentionally left at the trait default (`Ok(None)`, same as
/// every other HTTP board here): verified live that the API has no by-id/by-slug
/// lookup (`?id=`/`?jobSlug=` both 400 with "Unexpected parameter") — there is no
/// keyless way to re-fetch a single posting, so a shared jobicy.com link falls
/// through to `scrape_url`'s generic HTML fallback, same as any other
/// unregistered host.
use super::super::http::{fetch_text, html_to_markdown};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Job {
    id: Option<i64>,
    url: Option<String>,
    #[serde(rename = "jobTitle")]
    job_title: Option<String>,
    #[serde(rename = "companyName")]
    company_name: Option<String>,
    #[serde(rename = "jobGeo")]
    job_geo: Option<String>,
    #[serde(rename = "jobExcerpt")]
    job_excerpt: Option<String>,
    #[serde(rename = "jobDescription")]
    job_description: Option<String>,
    #[serde(rename = "pubDate")]
    pub_date: Option<String>,
}

/// Top-level response envelope. `jobs` is deserialized as raw `Value` rows
/// (not `Vec<Job>` directly) so [`rows_to_jobs`] can drop a single malformed
/// row instead of failing the whole batch — see its doc comment.
#[derive(Debug, Deserialize)]
struct Resp {
    #[serde(default)]
    jobs: Vec<serde_json::Value>,
}

/// Deserialize each job row independently, dropping (with a debug log) any
/// row that fails to type-check — e.g. a future response sending `id` as a
/// string on one row. Mirrors Comeet/Workable/Breezy/Rippling's `rows_to_jobs`
/// resilience idiom: without this, `Vec<Job>`'s atomic deserialize would fail
/// the WHOLE batch on a single drifted row (silent zero-jobs).
pub(crate) fn rows_to_jobs(values: Vec<serde_json::Value>) -> Vec<Job> {
    let total = values.len();
    let jobs: Vec<Job> = values
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<Job>(v) {
            Ok(job) => Some(job),
            Err(e) => {
                log::debug!("[jobicy] skipping malformed row: {e}");
                None
            }
        })
        .collect();
    let skipped = total - jobs.len();
    if skipped > 0 {
        log::warn!("[jobicy] skipped {skipped}/{total} malformed rows");
    }
    jobs
}

/// Validate that a job URL from the response is `http(s)://jobicy.com/…` (or
/// a `*.jobicy.com` subdomain), reusing the trust module's suffix-anchored
/// domain match. A drifting/hostile response could inject an arbitrary URL
/// into `JobPosting.url`; this is also the ToS-required attribution link (see
/// the module doc comment), so a non-jobicy.com URL is never usable here
/// regardless — the row is dropped rather than kept with a foreign URL.
fn is_valid_jobicy_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .map(|u| {
            (u.scheme() == "http" || u.scheme() == "https")
                && u.host_str()
                    .is_some_and(|h| crate::scraping::trust::matches_domain_list(h, &["jobicy.com"]))
        })
        .unwrap_or(false)
}

/// Map one already-deserialized Jobicy job into the app's `JobPosting`.
/// Extracted from `search()` so tests can drive the mapping directly without
/// a network call. `None` when a required field (id/title/url) is missing or
/// the url isn't a genuine jobicy.com link — dropped rather than fabricated,
/// mirroring the other feed boards here.
pub(crate) fn map_job(j: Job, scraper_id: &str, now: i64) -> Option<JobPosting> {
    let id = j.id?;
    let title = j.job_title.filter(|s| !s.trim().is_empty())?;
    let url = j
        .url
        .filter(|s| !s.trim().is_empty())
        .filter(|u| is_valid_jobicy_url(u))?;

    Some(JobPosting {
        id: format!("{scraper_id}:{id}"),
        external_id: Some(id.to_string()),
        title,
        company: j
            .company_name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Unknown".to_string()),
        location: j.job_geo.filter(|s| !s.trim().is_empty()),
        url,
        source: scraper_id.to_string(),
        // Full HTML → Markdown. Falls back to the (also HTML) excerpt on the
        // rare response missing `jobDescription`.
        description: j
            .job_description
            .or(j.job_excerpt)
            .map(|html| html_to_markdown(&html)),
        requirements: None,
        posted_at: j
            .pub_date
            .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
            .map(|dt| dt.timestamp_millis()),
        captured_at: now,
        extra: {
            let mut map = std::collections::HashMap::new();
            map.insert("remote".to_string(), serde_json::json!(true));
            map
        },
    })
}

pub struct JobicyScraper;

#[async_trait]
impl Scraper for JobicyScraper {
    fn id(&self) -> &'static str {
        "jobicy"
    }

    fn display_name(&self) -> &'static str {
        "Jobicy"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let q = input.query.trim();
        // Documented cap is 50; clamp defensively even though a live check
        // showed higher counts are honored — staying inside the documented
        // contract rather than relying on undocumented behavior.
        let count = input.amount.clamp(1, 50);

        // NOTE (`supports_location` stays the trait default `false`): Jobicy's
        // `geo` param is NOT a free-text location — it only accepts a fixed set
        // of predefined `geoSlug`s (verified live: an arbitrary value 400s with
        // "Invalid 'geo' value"). Passing the user's raw location string through
        // would break the whole search on any non-matching value, so this board
        // leaves `geo` unset and lets the engine's central post-filter handle it.
        let mut url = format!("https://jobicy.com/api/v2/remote-jobs?count={count}");
        if !q.is_empty() {
            // LIVE-VERIFIED (2026-07-17): unlike `geo`, `tag` genuinely performs
            // relevance-ranked full-text search over title/description, NOT a
            // fixed category whitelist — `tag=dev` surfaces "Sales Development
            // Representative" / "Developer Portal" (substring match on "dev",
            // not a "dev" category), and a full free-text phrase
            // `tag=senior%20backend%20engineer` returns highly relevant senior
            // backend roles. Confirmed by the ERROR MODE too: `geo=garbage`
            // 400s ("Invalid 'geo' value" — enum validation), while an
            // unmatched `tag` 404s with `{"success":false,"message":"Nothing
            // found..."}` — a legitimate zero-result search, not a rejected
            // value. That 404-for-zero-matches case is handled below (treated
            // as an authoritative empty result, never a fetch failure).
            url.push_str(&format!("&tag={}", urlencoding::encode(q)));
        }

        let res = fetch_text(&url, Default::default(), ctx.signal).await?;

        // Jobicy returns HTTP 404 with a valid JSON body
        // (`{"jobs":[],"success":false,"message":"Nothing found..."}`) for a
        // genuine zero-match `tag` search (live-verified) — a normal empty
        // result, not a fetch failure. Every OTHER non-2xx status (a real
        // routing/server error returns an HTML body, not this JSON shape)
        // still propagates as an `Err`, surfaced in `BoardScrapeSummary.error`,
        // per this codebase's "never a silent empty result on failure" rule.
        if res.status_code != 200 && res.status_code != 404 {
            return Err(anyhow::anyhow!("HTTP {}", res.status_code));
        }

        let resp: Resp = serde_json::from_str(&res.text).map_err(|e| {
            log::warn!(
                "[jobicy] response parse failure (HTTP {}): {e}",
                res.status_code
            );
            anyhow::anyhow!("response body did not match the expected schema")
        })?;

        let jobs = rows_to_jobs(resp.jobs);
        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for j in jobs {
            let Some(posting) = map_job(j, self.id(), now) else {
                continue;
            };

            if let Some(ref on_item) = ctx.on_item {
                on_item(posting.clone());
            }

            out.push(posting);
        }

        if let Some(ref on_progress) = ctx.on_progress {
            on_progress(1.0);
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

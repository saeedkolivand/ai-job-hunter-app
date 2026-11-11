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
use super::super::types::{
    BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode, WorkType,
};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct Job {
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
                && u.host_str().is_some_and(|h| {
                    crate::scraping::trust::matches_domain_list(h, &["jobicy.com"])
                })
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
        // Full HTML → Markdown. Falls back to the (also HTML) excerpt when
        // `jobDescription` is absent OR a blank string (some rows send `""`
        // rather than omitting the key) — mirrors how title/url/company
        // already treat blank strings as missing.
        description: j
            .job_description
            .filter(|s| !s.trim().is_empty())
            .or(j.job_excerpt.filter(|s| !s.trim().is_empty()))
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
            // All-remote by definition — no per-posting field to read, but a
            // declared value all the same so this board's rows participate in
            // the work-type filter/badge like every other board.
            map.insert("workType".to_string(), serde_json::json!(WorkType::Remote));
            map
        },
    })
}

/// Decide whether a raw HTTP response is a genuine failure or a
/// (possibly-404-but-JSON) empty/successful result, then parse it into
/// postings. Pure — no network, no `ScrapeContext` — so the load-bearing
/// "a real outage is never misread as empty" contract is hermetically unit
/// testable; previously it was only exercised by an `#[ignore]`d live-network
/// test.
///
/// Jobicy returns HTTP 404 with a valid JSON body
/// (`{"jobs":[],"success":false,"message":"Nothing found..."}`) for a genuine
/// zero-match `tag` search (live-verified) — a normal empty result, not a
/// fetch failure. Every OTHER non-2xx status (a real routing/server error
/// returns an HTML body, not this JSON shape) still propagates as an `Err`,
/// surfaced in `BoardScrapeSummary.error`, per this codebase's "never a
/// silent empty result on failure" rule.
pub(crate) fn parse_response(
    status_code: u16,
    body: &str,
    scraper_id: &str,
    now: i64,
) -> anyhow::Result<Vec<JobPosting>> {
    if status_code != 200 && status_code != 404 {
        return Err(anyhow::anyhow!("HTTP {status_code}"));
    }

    let resp: Resp = serde_json::from_str(body).map_err(|e| {
        log::warn!("[jobicy] response parse failure (HTTP {status_code}): {e}");
        anyhow::anyhow!("response body did not match the expected schema")
    })?;

    Ok(rows_to_jobs(resp.jobs)
        .into_iter()
        .filter_map(|j| map_job(j, scraper_id, now))
        .collect())
}

/// Jobicy rejects a `tag` outside 3..=20 characters with HTTP 400 (live:
/// "The 'tag' parameter must contain between 3 and 20 characters", #1501), so a
/// real search phrase like "senior backend engineer" failed the whole board.
/// Over-long phrases are cut at a word boundary (hard-cut when the first word
/// alone is too long); too-short ones drop the filter. The engine's central
/// post-filters only cover location and work type, NOT query text, so
/// `search` re-applies the original query with [`matches_all_tokens`] whenever
/// the tag differs from it.
pub(crate) fn tag_param(q: &str) -> Option<String> {
    const MAX: usize = 20;
    let q = q.trim();
    let mut out = String::new();
    for word in q.split_whitespace() {
        let next = if out.is_empty() {
            word.to_string()
        } else {
            format!("{out} {word}")
        };
        if next.chars().count() > MAX {
            if out.is_empty() {
                out = word.chars().take(MAX).collect();
            }
            break;
        }
        out = next;
    }
    (out.chars().count() >= 3).then_some(out)
}

/// Every whitespace-separated token of `query` appears (case-insensitive) in the
/// posting's title or description (markdown escapes ignored).
pub(crate) fn matches_all_tokens(posting: &JobPosting, query: &str) -> bool {
    let hay = format!(
        "{} {}",
        posting.title,
        posting.description.as_deref().unwrap_or("")
    )
    .to_lowercase()
    // `html_to_markdown` backslash-escapes `_`, `*` etc. ("snake\_case"), which
    // would break tokens like `snake_case`.
    .replace('\\', "");
    query
        .split_whitespace()
        .all(|t| hay.contains(&t.to_lowercase()))
}

/// Documented maximum `count`; clamped defensively even though a live check
/// showed higher counts are honored.
const MAX_COUNT: u32 = 50;

/// Request URL for `q`, plus whether results must be re-filtered locally by the
/// full query (the tag was cut or dropped). On that path the API's own ranking
/// no longer narrows to the query, so ask for the documented maximum rather
/// than `amount` or the local filter may discard everything.
pub(crate) fn build_url(q: &str, amount: u32) -> (String, bool) {
    let tag = tag_param(q);
    let local_filter = !q.is_empty() && tag.as_deref() != Some(q);
    let count = if local_filter {
        MAX_COUNT
    } else {
        amount.clamp(1, MAX_COUNT)
    };
    let mut url = format!("https://jobicy.com/api/v2/remote-jobs?count={count}");
    if let Some(tag) = &tag {
        url.push_str(&format!("&tag={}", urlencoding::encode(tag)));
    }
    (url, local_filter)
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

    fn is_all_remote(&self) -> bool {
        true
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
        // NOTE (`supports_location` stays the trait default `false`): Jobicy's
        // `geo` param is NOT a free-text location — it only accepts a fixed set
        // of predefined `geoSlug`s (verified live: an arbitrary value 400s with
        // "Invalid 'geo' value"). Passing the user's raw location string through
        // would break the whole search on any non-matching value, so this board
        // leaves `geo` unset and lets the engine's central post-filter handle it.
        let (url, local_filter) = build_url(q, input.amount);
        // `tag` (appended by `build_url`) is LIVE-VERIFIED (2026-07-17) as a
        // relevance-ranked full-text search over title/description, not a
        // category whitelist (`tag=dev` matches "Developer Portal"). An
        // unmatched `tag` 404s with `{"success":false,...}` — a legitimate
        // zero-result search, handled in `parse_response` below.

        let res = fetch_text(&url, Default::default(), ctx.signal).await?;

        let now = chrono::Utc::now().timestamp_millis();
        // 404-vs-other-status decision + parse lives in `parse_response` (pure,
        // hermetically unit tested) — see its doc comment for the contract.
        let mut out = parse_response(res.status_code, &res.text, self.id(), now)?;
        // The tag was cut or dropped: it no longer carries the whole query, so
        // keep only jobs that do.
        if local_filter {
            out.retain(|p| matches_all_tokens(p, q));
        }

        if let Some(ref on_item) = ctx.on_item {
            for posting in &out {
                on_item(posting.clone());
            }
        }

        if let Some(ref on_progress) = ctx.on_progress {
            on_progress(1.0);
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests;

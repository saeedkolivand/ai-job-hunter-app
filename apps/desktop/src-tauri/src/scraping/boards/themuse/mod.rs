/// The Muse — public browse-feed JSON API
///
/// Endpoint: `https://www.themuse.com/api/public/jobs?page={n}` (0-indexed)
/// No free-text keyword param — this is a browse feed filtered client-side,
/// same pattern as the other keyword aggregators (Remotive/RemoteOK/Arbeitnow)
/// that have no server-side search either.
///
/// Endpoint reconnaissance ported from santifer/career-ops (MIT), `providers/themuse.mjs`.
use super::super::http::fetch_json;
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::matches_filters;
use crate::error::AppError;
use async_trait::async_trait;
use serde::Deserialize;

const BOARD_ID: &str = "themuse";
const BASE_URL: &str = "https://www.themuse.com/api/public/jobs";

/// Page request budget cap — mirrors Arbeitnow's bound, not career-ops's
/// 100-page fetch (this is a UI-triggered search, not a bulk crawl).
const MAX_PAGES: u32 = 5;

/// The Muse's actual per-page result count is unconfirmed (unverified
/// endpoint). Used only to flag likely response-shape drift on page 0: a
/// truly single-page feed rarely holds this many jobs, so `page_count == 0`
/// alongside a page this full is far more likely a renamed/dropped field
/// than a legitimate one-page result set.
const LIKELY_FULL_PAGE_LEN: usize = 10;

#[derive(Debug, Deserialize)]
struct TmCompany {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmLocation {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmRefs {
    landing_page: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TmJob {
    name: Option<String>,
    refs: Option<TmRefs>,
    company: Option<TmCompany>,
    locations: Option<Vec<TmLocation>>,
}

#[derive(Debug, Deserialize)]
struct TmResponse {
    results: Vec<TmJob>,
    #[serde(default)]
    page_count: u32,
}

/// Denominator for `on_progress`: the actual page work being done, not the
/// request BUDGET (`max_pages`). A feed with fewer real pages than the budget
/// (e.g. `page_count=2` against a `max_pages=5` cap) previously reported
/// progress against the budget, so it stalled at `2/5` instead of reaching
/// `1.0`. `total_pages` is only known once page 0 responds (before that
/// callers pass the `1` initial value, matching `resp.page_count.max(1)`),
/// and is clamped to `max_pages` in case a feed reports more pages than the
/// request is allowed to fetch.
fn progress_denominator(total_pages: u32, max_pages: u32) -> u32 {
    total_pages.min(max_pages)
}

/// Require a URL beginning with `http://` or `https://`. Cheap sanity parse —
/// `refs.landing_page` is display-only (opened by the user, never fetched by
/// us) and varies per posting's own employer/ATS host, so unlike a
/// single-host API (e.g. Rippling) there is no fixed host to lock it to.
fn is_valid_http_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .map(|u| u.scheme() == "http" || u.scheme() == "https")
        .unwrap_or(false)
}

/// Map one page's parsed jobs into postings. Standalone (no `&self`) so it is
/// unit-testable against a JSON fixture. The response has no stable job id,
/// so the (validated) posting URL doubles as the id — same precedent as
/// Breezy/Pinpoint.
pub(crate) fn parse_themuse_response(jobs: Vec<TmJob>, now: i64) -> Vec<JobPosting> {
    let mut out = Vec::new();

    for j in jobs {
        let title = j.name.unwrap_or_default().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let landing_page = j.refs.and_then(|r| r.landing_page);
        let url = match landing_page.as_deref().map(str::trim) {
            Some(u) if is_valid_http_url(u) => u.to_string(),
            _ => continue,
        };

        let company = j
            .company
            .and_then(|c| c.name)
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "The Muse".to_string());

        let location = j
            .locations
            .and_then(|locs| locs.into_iter().next())
            .and_then(|l| l.name)
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());

        out.push(JobPosting {
            id: format!("{BOARD_ID}:{url}"),
            external_id: Some(url.clone()),
            title,
            company,
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

pub struct TheMuseScraper;

#[async_trait]
impl Scraper for TheMuseScraper {
    fn id(&self) -> &'static str {
        BOARD_ID
    }

    fn display_name(&self) -> &'static str {
        "The Muse"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // No server-side keyword param — filter client-side over title+company
        // (+ location), same convention as Remotive/RemoteOK/Arbeitnow.
        let max_pages = input.pages.clamp(1, MAX_PAGES);
        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];
        let mut total_pages = 1u32;

        for page in 0..max_pages {
            if ctx.signal.is_cancelled() {
                break;
            }
            if page >= total_pages {
                break;
            }

            let data = match fetch_json::<TmResponse>(
                &format!("{BASE_URL}?page={page}"),
                Default::default(),
                ctx.signal.clone(),
            )
            .await
            {
                Ok(d) => d,
                // A cancel firing mid-fetch is a clean stop, not a failure —
                // even on page 0 with nothing collected yet, this must return
                // `Ok(out)` (empty), not bubble as an error.
                Err(AppError::Cancelled) => break,
                Err(e) if out.is_empty() => return Err(e.into()),
                Err(e) => {
                    log::warn!(
                        "[themuse] page {page} failed: {e}; returning {} collected",
                        out.len()
                    );
                    break;
                }
            };

            // A non-2xx response or a body that doesn't match the expected
            // shape comes back as `None` from `fetch_json` — treat that page
            // as empty/stop rather than erroring out the whole scrape. Page 0
            // going `None` already surfaces as an empty `out` to the caller;
            // a later page going `None` is silent truncation otherwise, so
            // log it.
            let resp = match data {
                Some(r) => r,
                None => {
                    if page > 0 {
                        log::warn!(
                            "[themuse] page {page} returned None (non-2xx or shape mismatch); stopping with {} collected",
                            out.len()
                        );
                    }
                    break;
                }
            };

            // Fail closed (1 page) on a missing/zero `page_count`, but make
            // the drift visible: a full-looking page 0 with `page_count == 0`
            // is very likely a renamed/dropped field upstream, not a
            // legitimate one-page result set — silent truncation otherwise.
            if page == 0 && resp.page_count == 0 && resp.results.len() >= LIKELY_FULL_PAGE_LEN {
                log::warn!(
                    "[themuse] page_count missing/0 but page 0 was full ({} jobs) — response shape may have drifted; results may be truncated",
                    resp.results.len()
                );
            }
            total_pages = resp.page_count.max(1);

            if resp.results.is_empty() {
                break;
            }

            for posting in parse_themuse_response(resp.results, now) {
                if !matches_filters(
                    &posting,
                    &input.query,
                    input.location.as_deref().unwrap_or(""),
                ) {
                    continue;
                }

                if let Some(ref on_item) = ctx.on_item {
                    on_item(posting.clone());
                }
                out.push(posting);
            }

            if let Some(ref on_progress) = ctx.on_progress {
                let denom = progress_denominator(total_pages, max_pages);
                on_progress((page + 1) as f32 / denom as f32);
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;

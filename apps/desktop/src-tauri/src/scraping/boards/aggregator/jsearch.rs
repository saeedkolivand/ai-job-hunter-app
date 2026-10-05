//! The JSearch `JobProvider` (paid fallback tier) — split out of
//! `providers.rs` (R8 module-size guard).
//!
//! Visibility: items here are `pub(super)` (visible to `aggregator` and its
//! descendants, including `tests`) rather than fully private, purely to
//! preserve this behavior-preserving move.
use async_trait::async_trait;
use serde::Deserialize;

use crate::observability::sanitize_reason;
use crate::scraping::http::{fetch_json, html_to_markdown, FetchOptions};
use crate::scraping::types::JobPosting;

use super::JobProvider;

// ── Date-filter helpers ────────────────────────────────────────────────────────

/// Map a UI date-filter token to JSearch's `date_posted` query token
/// (`all|today|3days|week|month`). Sub-day windows floor at `3days` — like Adzuna,
/// JSearch has no sub-day granularity, and a `today` ceiling zeroed out autopilot
/// "recent" filters on quiet days. The freshest still surface first because the
/// JSearch request pairs this window with `&sort_by=date` (JSearch defaults to
/// relevance, not recency — the sort param is what makes the guarantee true).
/// No filter / unrecognized token caps at `month`.
///
// ponytail: intentional cross-provider recency skew for sub-day tokens (e.g.
// `"24h"`). The free/cheap providers can't do sub-day granularity, so Adzuna
// (`adzuna_max_days_old` → 3) and JSearch (here → `3days`) both widen to 3 days,
// while the paid Apify/LinkedIn path (`apify_f_tpr` → `r86400`) keeps a strict
// ≤24h window. Merged results therefore mix recency windows for sub-day filters —
// the deliberate tradeoff (surface *something* over nothing on quiet days); a
// future reader shouldn't "fix" the skew back into a hard clamp.
pub(super) fn jsearch_date_posted(date_filter: Option<&str>) -> &'static str {
    match date_filter {
        Some("15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "24h") => "3days",
        Some("week") => "week",
        _ => "month",
    }
}

// ── JSearch paging budget ─────────────────────────────────────────────────────

/// Production JSearch host (RapidAPI). Tests pass a local `wiremock` base —
/// mirrors [`JOOBLE_BASE_URL`] / `adzuna::ADZUNA_BASE_URL`.
pub(super) const JSEARCH_BASE_URL: &str = "https://jsearch.p.rapidapi.com";

/// Results JSearch returns per page.
pub(super) const JSEARCH_PAGE_SIZE: u32 = 10;

/// Hard ceiling on JSearch's `num_pages`.
///
// ponytail: JSearch is billed PER REQUEST and `num_pages` is charged
// multiplicatively (a 3-page request costs 3 calls), and it is only the FALLBACK
// tier — it fires when Adzuna is unconfigured or failed. 3 pages (≤30 postings)
// buys a usable result set without turning one fallback search into a double-digit
// bill. Like Adzuna's budget this is driven by the requested AMOUNT, never by
// `BoardSearchInput::pages`.
pub(super) const JSEARCH_MAX_PAGES: u32 = 3;

/// INVARIANT: retries=0 for every JSearch request (mirrors [`APIFY_RETRIES`] and
/// [`super::ADZUNA_RETRIES`]).
///
/// JSearch is billed PER REQUEST against a monthly RapidAPI plan, and one call
/// already costs `num_pages` of it. `fetch_text` re-sends on 429/503, so the
/// default `retries: 2` would make a 3-page fallback cost up to 9 billed calls —
/// and a 429 from a metered API IS the over-quota signal, so retrying it spends
/// the very budget that just ran out.
pub(super) const JSEARCH_RETRIES: u32 = 0;

/// JSearch `num_pages` for a target result count: `ceil(amount / 10)` clamped to
/// [`JSEARCH_MAX_PAGES`]. `None` → 1 (the pre-paging, cheapest behavior);
/// `amount = 0` still asks for one page, never zero.
pub(super) fn jsearch_num_pages(amount: Option<u32>) -> u32 {
    amount.map_or(1, |a| {
        a.div_ceil(JSEARCH_PAGE_SIZE).clamp(1, JSEARCH_MAX_PAGES)
    })
}

/// Build the `FetchOptions` for the JSearch search call.
///
/// The RapidAPI key goes in a header only — never the URL. `retries` is hardwired
/// to [`JSEARCH_RETRIES`] (0). Mirrors [`apify_fetch_options`]: the single source
/// of truth consumed by both the production call in `JSearchProvider::search` and
/// the invariant test in `tests/quota_neutral_runs.rs`, so dropping the override here breaks that test.
pub(super) fn jsearch_fetch_options(api_key: &str) -> FetchOptions {
    FetchOptions {
        headers: Some(vec![
            ("X-RapidAPI-Key".to_string(), api_key.to_string()),
            (
                "X-RapidAPI-Host".to_string(),
                "jsearch.p.rapidapi.com".to_string(),
            ),
        ]),
        retries: JSEARCH_RETRIES, // METERED: every send is a billed call
        ..FetchOptions::default()
    }
}

/// Build the JSearch search endpoint. Factored out of `JSearchProvider::search`
/// (mirrors [`jooble_endpoint`]) so the amount → `num_pages` mapping is pinned to
/// the URL that actually goes on the wire, without a network round trip.
///
/// `sort_by=date` pairs with the widened `date_posted` window: JSearch defaults to
/// relevance, which does NOT put the freshest posting on top, so the sort param is
/// what makes the freshness guarantee documented on [`jsearch_date_posted`] true.
pub(super) fn jsearch_url(
    base_url: &str,
    combined_query: &str,
    date_filter: Option<&str>,
    amount: Option<u32>,
) -> String {
    format!(
        "{base_url}/search?query={}&page=1&num_pages={}&date_posted={}&sort_by=date",
        urlencoding::encode(combined_query),
        jsearch_num_pages(amount),
        jsearch_date_posted(date_filter),
    )
}

// ── JSearch provider ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(super) struct JSearchJob {
    pub(super) job_id: String,
    pub(super) job_title: String,
    pub(super) employer_name: Option<String>,
    pub(super) job_city: Option<String>,
    pub(super) job_country: Option<String>,
    pub(super) job_apply_link: Option<String>,
    pub(super) job_google_link: Option<String>,
    pub(super) job_description: Option<String>,
    pub(super) job_posted_at_datetime_utc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct JSearchResp {
    pub(super) data: Vec<JSearchJob>,
}

pub(crate) struct JSearchProvider {
    pub(super) api_key: Option<String>,
}

impl JSearchProvider {
    pub(super) fn new() -> Self {
        use crate::ipc_contracts::provider_slots::JSEARCH_KEY;
        Self {
            api_key: crate::credentials::read_credential(&format!("ai:{JSEARCH_KEY}"))
                .unwrap_or_else(|e| {
                    log::warn!(
                        "[aggregator] {JSEARCH_KEY} keyring error: {}",
                        sanitize_reason(&e.to_string())
                    );
                    None
                }),
        }
    }
}

#[async_trait]
impl JobProvider for JSearchProvider {
    fn provider_id(&self) -> &'static str {
        "jsearch"
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }

    async fn search(
        &self,
        query: &str,
        location: &str,
        _country: &str,
        _country_guessed: bool,
        date_filter: Option<&str>,
        amount: Option<u32>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        if !self.is_configured() {
            return Err(anyhow::anyhow!("jsearch: not configured"));
        }

        let api_key = self.api_key.as_deref().unwrap_or("");

        // JSearch takes a single free-text query field; combine query + location.
        let combined = if location.is_empty() {
            query.to_string()
        } else {
            format!("{query} in {location}")
        };
        // Unlike Adzuna, JSearch returns N pages from ONE request (`num_pages`),
        // so the amount budget is a query param, not a loop.
        let url = jsearch_url(JSEARCH_BASE_URL, &combined, date_filter, amount);

        // A non-2xx or schema-drift response propagates as `Err` from `fetch_json`
        // (carrying the HTTP status); `?` surfaces it as a provider failure. The
        // "jsearch:" prefix is required — the aggregator board fronts three
        // providers, so an unattributed "HTTP 403" in BoardScrapeSummary.error
        // wouldn't say which one failed.
        let resp = fetch_json::<JSearchResp>(&url, jsearch_fetch_options(api_key), signal)
            .await
            .map_err(|e| anyhow::anyhow!("jsearch: {e}"))?;

        let now = chrono::Utc::now().timestamp_millis();
        let postings = resp
            .data
            .into_iter()
            .filter_map(|j| {
                let url = j
                    .job_apply_link
                    .clone()
                    .or_else(|| j.job_google_link.clone())?;
                let location = match (j.job_city.as_deref(), j.job_country.as_deref()) {
                    (Some(c), Some(co)) if !c.is_empty() && !co.is_empty() => {
                        Some(format!("{c}, {co}"))
                    }
                    (Some(c), _) if !c.is_empty() => Some(c.to_string()),
                    (_, Some(co)) if !co.is_empty() => Some(co.to_string()),
                    _ => None,
                };
                Some(JobPosting {
                    id: format!("aggregator:jsearch-{}", j.job_id),
                    external_id: Some(format!("jsearch-{}", j.job_id)),
                    title: j.job_title,
                    company: j.employer_name.unwrap_or_default(),
                    location,
                    url,
                    source: "aggregator".to_string(),
                    description: j.job_description.map(|d| html_to_markdown(&d)),
                    requirements: None,
                    posted_at: j
                        .job_posted_at_datetime_utc
                        .as_deref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.timestamp_millis()),
                    captured_at: now,
                    extra: std::collections::HashMap::new(),
                })
            })
            .collect();

        Ok(postings)
    }
}

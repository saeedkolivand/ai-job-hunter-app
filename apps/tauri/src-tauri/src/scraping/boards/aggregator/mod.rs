/// Aggregator board — Adzuna (primary) with JSearch (paid fallback).
///
/// Design:
/// * One `Scraper` in the registry (id = `"aggregator"`).
/// * Internally holds an ordered `JobProvider` registry: Adzuna → JSearch.
/// * Fallback semantics (enforced in `search_with_providers`):
///   - Adzuna configured + `Ok(items)` (even empty) → use those, do NOT call JSearch.
///   - Adzuna configured + `Err(_)` → log, try JSearch if configured.
///   - Adzuna not configured → try JSearch if configured.
///   - Neither configured → `Ok(vec![])` (keyless-empty, never an error).
/// * Keys are optional: absent = keyless-empty.  Never hardcoded, never logged.
/// * Keys are read from the OS keychain via `credentials::read_credential`:
///   - `ai:adzuna-app-id`   — Adzuna application ID
///   - `ai:adzuna-app-key`  — Adzuna application key
///   - `ai:jsearch-key`     — RapidAPI key for JSearch
///
/// Rate-limiting and cancellation are honoured: every network call flows
/// through `scraping::http::fetch_json` (which checks `ctx.signal` and calls
/// the per-host `rate_limiter`).
use async_trait::async_trait;
use serde::Deserialize;

use crate::scraping::http::{fetch_json, strip_html, FetchOptions};
use crate::scraping::types::{
    AuthRequirement, BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode,
};

// ── Serde helpers ─────────────────────────────────────────────────────────────

/// Accept either a JSON string or a JSON integer for the Adzuna `id` field,
/// normalizing both to `String`.  Adzuna documents the field as a string but
/// the live API returns it as an integer (e.g. `331705081`).
fn de_string_or_number<'de, D>(de: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        Str(String),
        Num(i64),
    }

    match StringOrNumber::deserialize(de)? {
        StringOrNumber::Str(s) => Ok(s),
        StringOrNumber::Num(n) => Ok(n.to_string()),
    }
}

// ── Date-filter helpers ────────────────────────────────────────────────────────

/// Map a UI date-filter token to Adzuna's `max_days_old` integer (whole days).
///
// ponytail: Adzuna's recency granularity is whole days, so all sub-day options
// collapse to 1 day (API ceiling). No filter / unrecognized token caps at 30 days
// so the aggregator never surfaces postings older than a month.
fn adzuna_max_days_old(date_filter: Option<&str>) -> u32 {
    match date_filter {
        Some("30m" | "1h" | "2h" | "4h" | "8h" | "24h") => 1,
        Some("week") => 7,
        _ => 30,
    }
}

/// Map a UI date-filter token to JSearch's `date_posted` query token. No filter /
/// unrecognized token caps at `month` (results no older than the past month).
fn jsearch_date_posted(date_filter: Option<&str>) -> &'static str {
    match date_filter {
        Some("30m" | "1h" | "2h" | "4h" | "8h" | "24h") => "today",
        Some("week") => "week",
        _ => "month",
    }
}

// ── Provider trait ────────────────────────────────────────────────────────────

/// A single search-API backend.  Object-safe so the scraper can hold a
/// `Vec<Box<dyn JobProvider>>` without generics leaking into the `Scraper` trait.
#[async_trait]
pub(crate) trait JobProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    /// True when the necessary API keys are present in the credential store.
    fn is_configured(&self) -> bool;
    /// Run a search.  Non-2xx or network errors are returned as `Err`.
    async fn search(
        &self,
        query: &str,
        location: &str,
        country: &str,
        date_filter: Option<&str>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>>;
}

// ── Adzuna provider ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AdzunaCompany {
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdzunaLocation {
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdzunaJob {
    #[serde(deserialize_with = "de_string_or_number")]
    id: String,
    title: String,
    company: Option<AdzunaCompany>,
    location: Option<AdzunaLocation>,
    redirect_url: String,
    description: Option<String>,
    created: Option<String>,
    salary_min: Option<f64>,
    salary_max: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct AdzunaResp {
    results: Vec<AdzunaJob>,
}

pub(crate) struct AdzunaProvider {
    app_id: Option<String>,
    app_key: Option<String>,
}

impl AdzunaProvider {
    fn new() -> Self {
        Self {
            app_id: crate::credentials::read_credential("ai:adzuna-app-id").unwrap_or_else(|e| {
                log::warn!("[aggregator] adzuna-app-id keyring error: {e}");
                None
            }),
            app_key: crate::credentials::read_credential("ai:adzuna-app-key").unwrap_or_else(|e| {
                log::warn!("[aggregator] adzuna-app-key keyring error: {e}");
                None
            }),
        }
    }
}

#[async_trait]
impl JobProvider for AdzunaProvider {
    fn provider_id(&self) -> &'static str {
        "adzuna"
    }

    fn is_configured(&self) -> bool {
        self.app_id.is_some() && self.app_key.is_some()
    }

    async fn search(
        &self,
        query: &str,
        location: &str,
        country: &str,
        date_filter: Option<&str>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        if !self.is_configured() {
            return Err(anyhow::anyhow!("adzuna: not configured"));
        }

        let app_id = self.app_id.as_deref().unwrap_or("");
        let app_key = self.app_key.as_deref().unwrap_or("");

        let country = if country.is_empty() { "de" } else { country };
        let country_enc = urlencoding::encode(country);
        let app_id_enc = urlencoding::encode(app_id);
        let app_key_enc = urlencoding::encode(app_key);
        let q_enc = urlencoding::encode(query);
        let loc_enc = urlencoding::encode(location);

        let mut url = format!(
            "https://api.adzuna.com/v1/api/jobs/{}/search/1\
             ?app_id={}&app_key={}&what={}&where={}&results_per_page=50&content-type=application/json",
            country_enc, app_id_enc, app_key_enc, q_enc, loc_enc
        );

        // Sort newest-first (Adzuna defaults to relevance, which floats stale
        // postings up) and always bound the window with max_days_old so nothing
        // older than the cap (30 days, or the user's tighter pick) is returned.
        url.push_str(&format!(
            "&sort_by=date&sort_direction=down&max_days_old={}",
            adzuna_max_days_old(date_filter)
        ));

        let result = fetch_json::<AdzunaResp>(&url, FetchOptions::default(), signal).await?;
        let resp = match result {
            Some(r) => r,
            None => {
                return Err(anyhow::anyhow!(
                    "adzuna: non-2xx response or unparseable body"
                ))
            }
        };

        let now = chrono::Utc::now().timestamp_millis();
        let postings = resp
            .results
            .into_iter()
            .map(|j| {
                let mut extra = std::collections::HashMap::new();
                if let Some(min) = j.salary_min {
                    extra.insert("salaryMin".to_string(), serde_json::json!(min));
                }
                if let Some(max) = j.salary_max {
                    extra.insert("salaryMax".to_string(), serde_json::json!(max));
                }
                JobPosting {
                    id: format!("aggregator:adzuna-{}", j.id),
                    external_id: Some(format!("adzuna-{}", j.id)),
                    title: j.title,
                    company: j.company.and_then(|c| c.display_name).unwrap_or_default(),
                    location: j.location.and_then(|l| l.display_name),
                    url: j.redirect_url,
                    source: "aggregator".to_string(),
                    description: j.description.map(|d| strip_html(&d)),
                    requirements: None,
                    posted_at: j
                        .created
                        .as_deref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.timestamp_millis()),
                    captured_at: now,
                    extra,
                }
            })
            .collect();

        Ok(postings)
    }
}

// ── JSearch provider ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct JSearchJob {
    job_id: String,
    job_title: String,
    employer_name: Option<String>,
    job_city: Option<String>,
    job_country: Option<String>,
    job_apply_link: Option<String>,
    job_google_link: Option<String>,
    job_description: Option<String>,
    job_posted_at_datetime_utc: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JSearchResp {
    data: Vec<JSearchJob>,
}

pub(crate) struct JSearchProvider {
    api_key: Option<String>,
}

impl JSearchProvider {
    fn new() -> Self {
        Self {
            api_key: crate::credentials::read_credential("ai:jsearch-key").unwrap_or_else(|e| {
                log::warn!("[aggregator] jsearch-key keyring error: {e}");
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
        date_filter: Option<&str>,
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
        let q_enc = urlencoding::encode(&combined);
        let mut url = format!(
            "https://jsearch.p.rapidapi.com/search?query={}&page=1&num_pages=1",
            q_enc
        );

        url.push_str(&format!("&date_posted={}", jsearch_date_posted(date_filter)));

        let result = fetch_json::<JSearchResp>(
            &url,
            FetchOptions {
                headers: Some(vec![
                    ("X-RapidAPI-Key".to_string(), api_key.to_string()),
                    (
                        "X-RapidAPI-Host".to_string(),
                        "jsearch.p.rapidapi.com".to_string(),
                    ),
                ]),
                ..FetchOptions::default()
            },
            signal,
        )
        .await?;

        let resp = match result {
            Some(r) => r,
            None => {
                return Err(anyhow::anyhow!(
                    "jsearch: non-2xx response or unparseable body"
                ))
            }
        };

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
                    description: j.job_description.map(|d| strip_html(&d)),
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

// ── Fallback logic ────────────────────────────────────────────────────────────

/// Run the provider chain: Adzuna primary, JSearch fallback.
///
/// Fallback rule (spec):
/// - Adzuna configured, `Ok(items)` (even empty) → return those; skip JSearch.
/// - Adzuna configured, `Err(_)`                 → log; try JSearch if configured.
/// - Adzuna not configured                        → try JSearch if configured.
/// - Neither configured                           → `Ok(vec![])` (keyless-empty).
///
/// Items from each provider are keyed by their `external_id` to deduplicate.
async fn search_with_providers(
    providers: &[Box<dyn JobProvider>],
    query: &str,
    location: &str,
    country: &str,
    date_filter: Option<&str>,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    if signal.is_cancelled() {
        return Ok(vec![]);
    }

    // Locate primary (Adzuna) and fallback (JSearch) by id.
    let primary = providers.iter().find(|p| p.provider_id() == "adzuna");
    let fallback = providers.iter().find(|p| p.provider_id() == "jsearch");

    // Run primary if configured.
    if let Some(p) = primary {
        if p.is_configured() {
            match p.search(query, location, country, date_filter, signal.clone()).await {
                Ok(items) => {
                    // Even empty → use result as-is; do NOT fall through to JSearch.
                    return Ok(dedupe(items));
                }
                Err(e) => {
                    log::warn!("[aggregator] adzuna error, attempting jsearch fallback: {e}");
                    // Fall through to JSearch below.
                }
            }
        }
    }

    // Guard: don't fire a paid JSearch call after cancellation.
    if signal.is_cancelled() {
        return Ok(vec![]);
    }

    // Try fallback.
    if let Some(f) = fallback {
        if f.is_configured() {
            return f.search(query, location, country, date_filter, signal).await.map(dedupe);
        }
    }

    // Neither configured.
    Ok(vec![])
}

/// Deduplicate by `external_id`, preserving first-seen order.
fn dedupe(items: Vec<JobPosting>) -> Vec<JobPosting> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|p| {
            let key = p.external_id.clone().unwrap_or_else(|| p.url.clone());
            seen.insert(key)
        })
        .collect()
}

// ── Scraper impl ──────────────────────────────────────────────────────────────

pub struct AggregatorScraper;

#[async_trait]
impl Scraper for AggregatorScraper {
    fn id(&self) -> &'static str {
        "aggregator"
    }

    fn display_name(&self) -> &'static str {
        "Aggregated Jobs"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    fn auth(&self) -> AuthRequirement {
        // Keys are optional config, not a login requirement.
        AuthRequirement::Guest
    }

    fn requires_company(&self) -> bool {
        false
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let query = input.query.trim();
        let location = input.location.as_deref().unwrap_or("").trim();
        let country = input
            .country_code
            .as_deref()
            .map(str::to_lowercase)
            .unwrap_or_else(|| "de".to_string());

        // Construct providers fresh per call so that key changes made in Settings
        // take effect immediately without requiring an app restart.
        let providers: Vec<Box<dyn JobProvider>> = vec![
            Box::new(AdzunaProvider::new()),
            Box::new(JSearchProvider::new()),
        ];
        let items = search_with_providers(
            &providers,
            query,
            location,
            &country,
            input.date_filter.as_deref(),
            ctx.signal.clone(),
        )
        .await?;

        let amount = input.amount as usize;
        let mut out = Vec::new();

        for posting in items.into_iter().take(amount) {
            if ctx.signal.is_cancelled() {
                break;
            }
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

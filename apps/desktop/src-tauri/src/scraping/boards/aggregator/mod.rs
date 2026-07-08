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
/// * Keys are read from the OS keychain via `credentials::read_credential`,
///   under the `ai:` keyring namespace + the BARE slot names generated from the
///   cross-language source of truth in `ipc_contracts::provider_slots`
///   (`packages/shared/src/provider-slots.ts`):
///   - `ai:adzuna-app-id`   (`provider_slots::ADZUNA_APP_ID`)  — Adzuna application ID
///   - `ai:adzuna-app-key`  (`provider_slots::ADZUNA_APP_KEY`) — Adzuna application key
///   - `ai:jsearch-key`     (`provider_slots::JSEARCH_KEY`)    — RapidAPI key for JSearch
///   - `ai:apify-token`     (`provider_slots::APIFY_TOKEN`)    — Apify Bearer token
///
/// Rate-limiting and cancellation are honoured: every network call flows
/// through `scraping::http::fetch_json` (which checks `ctx.signal` and calls
/// the per-host `rate_limiter`).
use async_trait::async_trait;
use serde::Deserialize;

use crate::scraping::http::{fetch_json, html_to_markdown, FetchOptions};
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

/// Like [`de_string_or_number`] but for an OPTIONAL field, tolerating `null` /
/// absent / string / number. Used for the Apify actor's loosely-typed `id` and
/// `postedAt`, which vary by run (string id, numeric id, or omitted).
fn de_opt_string_or_number<'de, D>(de: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(de)?;
    Ok(value.and_then(|v| match v {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }))
}

// ── Date-filter helpers ────────────────────────────────────────────────────────

/// Map a UI date-filter token to Adzuna's `max_days_old` integer (whole days).
///
// ponytail: Adzuna's recency granularity is whole days — it can't do sub-day. A
// 1-day ceiling zeroed out autopilot "recent" filters on quiet days (a normal
// query returns near-nothing in a single day), so sub-day windows FLOOR at 3 days
// and rely on the query's `sort_by=date` for freshness instead of a hard clamp.
// No filter / unrecognized token caps at 30 days so the aggregator never surfaces
// postings older than a month. (Coarse mapping; 3-day floor / 30-day ceiling.)
fn adzuna_max_days_old(date_filter: Option<&str>) -> u32 {
    match date_filter {
        Some("15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "24h") => 3,
        Some("week") => 7,
        _ => 30,
    }
}

/// Map a UI date-filter token to JSearch's `date_posted` query token
/// (`all|today|3days|week|month`). Sub-day windows floor at `3days` — like Adzuna,
/// JSearch has no sub-day granularity, and a `today` ceiling zeroed out autopilot
/// "recent" filters on quiet days (results are date-sorted, so the freshest still
/// surface first). No filter / unrecognized token caps at `month`.
fn jsearch_date_posted(date_filter: Option<&str>) -> &'static str {
    match date_filter {
        Some("15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "24h") => "3days",
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
    ///
    /// `amount` is a provider-specific result cap that callers may pass to
    /// cost-bounded providers (currently Apify only).  Providers that have no
    /// concept of a cap ignore it (`_amount`).
    async fn search(
        &self,
        query: &str,
        location: &str,
        country: &str,
        date_filter: Option<&str>,
        amount: Option<u32>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>>;
}

// ── Adzuna supported-country allowlist ───────────────────────────────────────

/// ISO 3166-1 alpha-2 country codes hosted by Adzuna's job-search API.
///
/// Source: Adzuna API documentation at <https://api.adzuna.com/v1/doc>
/// (path-parameter enumeration visible in the interactive endpoint reference).
/// Verified against the known set as of 2026-06-23; update this list if
/// Adzuna adds new markets (the path `/v1/api/jobs/{country}/search/1` returns
/// a non-2xx error body for any code not in this set, which is indistinguishable
/// from an auth failure without real keys — see code comment in `search`).
const ADZUNA_SUPPORTED_COUNTRIES: &[&str] = &[
    "at", // Austria
    "au", // Australia
    "be", // Belgium
    "br", // Brazil
    "ca", // Canada
    "ch", // Switzerland
    "de", // Germany
    "es", // Spain
    "fr", // France
    "gb", // United Kingdom
    "in", // India
    "it", // Italy
    "mx", // Mexico
    "nl", // Netherlands
    "nz", // New Zealand
    "pl", // Poland
    "sg", // Singapore
    "us", // United States
    "za", // South Africa
];

#[inline]
fn adzuna_supports_country(country: &str) -> bool {
    ADZUNA_SUPPORTED_COUNTRIES.contains(&country)
}

/// ISO-4217 currency for an Adzuna market. Adzuna's search API returns
/// `salary_min`/`salary_max` as bare numbers with no currency field, so the
/// currency has to be derived from the country the search targeted — one entry
/// per code in [`ADZUNA_SUPPORTED_COUNTRIES`]. `None` for any country not in
/// that list (the salary answer then falls back to a web lookup for currency
/// instead of guessing).
#[inline]
fn adzuna_currency_for_country(country: &str) -> Option<&'static str> {
    Some(match country {
        "at" | "be" | "de" | "es" | "fr" | "it" | "nl" => "EUR",
        "au" => "AUD",
        "br" => "BRL",
        "ca" => "CAD",
        "ch" => "CHF",
        "gb" => "GBP",
        "in" => "INR",
        "mx" => "MXN",
        "nz" => "NZD",
        "pl" => "PLN",
        "sg" => "SGD",
        "us" => "USD",
        "za" => "ZAR",
        _ => return None,
    })
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
        use crate::ipc_contracts::provider_slots::{ADZUNA_APP_ID, ADZUNA_APP_KEY};
        Self {
            app_id: crate::credentials::read_credential(&format!("ai:{ADZUNA_APP_ID}"))
                .unwrap_or_else(|e| {
                    log::warn!("[aggregator] {ADZUNA_APP_ID} keyring error: {e}");
                    None
                }),
            app_key: crate::credentials::read_credential(&format!("ai:{ADZUNA_APP_KEY}"))
                .unwrap_or_else(|e| {
                    log::warn!("[aggregator] {ADZUNA_APP_KEY} keyring error: {e}");
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
        _amount: Option<u32>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        if !self.is_configured() {
            return Err(anyhow::anyhow!("adzuna: not configured"));
        }

        // Reject unsupported countries before issuing any HTTP request.
        // Adzuna only hosts a fixed set of markets; an unsupported country code
        // would produce a non-2xx response (indistinguishable from an auth error
        // at the HTTP level without real keys). Returning Err here lets the
        // `search_with_providers` fallback chain transparently route to JSearch
        // (which uses free-text location and is globally scoped).
        let country = if country.is_empty() { "de" } else { country };
        if !adzuna_supports_country(country) {
            return Err(anyhow::anyhow!(
                "adzuna: country '{country}' is not in Adzuna's supported market list \
                 (supported: {}); configure a JSearch key for global coverage",
                ADZUNA_SUPPORTED_COUNTRIES.join(", ")
            ));
        }

        let app_id = self.app_id.as_deref().unwrap_or("");
        let app_key = self.app_key.as_deref().unwrap_or("");
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
            .map(|j| adzuna_job_to_posting(j, country, now))
            .collect();

        Ok(postings)
    }
}

/// Map one Adzuna result to a [`JobPosting`], deriving `extra.salaryCurrency`
/// from `country` (Adzuna reports bare salary numbers with no currency field).
/// Pulled out of `AdzunaProvider::search` so it's unit-testable without a
/// network call.
fn adzuna_job_to_posting(j: AdzunaJob, country: &str, now: i64) -> JobPosting {
    let mut extra = std::collections::HashMap::new();
    let has_salary = j.salary_min.is_some() || j.salary_max.is_some();
    if let Some(min) = j.salary_min {
        extra.insert("salaryMin".to_string(), serde_json::json!(min));
    }
    if let Some(max) = j.salary_max {
        extra.insert("salaryMax".to_string(), serde_json::json!(max));
    }
    // Currency is only meaningful alongside an amount; an unmapped country
    // omits it so the downstream salary answer falls back to a web lookup
    // instead of showing a wrong/absent currency.
    if has_salary {
        if let Some(currency) = adzuna_currency_for_country(country) {
            extra.insert("salaryCurrency".to_string(), serde_json::json!(currency));
        }
    }
    JobPosting {
        id: format!("aggregator:adzuna-{}", j.id),
        external_id: Some(format!("adzuna-{}", j.id)),
        title: j.title,
        company: j.company.and_then(|c| c.display_name).unwrap_or_default(),
        location: j.location.and_then(|l| l.display_name),
        url: j.redirect_url,
        source: "aggregator".to_string(),
        description: j.description.map(|d| html_to_markdown(&d)),
        requirements: None,
        posted_at: j
            .created
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp_millis()),
        captured_at: now,
        extra,
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
        use crate::ipc_contracts::provider_slots::JSEARCH_KEY;
        Self {
            api_key: crate::credentials::read_credential(&format!("ai:{JSEARCH_KEY}"))
                .unwrap_or_else(|e| {
                    log::warn!("[aggregator] {JSEARCH_KEY} keyring error: {e}");
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
        _amount: Option<u32>,
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

        url.push_str(&format!(
            "&date_posted={}",
            jsearch_date_posted(date_filter)
        ));

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

// ── Non-secret aggregator settings (plugin-store JSON) ──────────────────────────
//
// The "Include LinkedIn (Apify)" opt-in toggle and the optional actor-id override
// are NOT secrets, so they do not belong in the OS keychain. The renderer persists
// them with `@tauri-apps/plugin-store` to `<app_data_dir>/scraping-settings.json`;
// plugin-store resolves a relative store path against the app data dir — the SAME
// directory `platform::config::data_dir()` resolves for AppHandle-less workers, so
// the provider can read them here without an `AppHandle` (mirrors how API keys are
// read AppHandle-free via `credentials::read_credential`).
//
// The file name + key strings are the cross-language contract in
// `packages/shared/src/scraping-settings.ts`; the literals below are pinned to it
// by `aggregator_settings_keys_match_shared_contract` in `test.rs`.
const SCRAPING_SETTINGS_FILE: &str = "scraping-settings.json";
const SETTING_APIFY_ENABLED: &str = "apifyLinkedinEnabled";
const SETTING_APIFY_ACTOR_ID: &str = "apifyLinkedinActorId";

#[derive(Debug, Default, Clone)]
struct AggregatorSettings {
    /// Master opt-in for the paid Apify LinkedIn provider. Default `false`.
    apify_linkedin_enabled: bool,
    /// Optional actor-id override; `None` → the built-in default actor.
    apify_linkedin_actor_id: Option<String>,
}

/// Read the non-secret aggregator settings from the plugin-store JSON file.
///
/// Absent file, parse failure, or missing keys all degrade to defaults (toggle
/// OFF) — never an error. A missing/garbled settings file must never crash a
/// user-triggered search; it simply means the opt-in provider stays disabled.
fn read_aggregator_settings() -> AggregatorSettings {
    let path = crate::platform::config::data_dir().join(SCRAPING_SETTINGS_FILE);
    let json: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);

    let apify_linkedin_enabled = json
        .get(SETTING_APIFY_ENABLED)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let apify_linkedin_actor_id = json
        .get(SETTING_APIFY_ACTOR_ID)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    AggregatorSettings {
        apify_linkedin_enabled,
        apify_linkedin_actor_id,
    }
}

// ── Apify LinkedIn provider (additive, paid) ────────────────────────────────────

/// Default Apify actor: scrapes public LinkedIn jobs with no LinkedIn login,
/// billed pay-per-event (~$1.00 / 1000 results). Overridable via the non-secret
/// `apifyLinkedinActorId` setting.
const APIFY_DEFAULT_ACTOR: &str = "curious_coder~linkedin-jobs-scraper";

// ponytail: HARD cost ceiling. Apify bills per dataset result, so every run is
// bounded by `count = APIFY_MAX_ITEMS`; we NEVER issue an unbounded fetch. The
// opt-in toggle (gated in `is_configured`) is the second, mandatory cost gate —
// a stored token ALONE never triggers a paid run.
const APIFY_MAX_ITEMS: u32 = 50;

/// `run-sync-get-dataset-items` is capped at 300s server-side (returns 408 on
/// timeout); give the client a matching wall-clock ceiling so a stalled actor
/// run can't hang the scrape.
const APIFY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Server-side USD ceiling for pay-per-event actor overrides. Belt-and-suspenders
/// on top of `maxItems`: a user who overrides the actor to a pay-per-event model
/// is still bounded by this hard Apify platform limit.
const APIFY_MAX_CHARGE_USD: &str = "1.00";

/// INVARIANT: retries=0 for every Apify `run-sync-get-dataset-items` call.
/// The endpoint is NON-IDEMPOTENT and billed per result — a retry on 429/503/network
/// would start ANOTHER charged actor run (up to 3× cost with the default retries=2).
/// Shared by production code and tests so a change to either breaks the invariant check.
const APIFY_RETRIES: u32 = 0;

/// Validate an Apify actor id against the platform grammar `user~actor`.
///
/// Both parts must be non-empty and consist solely of `[A-Za-z0-9_.-]`.
/// A malformed id injected via `apifyLinkedinActorId` could otherwise reach
/// the API URL (even though the host is fixed, a path-traversal like
/// `../../v1/…` is still a concern). An invalid id falls back silently to
/// `APIFY_DEFAULT_ACTOR` — the provider logs a warning and continues.
fn is_valid_apify_actor_id(id: &str) -> bool {
    let valid_part = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
    };
    match id.split_once('~') {
        Some((user, actor)) => valid_part(user) && valid_part(actor),
        None => false,
    }
}

/// Build the Apify `run-sync-get-dataset-items` endpoint URL for a given actor.
///
/// `max_items` is the server-side platform cap for this request; callers compute
/// it as `min(APIFY_MAX_ITEMS, amount - primary.len())` so we never fetch more
/// than actually needed.  The Bearer token is NEVER included here — it goes in
/// the `Authorization` header only, keeping it out of request-URL logging.
///
/// This is the single source of truth consumed by both the production call in
/// [`ApifyLinkedInProvider::search`] and the invariant test in `test.rs`. A future
/// refactor that removes either cap would break the test that calls this function.
fn build_apify_endpoint(actor_id: &str, max_items: u32) -> String {
    format!(
        "https://api.apify.com/v2/acts/{}/run-sync-get-dataset-items\
         ?maxItems={}&maxTotalChargeUsd={}",
        actor_id, max_items, APIFY_MAX_CHARGE_USD
    )
}

/// Map a UI date-filter token to LinkedIn's `f_TPR` recency parameter. Sub-day
/// windows collapse to the past 24h (`r86400`); `week` → `r604800`; everything
/// else (month / no filter / unknown) caps at the past month (`r2592000`),
/// mirroring the 30-day ceiling the other providers enforce.
fn apify_f_tpr(date_filter: Option<&str>) -> &'static str {
    match date_filter {
        Some("15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "24h") => "r86400",
        Some("week") => "r604800",
        _ => "r2592000",
    }
}

/// Build the public LinkedIn jobs-search URL the actor expects as input (it
/// scrapes pre-built search URLs, not a raw keyword string). Query + location are
/// percent-encoded; recency comes from [`apify_f_tpr`].
fn build_linkedin_search_url(query: &str, location: &str, date_filter: Option<&str>) -> String {
    let q = urlencoding::encode(query);
    let loc = urlencoding::encode(location);
    let f_tpr = apify_f_tpr(date_filter);
    format!("https://www.linkedin.com/jobs/search/?keywords={q}&location={loc}&f_TPR={f_tpr}")
}

/// Try to parse the actor's `postedAt` into epoch millis: RFC-3339 first, then a
/// bare epoch (seconds scaled to millis, or millis as-is). A relative string
/// ("2 weeks ago") yields `None` — an absent posted date is acceptable.
fn parse_apify_posted_at(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    if let Ok(n) = s.parse::<i64>() {
        // < 10^12 ≈ seconds (any plausible ms epoch is far larger).
        return Some(if n < 1_000_000_000_000 { n * 1000 } else { n });
    }
    None
}

/// One dataset item from the Apify actor. Every field is optional + defensively
/// aliased: the actor's output shape drifts between runs/versions, so we accept
/// the documented field names plus sensible fallbacks and skip anything unusable.
#[derive(Debug, Clone, Deserialize)]
struct ApifyItem {
    #[serde(default, alias = "jobTitle")]
    title: Option<String>,
    #[serde(default, rename = "companyName")]
    company_name: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default, rename = "jobUrl")]
    job_url: Option<String>,
    #[serde(default, deserialize_with = "de_opt_string_or_number")]
    id: Option<String>,
    #[serde(
        default,
        rename = "postedAt",
        deserialize_with = "de_opt_string_or_number"
    )]
    posted_at: Option<String>,
    #[serde(default, rename = "descriptionText")]
    description_text: Option<String>,
    #[serde(default, rename = "jobDescription")]
    job_description: Option<String>,
    #[serde(default, rename = "descriptionHtml")]
    description_html: Option<String>,
}

/// Validate that a URL from the Apify actor is HTTPS on a `linkedin.com` host.
///
/// A drifting or user-overridden actor could inject arbitrary URLs into
/// `JobPosting.url`.  We constrain the output to the only expected domain
/// (`linkedin.com` / `*.linkedin.com`) and scheme (`https`).  Items whose URL
/// fails validation are dropped — same as items missing title/url.
fn is_valid_apify_linkedin_url(url: &str) -> bool {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        // host_str() is already lowercase after Url::parse (URL standard).
        return parsed.scheme() == "https"
            && parsed
                .host_str()
                .is_some_and(|h| h == "linkedin.com" || h.ends_with(".linkedin.com"));
    }
    false
}

/// Build the `FetchOptions` for the Apify `run-sync-get-dataset-items` call.
///
/// The Bearer token goes in the Authorization header only — never the URL.
/// `retries` is hardwired to `APIFY_RETRIES` (0): the endpoint is NON-IDEMPOTENT
/// and billed per result; a retry would start another charged run.
///
/// This is the single source of truth consumed by [`ApifyLinkedInProvider::search`]
/// and by the invariant test in `test.rs`.  Removing the `retries` override here
/// breaks the test.
fn apify_fetch_options(body: String, token: &str) -> FetchOptions {
    FetchOptions {
        method: Some(reqwest::Method::POST),
        body: Some(body),
        headers: Some(vec![
            ("authorization".to_string(), format!("Bearer {token}")),
            ("content-type".to_string(), "application/json".to_string()),
        ]),
        timeout: Some(APIFY_TIMEOUT),
        retries: APIFY_RETRIES, // NON-IDEMPOTENT: each run is billed — never retry
        ..FetchOptions::default()
    }
}

/// Defensively map an [`ApifyItem`] to a [`JobPosting`]. Returns `None` when the
/// item lacks BOTH a usable title and a usable URL (no `jobUrl` and no `id` to
/// construct one) — such an item can't be opened, so it's dropped.
fn map_apify_item(item: ApifyItem, now: i64) -> Option<JobPosting> {
    let title = item
        .title
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;

    let id = item
        .id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // URL: explicit jobUrl wins; for the id-constructed fallback, require a
    // digits-only id so we never interpolate an arbitrary string into a path.
    let url = item
        .job_url
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            id.as_deref()
                .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
                .map(|id| format!("https://www.linkedin.com/jobs/view/{id}"))
        })?;

    // Security: drop items whose URL is not HTTPS on a linkedin.com host.
    // A drifting actor can inject non-LinkedIn or non-HTTPS URLs; we reject those.
    if !is_valid_apify_linkedin_url(&url) {
        return None;
    }

    let description = item
        .description_text
        .or(item.job_description)
        .or(item.description_html)
        .map(|d| html_to_markdown(&d));

    let posted_at = item.posted_at.as_deref().and_then(parse_apify_posted_at);

    // Stable external id for dedupe: the LinkedIn job id when present, else the URL.
    let external_id = id
        .map(|id| format!("linkedin-{id}"))
        .unwrap_or_else(|| format!("linkedin-{url}"));

    Some(JobPosting {
        id: format!("aggregator:{external_id}"),
        external_id: Some(external_id),
        title,
        company: item
            .company_name
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
        location: item
            .location
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        url,
        source: "aggregator".to_string(),
        description,
        requirements: None,
        posted_at,
        captured_at: now,
        extra: std::collections::HashMap::new(),
    })
}

pub(crate) struct ApifyLinkedInProvider {
    token: Option<String>,
    /// The opt-in toggle. `is_configured()` requires this AND a token.
    enabled: bool,
    actor_id: String,
}

impl ApifyLinkedInProvider {
    fn new() -> Self {
        use crate::ipc_contracts::provider_slots::APIFY_TOKEN;
        let token = crate::credentials::read_credential(&format!("ai:{APIFY_TOKEN}"))
            .unwrap_or_else(|e| {
                log::warn!("[aggregator] {APIFY_TOKEN} keyring error: {e}");
                None
            });
        let settings = read_aggregator_settings();
        // Validate the user-supplied actor id before interpolating it into the
        // API path. Falls back to the default actor on mismatch; never panics.
        let actor_id = settings
            .apify_linkedin_actor_id
            .filter(|id| {
                if is_valid_apify_actor_id(id) {
                    true
                } else {
                    log::warn!(
                        "[aggregator] apifyLinkedinActorId is not a valid Apify actor id \
                         (expected user~actor grammar); falling back to the default actor"
                    );
                    false
                }
            })
            .unwrap_or_else(|| APIFY_DEFAULT_ACTOR.to_string());
        Self {
            token,
            enabled: settings.apify_linkedin_enabled,
            actor_id,
        }
    }
}

#[async_trait]
impl JobProvider for ApifyLinkedInProvider {
    fn provider_id(&self) -> &'static str {
        "apify_linkedin"
    }

    fn is_configured(&self) -> bool {
        // BOTH gates are mandatory: an Apify token present AND the user opted in.
        // Never run a paid scrape just because a token happens to be stored.
        self.token.is_some() && self.enabled
    }

    async fn search(
        &self,
        query: &str,
        location: &str,
        _country: &str,
        date_filter: Option<&str>,
        amount: Option<u32>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        if !self.is_configured() {
            return Err(anyhow::anyhow!("apify_linkedin: not configured"));
        }

        let token = self.token.as_deref().unwrap_or("");
        let search_url = build_linkedin_search_url(query, location, date_filter);

        // Dynamic cost cap: honour the caller's remaining budget (amount - primary.len())
        // passed in by `search_with_providers`, capped at the absolute maximum.
        // `maxItems` is the Apify platform-enforced server-side cap; `count` in the
        // actor body is the actor-input budget (a user-overridden actor might ignore
        // `count`, so both must agree). Bearer token stays in the Authorization header
        // only — never the URL or query string.
        let max_items = amount.unwrap_or(APIFY_MAX_ITEMS).min(APIFY_MAX_ITEMS);
        let endpoint = build_apify_endpoint(&self.actor_id, max_items);

        let body = serde_json::json!({
            "urls": [search_url],
            "count": max_items,
        })
        .to_string();

        // POST via the shared scraping client.
        //
        // INVARIANT: retries=0 (via `apify_fetch_options`).  The endpoint is
        // NON-IDEMPOTENT and billed per result — a retry would start ANOTHER charged
        // actor run (up to 3× cost with the default retries=2). Never retry.
        //
        // `tokio::select!` races the paid fetch against the cancellation signal so
        // a user cancel mid-flight is honoured within one poll cycle.
        let raw = tokio::select! {
            _ = signal.cancelled() => {
                return Err(anyhow::anyhow!("apify_linkedin: cancelled"));
            }
            result = fetch_json::<Vec<ApifyItem>>(
                &endpoint,
                apify_fetch_options(body, token),
                signal.clone(),
            ) => result?
        };
        let items = raw.ok_or_else(|| {
            anyhow::anyhow!(
                "apify_linkedin: non-2xx response, timeout (408), or unparseable dataset body"
            )
        })?;

        let now = chrono::Utc::now().timestamp_millis();
        Ok(items
            .into_iter()
            .filter_map(|it| map_apify_item(it, now))
            .collect())
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
/// - Adzuna configured + `Err(_)`, JSearch absent → `Err(diagnostic)` so the
///   engine surfaces it as a board error rather than a silent zero-result run.
///
/// Items from each provider are keyed by their `external_id` to deduplicate.
async fn primary_chain(
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

    // Track whether Adzuna was configured-but-failed so we can distinguish
    // "keys present, request failed" from "no keys at all" at the end.
    let mut adzuna_configured_failed: Option<anyhow::Error> = None;

    // Run primary if configured.
    if let Some(p) = primary {
        if p.is_configured() {
            match p
                .search(query, location, country, date_filter, None, signal.clone())
                .await
            {
                Ok(items) => {
                    // Even empty → use result as-is; do NOT fall through to JSearch.
                    return Ok(dedupe(items));
                }
                Err(e) => {
                    log::warn!("[aggregator] adzuna error, attempting jsearch fallback: {e}");
                    adzuna_configured_failed = Some(e);
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
            return f
                .search(query, location, country, date_filter, None, signal)
                .await
                .map(dedupe);
        }
    }

    // Adzuna had keys but failed (e.g. unsupported country) AND JSearch is not
    // configured → surface a diagnostic error instead of a silent empty result.
    // The engine records this in BoardScrapeSummary.error, which the Jobs page
    // renders as a partial-failure warning and autopilot logs as a skipped board.
    if let Some(e) = adzuna_configured_failed {
        return Err(anyhow::anyhow!(
            "{e}; add a JSearch key in Settings → API Keys for global coverage"
        ));
    }

    // Neither provider configured → keyless-empty (intended, never an error).
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

/// Normalise a URL for deduplication.
///
/// For `linkedin.com` hosts, strip the query string so tracking-only variants
/// (`?trk=…`, `?refId=…`) of the same job URL are treated as identical.
/// For every other host, keep the query string intact: some boards encode the
/// job id in query params, so stripping would merge distinct jobs.
fn canonical_url(url: &str) -> String {
    let trimmed = url.trim();
    // `Url::parse` normalises scheme and host to lowercase (URL standard) while
    // preserving the original-case path and query — so host comparison below
    // is already case-insensitive without lowercasing the entire URL string.
    if let Ok(mut parsed) = reqwest::Url::parse(trimmed) {
        if parsed
            .host_str()
            .is_some_and(|h| h == "linkedin.com" || h.ends_with(".linkedin.com"))
        {
            parsed.set_query(None);
            return parsed.to_string();
        }
        // Non-LinkedIn: return the parsed URL (host normalised to lowercase,
        // path + query preserved in original case — case-significant on boards
        // that encode the job id in query params or case-sensitive path segments).
        return parsed.to_string();
    }
    trimmed.to_string()
}

/// Deduplicate the cross-provider merge by URL, preserving first-seen order.
///
/// `dedupe` keys on `external_id`, which is provider-prefixed (`adzuna-…` vs
/// `linkedin-…`) and so never collides across providers even for the SAME job.
/// The additive merge therefore keys on the canonical URL instead, so a posting
/// surfaced by both the primary chain and the LinkedIn provider appears once
/// (primary first, since it is extended onto the front).
///
/// LinkedIn tracking params (`?trk=…`, `?refId=…`) are stripped by
/// [`canonical_url`] before keying so the same logical job dedupes regardless
/// of which tracking variant was captured.
fn dedupe_by_url(items: Vec<JobPosting>) -> Vec<JobPosting> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|p| seen.insert(canonical_url(&p.url)))
        .collect()
}

/// Top-level provider orchestration.
///
/// 1. **Primary result** — the Adzuna → JSearch fallback chain ([`primary_chain`]),
///    with its existing semantics fully preserved.
/// 2. **Additive LinkedIn (Apify)** — runs IN ADDITION to (never as a fallback of)
///    the primary result, and ONLY when `apify_linkedin` is configured (the toggle
///    is ON and a token is present). Its results are merged onto the primary,
///    deterministically (primary first) and deduped by URL.
///
/// When the LinkedIn provider is absent or not configured — the default, and what
/// the Adzuna/JSearch tests exercise — this returns the primary result byte-for-byte,
/// so all existing fallback + keyless-empty semantics are unchanged.
async fn search_with_providers(
    providers: &[Box<dyn JobProvider>],
    query: &str,
    location: &str,
    country: &str,
    date_filter: Option<&str>,
    amount: usize,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    let primary = primary_chain(
        providers,
        query,
        location,
        country,
        date_filter,
        signal.clone(),
    )
    .await;

    let linkedin = providers
        .iter()
        .find(|p| p.provider_id() == "apify_linkedin");
    let li_configured = linkedin.map(|p| p.is_configured()).unwrap_or(false);

    // Not opted in → identical to the legacy Adzuna→JSearch path (Err and all).
    if !li_configured {
        return primary;
    }

    // Cost gate: skip the paid Apify call when the primary result already
    // satisfies the requested amount — LinkedIn is a fill for UNMET capacity,
    // not unconditional.
    if let Ok(ref items) = primary {
        if items.len() >= amount {
            return primary;
        }
    }

    // Don't fire a paid Apify run after cancellation.
    // Cap: only fetch as many results as still needed; never exceed APIFY_MAX_ITEMS.
    let primary_len = primary.as_ref().map(|v| v.len()).unwrap_or(0);
    let remaining = amount.saturating_sub(primary_len);
    let apify_cap = remaining.min(APIFY_MAX_ITEMS as usize) as u32;

    let li_items = if signal.is_cancelled() {
        Vec::new()
    } else {
        match linkedin
            .expect("li_configured implies the provider is present")
            .search(
                query,
                location,
                country,
                date_filter,
                Some(apify_cap),
                signal,
            )
            .await
        {
            Ok(items) => items,
            Err(e) => {
                // Tolerate one provider erroring — log and merge what we have.
                log::warn!("[aggregator] apify_linkedin error (additive, ignored): {e}");
                Vec::new()
            }
        }
    };

    match primary {
        Ok(mut items) => {
            items.extend(li_items);
            Ok(dedupe_by_url(items))
        }
        // Primary failed (e.g. Adzuna unsupported-country diagnostic + no JSearch).
        // Surface the diagnostic only when LinkedIn also produced nothing; if it
        // returned results, prefer showing them over hiding them behind the error.
        Err(e) => {
            if li_items.is_empty() {
                Err(e)
            } else {
                Ok(dedupe_by_url(li_items))
            }
        }
    }
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
            // Additive, opt-in, paid: only runs when the toggle is ON and a token
            // is present (gated in `ApifyLinkedInProvider::is_configured`).
            Box::new(ApifyLinkedInProvider::new()),
        ];
        let amount = input.amount as usize;
        let items = search_with_providers(
            &providers,
            query,
            location,
            &country,
            input.date_filter.as_deref(),
            amount,
            ctx.signal.clone(),
        )
        .await?;
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

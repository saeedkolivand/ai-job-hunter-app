/// The three `JobProvider` implementations backing the aggregator board: Adzuna
/// (primary), JSearch (paid fallback), and Apify LinkedIn (additive, opt-in,
/// paid). Split out of `mod.rs` (R8 module-size guard) — fallback orchestration
/// (`primary_chain`/`search_with_providers`) and the `Scraper` impl stay there.
///
/// Visibility: items here are `pub(super)` (visible to `aggregator` and its
/// descendants, including `test.rs`) rather than fully private, purely to
/// preserve this behavior-preserving move — no API surface beyond `aggregator`
/// is intended.
use async_trait::async_trait;
use serde::Deserialize;

use crate::scraping::http::{fetch_json, html_to_markdown, FetchOptions};
use crate::scraping::types::JobPosting;

use super::JobProvider;

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
pub(super) fn adzuna_max_days_old(date_filter: Option<&str>) -> u32 {
    match date_filter {
        Some("15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "24h") => 3,
        Some("week") => 7,
        _ => 30,
    }
}

/// Whether to broaden a sparse Adzuna result to a country-wide (`where=""`) retry.
/// Only for an explicitly-supplied country (`!country_guessed`) — broadening a
/// GUESSED market would defeat primary_chain's guessed-market → JSearch fallback,
/// which keys off Adzuna returning empty.
pub(super) fn should_broaden(country_guessed: bool, where_val: &str, count: usize) -> bool {
    !country_guessed && !where_val.is_empty() && count < super::ADZUNA_BROADEN_FLOOR
}

/// Adzuna's `where` wants a place *inside* the market (the country is already the
/// URL path segment), so a trailing ", Germany"/", Deutschland" just over-narrows the
/// geocode. Keep the first comma-segment (the city/region), trimmed.
// ponytail: first-segment heuristic. A country-name-only location (e.g. "germany")
// already returns Adzuna's full page, so no country-name table is needed.
pub(super) fn adzuna_where(location: &str) -> &str {
    location.split(',').next().map(str::trim).unwrap_or("")
}

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

// ── Adzuna supported-country allowlist ───────────────────────────────────────

/// ISO 3166-1 alpha-2 country codes hosted by Adzuna's job-search API.
///
/// Source: Adzuna API documentation at <https://api.adzuna.com/v1/doc>
/// (path-parameter enumeration visible in the interactive endpoint reference).
/// Verified against the known set as of 2026-06-23; update this list if
/// Adzuna adds new markets (the path `/v1/api/jobs/{country}/search/1` returns
/// a non-2xx error body for any code not in this set, which is indistinguishable
/// from an auth failure without real keys — see code comment in `search`).
pub(super) const ADZUNA_SUPPORTED_COUNTRIES: &[&str] = &[
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
pub(super) fn adzuna_supports_country(country: &str) -> bool {
    ADZUNA_SUPPORTED_COUNTRIES.contains(&country)
}

/// ISO-4217 currency for an Adzuna market. Adzuna's search API returns
/// `salary_min`/`salary_max` as bare numbers with no currency field, so the
/// currency has to be derived from the country the search targeted — one entry
/// per code in [`ADZUNA_SUPPORTED_COUNTRIES`]. `None` for any country not in
/// that list (the salary answer then falls back to a web lookup for currency
/// instead of guessing).
#[inline]
pub(super) fn adzuna_currency_for_country(country: &str) -> Option<&'static str> {
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
pub(super) struct AdzunaCompany {
    pub(super) display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AdzunaLocation {
    pub(super) display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AdzunaJob {
    #[serde(deserialize_with = "de_string_or_number")]
    pub(super) id: String,
    pub(super) title: String,
    pub(super) company: Option<AdzunaCompany>,
    pub(super) location: Option<AdzunaLocation>,
    pub(super) redirect_url: String,
    pub(super) description: Option<String>,
    pub(super) created: Option<String>,
    pub(super) salary_min: Option<f64>,
    pub(super) salary_max: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AdzunaResp {
    pub(super) results: Vec<AdzunaJob>,
}

pub(crate) struct AdzunaProvider {
    pub(super) app_id: Option<String>,
    pub(super) app_key: Option<String>,
}

impl AdzunaProvider {
    pub(super) fn new() -> Self {
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
        country_guessed: bool,
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

        // Drop redundant country suffixes so a ", Germany"/", Deutschland" tail
        // doesn't over-narrow the geocode (the country is already the URL path).
        let where_hygienic = adzuna_where(location);

        let postings = fetch_adzuna_page(
            country,
            app_id,
            app_key,
            query,
            where_hygienic,
            date_filter,
            signal.clone(),
        )
        .await?;

        // Broaden on near-empty: even a hygienic `where` can over-narrow a sparse
        // market, so if a real Adzuna market returned under the floor, retry ONCE
        // country-wide (`where=""`) — same `what`, sort, and `max_days_old` — and
        // keep whichever set is larger. A transient error on the retry keeps the
        // narrow result rather than discarding it.
        //
        // GUARD: never broaden a GUESSED market (`country_guessed`). Turning a
        // guessed-market empty/near-empty into a non-empty country-wide result
        // would defeat `primary_chain`'s guessed-market guard, which relies on
        // an empty Adzuna result to fall through to JSearch (global, free-text
        // location) when the guess is probably wrong (e.g. "London" defaulting
        // to "de"). Only broaden for an explicitly-supplied country.
        if should_broaden(country_guessed, where_hygienic, postings.len()) {
            match fetch_adzuna_page(country, app_id, app_key, query, "", date_filter, signal).await
            {
                Ok(broadened) if broadened.len() > postings.len() => {
                    // PRIVACY: never log the raw `where`/location — free-text PII.
                    log::info!(
                        "[aggregator] adzuna sparse result ({}), broadened country-wide ({})",
                        postings.len(),
                        broadened.len()
                    );
                    return Ok(broadened);
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!(
                        "[aggregator] adzuna broaden retry failed, keeping narrow result: {e}"
                    )
                }
            }
        }

        Ok(postings)
    }
}

/// Map one Adzuna result to a [`JobPosting`], deriving `extra.salaryCurrency`
/// from `country` (Adzuna reports bare salary numbers with no currency field).
/// Pulled out of `AdzunaProvider::search` so it's unit-testable without a
/// network call.
pub(super) fn adzuna_job_to_posting(j: AdzunaJob, country: &str, now: i64) -> JobPosting {
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

/// Build + fetch + parse ONE Adzuna page for a given `where` value.
///
/// Factored out of `AdzunaProvider::search` so the near-empty broaden retry can
/// reissue the exact same request (same `what`, `sort_by=date`, `results_per_page`,
/// and `max_days_old`) with only `where` changed. Called at most twice per search.
async fn fetch_adzuna_page(
    country: &str,
    app_id: &str,
    app_key: &str,
    query: &str,
    where_val: &str,
    date_filter: Option<&str>,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    // Sort newest-first (Adzuna defaults to relevance, which floats stale postings
    // up) and always bound the window with max_days_old so nothing older than the
    // cap (30 days, or the user's tighter pick) is returned.
    let url = format!(
        "https://api.adzuna.com/v1/api/jobs/{}/search/1\
         ?app_id={}&app_key={}&what={}&where={}&results_per_page=50&content-type=application/json\
         &sort_by=date&sort_direction=down&max_days_old={}",
        urlencoding::encode(country),
        urlencoding::encode(app_id),
        urlencoding::encode(app_key),
        urlencoding::encode(query),
        urlencoding::encode(where_val),
        adzuna_max_days_old(date_filter),
    );

    let resp = match fetch_json::<AdzunaResp>(&url, FetchOptions::default(), signal).await? {
        Some(r) => r,
        None => {
            return Err(anyhow::anyhow!(
                "adzuna: non-2xx response or unparseable body"
            ))
        }
    };

    let now = chrono::Utc::now().timestamp_millis();
    Ok(resp
        .results
        .into_iter()
        .map(|j| adzuna_job_to_posting(j, country, now))
        .collect())
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
        _country_guessed: bool,
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

        // Sort newest-first (JSearch defaults to relevance, which does NOT put the
        // freshest posting on top) so the widened `date_posted` window still surfaces
        // the most-recent jobs first — matching Adzuna's `sort_by=date` and honouring
        // the freshness guarantee documented on `jsearch_date_posted`.
        url.push_str(&format!(
            "&date_posted={}&sort_by=date",
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
pub(super) const SCRAPING_SETTINGS_FILE: &str = "scraping-settings.json";
pub(super) const SETTING_APIFY_ENABLED: &str = "apifyLinkedinEnabled";
pub(super) const SETTING_APIFY_ACTOR_ID: &str = "apifyLinkedinActorId";

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
pub(super) const APIFY_DEFAULT_ACTOR: &str = "curious_coder~linkedin-jobs-scraper";

// ponytail: HARD cost ceiling. Apify bills per dataset result, so every run is
// bounded by `count = APIFY_MAX_ITEMS`; we NEVER issue an unbounded fetch. The
// opt-in toggle (gated in `is_configured`) is the second, mandatory cost gate —
// a stored token ALONE never triggers a paid run.
pub(super) const APIFY_MAX_ITEMS: u32 = 50;

/// `run-sync-get-dataset-items` is capped at 300s server-side (returns 408 on
/// timeout); give the client a matching wall-clock ceiling so a stalled actor
/// run can't hang the scrape.
const APIFY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Server-side USD ceiling for pay-per-event actor overrides. Belt-and-suspenders
/// on top of `maxItems`: a user who overrides the actor to a pay-per-event model
/// is still bounded by this hard Apify platform limit.
pub(super) const APIFY_MAX_CHARGE_USD: &str = "1.00";

/// INVARIANT: retries=0 for every Apify `run-sync-get-dataset-items` call.
/// The endpoint is NON-IDEMPOTENT and billed per result — a retry on 429/503/network
/// would start ANOTHER charged actor run (up to 3× cost with the default retries=2).
/// Shared by production code and tests so a change to either breaks the invariant check.
pub(super) const APIFY_RETRIES: u32 = 0;

/// Validate an Apify actor id against the platform grammar `user~actor`.
///
/// Both parts must be non-empty and consist solely of `[A-Za-z0-9_.-]`.
/// A malformed id injected via `apifyLinkedinActorId` could otherwise reach
/// the API URL (even though the host is fixed, a path-traversal like
/// `../../v1/…` is still a concern). An invalid id falls back silently to
/// `APIFY_DEFAULT_ACTOR` — the provider logs a warning and continues.
pub(super) fn is_valid_apify_actor_id(id: &str) -> bool {
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
pub(super) fn build_apify_endpoint(actor_id: &str, max_items: u32) -> String {
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
pub(super) fn apify_f_tpr(date_filter: Option<&str>) -> &'static str {
    match date_filter {
        Some("15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "24h") => "r86400",
        Some("week") => "r604800",
        _ => "r2592000",
    }
}

/// Build the public LinkedIn jobs-search URL the actor expects as input (it
/// scrapes pre-built search URLs, not a raw keyword string). Query + location are
/// percent-encoded; recency comes from [`apify_f_tpr`].
pub(super) fn build_linkedin_search_url(
    query: &str,
    location: &str,
    date_filter: Option<&str>,
) -> String {
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
pub(super) struct ApifyItem {
    #[serde(default, alias = "jobTitle")]
    pub(super) title: Option<String>,
    #[serde(default, rename = "companyName")]
    pub(super) company_name: Option<String>,
    #[serde(default)]
    pub(super) location: Option<String>,
    #[serde(default, rename = "jobUrl")]
    pub(super) job_url: Option<String>,
    #[serde(default, deserialize_with = "de_opt_string_or_number")]
    pub(super) id: Option<String>,
    #[serde(
        default,
        rename = "postedAt",
        deserialize_with = "de_opt_string_or_number"
    )]
    pub(super) posted_at: Option<String>,
    #[serde(default, rename = "descriptionText")]
    pub(super) description_text: Option<String>,
    #[serde(default, rename = "jobDescription")]
    pub(super) job_description: Option<String>,
    #[serde(default, rename = "descriptionHtml")]
    pub(super) description_html: Option<String>,
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
pub(super) fn apify_fetch_options(body: String, token: &str) -> FetchOptions {
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
pub(super) fn map_apify_item(item: ApifyItem, now: i64) -> Option<JobPosting> {
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
    pub(super) token: Option<String>,
    /// The opt-in toggle. `is_configured()` requires this AND a token.
    pub(super) enabled: bool,
    pub(super) actor_id: String,
}

impl ApifyLinkedInProvider {
    pub(super) fn new() -> Self {
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
        _country_guessed: bool,
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

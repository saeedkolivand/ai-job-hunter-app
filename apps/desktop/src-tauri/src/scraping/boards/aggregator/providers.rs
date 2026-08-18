/// The `JobProvider` implementations backing the aggregator board's non-primary
/// tiers: JSearch (paid fallback), Jooble (last-resort fallback), and Apify
/// LinkedIn (additive, opt-in, paid). Split out of `mod.rs` (R8 module-size
/// guard) — fallback orchestration (`primary_chain`/`search_with_providers`) and
/// the `Scraper` impl stay there, and the PRIMARY tier lives in `adzuna.rs`
/// (split out of this file by the same guard).
///
/// Visibility: items here are `pub(super)` (visible to `aggregator` and its
/// descendants, including `test.rs`) rather than fully private, purely to
/// preserve this behavior-preserving move — no API surface beyond `aggregator`
/// is intended.
use async_trait::async_trait;
use serde::Deserialize;

use crate::observability::sanitize_reason;
use crate::scraping::http::{fetch_json, html_to_markdown, FetchOptions};
use crate::scraping::types::JobPosting;

use super::JobProvider;

// ── Serde helpers ─────────────────────────────────────────────────────────────

/// Like `adzuna::de_string_or_number` but for an OPTIONAL field, tolerating `null` /
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
/// the invariant test in `test.rs`, so dropping the override here breaks that test.
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

// ── Jooble provider (last-resort fallback, after Adzuna + JSearch) ──────────────

#[derive(Debug, Deserialize)]
pub(super) struct JoobleJob {
    #[serde(default, deserialize_with = "de_opt_string_or_number")]
    pub(super) id: Option<String>,
    pub(super) title: Option<String>,
    pub(super) company: Option<String>,
    pub(super) location: Option<String>,
    /// TRUNCATED description — same caveat as Adzuna's `description`: never
    /// treat this as full job text.
    pub(super) snippet: Option<String>,
    /// Free-text salary string (e.g. "$50,000 - $70,000"); not parsed into a
    /// number — stashed verbatim in `extra.salaryText` rather than risking a
    /// wrong numeric parse of an undocumented format.
    pub(super) salary: Option<String>,
    pub(super) link: Option<String>,
    /// ISO-8601.
    pub(super) updated: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct JoobleResp {
    pub(super) jobs: Vec<JoobleJob>,
}

/// Production Jooble host. Factored into a constant (rather than inlined in
/// [`fetch_jooble`]) purely so `JoobleProvider::search` reads as "the real
/// host" at a glance; tests pass a local `wiremock` base instead.
pub(super) const JOOBLE_BASE_URL: &str = "https://jooble.org";

/// Build the Jooble search endpoint for a given base + key. Factored out of
/// [`fetch_jooble`] so the URL shape (key in the PATH, not a header/query) is
/// independently testable.
pub(super) fn jooble_endpoint(base_url: &str, api_key: &str) -> String {
    format!("{base_url}/api/{}", urlencoding::encode(api_key))
}

/// Map one Jooble result to a [`JobPosting`]. Drops items missing a usable
/// title or link (mirrors JSearch's `filter_map` policy — such an item can't
/// be shown or opened). Pulled out of [`fetch_jooble`] so the mapping is
/// unit-testable without a network call.
pub(super) fn map_jooble_job(j: JoobleJob, now: i64) -> Option<JobPosting> {
    let title = j
        .title
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let url = j
        .link
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let id_part =
        j.id.map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| url.clone());

    let mut extra = std::collections::HashMap::new();
    if let Some(salary) = j
        .salary
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        extra.insert("salaryText".to_string(), serde_json::json!(salary));
    }

    Some(JobPosting {
        id: format!("aggregator:jooble-{id_part}"),
        external_id: Some(format!("jooble-{id_part}")),
        title,
        company: j.company.unwrap_or_default(),
        location: j
            .location
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        url,
        source: "aggregator".to_string(),
        description: j.snippet.map(|d| html_to_markdown(&d)),
        requirements: None,
        // Jooble's real `updated` value has NO timezone offset (e.g.
        // "2026-05-15T00:00:00.0000000") despite looking ISO-8601-ish, so plain
        // RFC3339 parsing fails for every live job — `posted_at` silently stayed
        // `None` for all of them. Try RFC3339 first (in case an entry ever does
        // carry an offset), then fall back to a naive datetime — tolerant of
        // Jooble's fractional-seconds-no-tz form — assumed UTC.
        posted_at: j.updated.as_deref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp_millis())
                .or_else(|| {
                    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                        .ok()
                        .map(|ndt| ndt.and_utc().timestamp_millis())
                })
        }),
        captured_at: now,
        extra,
    })
}

/// Build + fetch + parse the Jooble search response for a given base URL.
/// Factored out of `JoobleProvider::search` (mirrors `fetch_adzuna_page`'s
/// testable-base pattern) so the non-2xx / mapping contract is unit-testable
/// against a local mock server without a real key or hitting the live host.
pub(super) async fn fetch_jooble(
    base_url: &str,
    api_key: &str,
    query: &str,
    location: &str,
    amount: Option<u32>,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    // Jooble's contract puts the key in the URL PATH, not a header or query
    // param: `POST /api/{apiKey}`. `redact_path: true` below keeps it out of
    // `fetch_json`'s non-2xx / schema-drift log lines (the shared query-only
    // redaction doesn't cover a path-embedded secret) — including on the exact
    // "bad key" 403 that would otherwise echo it straight back.
    let url = jooble_endpoint(base_url, api_key);

    let body = match amount {
        Some(n) => {
            serde_json::json!({ "keywords": query, "location": location, "ResultOnPage": n })
        }
        None => serde_json::json!({ "keywords": query, "location": location }),
    };

    // A non-2xx or schema-drift response propagates as `Err` from `fetch_json`
    // (carrying the HTTP status); `?` surfaces it as a provider failure. The
    // "jooble:" prefix is required — the aggregator board fronts multiple
    // providers, so an unattributed "HTTP 403" in BoardScrapeSummary.error
    // wouldn't say which one failed.
    let resp = fetch_json::<JoobleResp>(
        &url,
        FetchOptions {
            method: Some(reqwest::Method::POST),
            body: Some(body.to_string()),
            headers: Some(vec![(
                "content-type".to_string(),
                "application/json".to_string(),
            )]),
            redact_path: true,
            ..FetchOptions::default()
        },
        signal,
    )
    .await
    .map_err(|e| anyhow::anyhow!("jooble: {e}"))?;

    let now = chrono::Utc::now().timestamp_millis();
    Ok(resp
        .jobs
        .into_iter()
        .filter_map(|j| map_jooble_job(j, now))
        .collect())
}

pub(crate) struct JoobleProvider {
    pub(super) api_key: Option<String>,
}

impl JoobleProvider {
    pub(super) fn new() -> Self {
        use crate::ipc_contracts::provider_slots::JOOBLE_KEY;
        Self {
            api_key: crate::credentials::read_credential(&format!("ai:{JOOBLE_KEY}"))
                .unwrap_or_else(|e| {
                    log::warn!(
                        "[aggregator] {JOOBLE_KEY} keyring error: {}",
                        sanitize_reason(&e.to_string())
                    );
                    None
                }),
        }
    }
}

#[async_trait]
impl JobProvider for JoobleProvider {
    fn provider_id(&self) -> &'static str {
        "jooble"
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
        _date_filter: Option<&str>,
        amount: Option<u32>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        if !self.is_configured() {
            return Err(anyhow::anyhow!("jooble: not configured"));
        }
        let api_key = self.api_key.as_deref().unwrap_or("");
        fetch_jooble(JOOBLE_BASE_URL, api_key, query, location, amount, signal).await
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
                log::warn!(
                    "[aggregator] {APIFY_TOKEN} keyring error: {}",
                    sanitize_reason(&e.to_string())
                );
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
        // A non-2xx / timeout / schema-drift response propagates as `Err` from
        // `fetch_json` (carrying the HTTP status); `?` surfaces it as a provider
        // failure instead of a silent empty dataset.
        let items = tokio::select! {
            _ = signal.cancelled() => {
                return Err(anyhow::anyhow!("apify_linkedin: cancelled"));
            }
            result = fetch_json::<Vec<ApifyItem>>(
                &endpoint,
                apify_fetch_options(body, token),
                signal.clone(),
            ) => result?
        };

        let now = chrono::Utc::now().timestamp_millis();
        Ok(items
            .into_iter()
            .filter_map(|it| map_apify_item(it, now))
            .collect())
    }
}

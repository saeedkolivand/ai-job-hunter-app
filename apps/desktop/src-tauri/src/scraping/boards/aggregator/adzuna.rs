/// The Adzuna `JobProvider` — the aggregator's PRIMARY tier: market allowlist,
/// currency map, response mapping, the amount-bounded page loop, and the
/// near-empty country-wide broaden retry.
///
/// Split out of `providers.rs` (R8 module-size guard) when the page loop landed;
/// `providers.rs` keeps the remaining providers (JSearch, Jooble, Apify LinkedIn)
/// and `mod.rs` keeps the fallback orchestration plus the `Scraper` impl.
///
/// Visibility: items here are `pub(super)` (visible to `aggregator` and its
/// descendants, including `test.rs`) rather than fully private — no API surface
/// beyond `aggregator` is intended.
use std::sync::Arc;

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

// ── Date-filter + location helpers ────────────────────────────────────────────

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

/// User-facing note token when a GUESSED market (no `country_code` was supplied)
/// returned an AUTHORITATIVE result set — `count` at or above the broaden floor,
/// so [`super::primary_chain`] returns it as-is instead of routing to the global
/// (JSearch) fallback. Surfaces the otherwise-silent market guess so the user can
/// set a country for deterministic results.
///
/// `None` for an explicit country, an empty location, or a sub-floor count (which
/// `primary_chain` re-routes to the global fallback, so no guessed-market results
/// are actually shown — flagging the guess there would mislead). Country code
/// only — never the raw location (free-text PII).
pub(super) fn guessed_market_note(
    country_guessed: bool,
    location: &str,
    count: usize,
    country: &str,
) -> Option<String> {
    // `.trim()` here is defensive for direct unit-test callers — the real call
    // site (`AdzunaProvider::search`) already passes a caller-trimmed `location`.
    (country_guessed && !location.trim().is_empty() && count >= super::ADZUNA_BROADEN_FLOOR)
        .then(|| format!("guessed-market:{country}"))
}

/// Adzuna's `where` wants a place *inside* the market (the country is already the
/// URL path segment), so a trailing ", Germany"/", Deutschland" just over-narrows the
/// geocode. Keep the first comma-segment (the city/region), trimmed.
// ponytail: first-segment heuristic. A country-name-only location (e.g. "germany")
// already returns Adzuna's full page, so no country-name table is needed.
pub(super) fn adzuna_where(location: &str) -> &str {
    location.split(',').next().map(str::trim).unwrap_or("")
}

// ── Adzuna paging budget ──────────────────────────────────────────────────────

/// Production Adzuna host. Factored into a constant (rather than inlined in
/// [`fetch_adzuna_page`]) purely so `AdzunaProvider::search` reads as "the real
/// host" at a glance; tests pass a local `wiremock` base instead — mirrors
/// `JOOBLE_BASE_URL`.
pub(super) const ADZUNA_BASE_URL: &str = "https://api.adzuna.com";

/// Adzuna's `results_per_page` — the most one request can return. A page that
/// comes back with FEWER than this is the last page, so the loop stops there.
pub(super) const ADZUNA_PAGE_SIZE: usize = 50;

/// Hard ceiling on requests per Adzuna search.
///
// ponytail: Adzuna's free tier is a DAILY CALL quota, not a per-search one, so
// every extra page is spend taken from the rest of the day's searches. Two pages
// (≤100 postings) already covers the UI's own 100-item amount cap, and the cap is
// deliberately NOT driven by `BoardSearchInput::pages` — the manual search path
// hardcodes `pages = MAX_PAGE_BUDGET`, so a pages-driven loop would silently
// multiply the quota cost of EVERY search. Amount-driven means a small search
// stays exactly as cheap as it is today (one request).
pub(super) const ADZUNA_MAX_PAGES: u32 = 2;

/// INVARIANT: retries=0 for every Adzuna request (mirrors [`super::APIFY_RETRIES`]).
///
/// Adzuna's free tier is a HARD DAILY CALL quota, and `fetch_text` re-sends on
/// 429/503 — so the default `retries: 2` turns each page into up to 3 quota
/// calls and makes the worst-case cost of one search 9 instead of 3. Worse, a
/// 429 from a metered API IS the over-quota signal: retrying it spends more of
/// the very budget that just ran out. Bounded page count is the only knob that
/// should govern spend here; transient-failure resilience is provided by the
/// `primary_chain` fallback to JSearch/Jooble, not by re-billing Adzuna.
pub(super) const ADZUNA_RETRIES: u32 = 0;

/// How many Adzuna pages to request for a target result count.
///
/// `None` (no caller-supplied amount) → 1 page, the pre-paging behavior and the
/// quota-neutral default. Otherwise `ceil(amount / 50)`, clamped to
/// [`ADZUNA_MAX_PAGES`]; `amount = 0` still means one request, never zero.
pub(super) fn adzuna_page_budget(amount: Option<u32>) -> u32 {
    amount.map_or(1, |a| {
        a.div_ceil(ADZUNA_PAGE_SIZE as u32)
            .clamp(1, ADZUNA_MAX_PAGES)
    })
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
    /// Optional side-channel for user-facing location-policy notes (guessed
    /// market, sparse city → country-wide broadening). Injected by
    /// `AggregatorScraper::search` from the `ScrapeContext`; `None` in unit tests
    /// and credential-state probes. `Arc` (Send + Sync) so the provider stays
    /// `Sync` while it holds the sink across `.await`.
    pub(super) note_sink: Option<Arc<dyn Fn(String) + Send + Sync>>,
    /// API host. [`ADZUNA_BASE_URL`] in production ([`Self::new`]); tests point it
    /// at a local `wiremock` server via [`Self::with_base_url`] so the POLICY that
    /// runs on top of the page loop — the guessed-market note and the near-empty
    /// broaden retry, both of which read the loop's post-dedup count — is testable
    /// through `search` itself rather than only through the fetchers underneath it.
    pub(super) base_url: String,
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
            note_sink: None,
            base_url: ADZUNA_BASE_URL.to_string(),
        }
    }

    /// Attach a location-policy note sink (from the aggregator's `ScrapeContext`).
    pub(super) fn with_note_sink(
        mut self,
        sink: Option<Arc<dyn Fn(String) + Send + Sync>>,
    ) -> Self {
        self.note_sink = sink;
        self
    }

    /// Point the provider at a different API host. Test-only seam (see
    /// [`Self::base_url`]); production always uses [`ADZUNA_BASE_URL`].
    #[cfg(test)]
    pub(super) fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Emit an informational location-policy note through the injected sink, if any.
    fn report_note(&self, note: String) {
        if let Some(ref sink) = self.note_sink {
            sink(note);
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
        amount: Option<u32>,
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

        let req = AdzunaPageRequest {
            base_url: &self.base_url,
            country,
            app_id,
            app_key,
            query,
            where_val: where_hygienic,
            date_filter,
        };

        let postings = fetch_adzuna_pages(req, amount, signal.clone()).await?;

        // Surface the guessed-market policy when this guess produced the
        // authoritative result (>= floor, so `primary_chain` keeps it). `broaden`
        // never fires for a guessed market, so `postings.len()` here is final for
        // that branch. Country code only — the raw location is never emitted.
        if let Some(note) = guessed_market_note(country_guessed, location, postings.len(), country)
        {
            self.report_note(note);
        }

        // Broaden on near-empty: even a hygienic `where` can over-narrow a sparse
        // market, so if a real Adzuna market returned under the floor, retry ONCE
        // country-wide (`where=""`) — same `what`, sort, and `max_days_old` — and
        // keep whichever set is larger. A transient error on the retry keeps the
        // narrow result rather than discarding it.
        //
        // SINGLE PAGE, deliberately: paging the retry as well would multiply the
        // two budgets. The retry only fires when the paged loop collected fewer
        // than `ADZUNA_BROADEN_FLOOR` (3) results, which is USUALLY because page 1
        // came back short and the loop stopped after one fetch — but not always: a
        // FULL page 1 whose postings all dedupe away keeps the loop going, so the
        // worst-case cost of a search is `ADZUNA_MAX_PAGES + 1` DAILY-QUOTA CALLS,
        // not 2. Quota calls and fetches are the same number only because
        // `ADZUNA_RETRIES` is 0; with the default `retries: 2` each fetch would
        // bill up to 3 (see the constant).
        //
        // GUARD: never broaden a GUESSED market (`country_guessed`). Turning a
        // guessed-market empty/near-empty into a non-empty country-wide result
        // would defeat `primary_chain`'s guessed-market guard, which relies on
        // an empty Adzuna result to fall through to JSearch (global, free-text
        // location) when the guess is probably wrong (e.g. "London" defaulting
        // to "de"). Only broaden for an explicitly-supplied country.
        // GUARD: never spend the broaden retry's quota call after a Stop. The loop
        // above returns `Ok(what it had)` on cancel, and a cancelled run is short
        // by construction — without this check the deliberate stop would look like
        // a sparse market and buy one more request on the way out.
        if !signal.is_cancelled() && should_broaden(country_guessed, where_hygienic, postings.len())
        {
            match fetch_adzuna_page(
                AdzunaPageRequest {
                    where_val: "",
                    ..req
                },
                1,
                signal,
            )
            .await
            {
                Ok(broadened) if broadened.len() > postings.len() => {
                    // PRIVACY: never log the raw `where`/location — free-text PII.
                    log::info!(
                        "[aggregator] adzuna sparse result ({}), broadened country-wide ({})",
                        postings.len(),
                        broadened.len()
                    );
                    // Surface the sparse-city → country-wide broadening. Country
                    // code only — never the raw location (free-text PII).
                    self.report_note(format!("broadened:{country}"));
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

/// Everything about an Adzuna search request EXCEPT the page number.
///
/// Grouped into a (`Copy`) struct rather than eight positional parameters so the
/// page loop and the near-empty broaden retry can reissue the identical query
/// with only `page` / `where_val` changed — and so the fetchers stay under the
/// crate's 8-argument clippy ceiling (`clippy.toml`).
#[derive(Clone, Copy)]
pub(super) struct AdzunaPageRequest<'a> {
    /// Real host in production ([`ADZUNA_BASE_URL`]); a local `wiremock` base in tests.
    pub(super) base_url: &'a str,
    pub(super) country: &'a str,
    pub(super) app_id: &'a str,
    pub(super) app_key: &'a str,
    pub(super) query: &'a str,
    pub(super) where_val: &'a str,
    pub(super) date_filter: Option<&'a str>,
}

/// Collect up to [`adzuna_page_budget`] pages for `amount`, newest-first.
///
/// Loop contract (each clause is separately covered in `test.rs`):
/// * **Amount-bounded** — `amount <= 50` (or `None`) issues exactly ONE request,
///   so a small search costs the same daily quota as it did before paging.
/// * **Short page stops** — a page returning fewer than [`ADZUNA_PAGE_SIZE`]
///   items is Adzuna's last page; no further request is spent.
/// * **Page 1 failure is THE failure** — it propagates as `Err`, preserving the
///   pre-paging contract `primary_chain` relies on to fall through to JSearch.
/// * **Mid-loop failure fails open** — a later page's error keeps the pages
///   already collected (same policy as the broaden retry in `search`).
/// * **Cancellation is a clean stop, never an error** — a stop landing between
///   pages OR mid-flight inside a fetch never spends another request and returns
///   what was already collected as `Ok`, page 1 included (a cancel is not an
///   Adzuna failure, so it must not trigger the JSearch fallback diagnostic).
///
/// Results are de-duplicated across pages by `JobPosting::id` (an Adzuna posting
/// id is stable and unique, but the `sort_by=date` window shifts as new postings
/// land between requests, so the same posting can legitimately appear on both
/// pages). Page fullness is judged on the RAW response length, never the
/// post-dedup count — a page full of repeats still means Adzuna has more to give.
pub(super) async fn fetch_adzuna_pages(
    req: AdzunaPageRequest<'_>,
    amount: Option<u32>,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    let budget = adzuna_page_budget(amount);
    let mut out: Vec<JobPosting> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for page in 1..=budget {
        match fetch_adzuna_page(req, page, signal.clone()).await {
            Ok(batch) => {
                let full_page = batch.len() >= ADZUNA_PAGE_SIZE;
                out.extend(batch.into_iter().filter(|p| seen.insert(p.id.clone())));
                if !full_page {
                    break;
                }
                // No sleep here: `fetch_json` already waits for a per-host
                // rate-limiter slot before every request. At the current budget
                // that limiter is nominal — 30 requests / 60 s vs. our 2 — so it
                // only starts pacing if `ADZUNA_MAX_PAGES` is ever raised or
                // several searches overlap on the same host.
                //
                // Cancellation, though, IS load-bearing here: without this check a
                // user's Stop would reach `fetch_json`, come back as `Cancelled`,
                // and land in the fail-open arm below — logging a misleading
                // "page N failed" warning for what is a clean, deliberate stop.
                if signal.is_cancelled() {
                    break;
                }
            }
            // A user's Stop is a clean stop, never a provider failure — the same
            // contract the between-pages check above keeps. `fetch_json` surfaces
            // a cancel observed MID-FLIGHT as `AppError::Cancelled`, which without
            // this arm would leave page 1 as `Err` and make `primary_chain` log a
            // "adzuna error, attempting jsearch fallback" that never happens.
            Err(_) if signal.is_cancelled() => break,
            // Page 1 IS the provider's result — its failure must stay a provider
            // failure so `primary_chain` can fall through to JSearch.
            Err(e) if page == 1 => return Err(e),
            Err(e) => {
                // PRIVACY: never log the raw `where`/location — free-text PII.
                log::warn!(
                    "[aggregator] adzuna page {page} failed, keeping {} result(s) already collected: {e}",
                    out.len()
                );
                break;
            }
        }
    }

    Ok(out)
}

/// Build the `FetchOptions` for an Adzuna page request.
///
/// `retries` is hardwired to [`ADZUNA_RETRIES`] (0). Mirrors
/// [`super::apify_fetch_options`]: the single source of truth consumed by both
/// the production call in [`fetch_adzuna_page`] and the invariant test in
/// `test.rs`, so dropping the override here breaks that test.
pub(super) fn adzuna_fetch_options() -> FetchOptions {
    FetchOptions {
        retries: ADZUNA_RETRIES, // METERED: each send bills the daily quota
        ..FetchOptions::default()
    }
}

/// Build + fetch + parse ONE Adzuna page.
///
/// Factored out of `AdzunaProvider::search` so [`fetch_adzuna_pages`] and the
/// near-empty broaden retry can reissue the exact same request (same `what`,
/// `sort_by=date`, `results_per_page`, and `max_days_old`) with only `page` /
/// `where` changed.
async fn fetch_adzuna_page(
    req: AdzunaPageRequest<'_>,
    page: u32,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    let AdzunaPageRequest {
        base_url,
        country,
        app_id,
        app_key,
        query,
        where_val,
        date_filter,
    } = req;

    // Sort newest-first (Adzuna defaults to relevance, which floats stale postings
    // up) and always bound the window with max_days_old so nothing older than the
    // cap (30 days, or the user's tighter pick) is returned. The page number is a
    // PATH segment (`…/search/{page}`), 1-based — not a query param.
    let url = format!(
        "{base_url}/v1/api/jobs/{}/search/{page}\
         ?app_id={}&app_key={}&what={}&where={}&results_per_page={ADZUNA_PAGE_SIZE}&content-type=application/json\
         &sort_by=date&sort_direction=down&max_days_old={}",
        urlencoding::encode(country),
        urlencoding::encode(app_id),
        urlencoding::encode(app_key),
        urlencoding::encode(query),
        urlencoding::encode(where_val),
        adzuna_max_days_old(date_filter),
    );

    // A non-2xx or schema-drift response propagates as `Err` from `fetch_json`
    // (carrying the HTTP status); `?` surfaces it as a provider failure. The
    // "adzuna:" prefix is required — the aggregator board fronts three
    // providers, so an unattributed "HTTP 403" in BoardScrapeSummary.error
    // wouldn't say which one failed.
    let resp = fetch_json::<AdzunaResp>(&url, adzuna_fetch_options(), signal)
        .await
        .map_err(|e| anyhow::anyhow!("adzuna: {e}"))?;

    let now = chrono::Utc::now().timestamp_millis();
    Ok(resp
        .results
        .into_iter()
        .map(|j| adzuna_job_to_posting(j, country, now))
        .collect())
}

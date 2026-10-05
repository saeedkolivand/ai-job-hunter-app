//! The Apify LinkedIn `JobProvider` (additive, opt-in, paid tier) — split
//! out of `providers.rs` (R8 module-size guard).
//!
//! Visibility: items here are `pub(super)` (visible to `aggregator` and its
//! descendants, including `tests`) rather than fully private, purely to
//! preserve this behavior-preserving move.
use async_trait::async_trait;
use serde::Deserialize;

use crate::observability::sanitize_reason;
use crate::scraping::http::{fetch_json, html_to_markdown, FetchOptions};
use crate::scraping::types::JobPosting;

use super::apify_settings::read_aggregator_settings;
use super::serde_helpers::de_opt_string_or_number;
use super::JobProvider;

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
/// [`ApifyLinkedInProvider::search`] and the invariant test in `tests/apify_fixes.rs`. A future
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
/// and by the invariant test in `tests/apify_fixes.rs`.  Removing the `retries` override here
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

//! freehire — the aggregator's KEYLESS tier.
//!
//! Requested in issue #1002 by freehire's own maintainer, who disclosed the
//! interest. Implemented here from their **published `openapi.yaml`** rather
//! than from the PR they offered: this repo has no CLA, so a contribution stays
//! under its author's copyright and cannot be relicensed, and ADR-023 names
//! commercial licensing as a rationale. Reading their PR and rewriting it would
//! be derivative — the worst of both worlds — so the spec is the only source
//! used.
//!
//! **Why it is worth a tier at all:** every other provider needs an API key, so
//! a fresh install with no keys gets a `needs-keys` skip and zero jobs. freehire
//! answers `GET /api/v1/agent/jobs/search` **unauthenticated**, which makes the
//! aggregator board useful before the user has configured anything. That is the
//! entire value; it is not a better data source than the keyed tiers and is
//! never preferred over one.
//!
//! **Tier position: last.** It runs only once Adzuna, JSearch and Jooble have
//! all failed to produce a decisive result — same rule that already governs
//! Jooble, one step further down. A keyed provider's `Ok` (even empty) still
//! short-circuits before this is reached, so a user with a working Adzuna key
//! and a genuinely empty search never contacts freehire at all.
//!
//! Two extra conditions, both added after a review reproduced the bugs their
//! absence caused (see `primary_chain`):
//!
//! * it does NOT run when a configured provider actually FAILED — a revoked or
//!   rate-limited key has to reach `BoardScrapeSummary.error`, and an always-on
//!   tier answering in its place would hide that on every search;
//! * when it does run after a distrusted guessed market, its results are
//!   MERGED behind the sparse keyed hits rather than replacing them, because
//!   on a guessed market this tier is location-blind and those hits are not.
//!
//! **Degradation:** any non-2xx, timeout, or schema drift resolves to `Ok(empty)`,
//! NOT `Err`. Every other provider reports its failure because a configured key
//! means the user asked for that provider specifically. freehire is configured
//! by nobody — it is always on — so surfacing its outage as a board error would
//! turn a third party's downtime into an error banner on a search the user never
//! pointed at them. There is no SLA, no rate-limit header of any kind, and no
//! documented limit (verified: no `X-RateLimit-*`, no `Retry-After`,
//! Cloudflare-fronted), so it must be treated as always-possibly-absent.
//!
//! Data rights: freehire's backend is MIT, but MIT covers the CODE and not the
//! postings — they disclaim ownership of the data and grant no redistribution
//! right. Their ToS permits "our documented API" and forbids scraping beyond it,
//! which is exactly what this does. Same chain-of-title exposure as the Adzuna /
//! JSearch tiers already shipped, so not categorically riskier than the status
//! quo (see ADR-026).

use async_trait::async_trait;
use serde::Deserialize;

use crate::scraping::http::{fetch_json, html_to_markdown, FetchOptions};
use crate::scraping::types::JobPosting;

use super::JobProvider;

/// Documented base (`servers[0].url` in the published spec).
pub(super) const FREEHIRE_BASE_URL: &str = "https://freehire.me/api/v1";

/// Spec ceiling for `limit`. Sending more is a 4xx, not a clamp.
const MAX_LIMIT: u32 = 100;

/// What we ask for when the caller expressed no item-count intent. The keyless
/// tier has no spend to bound, so this is purely "enough to be useful without
/// pulling a page nobody reads".
const DEFAULT_LIMIT: u32 = 50;

#[derive(Debug, Deserialize)]
struct FreehireEnvelope {
    #[serde(default)]
    data: Vec<FreehireJob>,
}

/// Only the fields this mapping consumes. `#[serde(default)]` throughout and no
/// `deny_unknown_fields`: the spec is a third party's and may gain fields, and a
/// keyless tier must never fail a search over a shape change.
#[derive(Debug, Deserialize)]
struct FreehireJob {
    #[serde(default)]
    public_slug: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    company: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    posted_at: Option<String>,
    /// freehire's own upstream ("workable", "ashby", "arbeitnow", …). Kept in
    /// `extra` so the Jobs page can say where a posting actually came from
    /// rather than attributing all of them to freehire.
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    work_mode: Option<String>,
}

fn map_freehire_job(j: FreehireJob, now: i64) -> Option<JobPosting> {
    let title = j
        .title
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let url = j
        .url
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;

    // `public_slug` is freehire's stable per-posting key; the URL is the
    // fallback so a slugless row still dedupes against itself rather than
    // colliding with every other slugless row under one shared id.
    let id_part = j
        .public_slug
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| url.clone());

    let mut extra = std::collections::HashMap::new();
    if let Some(upstream) = j.source.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        extra.insert(
            "aggregatorSource".to_string(),
            serde_json::Value::String(upstream.to_string()),
        );
    }
    if let Some(mode) = j
        .work_mode
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        extra.insert(
            "workMode".to_string(),
            serde_json::Value::String(mode.to_string()),
        );
    }

    Some(JobPosting {
        id: format!("aggregator:freehire-{id_part}"),
        external_id: Some(format!("freehire-{id_part}")),
        title,
        company: j.company.unwrap_or_default(),
        location: j
            .location
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        url,
        source: "aggregator".to_string(),
        // `description_format=markdown` is requested below, so this SHOULD
        // already be markdown — but it still goes through `html_to_markdown`,
        // like every sibling provider. Two reasons the conversion is not
        // redundant: it early-returns tag-free input verbatim (that is exactly
        // why Adzuna's already-markdown text is not double-escaped), so it
        // costs nothing when the parameter works; and freehire re-aggregates
        // workable/ashby/arbeitnow, any of which can leave HTML in a field
        // labelled markdown. Without it that HTML lands raw in SQLite, in
        // prompts via `posting_text_blob`, and as literal `<p>` in the
        // renderer's markdown view.
        description: j
            .description
            .map(|s| html_to_markdown(&s))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        requirements: None,
        posted_at: j.posted_at.as_deref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp_millis())
        }),
        captured_at: now,
        extra,
    })
}

/// Build + fetch + parse one freehire search page.
///
/// Takes `base_url` for the same reason `fetch_jooble` and `fetch_adzuna_page`
/// do: the non-2xx and mapping contracts stay unit-testable against a local
/// mock server without hitting the live host.
///
/// Returns `Err` here — the SILENT degradation lives one level up in
/// `FreehireProvider::search`, so these paths stay observable in a test while
/// the caller still can't turn a freehire outage into a user-facing board error.
///
/// **`location` is deliberately unused.** The documented search takes
/// `countries` (and region/work-mode/skill facets) but has NO city or free-text
/// location parameter, so the only place a city could go is `q` — which
/// full-text-matches title, company and description. Folding a city in there
/// silently drops every posting that does not happen to spell its city out,
/// including remote ones, which is worse than the country-level filter this
/// tier already applies. City precision stays the keyed tiers' job; this one is
/// a keyless floor, not a replacement for them.
pub(super) async fn fetch_freehire(
    base_url: &str,
    query: &str,
    country: &str,
    country_guessed: bool,
    amount: Option<u32>,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    let limit = amount.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let mut url = format!(
        "{}/agent/jobs/search?limit={limit}&description_format=markdown",
        base_url.trim_end_matches('/')
    );
    // The spec's full-text parameter is `q`. (There is an undocumented
    // `/jobs/search` taking `query`; it is deliberately not used — building
    // from the published spec is the whole basis for implementing this
    // ourselves, and an undocumented endpoint carries no such promise.)
    if !query.is_empty() {
        url.push_str(&format!("&q={}", urlencoding::encode(query)));
    }
    // Only filter by country when the caller actually chose one. A GUESSED
    // country is `AggregatorScraper::search`'s "de" default, and pinning the
    // keyless tier to a guessed market is how the guessed-market bug this
    // repo already fixed for Adzuna would reappear here.
    //
    // The consequence is deliberate and worth stating plainly: on a guessed
    // market this tier is location-blind, because `location` reaches it
    // nowhere at all (see the note on this function's `location` omission).
    // A globally-unfiltered result is therefore WEAKER than a keyed tier's
    // sparse guessed-market hits, not a replacement for them — which is why
    // `primary_chain` merges the two instead of letting this one win.
    if !country_guessed && !country.is_empty() {
        url.push_str(&format!("&countries={}", urlencoding::encode(country)));
    }

    let resp = fetch_json::<FreehireEnvelope>(
        &url,
        FetchOptions {
            // One retry rather than the default two: no documented rate limit
            // means no way to back off correctly, and this tier is optional by
            // construction — being quiet is cheaper than being persistent.
            retries: 1,
            timeout: Some(std::time::Duration::from_secs(15)),
            ..FetchOptions::default()
        },
        signal,
    )
    .await
    .map_err(|e| anyhow::anyhow!("freehire: {e}"))?;

    let now = chrono::Utc::now().timestamp_millis();
    Ok(resp
        .data
        .into_iter()
        .filter_map(|j| map_freehire_job(j, now))
        .collect())
}

/// The keyless tier. Holds no credential, so there is nothing to construct
/// from and nothing to fail reading — only the base URL, which exists as a
/// field purely so the silent-degradation contract below is reachable from a
/// test. That contract (a failure becomes `Ok(empty)`, never `Err`) is the
/// single most important thing about this provider and the easiest to
/// regress, so it must not be a claim only the live host can check.
pub(super) struct FreehireProvider {
    base_url: String,
}

impl FreehireProvider {
    pub(super) fn new() -> Self {
        Self {
            base_url: FREEHIRE_BASE_URL.to_string(),
        }
    }

    /// Point the provider at a mock server.
    #[cfg(test)]
    pub(super) fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl JobProvider for FreehireProvider {
    fn provider_id(&self) -> &'static str {
        "freehire"
    }

    /// Always. This is the one provider with no key, and it is what makes the
    /// aggregator board produce results on a fresh install — see
    /// `aggregator_has_configured_provider`, which counts it for exactly that
    /// reason.
    fn is_configured(&self) -> bool {
        true
    }

    async fn search(
        &self,
        query: &str,
        _location: &str,
        country: &str,
        country_guessed: bool,
        _date_filter: Option<&str>,
        amount: Option<u32>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // The silent-degradation boundary (see the module doc). Nobody opted
        // into this provider, so its failure is never the user's error to read.
        match fetch_freehire(
            &self.base_url,
            query,
            country,
            country_guessed,
            amount,
            signal,
        )
        .await
        {
            Ok(items) => Ok(items),
            Err(e) => {
                log::warn!("[aggregator] freehire keyless tier unavailable: {e}");
                Ok(vec![])
            }
        }
    }
}

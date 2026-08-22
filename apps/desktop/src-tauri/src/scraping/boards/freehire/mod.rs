//! freehire — the keyless, keyword-searchable job board.
//!
//! Requested in issue #1002 by freehire's own maintainer, who disclosed the
//! interest. Implemented here from their **published `openapi.yaml`** rather
//! than from the PR they offered: this repo has no CLA, so a contribution stays
//! under its author's copyright and cannot be relicensed, and ADR-023 names
//! commercial licensing as a rationale. Reading their PR and rewriting it would
//! be derivative — the worst of both worlds — so the spec is the only source
//! used.
//!
//! **Why it earns a place at all:** every other keyless board is a niche or
//! regional feed, and every broad source the app has needs an API key. freehire
//! answers `GET /api/v1/agent/jobs/search` **unauthenticated**, so it is the one
//! broad board a fresh install can search with before configuring anything.
//!
//! **History — it used to be the aggregator's keyless floor.** It ran last in
//! `aggregator::primary_chain`, only once every keyed provider had failed or come
//! back empty, and it degraded every fault to `Ok(empty)` because nobody had
//! opted into it. Both of those rules were consequences of it being ALWAYS ON
//! inside another board, and both are gone: it is now selected explicitly, so it
//! runs when — and only when — the user picks it, and a failure is reported
//! rather than swallowed (see `FreehireScraper::search`). The aggregator is once
//! again keyed providers only, and reports `needs-keys` without them.
//!
//! **Rate limits (as of the maintainer's issue #1026 disclosure, live-verified
//! 2026-08-18):** 600 req/min for ordinary reads, 300 req/min for
//! `/agent/jobs/search` specifically (the endpoint this module calls). Every
//! response — success included — carries `X-RateLimit-Limit/Remaining/Reset`;
//! going over budget answers `429` with `Retry-After`. The limiter is a leaky
//! bucket, not a counter (`Remaining` can trail this module's own request
//! count by one — pace against it, never reconcile it exactly). No new
//! rate-limiting code lives here: `scraping::http::fetch_text` already backs
//! off on `429`/`503` honoring `Retry-After` when present, for every board
//! that goes through it. This board's own traffic (one request per search)
//! sits nowhere near either ceiling.
//!
//! **User-Agent:** every request carries an identifying UA
//! (`freehire_user_agent`) — app name, crate version, and this repo's public
//! URL, nothing else (no user data, no machine id, no locale). The
//! maintainer's issue #1026 requests but does not enforce or validate it;
//! nothing behaves differently without it. It buys advance notice before a
//! field or limit changes, instead of a `429` being the first sign. This is
//! per-request (`FetchOptions::user_agent`), not the shared client's default
//! UA — other boards keep looking like an ordinary browser.
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
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};

/// Documented base (`servers[0].url` in the published spec).
const FREEHIRE_BASE_URL: &str = "https://freehire.me/api/v1";

/// Spec ceiling for `limit`. Sending more is a 4xx, not a clamp.
const MAX_LIMIT: u32 = 100;

/// What we ask for when the caller expressed no item-count intent. The keyless
/// board has no spend to bound, so this is purely "enough to be useful without
/// pulling a page nobody reads".
const DEFAULT_LIMIT: u32 = 50;

/// Map a UI date-filter token to freehire's `posted_within_days` (whole days,
/// `minimum: 1` per the live `openapi.yaml`). `None` (no filter chosen) omits
/// the parameter entirely — freehire's own semantics for "omit" is "no
/// freshness restriction" (`PostedWithinDays`'s description), so unlike
/// Adzuna/JSearch (which always send SOME day cap, even with no filter) this
/// board's no-filter search stays genuinely unfiltered.
///
/// Same 3-day floor as `adzuna::adzuna_max_days_old` / `providers::jsearch_date_posted`
/// for the identical reason: `posted_within_days` has no sub-day granularity —
/// a whole day is its smallest unit, and the spec forbids `0` outright — and a
/// naive same-day floor zeroed out autopilot "recent" filters on quiet days
/// (see those two functions' doc comments). Reusing the exact established
/// floor rather than inventing a new number keeps every source's recency
/// skew for sub-day tokens identical.
fn freehire_posted_within_days(date_filter: Option<&str>) -> Option<u32> {
    match date_filter {
        None => None,
        Some("15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "24h") => Some(3),
        Some("week") => Some(7),
        // "month" and any future/unrecognized token.
        Some(_) => Some(30),
    }
}

/// Identifying `User-Agent` the maintainer's issue #1026 now requests (never
/// enforced, never validated, nothing behaves differently without it — see
/// the module doc). Carries ONLY the app name, this crate's version, and the
/// public repo URL — no user data, no machine id, no locale. Mirrors the
/// existing `"ai-job-hunter/1.0"` convention `commands::geocoding` already
/// uses for Photon, with a version that tracks releases instead of a
/// hand-typed number that can rot.
fn freehire_user_agent() -> String {
    format!(
        "ai-job-hunter/{} (+https://github.com/saeedkolivand/ai-job-hunter-app)",
        env!("CARGO_PKG_VERSION")
    )
}

#[derive(Debug, Deserialize)]
struct FreehireEnvelope {
    #[serde(default)]
    data: Vec<FreehireJob>,
    #[serde(default)]
    meta: FreehireMeta,
}

/// Only the field this mapping reads. `#[serde(default)]` + no
/// `deny_unknown_fields`, matching [`FreehireJob`]'s contract — `meta` may
/// carry `total`/`limit`/`offset` too, which nothing here needs.
#[derive(Debug, Default, Deserialize)]
struct FreehireMeta {
    /// Present only when the request carried a parameter the search did not
    /// read (`openapi.yaml`'s `PaginationMeta.ignored_params`) — see
    /// `fetch_freehire`'s doc for what this module does about it.
    #[serde(default)]
    ignored_params: Vec<FreehireIgnoredParam>,
}

#[derive(Debug, Deserialize)]
struct FreehireIgnoredParam {
    param: String,
    #[serde(default)]
    did_you_mean: Option<String>,
}

/// Only the fields this mapping consumes. `#[serde(default)]` throughout and no
/// `deny_unknown_fields`: the spec is a third party's and may gain fields, and a
/// keyless board must never fail a search over a shape change.
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
///
/// Not adding `cities` either, though issue #1026 confirms it is real: geography
/// is a single OR-group on this API (`countries=gb` → 89,211;
/// `countries=gb&cities=London` → 89,617 — WIDER, not narrower, live-verified),
/// so folding a city in would need a location-UI decision this change doesn't
/// make. Left as a follow-up, not silently worked around here.
///
/// **`reality=fresh` is always sent, unconditionally.** freehire's `reality`
/// facet (`fresh`/`stale`/`likely-evergreen`) is live-verified to cut the
/// unfiltered catalogue roughly in half (1,477,440 → 662,112 in the same
/// spot-check that verified `posted_within_days` below). No toggle for it:
/// every other quality improvement this aggregator applies — `sort_by=date`
/// (Adzuna/JSearch), `dedupe_by_url` (all tiers) — is likewise always-on and
/// has no user-facing knob anywhere in this board, so a bespoke settings
/// toggle for just this one facet, on just this one last-resort tier, would be
/// the odd one out. This tier's own module doc already frames its purpose as
/// "enough to be useful without pulling a page nobody reads" ([`DEFAULT_LIMIT`]);
/// a `stale`/`likely-evergreen`-heavy result set (over half the unfiltered
/// catalogue) undercuts exactly that, and the filter costs nothing extra to
/// apply (already in the payload per the maintainer).
///
/// **`meta.ignored_params` is checked and treated as a failure, not a warning.**
/// freehire ignores an unrecognized query key rather than rejecting it — a
/// typo returns the WHOLE catalogue, indistinguishable from a legitimate broad
/// result, unless the response is checked (issue #1026, live-verified: a
/// `country=gb` typo comes back 200 with `meta.ignored_params:
/// [{"param":"country","did_you_mean":"countries"}]` and the full unfiltered
/// set in `data`). If any of THIS module's own params (`q`, `countries`,
/// `posted_within_days`, `reality`, …) ever appear there — a future upstream
/// rename would do it — this function returns `Err` instead of the response's
/// `data`, so the silent-degradation boundary in `FreehireProvider::search`
/// turns it into `Ok(empty)` for this tier, exactly like any other freehire
/// failure. The one option NOT taken is returning `resp.data` anyway: that
/// would silently hand back an unfiltered set dressed as a filtered one, which
/// is the one thing issue #1026 calls out as unacceptable.
async fn fetch_freehire(
    base_url: &str,
    query: &str,
    country: Option<&str>,
    date_filter: Option<&str>,
    amount: Option<u32>,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    let limit = amount.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let mut url = format!(
        "{}/agent/jobs/search?limit={limit}&description_format=markdown&reality=fresh",
        base_url.trim_end_matches('/')
    );
    // The spec's full-text parameter is `q`. (There is an undocumented
    // `/jobs/search` taking `query`; it is deliberately not used — building
    // from the published spec is the whole basis for implementing this
    // ourselves, and an undocumented endpoint carries no such promise.)
    if !query.is_empty() {
        url.push_str(&format!("&q={}", urlencoding::encode(query)));
    }
    // `None` means "do not filter by country", and that is what this board
    // sends today: see `Scraper::supports_location`, which stays `false` here
    // because freehire's geography facets are a single OR-GROUP — measured,
    // `countries=gb` returns 89,211 postings and `countries=gb&cities=London`
    // returns 89,617, so adding a place WIDENS the result instead of narrowing
    // it. A board that claimed server-side location support would switch off
    // the engine's central post-filter, which is the only thing actually
    // narrowing this board's results to the requested place.
    //
    // The parameter is kept (rather than dropped) because the facet itself is
    // real and correct for a caller that has a CHOSEN, non-guessed country;
    // there simply is no such caller now that `BoardSearchInput` carries only
    // a free-text location.
    if let Some(country) = country.map(str::trim).filter(|c| !c.is_empty()) {
        url.push_str(&format!("&countries={}", urlencoding::encode(country)));
    }
    if let Some(days) = freehire_posted_within_days(date_filter) {
        url.push_str(&format!("&posted_within_days={days}"));
    }

    let resp = fetch_json::<FreehireEnvelope>(
        &url,
        FetchOptions {
            // One retry rather than the default two: this tier is optional by
            // construction (nobody configured it) and runs only after every
            // keyed tier has already failed or come back empty, so being quiet
            // is cheaper than being persistent — NOT because there is nowhere
            // to learn how long to wait: `fetch_text` already backs off on
            // 429/503 honoring `Retry-After` when the response carries one
            // (see the module doc's rate-limit section).
            retries: 1,
            timeout: Some(std::time::Duration::from_secs(15)),
            user_agent: Some(freehire_user_agent()),
            ..FetchOptions::default()
        },
        signal,
    )
    .await
    .map_err(|e| anyhow::anyhow!("freehire: {e}"))?;

    if !resp.meta.ignored_params.is_empty() {
        // See this function's doc for why this is `Err`, not a warning that
        // still returns `resp.data`. Formatted by hand (not `{:?}`) so both
        // `param` and `did_you_mean` are read directly — `#[derive(Debug)]`
        // alone doesn't satisfy rustc's dead-code check on a struct's fields.
        let detail = resp
            .meta
            .ignored_params
            .iter()
            .map(|p| match &p.did_you_mean {
                Some(suggestion) => format!("{} (did you mean {suggestion}?)", p.param),
                None => p.param.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Err(anyhow::anyhow!(
            "freehire: upstream ignored request parameter(s), refusing to treat the response \
             as filtered: {detail}"
        ));
    }

    let now = chrono::Utc::now().timestamp_millis();
    Ok(resp
        .data
        .into_iter()
        .filter_map(|j| map_freehire_job(j, now))
        .collect())
}

/// freehire as a board the user picks in the catalog.
///
/// It holds no credential, so unlike every keyed source there is nothing to
/// configure and nothing to fail reading — selecting it is the whole setup.
pub struct FreehireScraper;

#[async_trait]
impl Scraper for FreehireScraper {
    fn id(&self) -> &'static str {
        "freehire"
    }

    fn display_name(&self) -> &'static str {
        "freehire"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    /// `false`, deliberately. freehire's geography facets are one OR-GROUP:
    /// measured, `countries=gb` returns 89,211 postings and
    /// `countries=gb&cities=London` returns 89,617 — naming a place WIDENS the
    /// result. Claiming server-side support would switch off the engine's
    /// central post-filter, which is the only thing narrowing this board to the
    /// requested location. See `fetch_freehire`'s `country` parameter.
    fn supports_location(&self) -> bool {
        false
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // A FAILURE IS REPORTED, not swallowed. As the aggregator's always-on
        // keyless tier this module degraded every fault to `Ok(empty)`, because
        // nobody had opted into it and a third party's outage should not raise
        // an error banner on a search the user never pointed at them. As a board
        // in the catalog that premise is gone: the user chose freehire, so its
        // outage is exactly the diagnostic they need, and an empty result would
        // read as "no such jobs" instead.
        let out = fetch_freehire(
            FREEHIRE_BASE_URL,
            input.query.trim(),
            // No country filter — see `supports_location`.
            None,
            input.date_filter.as_deref(),
            Some(input.amount),
            ctx.signal.clone(),
        )
        .await?;

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
mod test;

use crate::scraping::linkedin::api_client::{JobsSearchParams, LinkedInJobsApiClient};
use crate::scraping::linkedin::client::LinkedInHttpClient;
use crate::scraping::types::BoardSearchInput;
use crate::scraping::types::{JobPosting, ScrapeContext, Scraper};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

pub struct LinkedInScraper;

/// In-process cache of resolved geoIds, keyed by `(lowercased location query,
/// lowercased country code)`. LinkedIn geoIds are stable, so caching successful
/// resolutions avoids re-hitting the typeahead endpoint for repeated searches in
/// the same session (e.g. every autopilot run for the same target). Only
/// successful resolutions are cached — a transient network failure is left
/// uncached so the next search retries. Bounded in practice by the small number
/// of distinct (location, country) pairs a user searches.
static GEO_ID_CACHE: LazyLock<Mutex<HashMap<(String, String), String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// English country-name aliases used to bias typeahead selection toward the
/// requested ISO alpha-2 country. LinkedIn's typeahead `displayName` ends with
/// the English country name (e.g. "Berlin, Germany" vs "Berlin, Connecticut,
/// United States"), and blindly taking the first hit leaks the search into the
/// wrong country (#49). Covers the markets the app searches; an unlisted code
/// returns `None`, so selection falls back to the first hit (no regression).
fn country_aliases(cc: &str) -> Option<&'static [&'static str]> {
    Some(match cc {
        "de" => &["germany"],
        "at" => &["austria"],
        "ch" => &["switzerland"],
        "be" => &["belgium"],
        "gb" | "uk" => &["united kingdom"],
        "ie" => &["ireland"],
        "us" => &["united states"],
        "ca" => &["canada"],
        "fr" => &["france"],
        "nl" => &["netherlands"],
        "es" => &["spain"],
        "it" => &["italy"],
        "pl" => &["poland"],
        "pt" => &["portugal"],
        "se" => &["sweden"],
        "au" => &["australia"],
        "nz" => &["new zealand"],
        "in" => &["india"],
        "sg" => &["singapore"],
        "br" => &["brazil"],
        "mx" => &["mexico"],
        "za" => &["south africa"],
        _ => return None,
    })
}

/// Extract a geoId string from one typeahead hit's `id` (number or string).
fn hit_geo_id(hit: &serde_json::Value) -> Option<String> {
    match hit.get("id")? {
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::String(s) => (!s.is_empty()).then(|| s.clone()),
        _ => None,
    }
}

/// Pick a geoId from the typeahead hits, biased toward `country_code` when set.
/// When a country_code maps to known name aliases AND some hit's `displayName`
/// matches that country, the FIRST such hit wins; otherwise (no country_code, an
/// unlisted code, or no hit matches) fall back to the first usable hit — the
/// prior behaviour, so this never regresses a resolvable location. Pure +
/// unit-testable (no network).
fn select_geo_id(hits: &[serde_json::Value], country_code: Option<&str>) -> Option<String> {
    if let Some(aliases) = country_code
        .map(str::to_lowercase)
        .filter(|s| !s.is_empty())
        .and_then(|cc| country_aliases(&cc))
    {
        let biased = hits.iter().find(|h| {
            hit_geo_id(h).is_some()
                && h.get("displayName")
                    .and_then(|v| v.as_str())
                    .map(|dn| {
                        // Anchor to the TRAILING segment, not a substring anywhere —
                        // `displayName` ends with the English country name, so
                        // `contains` false-positived on "India" matching "Indiana,
                        // United States" and "Ireland" matching "Northern Ireland,
                        // United Kingdom".
                        let dn = dn.trim().to_lowercase();
                        aliases.iter().any(|a| dn.ends_with(a))
                    })
                    .unwrap_or(false)
        });
        if let Some(hit) = biased {
            return hit_geo_id(hit);
        }
    }
    hits.iter().find_map(hit_geo_id)
}

/// Best-effort LinkedIn geoId lookup via the public jobs-guest typeahead,
/// biased toward `country_code` and cached in-process (see `GEO_ID_CACHE`).
/// Returns `None` on any failure so callers fall back to the free-text location
/// filter (no regression). The endpoint is unofficial — verify behaviour with a
/// real scrape after changing this (live-verified 2026-07-11: returns a
/// `text/plain` JSON array of `{id, displayName, …}`, several hits per query).
async fn resolve_geo_id(location: &str, country_code: Option<&str>) -> Option<String> {
    let key = (
        location.trim().to_lowercase(),
        country_code.unwrap_or_default().to_lowercase(),
    );
    if let Some(cached) = GEO_ID_CACHE.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Some(cached);
    }

    let url = format!(
        "https://www.linkedin.com/jobs-guest/api/typeaheadHits?typeaheadType=GEO&query={}",
        urlencoding::encode(location)
    );
    let resp = crate::net::http::shared()
        .get(&url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (compatible; ai-job-hunter/1.0)",
        )
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    let hits: Vec<serde_json::Value> = resp.json().await.ok()?;
    let geo = select_geo_id(&hits, country_code)?;

    if let Ok(mut cache) = GEO_ID_CACHE.lock() {
        cache.insert(key, geo.clone());
    }
    Some(geo)
}

#[async_trait]
impl Scraper for LinkedInScraper {
    fn id(&self) -> &'static str {
        "linkedin"
    }

    fn display_name(&self) -> &'static str {
        "LinkedIn"
    }

    fn mode(&self) -> crate::scraping::types::ScraperMode {
        crate::scraping::types::ScraperMode::Http
    }

    fn auth(&self) -> crate::scraping::types::AuthRequirement {
        crate::scraping::types::AuthRequirement::Optional
    }

    fn supports_location(&self) -> bool {
        // LinkedIn narrows server-side: it resolves the free-text location to a
        // geoId typeahead and passes `distance` (radius) to the jobs search.
        true
    }

    async fn search(&self, input: BoardSearchInput, ctx: ScrapeContext) -> Result<Vec<JobPosting>> {
        // Try to load session data from disk
        let session_data = self.load_session_data().await;
        if session_data.is_some() {
            // Touch the auth-status heartbeat so freshness countdown restarts.
            crate::scraping::board_login::touch_session(
                &crate::platform::config::data_dir(),
                "linkedin",
            );
        }

        // Create HTTP client with session
        let http_client = LinkedInHttpClient::new(session_data)?;
        let api_client = LinkedInJobsApiClient::new(http_client);

        // Resolve a precise geoId for the location (best-effort, country-biased,
        // cached). On failure we fall back to the free-text `location` filter
        // below (no regression, #49).
        let geo_id = match input.location.as_deref() {
            Some(loc) if !loc.trim().is_empty() => {
                resolve_geo_id(loc, input.country_code.as_deref()).await
            }
            _ => None,
        };

        let params = JobsSearchParams {
            keywords: input.query.clone(),
            location: input.location.clone(),
            geo_id,
            distance: input.radius_km,
            start: 0,
            date_filter: input.date_filter.clone(),
            job_type: input.job_type.clone(),
            work_type: input.work_type.clone(),
            experience_level: input.experience_level.clone(),
            easy_apply: input.easy_apply,
            actively_hiring: input.actively_hiring,
            verified: input.verified,
            sort_by: input.sort_by.clone(),
        };

        let max_pages = input.pages.clamp(1, 10) as usize;
        let signal = Some(&ctx.signal);

        api_client
            .search_paginated(&params, max_pages, signal, ctx.on_progress, ctx.on_item)
            .await
    }
}

impl LinkedInScraper {
    /// Load LinkedIn session by reading the cookies.json exported by the
    /// board login flow (`board_login::open_login`). Returns None if the user
    /// has not logged in or the cookies don't contain `li_at`.
    async fn load_session_data(
        &self,
    ) -> Option<crate::scraping::linkedin::session::LinkedInSessionData> {
        use crate::scraping::board_login;
        use crate::scraping::linkedin::session::{Cookie, LinkedInSessionData};

        let data_dir = crate::platform::config::data_dir();
        if board_login::session_is_stale(&data_dir, "linkedin") {
            log::warn!("[linkedin] session is stale — falling back to guest mode");
            return None;
        }
        let cookies = board_login::load_cookies(&data_dir, "linkedin");
        if cookies.is_empty() {
            return None;
        }

        let li_at = cookies
            .iter()
            .find(|c| c.name == "li_at" && c.domain.contains("linkedin.com"))?
            .value
            .clone();

        let jsession_id = cookies
            .iter()
            .find(|c| c.name == "JSESSIONID" && c.domain.contains("linkedin.com"))
            .map(|c| c.value.trim_matches('"').to_string());

        // LinkedIn embeds the CSRF token in JSESSIONID as `ajax:<token>`.
        let csrf_token = jsession_id
            .as_ref()
            .and_then(|v| v.strip_prefix("ajax:").map(str::to_string));

        let mapped_cookies = cookies
            .iter()
            .map(|c| Cookie {
                name: c.name.clone(),
                value: c.value.clone(),
                domain: Some(c.domain.clone()),
                path: Some(c.path.clone()),
                expires: c.expires,
            })
            .collect();

        Some(LinkedInSessionData {
            cookies: mapped_cookies,
            li_at,
            jsession_id,
            csrf_token,
            last_updated: chrono::Utc::now().timestamp_millis() as u64,
        })
    }
}

#[cfg(test)]
mod test;

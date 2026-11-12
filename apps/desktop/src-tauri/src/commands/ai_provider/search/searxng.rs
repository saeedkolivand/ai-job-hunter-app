//! SearXNG-compatible search (SearXNG <https://docs.searxng.org>, RustXNG
//! <https://github.com/sahenjp/RustXNG>) — a keyless, self-hosted metasearch
//! [`super::WebSearcher`] backend. The user runs an instance and points the app
//! at it; the app only reads its JSON API (`GET {base}/search?q=…&format=json`).
//! One backend serves every compatible server: both emit `results[]` of
//! `{ url, title, content }` (RustXNG: `src/results.rs` `SearchResult`, wrapped by
//! `src/aggregator.rs` `SearchResponse.results`). RustXNG's MCP tool names the
//! snippet `snippet` instead (`src/mcp/mod.rs` `SearchItem`), so that is read as
//! a fallback. No key, no per-query cost, so no daily budget bucket (unlike Exa).

use async_trait::async_trait;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

use super::super::research::SearchResult;
use super::super::{timeouts, ProviderId, RequestTrace};
use super::{cap_chars, WebSearcher};

pub struct SearxngSearcher {
    base: String,
}

impl SearxngSearcher {
    /// `None` when no URL is stored or the stored one fails validation (a restored backup can
    /// hold anything — the writer's check is re-run at the point of use).
    pub fn from_settings(app: &AppHandle) -> Option<Self> {
        let url = app
            .try_state::<crate::ai_config::AiConfigStore>()?
            .searxng_url()?;
        Self::new(&url)
    }

    pub(super) fn new(url: &str) -> Option<Self> {
        let base = normalize_searxng_url(url).ok()??;
        Some(Self { base })
    }
}

/// Whether `url` may be reached over plain `http`: loopback, private/LAN addresses, `localhost`
/// and `.local` names. Anything else is a public host and must use `https`.
fn is_local_host(url: &reqwest::Url) -> bool {
    use std::net::{Ipv4Addr, Ipv6Addr};
    let Some(host) = url.host_str() else {
        return false;
    };
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return ip.is_loopback() || ip.is_private();
    }
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<Ipv6Addr>() {
        return ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00;
    }
    host == "localhost" || host.ends_with(".local")
}

/// Validate and normalise a user-entered SearXNG base URL: trimmed, http/https with a host,
/// no credentials, no query/fragment, no trailing slash; plain `http` only for a local host.
/// `Ok(None)` for blank input (clears the setting). Credentials are refused because the URL
/// is stored in plaintext, exported in backups and synced.
pub fn normalize_searxng_url(raw: &str) -> AppResult<Option<String>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    crate::net::ssrf::validate_provider_base_url(raw)?;
    let url = reqwest::Url::parse(raw)
        .map_err(|e| AppError::Validation(format!("Invalid SearXNG URL: {e}")))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::Validation(
            "SearXNG URL must not contain a username or password.".into(),
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(AppError::Validation(
            "SearXNG URL must not contain a query or fragment.".into(),
        ));
    }
    if url.scheme() == "http" && !is_local_host(&url) {
        return Err(AppError::Validation(
            "SearXNG URL must use https unless it points at this machine or the local network."
                .into(),
        ));
    }
    Ok(Some(url.as_str().trim_end_matches('/').to_string()))
}

/// A SearXNG-compatible non-2xx as a typed error. The server is keyless and user-run, so NONE
/// of these fall through to another backend, and the messages are FIXED: no status digits, no
/// body, no URL, so no remote or scraped text can make `FailureKind` read a bare "401"/"402"
/// as billing or auth. A 401/403 is almost always `json` missing from the server's `formats`,
/// or the user's own proxy refusing; both are fixed by the user, not by another vendor.
pub(super) fn searxng_status_error(status: reqwest::StatusCode) -> AppError {
    match status.as_u16() {
        401 | 403 => AppError::Config(
            "searxng: the server refused the request; enable JSON output (`json` under `formats`) on the server."
                .into(),
        ),
        429 => AppError::RateLimited("searxng: the server is rate limiting requests.".into()),
        500..=599 => AppError::Network("searxng: the server reported an error.".into()),
        _ => AppError::Provider("searxng: the server sent an unexpected response.".into()),
    }
}

/// A transport failure, classified by the reqwest variant with a fixed message. `Display` on
/// `reqwest::Error` embeds the request URL (host AND the `q=` query), so it is never used.
fn transport_error(e: &reqwest::Error) -> AppError {
    if e.is_timeout() {
        AppError::Timeout("searxng: the server did not answer in time.".into())
    } else {
        AppError::Network("searxng: the server could not be reached.".into())
    }
}

#[async_trait]
impl WebSearcher for SearxngSearcher {
    async fn try_search(&self, query: &str, limit: usize) -> AppResult<Vec<SearchResult>> {
        // The base URL is the user's own host; the label carries only the identity.
        let trace = RequestTrace::begin(
            ProviderId::Ollama,
            "searxng",
            "searxng:/search",
            "searxng",
            false,
        );
        let resp = crate::net::http::shared()
            .get(format!("{}/search", self.base))
            .query(&[("q", query), ("format", "json")])
            .timeout(timeouts::SEARXNG_SEARCH)
            .send()
            .await
            .map_err(|e| {
                trace.end(None, false);
                transport_error(&e)
            })?;
        let status = resp.status();
        if !status.is_success() {
            trace.end(Some(status.as_u16()), false);
            return Err(searxng_status_error(status));
        }
        let body =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|_| {
                    trace.end(Some(status.as_u16()), false);
                    AppError::Provider("searxng: the response was not valid JSON.".into())
                })?;
        let parsed = parse_searxng_results(&body, limit);
        trace.end(Some(status.as_u16()), parsed.is_ok());
        parsed
    }
}

/// Map a SearXNG-compatible `/search?format=json` body to [`SearchResult`]s. Pure + unit-tested.
///
/// A body with no `results` array is NOT "the web had nothing": it is not search output at all
/// (an error page served as JSON, a proxy), so it is a typed error. An EMPTY `results` next to
/// a non-empty `unresponsive_engines` is the server saying its engines failed (CAPTCHA,
/// suspended), which is `Unavailable`, not "found nothing" (#1500). Only a plain empty
/// `results` is an empty `Ok`. A result with neither `content` nor `snippet`, or without an
/// http(s) `url`, is dropped; `content` and `title` are capped on a char boundary.
pub(super) fn parse_searxng_results(
    body: &serde_json::Value,
    limit: usize,
) -> AppResult<Vec<SearchResult>> {
    const CONTENT_CAP: usize = 1_000;
    const URL_CAP: usize = 2_048;

    let results = body
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| AppError::Provider("searxng: the response had no results list.".into()))?;
    let engines_failed = body
        .get("unresponsive_engines")
        .and_then(|u| u.as_array())
        .is_some_and(|u| !u.is_empty());
    if results.is_empty() && engines_failed {
        return Err(AppError::Network(
            "searxng: its search engines are unavailable.".into(),
        ));
    }
    let text = |r: &serde_json::Value, key: &str| {
        r.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    Ok(results
        .iter()
        .filter_map(|r| {
            let url = text(r, "url");
            if url.chars().count() > URL_CAP
                || !(url.starts_with("http://") || url.starts_with("https://"))
            {
                return None;
            }
            let content = Some(text(r, "content")).filter(|c| !c.is_empty());
            let snippet = cap_chars(&content.unwrap_or_else(|| text(r, "snippet")), CONTENT_CAP);
            (!snippet.is_empty()).then(|| SearchResult {
                title: cap_chars(&text(r, "title"), super::TITLE_CAP),
                snippet,
                url,
            })
        })
        .take(limit)
        .collect())
}

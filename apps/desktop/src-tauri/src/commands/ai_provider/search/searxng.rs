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
use super::WebSearcher;

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

/// Validate and normalise a user-entered SearXNG base URL: trimmed, http/https with a host,
/// no query/fragment, no trailing slash. `Ok(None)` for blank input (clears the setting).
pub fn normalize_searxng_url(raw: &str) -> AppResult<Option<String>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    crate::net::ssrf::validate_provider_base_url(raw)?;
    let url = reqwest::Url::parse(raw)
        .map_err(|e| AppError::Validation(format!("Invalid SearXNG URL: {e}")))?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err(AppError::Validation(
            "SearXNG URL must not contain a query or fragment.".into(),
        ));
    }
    Ok(Some(url.as_str().trim_end_matches('/').to_string()))
}

/// A SearXNG non-2xx as a typed error. Body deliberately NOT read (it is an HTML page or a
/// reverse proxy's text, and the message ends up in a run's failure record). An instance that
/// has not enabled `json` under `search.formats` answers 403; that reads as unusable, so it is
/// worded as an auth refusal and the pass moves on to the next backend.
pub(super) fn searxng_status_error(status: reqwest::StatusCode) -> AppError {
    match status.as_u16() {
        401 | 403 => AppError::Config(
            "searxng: unauthorized or JSON output disabled (403); enable `json` in search.formats."
                .into(),
        ),
        429 => AppError::RateLimited("searxng: rate limit reached.".into()),
        500..=599 => AppError::Network(format!("searxng: service error ({}).", status.as_u16())),
        code => AppError::Provider(format!("searxng returned {code}")),
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
                AppError::Network(format!("searxng unreachable: {e}"))
            })?;
        let status = resp.status();
        if !status.is_success() {
            trace.end(Some(status.as_u16()), false);
            return Err(searxng_status_error(status));
        }
        let body =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| {
                    trace.end(Some(status.as_u16()), false);
                    AppError::Provider(format!("searxng response was not JSON: {e}"))
                })?;
        let parsed = parse_searxng_results(&body, limit);
        trace.end(Some(status.as_u16()), parsed.is_ok());
        parsed
    }
}

/// Map a SearXNG-compatible `/search?format=json` body to [`SearchResult`]s. Pure + unit-tested.
///
/// A body with no `results` array is NOT "the web had nothing" — it is a response that is not
/// SearXNG's search output (an error page served as JSON, a proxy), so it is a typed error;
/// only a present-but-empty `results` is an empty `Ok`. A result with neither `content` nor `snippet` is dropped
/// (a bare title costs synthesis tokens for nothing); `content` is capped on a char boundary.
pub(super) fn parse_searxng_results(
    body: &serde_json::Value,
    limit: usize,
) -> AppResult<Vec<SearchResult>> {
    const CONTENT_CAP: usize = 1_000;

    let results = body
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| {
            AppError::Provider("searxng search parse failed: no results array".into())
        })?;
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
            let content = Some(text(r, "content")).filter(|c| !c.is_empty());
            let snippet: String = content
                .unwrap_or_else(|| text(r, "snippet"))
                .chars()
                .take(CONTENT_CAP)
                .collect();
            (!snippet.is_empty()).then(|| SearchResult {
                title: text(r, "title"),
                snippet,
                url: text(r, "url"),
            })
        })
        .take(limit)
        .collect())
}

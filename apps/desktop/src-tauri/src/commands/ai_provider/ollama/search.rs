//! Ollama's first-party Web Search API — the transport, response parsing,
//! and the [`WebSearcher`](super::super::search::WebSearcher) impl every
//! Ollama-family research facet uses. Split out of `ollama.rs` (R8 LOC cap)
//! — a pure move.

use async_trait::async_trait;
use serde_json::json;
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::{AppError, AppResult};

use super::super::research::SearchResult;
use super::super::{timeouts, ProviderId, RequestTrace};
use super::{ACCOUNT_KEY, WEB_SEARCH_URL};

/// Call Ollama's Web Search API and return up to `limit` result snippets. `key`
/// is the Ollama account key (`ai:ollama-cloud`). Pure transport — any error is
/// surfaced for the caller to swallow, so a missing/invalid key never breaks
/// generation.
pub async fn ollama_web_search(
    key: &str,
    query: &str,
    limit: usize,
) -> AppResult<Vec<SearchResult>> {
    let resp = crate::net::http::shared()
        .post(WEB_SEARCH_URL)
        .timeout(timeouts::OLLAMA_WEB_SEARCH)
        .bearer_auth(key)
        .json(&json!({ "query": query, "max_results": limit.min(10) }))
        .send()
        .await
        .map_err(|e| format!("ollama web_search request: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body =
            crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
        return Err(AppError::Network(format!(
            "ollama web_search {status}: {body}"
        )));
    }
    let body: serde_json::Value =
        crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
            .await
            .map_err(|e| format!("ollama web_search parse: {e}"))?;
    Ok(parse_web_search(&body, limit))
}

/// Map an Ollama `web_search` response (`{ results: [{title,url,content}] }`) to
/// `SearchResult`. Pure + unit-tested.
pub(super) fn parse_web_search(body: &serde_json::Value, limit: usize) -> Vec<SearchResult> {
    body.get("results")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .take(limit)
                .map(|item| SearchResult {
                    title: item
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet: item
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: item
                        .get("url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Shared search step for every Ollama-family research facet: resolve the
/// account key and run the Ollama Web Search API. Returns an empty `Vec` when
/// the key is missing or the search fails, so callers degrade to `""` without
/// each re-implementing the key-check + trace boilerplate.
async fn ollama_search(model: &str, key: &str, query: &str, limit: usize) -> Vec<SearchResult> {
    if key.trim().is_empty() {
        return Vec::new();
    }
    let trace = RequestTrace::begin(
        ProviderId::OllamaCloud,
        model,
        "/api/web_search",
        "https://ollama.com",
        false,
    );
    match ollama_web_search(key, query, limit).await {
        Ok(r) => {
            trace.end(Some(200), true);
            r
        }
        Err(e) => {
            trace.end(None, false);
            tracing::warn!("ollama web_search failed: {e}");
            Vec::new()
        }
    }
}

/// The Ollama Web Search API as a [`WebSearcher`](super::super::search::WebSearcher).
///
/// The search half of Ollama-family research. The synthesize half is generic and
/// lives in [`super::super::search`] — this is the only Ollama-specific part, which is
/// why the pipeline could be shared with a configurable backend at all.
pub struct OllamaSearcher {
    /// Trace label only — the Web Search API takes no model.
    model: String,
    /// Resolved once at construction. `ollama_search` used to re-read it on
    /// every call, which with the capability probe meant three keychain lookups
    /// for one research pass.
    key: String,
}

impl OllamaSearcher {
    /// `None` when no ollama.com account key is stored. Local Ollama advertises
    /// `supports_web_search` but the API needs a CLOUD key, so a keyless local
    /// install has no usable native search — the case the configurable fallback
    /// exists for.
    pub fn from_credentials(app: &AppHandle, model: &str) -> Option<Self> {
        let key = get_provider_key(app, ACCOUNT_KEY)?;
        let key = key.trim().to_string();
        (!key.is_empty()).then(|| Self {
            model: model.to_string(),
            key,
        })
    }
}

#[async_trait]
impl super::super::search::WebSearcher for OllamaSearcher {
    async fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        ollama_search(&self.model, &self.key, query, limit).await
    }
}

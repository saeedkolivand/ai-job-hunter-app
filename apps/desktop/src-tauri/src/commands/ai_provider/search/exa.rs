//! Exa (<https://exa.ai>) — a hosted retrieval API [`super::WebSearcher`]
//! backend. One POST, one key, no local runtime. Split out of `search/mod.rs`
//! (R8 line-budget split): the concrete retrieval backend is a separate
//! concern from the routing policy that picks between backends.

use async_trait::async_trait;
use serde_json::json;
use tauri::{AppHandle, Manager};

use super::super::research::SearchResult;
use super::super::{timeouts, ProviderId, RequestTrace};
use super::WebSearcher;

/// Credential slot for the Exa key: `ai:exa` in the OS keychain, via the same
/// `ai_set_provider_key`/`ai_has_provider_key` commands every provider key uses.
/// `ollama-cloud`'s account key is the precedent for a credential whose name is
/// not a generation `ProviderId`.
pub const EXA_KEY: &str = "exa";

const EXA_SEARCH_URL: &str = "https://api.exa.ai/search";

pub struct ExaSearcher {
    app: AppHandle,
    key: String,
}

impl ExaSearcher {
    /// `None` when no key is stored, so the caller can't construct a searcher
    /// that is guaranteed to return nothing.
    pub fn from_credentials(app: &AppHandle) -> Option<Self> {
        let key = crate::commands::ai::get_provider_key(app, EXA_KEY)?;
        let key = key.trim().to_string();
        (!key.is_empty()).then(|| Self {
            app: app.clone(),
            key,
        })
    }

    /// Charge one Exa search against the shared per-vendor daily ceiling.
    ///
    /// Exa bills per request and is a DIFFERENT vendor than the AI provider, so
    /// it gets its own bucket rather than spending the provider's. The counter
    /// map is `(utc_day, vendor)`-keyed, so a name that is not a `ProviderId`
    /// needs no schema change. `false` means the ceiling is reached and the
    /// caller must not issue the request.
    fn charge_daily(&self) -> bool {
        let Some(limiter) = self
            .app
            .try_state::<std::sync::Arc<crate::limits::Limiter>>()
        else {
            // No limiter in state (only reachable in a partially-built app) —
            // fail CLOSED on a billable call rather than assume budget.
            tracing::warn!("exa search: limiter unavailable, skipping search");
            return false;
        };
        match limiter.charge_provider_daily(EXA_KEY, crate::limits::PROVIDER_DAILY_MAX) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("exa search: daily budget exceeded: {e}");
                false
            }
        }
    }
}

#[async_trait]
impl WebSearcher for ExaSearcher {
    async fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        // Charged BEFORE the request because this IS the billable call — unlike
        // the command-layer charges, which sit before an admission check.
        if !self.charge_daily() {
            return Vec::new();
        }
        let trace = RequestTrace::begin(
            // Traced under the ACTIVE provider is wrong here — the call is Exa's.
            // `ProviderId` has no Exa arm on purpose (a search backend is not a
            // generation provider), so the endpoint label carries the identity.
            ProviderId::Ollama,
            "exa",
            "exa:/search",
            "https://api.exa.ai",
            false,
        );
        // `highlights` are the model-selected relevant spans — the closest match
        // to the `snippet` shape `research::synth_user` already formats. `text`
        // is the whole page and would blow the synthesis prompt.
        let body = json!({
            "query": query,
            "numResults": limit.min(10),
            "type": "auto",
            "contents": { "highlights": true },
        });
        let resp = crate::net::http::shared()
            .post(EXA_SEARCH_URL)
            .timeout(timeouts::EXA_SEARCH)
            .header("x-api-key", &self.key)
            .json(&body)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                tracing::warn!("exa search request failed: {e}");
                return Vec::new();
            }
        };
        let status = resp.status();
        if !status.is_success() {
            trace.end(Some(status.as_u16()), false);
            // Body deliberately NOT logged: an auth failure echoes request
            // context, and this line is the one a diagnostics bundle ships.
            tracing::warn!("exa search returned {status}");
            return Vec::new();
        }
        let body = match crate::net::http::read_json_capped(
            resp,
            crate::net::http::DEFAULT_MAX_BODY_BYTES,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                trace.end(Some(status.as_u16()), false);
                tracing::warn!("exa search parse failed: {e}");
                return Vec::new();
            }
        };
        trace.end(Some(status.as_u16()), true);
        parse_exa_results(&body, limit)
    }
}

/// Map an Exa `/search` response body to [`SearchResult`]s. Pure + unit-tested —
/// the only part of the Exa integration a test can reach without a network.
///
/// Prefers `highlights` (relevance-selected spans) and falls back to `text` when
/// a result has none, capped, since `text` is the entire page. A result with
/// neither is dropped rather than passed through empty: a title with no content
/// adds nothing to the synthesis prompt but still costs tokens.
pub(super) fn parse_exa_results(body: &serde_json::Value, limit: usize) -> Vec<SearchResult> {
    const TEXT_FALLBACK_CAP: usize = 1_000;

    body.get("results")
        .and_then(|r| r.as_array())
        .map(|results| {
            results
                .iter()
                .filter_map(|r| {
                    let snippet = r
                        .get("highlights")
                        .and_then(|h| h.as_array())
                        .map(|spans| {
                            spans
                                .iter()
                                .filter_map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .filter(|s| !s.trim().is_empty())
                        .or_else(|| {
                            r.get("text")
                                .and_then(|t| t.as_str())
                                .filter(|t| !t.trim().is_empty())
                                // `chars().take` — a byte slice could split a
                                // multi-byte char and panic.
                                .map(|t| t.chars().take(TEXT_FALLBACK_CAP).collect())
                        })?;
                    Some(SearchResult {
                        title: r
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        snippet,
                        url: r
                            .get("url")
                            .and_then(|u| u.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    })
                })
                .take(limit)
                .collect()
        })
        .unwrap_or_default()
}

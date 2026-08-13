//! Web-search backends — the retrieval half of company research.
//!
//! ## Why this is a separate axis from the AI provider
//!
//! Research is two steps: **search** (fetch snippets about a company) and
//! **synthesize** (turn snippets into a brief). Providers with a model-side
//! search tool (OpenAI/Anthropic/Gemini, CLI agents) do both in one call. The
//! Ollama family cannot, so it already did them separately — search via the
//! Ollama Web Search API, then synthesize with its own model.
//!
//! That second shape is the general one, and the only provider-specific part of
//! it is the search call. Factoring it out behind [`WebSearcher`] means a
//! provider with no usable search of its own can still research, using a
//! search backend the user configures — today [`ExaSearcher`].
//!
//! A **search backend is not an AI provider**: it returns web results and cannot
//! generate. It deliberately does not appear in `ProviderId`, in
//! `Completer::from_active`'s routing, or in the renderer's provider registry.
//!
//! ## Who runs
//!
//! [`resolve_search_backend`] decides from CONFIGURATION, before any call:
//! native when the provider has a usable search, otherwise the configured
//! fallback, otherwise nothing. A native search that runs and returns nothing is
//! NOT retried against the fallback — one research pass makes one search, so
//! cost stays predictable and only one vendor sees the query.
//!
//! Synthesis always stays on the user's own model. Exa's own answer endpoint
//! would be fewer calls, but it would bypass both `research::SYNTH_SYSTEM`'s
//! prompt-injection guard (search results are attacker-reachable text) and the
//! `is_no_info` filter, and it would move generation spend to a second vendor.

use async_trait::async_trait;
use serde_json::json;
use tauri::{AppHandle, Manager};

use super::research::SearchResult;
use super::{timeouts, ProviderId, RequestTrace};

/// Credential slot for the Exa key: `ai:exa` in the OS keychain, via the same
/// `ai_set_provider_key`/`ai_has_provider_key` commands every provider key uses.
/// `ollama-cloud`'s account key is the precedent for a credential whose name is
/// not a generation `ProviderId`.
pub const EXA_KEY: &str = "exa";

const EXA_SEARCH_URL: &str = "https://api.exa.ai/search";

/// Which backend serves one research pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchBackend {
    /// The provider's own model-side search (or the Ollama Web Search API).
    Native,
    /// The user-configured fallback.
    Exa,
    /// Nothing configured — research degrades to an empty brief.
    None,
}

impl SearchBackend {
    /// Stable string term for a cache key (e.g.
    /// `cover_letter::research`'s `company_brief` key) — not `Debug`, so a
    /// variant rename can't silently change every existing key.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Exa => "exa",
            Self::None => "none",
        }
    }
}

/// Pick the backend for one research pass. Pure, so the policy is testable
/// without an `AppHandle` — this crate has no `tauri::test` mock-app harness,
/// and the same extraction is why `OpenAiClient::supports_web_search` and
/// `salary_research::role_is_missing` exist as standalone predicates.
///
/// Fallback-ONLY by design: a user whose provider already searches keeps using
/// it and never silently starts paying a second vendor, even with an Exa key
/// stored. That is the whole decision, and the test on it is what stops this
/// quietly becoming Exa-preferred later.
pub fn resolve_search_backend(native_ready: bool, exa_key_present: bool) -> SearchBackend {
    if native_ready {
        SearchBackend::Native
    } else if exa_key_present {
        SearchBackend::Exa
    } else {
        SearchBackend::None
    }
}

/// A source of web-search snippets for research.
///
/// Returns an empty `Vec` rather than an error on every failure — a missing key,
/// a refused request, an unparseable body. Research degrades to "no brief" and
/// generation proceeds; it must never fail because a search did.
#[async_trait]
pub trait WebSearcher: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Vec<SearchResult>;
}

/// Exa (<https://exa.ai>) — a hosted retrieval API. One POST, one key, no local
/// runtime.
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
pub fn parse_exa_results(body: &serde_json::Value, limit: usize) -> Vec<SearchResult> {
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

/// Whether research can actually run for `provider`/`model` right now.
///
/// This is what `supportsWebSearch` reports, and it is deliberately about
/// CONFIGURATION, not about what a provider advertises. The static capability
/// flag says local Ollama "supports web search", which is true of the family and
/// false of a keyless install — so the toggle read ON while every brief came
/// back empty. A user cannot act on that; they can act on "no search backend is
/// configured".
///
/// True when the provider's model searches for itself
/// (`capabilities().supports_web_search` AND no separate searcher needed), when
/// its own searcher is configured, or when a fallback backend is.
pub fn research_available<P: super::AiProvider + ?Sized>(
    app: &AppHandle,
    provider: &P,
    model: &str,
) -> bool {
    // A provider whose MODEL searches (OpenAI/Anthropic/Gemini, CLI agents)
    // reuses its generation key, so if generation is configured, so is search.
    provider.has_native_search(model)
        || provider.native_searcher(app, model).is_some()
        || ExaSearcher::from_credentials(app).is_some()
}

// ── Backend resolution ────────────────────────────────────────────────────────

/// The search backend that will actually serve the NEXT research pass for
/// `provider`/`model` — the same routing [`crate::pipeline::Completer::research`]
/// performs: the native bypass (`AiProvider::research` is called directly and
/// never reaches this module at all) when [`super::AiProvider::has_native_search`]
/// is true, otherwise the same [`resolve_search_backend`] resolution
/// [`searcher_for`] uses below.
///
/// Exposed as its own function (rather than left implicit inside
/// `searcher_for`) because a caller that only needs the IDENTITY of the
/// backend — not a live [`WebSearcher`] — would otherwise have to construct
/// one just to throw it away. [`crate::pipeline::Completer::search_backend`]
/// is the one production caller: the retrieval half of a research call
/// determines the brief just as much as the synthesizing model does, so it
/// is a `company_brief` cache-key term
/// (`cover_letter::research::cache_key`) — `searcher_for` picks Native vs.
/// Exa from CREDENTIAL PRESENCE at call time, not from `(provider, model)`,
/// so the SAME provider + model can still retrieve from a different backend
/// (e.g. an Exa key added/removed) between two calls.
pub fn search_backend_for<P: super::AiProvider + ?Sized>(
    app: &AppHandle,
    provider: &P,
    model: &str,
) -> SearchBackend {
    if provider.has_native_search(model) {
        return SearchBackend::Native;
    }
    resolve_search_backend(
        provider.native_searcher(app, model).is_some(),
        ExaSearcher::from_credentials(app).is_some(),
    )
}

/// The search backend for one research pass, or `None` when nothing is
/// configured (research then degrades to an empty brief).
///
/// Ollama-family providers get the Ollama Web Search API when their account key
/// is present — that is their "native" search. Everyone else reaching this
/// function has no model-side search at all (providers that DO have one override
/// `AiProvider::research` and never get here), so they go straight to the
/// configured fallback.
pub fn searcher_for<P: super::AiProvider + ?Sized>(
    app: &AppHandle,
    provider: &P,
    model: &str,
) -> Option<Box<dyn WebSearcher>> {
    let native: Option<Box<dyn WebSearcher>> = provider
        .native_searcher(app, model)
        .map(|s| s as Box<dyn WebSearcher>);
    match resolve_search_backend(
        native.is_some(),
        ExaSearcher::from_credentials(app).is_some(),
    ) {
        SearchBackend::Native => native,
        SearchBackend::Exa => {
            ExaSearcher::from_credentials(app).map(|s| Box::new(s) as Box<dyn WebSearcher>)
        }
        SearchBackend::None => None,
    }
}

/// Company-research brief: search, then synthesize with the caller's OWN model.
///
/// The generic half of what used to be `ollama_research` — only the search step
/// was ever Ollama-specific. Returns `""` (never an error) when no backend is
/// configured or the search finds nothing, so generation always proceeds.
pub async fn searched_research<P: super::AiProvider + ?Sized>(
    app: &AppHandle,
    provider: &P,
    model: &str,
    company: &str,
    role: &str,
) -> crate::error::AppResult<String> {
    let Some(searcher) = searcher_for(app, provider, model) else {
        return Ok(String::new());
    };
    let results = searcher
        .search(&super::research::search_query(company), 5)
        .await;
    if results.is_empty() {
        return Ok(String::new());
    }
    let user = super::research::synth_user(company, role, &results);
    provider
        .complete(app, model, super::research::SYNTH_SYSTEM, &user, Some(0.2))
        .await
}

/// Salary-range sibling of [`searched_research`] — same shape, salary prompts
/// (compact JSON contract, see `research::salary_system`). `country`/`currency`
/// ground the report in the job's actual currency.
#[allow(clippy::too_many_arguments)]
pub async fn searched_research_salary<P: super::AiProvider + ?Sized>(
    app: &AppHandle,
    provider: &P,
    model: &str,
    role: &str,
    company: &str,
    location: &str,
    country: &str,
    currency: &str,
) -> crate::error::AppResult<String> {
    let Some(searcher) = searcher_for(app, provider, model) else {
        return Ok(String::new());
    };
    let query = super::research::salary_search_query(role, company, location, country, currency);
    let results = searcher.search(&query, 5).await;
    if results.is_empty() {
        return Ok(String::new());
    }
    let user =
        super::research::salary_synth_user(role, company, location, country, currency, &results);
    provider
        .complete(
            app,
            model,
            &super::research::salary_system(currency),
            &user,
            Some(0.2),
        )
        .await
}

/// Application-answer sibling of [`searched_research`], scoped to a single
/// question rather than a general company brief.
pub async fn searched_research_answer<P: super::AiProvider + ?Sized>(
    app: &AppHandle,
    provider: &P,
    model: &str,
    question: &str,
    role: &str,
    company: &str,
) -> crate::error::AppResult<String> {
    let Some(searcher) = searcher_for(app, provider, model) else {
        return Ok(String::new());
    };
    let query = super::research::answer_search_query(question, role, company);
    let results = searcher.search(&query, 5).await;
    if results.is_empty() {
        return Ok(String::new());
    }
    let user = super::research::answer_synth_user(question, role, company, &results);
    provider
        .complete(
            app,
            model,
            super::research::ANSWER_SYNTH_SYSTEM,
            &user,
            Some(0.2),
        )
        .await
}

#[cfg(test)]
#[path = "test.rs"]
mod test;

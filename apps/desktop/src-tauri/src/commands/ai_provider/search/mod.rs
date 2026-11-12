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
//! search backend the user configures — [`SearxngSearcher`] (keyless, self-hosted;
//! SearXNG-compatible servers such as SearXNG and RustXNG) or [`ExaSearcher`]
//! (hosted, keyed).
//!
//! A **search backend is not an AI provider**: it returns web results and cannot
//! generate. It deliberately does not appear in `ProviderId`, in
//! `Completer::from_active`'s routing, or in the renderer's provider registry.
//!
//! ## Who runs
//!
//! [`search_order`] decides from CONFIGURATION, before any call: native when the
//! provider has a usable search, then SearXNG-compatible (free, local), then Exa
//! (paid). [`resolve_search_backend`] is its head. The pass walks that order only
//! when a backend FAILS with billing or auth ([`falls_through`]): an out-of-credit
//! or rejected key is a fact about that backend, so the next one is tried, each at
//! most once. A search that runs and returns NOTHING, is rate limited, or is
//! unavailable is NOT retried elsewhere: cost stays predictable and one query is
//! not fanned out to several vendors (ADR 0023).
//!
//! Synthesis always stays on the user's own model. Exa's own answer endpoint
//! would be fewer calls, but it would bypass both `research::SYNTH_SYSTEM`'s
//! prompt-injection guard (search results are attacker-reachable text) and the
//! `is_no_info` filter, and it would move generation spend to a second vendor.

use async_trait::async_trait;
use tauri::AppHandle;

use super::research::SearchResult;
use crate::error::{AppError, AppResult, FailureKind};

mod exa;
mod searxng;
pub use exa::{ExaSearcher, EXA_KEY};
pub use searxng::{normalize_searxng_url, SearxngSearcher};

/// `SearchBackend` lives in the L1 `crate::ai_provider` module, not here —
/// `cover_letter::research::cache_key` (L2) needs to name it for its
/// `company_brief` cache-key term, and reaching up into this L3 module for a
/// plain value type would be an upward (R7) layer violation. Re-exported so
/// every existing consumer of `commands::ai_provider::search::SearchBackend`
/// keeps compiling unchanged; see that module's doc comment for the full
/// reasoning.
pub use crate::ai_provider::{SearchBackend, ServedBackend};

/// Longest title passed on to synthesis, in chars. A title is remote text going into a prompt.
pub(super) const TITLE_CAP: usize = 200;

/// `s` cut to `max` chars (never mid-character).
pub(super) fn cap_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// The backends one research pass may use, in the order it tries them: native, then
/// SearXNG-compatible (free and local), then Exa (paid). Pure, so the policy is testable
/// without an `AppHandle` -- this crate has no `tauri::test` mock-app harness, and the same
/// extraction is why `OpenAiClient::supports_web_search` and `salary_research::role_is_missing`
/// exist as standalone predicates.
///
/// The head runs FIRST; the rest are fallbacks, reached only through [`falls_through`]. A user
/// whose provider already searches keeps using it and never silently starts paying a second
/// vendor just because a key is stored.
pub fn search_order(
    native_ready: bool,
    searxng_ready: bool,
    exa_key_present: bool,
) -> Vec<SearchBackend> {
    [
        (SearchBackend::Native, native_ready),
        (SearchBackend::Searxng, searxng_ready),
        (SearchBackend::Exa, exa_key_present),
    ]
    .into_iter()
    .filter_map(|(backend, ready)| ready.then_some(backend))
    .collect()
}

/// The backend that runs first -- the head of [`search_order`], or `None`.
pub fn resolve_search_backend(
    native_ready: bool,
    searxng_ready: bool,
    exa_key_present: bool,
) -> SearchBackend {
    search_order(native_ready, searxng_ready, exa_key_present)
        .first()
        .copied()
        .unwrap_or(SearchBackend::None)
}

/// Whether a failed search moves on to the next backend: only billing (out of credit) and auth
/// (bad key / refused). Rate limiting, an outage, unusable output and an EMPTY result do not --
/// retrying those elsewhere would multiply cost and leak the query to more vendors.
pub fn falls_through(e: &AppError) -> bool {
    matches!(
        FailureKind::of(e),
        Some(FailureKind::Billing | FailureKind::Auth)
    )
}

/// The resolved backends of one pass, in [`search_order`] order, behaving as ONE
/// [`WebSearcher`] that falls through per [`falls_through`].
pub struct SearchChain {
    candidates: Vec<(SearchBackend, Box<dyn WebSearcher>)>,
    served: ServedBackend,
}

impl SearchChain {
    /// Pair the three ALREADY-resolved candidates with their tags in [`search_order`] order --
    /// the one place a tag meets its searcher.
    fn new(
        native: Option<Box<dyn WebSearcher>>,
        searxng: Option<Box<dyn WebSearcher>>,
        exa: Option<Box<dyn WebSearcher>>,
    ) -> Self {
        let order = search_order(native.is_some(), searxng.is_some(), exa.is_some());
        let mut pool = [
            (SearchBackend::Native, native),
            (SearchBackend::Searxng, searxng),
            (SearchBackend::Exa, exa),
        ];
        let candidates: Vec<_> = order
            .into_iter()
            .filter_map(|b| Some((b, pool.iter_mut().find(|(k, _)| *k == b)?.1.take()?)))
            .collect();
        Self {
            candidates,
            served: ServedBackend::default(),
        }
    }

    fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    fn first_backend(&self) -> SearchBackend {
        self.candidates
            .first()
            .map_or(SearchBackend::None, |(b, _)| *b)
    }
}

#[async_trait]
impl WebSearcher for SearchChain {
    async fn try_search(&self, query: &str, limit: usize) -> AppResult<Vec<SearchResult>> {
        let mut rest = self.candidates.iter().peekable();
        while let Some((backend, searcher)) = rest.next() {
            let outcome = searcher.try_search(query, limit).await;
            // Recorded only once the search RETURNED, so a later failure (synthesis on the
            // user's own model) or a deadline is never pinned on a backend still in flight.
            self.served.set(*backend, outcome.is_ok());
            match outcome {
                Err(e) if rest.peek().is_some() && falls_through(&e) => {
                    tracing::warn!(
                        backend = backend.as_str(),
                        kind = FailureKind::of(&e).map_or("unclassified", FailureKind::as_str),
                        "search: backend failed, trying the next one"
                    );
                }
                other => return other,
            }
        }
        Ok(Vec::new())
    }
}

/// A source of web-search snippets for research.
///
/// [`try_search`](WebSearcher::try_search) reports a failure; [`search`](WebSearcher::search)
/// degrades it to an empty `Vec`. Either way research degrades to "no brief" at the caller and
/// generation proceeds; it must never fail because a search did.
#[async_trait]
pub trait WebSearcher: Send + Sync {
    /// The search, reporting WHY it found nothing (#1500: an out-of-credit account must not
    /// read as "the web had nothing"). Required, so no searcher can inherit a swallow.
    async fn try_search(&self, query: &str, limit: usize) -> AppResult<Vec<SearchResult>>;

    /// [`try_search`](Self::try_search), degrading every failure to no results for the callers
    /// that have no use for the reason.
    async fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        self.try_search(query, limit).await.unwrap_or_default()
    }
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
        || SearxngSearcher::from_settings(app).is_some()
        || ExaSearcher::from_credentials(app).is_some()
}

// ── Backend resolution ────────────────────────────────────────────────────────

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
    let chain = chain_from(app, provider, model);
    (!chain.is_empty()).then(|| Box::new(chain) as Box<dyn WebSearcher>)
}

/// Read the three backends' configuration once and pair them.
fn chain_from<P: super::AiProvider + ?Sized>(
    app: &AppHandle,
    provider: &P,
    model: &str,
) -> SearchChain {
    SearchChain::new(
        provider
            .native_searcher(app, model)
            .map(|s| s as Box<dyn WebSearcher>),
        SearxngSearcher::from_settings(app).map(|s| Box::new(s) as Box<dyn WebSearcher>),
        ExaSearcher::from_credentials(app).map(|s| Box::new(s) as Box<dyn WebSearcher>),
    )
}

/// One company-research pass's routing, resolved from credentials/config
/// EXACTLY ONCE — see [`resolve`](Self::resolve) — then threaded through
/// both the `company_brief` cache-key term ([`backend`](Self::backend)) and
/// the fetch ([`fetch_company_brief`]).
///
/// **Fixes a real bug** (PR #989 CodeRabbit MAJOR): the previous shape
/// resolved the backend TWICE for one research pass — once (via a
/// since-removed `search_backend_for`) to build the cache key, and again
/// inside the fetch (via [`searcher_for`], which reads the SAME
/// credentials) — with an `.await`ed provider call in between. If
/// credentials changed in that window (an Exa key added or removed while a
/// request was in flight), the two resolutions could disagree: the brief
/// would get cached under a key naming the OLD backend while a DIFFERENT
/// backend actually produced it, and that mismatched row would then serve
/// for the full 7-day TTL.
///
/// Resolving once and threading the result through — rather than exposing a
/// second "predict the backend" function alongside the fetch — makes the
/// mismatch structurally unrepresentable: [`resolve`](Self::resolve) is the
/// ONLY function that reads these credentials for a given pass, and the
/// ONLY way to construct this type. It also closes a related MEDIUM: the
/// `has_native_search` bypass used to be checked independently in two
/// places (here and inside the old `Completer::research`), which could
/// drift the same way; it is now checked in exactly this one place for a
/// company-research pass.
pub enum CompanySearchRoute {
    /// The provider's own model searches (`AiProvider::research`) — no
    /// separate [`WebSearcher`] call.
    Native,
    /// Search-then-synthesize over the already-resolved backends (an empty
    /// chain when nothing usable is configured).
    Backend(SearchChain),
}

impl CompanySearchRoute {
    /// Resolve — the exact routing the old `Completer::research` used to
    /// re-derive independently on every call. The caller MUST reuse the
    /// result for both the cache-key term and the fetch, never call this a
    /// second time for the same pass.
    pub fn resolve<P: super::AiProvider + ?Sized>(
        app: &AppHandle,
        provider: &P,
        model: &str,
    ) -> Self {
        if provider.has_native_search(model) {
            // Short-circuits BEFORE any credential read, same as the code
            // this replaces — a native-search provider never pays for a
            // wasted account/Exa lookup.
            return Self::Native;
        }
        Self::Backend(chain_from(app, provider, model))
    }

    /// The `company_brief` cache-key term for this route: the backend that runs FIRST. A
    /// pass that falls through is still keyed by it -- the credentials are what the key
    /// guards, and a brief is the same content whichever backend fetched it.
    pub fn backend(&self) -> SearchBackend {
        match self {
            Self::Native => SearchBackend::Native,
            Self::Backend(chain) => chain.first_backend(),
        }
    }

    /// A handle the caller keeps across the move into [`fetch_company_brief`], to name the
    /// backend that served the pass afterwards.
    pub fn served(&self) -> ServedBackend {
        match self {
            Self::Native => ServedBackend::of(SearchBackend::Native),
            Self::Backend(chain) => chain.served.clone(),
        }
    }
}

/// Company-research brief along an ALREADY-resolved [`CompanySearchRoute`] —
/// see its doc comment for why this must never re-resolve credentials.
/// Returns `""` (never an error) when no backend is configured or the
/// search finds nothing, so generation always proceeds.
pub async fn fetch_company_brief<P: super::AiProvider + ?Sized>(
    app: &AppHandle,
    provider: &P,
    model: &str,
    route: CompanySearchRoute,
    company: &str,
    role: &str,
) -> crate::error::AppResult<String> {
    let searcher = match route {
        CompanySearchRoute::Native => {
            return provider.research(app, model, company, role).await;
        }
        CompanySearchRoute::Backend(chain) if chain.is_empty() => return Ok(String::new()),
        CompanySearchRoute::Backend(chain) => chain,
    };
    search_then_synthesize(
        &searcher,
        &super::research::search_query(company),
        |results| super::research::synth_user(company, role, results),
        |user| async move {
            provider
                .complete(app, model, super::research::SYNTH_SYSTEM, &user, Some(0.2))
                .await
        },
    )
    .await
}

/// Search, then synthesize — the shared shape of the three research facets, free of `AppHandle`
/// so the failure behaviour is testable. A search that FAILED is an `Err` (the caller decides
/// how to degrade, and says why); a search that found nothing is `Ok("")` with no synthesis call.
async fn search_then_synthesize<Fut>(
    searcher: &dyn WebSearcher,
    query: &str,
    user_prompt: impl FnOnce(&[SearchResult]) -> String,
    synthesize: impl FnOnce(String) -> Fut,
) -> AppResult<String>
where
    Fut: std::future::Future<Output = AppResult<String>>,
{
    let results = searcher.try_search(query, 5).await?;
    if results.is_empty() {
        return Ok(String::new());
    }
    synthesize(user_prompt(&results)).await
}

/// Salary-range sibling of [`fetch_company_brief`] — same shape (search then
/// synthesize via [`searcher_for`]), salary prompts (compact JSON contract,
/// see `research::salary_system`). `country`/`currency` ground the report in
/// the job's actual currency.
///
/// Unlike the company-brief path, this one still resolves its searcher
/// internally on every call — `salary_research::SalaryResearch::enrich`'s
/// cache key has no backend term today (a separate, already-tracked
/// follow-up), so there is no "resolve once, reuse for the key" requirement
/// here yet.
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
    search_then_synthesize(
        searcher.as_ref(),
        &query,
        |results| {
            super::research::salary_synth_user(role, company, location, country, currency, results)
        },
        |user| async move {
            let system = super::research::salary_system(currency);
            provider
                .complete(app, model, &system, &user, Some(0.2))
                .await
        },
    )
    .await
}

/// Application-answer sibling of [`fetch_company_brief`], scoped to a single
/// question rather than a general company brief. Same caveat as
/// [`searched_research_salary`]: resolves its searcher internally, since its
/// caller's cache (if any) has no backend term to keep coherent.
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
    search_then_synthesize(
        searcher.as_ref(),
        &query,
        |results| super::research::answer_synth_user(question, role, company, results),
        |user| async move {
            provider
                .complete(
                    app,
                    model,
                    super::research::ANSWER_SYNTH_SYSTEM,
                    &user,
                    Some(0.2),
                )
                .await
        },
    )
    .await
}

#[cfg(test)]
mod tests;

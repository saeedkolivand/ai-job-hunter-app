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
use tauri::AppHandle;

use super::research::SearchResult;

mod exa;
pub use exa::{ExaSearcher, EXA_KEY};

/// `SearchBackend` lives in the L1 `crate::ai_provider` module, not here —
/// `cover_letter::research::cache_key` (L2) needs to name it for its
/// `company_brief` cache-key term, and reaching up into this L3 module for a
/// plain value type would be an upward (R7) layer violation. Re-exported so
/// every existing consumer of `commands::ai_provider::search::SearchBackend`
/// keeps compiling unchanged; see that module's doc comment for the full
/// reasoning.
pub use crate::ai_provider::SearchBackend;

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
    /// Search-then-synthesize, with the searcher already resolved (`None`
    /// when nothing usable is configured).
    Backend(SearchBackend, Option<Box<dyn WebSearcher>>),
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
        Self::resolve_via_backend(
            provider
                .native_searcher(app, model)
                .map(|s| s as Box<dyn WebSearcher>),
            ExaSearcher::from_credentials(app).map(|s| Box::new(s) as Box<dyn WebSearcher>),
        )
    }

    /// The non-native half of [`resolve`](Self::resolve): given the two
    /// ALREADY-resolved candidates (not re-read here), picks the backend and
    /// pairs it with the matching searcher — the ONE place that pairing
    /// happens. Free of `AppHandle` (the candidates are already resolved),
    /// so every branch is a direct unit test rather than something only
    /// provable by inspection.
    fn resolve_via_backend(
        native: Option<Box<dyn WebSearcher>>,
        exa: Option<Box<dyn WebSearcher>>,
    ) -> Self {
        let backend = resolve_search_backend(native.is_some(), exa.is_some());
        let searcher = match backend {
            SearchBackend::Native => native,
            SearchBackend::Exa => exa,
            SearchBackend::None => None,
        };
        Self::Backend(backend, searcher)
    }

    /// The `company_brief` cache-key term for this route.
    pub fn backend(&self) -> SearchBackend {
        match self {
            Self::Native => SearchBackend::Native,
            Self::Backend(backend, _) => *backend,
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
        CompanySearchRoute::Backend(_, None) => return Ok(String::new()),
        CompanySearchRoute::Backend(_, Some(searcher)) => searcher,
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
mod tests;

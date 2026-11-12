//! Provider-adjacent value types shared across layers without pulling in the
//! L3 shell.
//!
//! This is a deliberately minimal seed of the roadmap's "relocate
//! `ai_provider` out of `commands/`" item (`docs/architecture-rules.md`) —
//! ONE pure value type, not the module's routing/credential logic, which
//! still needs `AppHandle` and stays L3 (`commands::ai_provider`). It exists
//! because [`SearchBackend`] is named in a public signature by two L2
//! siblings — `pipeline::Completer::search_backend()` and
//! `cover_letter::research::cache_key` — and reaching up into
//! `commands::ai_provider::search::SearchBackend` from either is an upward
//! (R7) layer violation. `commands::ai_provider::search` `pub use`s this type
//! back, so its existing L3 consumers (`resolve_search_backend`,
//! `searcher_for`, `search_backend_for`) are unaffected — only the TYPE
//! moved, not the credential-reading resolution logic.

/// Which backend serves one research pass — see
/// `commands::ai_provider::search`'s module doc for the full "search vs.
/// synthesize" design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchBackend {
    /// The provider's own model-side search (or the Ollama Web Search API).
    Native,
    /// A SearXNG instance the user runs themselves (keyless, free).
    Searxng,
    /// The user-configured paid fallback.
    Exa,
    /// Nothing configured — research degrades to an empty brief.
    None,
}

use std::sync::Arc;

/// Which backend a research pass reached, shared between the chain (which writes it as it
/// goes) and the caller (which reads it after the pass). Set to a backend only once its
/// search has returned, with whether that search succeeded.
#[derive(Clone, Default)]
pub struct ServedBackend(Arc<parking_lot::Mutex<Option<(SearchBackend, bool)>>>);

impl ServedBackend {
    pub(crate) fn of(backend: SearchBackend) -> Self {
        Self(Arc::new(parking_lot::Mutex::new(Some((backend, true)))))
    }

    pub(crate) fn set(&self, backend: SearchBackend, succeeded: bool) {
        *self.0.lock() = Some((backend, succeeded));
    }

    /// The backend whose search returned last, succeeded or not.
    pub fn get(&self) -> Option<SearchBackend> {
        self.0.lock().map(|(backend, _)| backend)
    }

    /// The backend whose search returned last, only when that search succeeded.
    pub fn get_succeeded(&self) -> Option<SearchBackend> {
        self.0.lock().and_then(|(b, ok)| ok.then_some(b))
    }
}

impl SearchBackend {
    /// Stable string term for a cache key (e.g.
    /// `cover_letter::research`'s `company_brief` key) — not `Debug`, so a
    /// variant rename can't silently change every existing key.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Searxng => "searxng",
            Self::Exa => "exa",
            Self::None => "none",
        }
    }

    /// The wire token of a backend that can SERVE a pass; `None` for anything else (including
    /// `"none"`), so a value read back from stored metrics never passes through unrecognised.
    pub fn parse_served(token: &str) -> Option<Self> {
        [Self::Native, Self::Searxng, Self::Exa]
            .into_iter()
            .find(|b| b.as_str() == token)
    }
}

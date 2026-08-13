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

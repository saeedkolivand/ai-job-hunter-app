//! Unit tests for the search-backend policy and the Exa response mapping —
//! the two parts reachable without a live `AppHandle` or a network.

use serde_json::json;

use super::*;

// ── resolve_search_backend ───────────────────────────────────────────────────

/// The load-bearing decision: a provider that can already search keeps using its
/// own search even when an Exa key is stored.
///
/// Without this test, "fallback only" could quietly become "Exa preferred" — a
/// change that costs money at a second vendor and that no other test would
/// notice, because both variants produce a working brief.
#[test]
fn a_working_native_search_is_never_replaced_by_exa() {
    assert_eq!(
        resolve_search_backend(true, true),
        SearchBackend::Native,
        "an Exa key must not override a provider that can already search"
    );
    assert_eq!(resolve_search_backend(true, false), SearchBackend::Native);
}

/// The population this feature exists for: local Ollama with no ollama.com
/// account key, and every `openai-compatible` gateway. They get nothing today.
#[test]
fn exa_serves_a_provider_with_no_usable_native_search() {
    assert_eq!(resolve_search_backend(false, true), SearchBackend::Exa);
}

#[test]
fn nothing_configured_means_no_search_at_all() {
    // Not an error — research degrades to an empty brief and generation
    // proceeds, which is the existing contract.
    assert_eq!(resolve_search_backend(false, false), SearchBackend::None);
}

// ── parse_exa_results ────────────────────────────────────────────────────────

#[test]
fn maps_highlights_to_the_snippet_shape_synthesis_expects() {
    let body = json!({
        "results": [{
            "title": "Codefield — About",
            "url": "https://codefield.nl/about",
            "highlights": ["Founded in 2019 in Utrecht.", "Around 20 employees."],
            "text": "…the entire page, which must NOT be preferred…",
        }]
    });
    let out = parse_exa_results(&body, 5);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].title, "Codefield — About");
    assert_eq!(out[0].url, "https://codefield.nl/about");
    assert_eq!(
        out[0].snippet, "Founded in 2019 in Utrecht. Around 20 employees.",
        "highlights are the relevance-selected spans and must win over full text"
    );
}

#[test]
fn falls_back_to_text_when_a_result_has_no_highlights() {
    let body = json!({
        "results": [{ "title": "T", "url": "https://x.test", "text": "Some page body." }]
    });
    let out = parse_exa_results(&body, 5);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].snippet, "Some page body.");
}

#[test]
fn caps_the_text_fallback_on_a_char_boundary() {
    // `text` is the WHOLE page; uncapped it would dominate the synthesis prompt.
    // Multi-byte content pins that the cap slices chars, not bytes — a byte
    // slice mid-character panics.
    let long = "é".repeat(5_000);
    let body = json!({ "results": [{ "title": "T", "url": "u", "text": long }] });
    let out = parse_exa_results(&body, 5);
    assert_eq!(out[0].snippet.chars().count(), 1_000);
}

#[test]
fn drops_a_result_with_neither_highlights_nor_text() {
    // A bare title costs synthesis tokens and contributes nothing.
    let body = json!({
        "results": [
            { "title": "Empty", "url": "u" },
            { "title": "Real", "url": "u2", "highlights": ["content"] },
        ]
    });
    let out = parse_exa_results(&body, 5);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].title, "Real");
}

#[test]
fn honours_the_limit() {
    let results: Vec<_> = (0..10)
        .map(|i| json!({ "title": format!("t{i}"), "url": "u", "highlights": ["x"] }))
        .collect();
    let out = parse_exa_results(&json!({ "results": results }), 3);
    assert_eq!(out.len(), 3);
}

#[test]
fn a_malformed_or_empty_body_yields_no_results_rather_than_an_error() {
    // Every failure mode in this module degrades to "no search results", which
    // the callers already handle as "no brief".
    assert!(parse_exa_results(&json!({}), 5).is_empty());
    assert!(parse_exa_results(&json!({ "results": "not-an-array" }), 5).is_empty());
    assert!(parse_exa_results(&json!({ "error": "bad key" }), 5).is_empty());
}

// ── CompanySearchRoute::resolve_via_backend: the PR #989 TOCTOU-fix pairing ──
//
// CodeRabbit MAJOR: the OLD shape resolved the company-research backend
// TWICE for one pass — once to build the `company_brief` cache key, again
// inside the fetch — with an `.await`ed provider call in between. If
// credentials changed in that window (an Exa key added/removed mid-flight),
// the two could disagree and mislabel a cached brief for 7 days.
// `CompanySearchRoute::resolve` now resolves once; `resolve_via_backend` is
// the pure half of that (its two candidates are ALREADY resolved, so it
// takes no `AppHandle`), which is what makes every pairing a direct unit
// test here rather than something only provable by inspection — this crate
// has no `tauri::test` mock-app harness to exercise `resolve` itself (same
// limitation `cover_letter::research`'s and `salary_research`'s test
// modules document).

use async_trait::async_trait;

struct FakeSearcher;

#[async_trait]
impl WebSearcher for FakeSearcher {
    async fn search(&self, _query: &str, _limit: usize) -> Vec<SearchResult> {
        Vec::new()
    }
}

fn fake_searcher() -> Option<Box<dyn WebSearcher>> {
    Some(Box::new(FakeSearcher))
}

#[test]
fn resolve_via_backend_never_prefers_exa_over_a_ready_native_searcher() {
    // The SAME fallback-ONLY invariant `resolve_search_backend` guards, now
    // pinned at the level that pairs the tag with the actual searcher a
    // fetch would use — a Native tag must never be paired with `None` (or an
    // Exa searcher) when a native one was available.
    let route = CompanySearchRoute::resolve_via_backend(fake_searcher(), fake_searcher());
    assert_eq!(route.backend(), SearchBackend::Native);
    assert!(matches!(
        route,
        CompanySearchRoute::Backend(SearchBackend::Native, Some(_))
    ));
}

#[test]
fn resolve_via_backend_falls_back_to_exa_when_native_is_absent() {
    let route = CompanySearchRoute::resolve_via_backend(None, fake_searcher());
    assert_eq!(route.backend(), SearchBackend::Exa);
    assert!(matches!(
        route,
        CompanySearchRoute::Backend(SearchBackend::Exa, Some(_))
    ));
}

#[test]
fn resolve_via_backend_is_none_when_nothing_is_configured() {
    let route = CompanySearchRoute::resolve_via_backend(None, None);
    assert_eq!(route.backend(), SearchBackend::None);
    assert!(matches!(
        route,
        CompanySearchRoute::Backend(SearchBackend::None, None)
    ));
}

#[test]
fn the_native_route_never_carries_a_searcher() {
    // The bypass variant: no separate `WebSearcher` at all — `backend()`
    // still reports Native, matching the single arm `fetch_company_brief`
    // takes for it (a direct `AiProvider::research` call, never touching
    // `resolve_via_backend` at all).
    assert_eq!(CompanySearchRoute::Native.backend(), SearchBackend::Native);
}

// ── has_native_search: the routing predicate ─────────────────────────────────
//
// `Completer::research*` asks this ONE question to decide native-call vs
// search-then-synthesize. It gets its own tests because the previous shape —
// each provider deciding for itself — failed silently: `OpenAiClient` overrides
// all three research methods for every id it serves, so an `openai-compatible`
// gateway returned "" instead of reaching the fallback, while the UI reported
// that research was available.

use super::super::{resolve, ProviderId};

#[test]
fn providers_whose_model_searches_take_the_native_path() {
    for id in [
        ProviderId::OpenAi,
        ProviderId::Anthropic,
        ProviderId::Gemini,
    ] {
        let client = resolve(id, None);
        assert!(
            client.has_native_search("some-model"),
            "{id:?} searches with its own model and must not be routed to a backend"
        );
    }
}

#[test]
fn an_openai_compatible_gateway_is_routed_to_a_search_backend() {
    // The regression this predicate exists for. `supports_web_search` is false
    // for this id, so it must NOT take the native path — it has none, and
    // taking it is what produced an empty brief for every LM Studio / vLLM /
    // OpenRouter / Groq / Together / DeepSeek user.
    let client = resolve(ProviderId::OpenAiCompatible, None);
    assert!(
        !client.has_native_search("any-model"),
        "a gateway with no web search must be routed to the shared search path"
    );
}

#[test]
fn the_ollama_family_is_routed_to_a_search_backend() {
    // Both advertise `supports_web_search` for the FAMILY, but neither model
    // searches — they call a separate hosted API — so both must take the
    // search-then-synthesize path and resolve a searcher there.
    for id in [ProviderId::Ollama, ProviderId::OllamaCloud] {
        let client = resolve(id, None);
        assert!(
            !client.has_native_search("gemma4:31b"),
            "{id:?} needs an explicit search backend, not a native call"
        );
    }
}

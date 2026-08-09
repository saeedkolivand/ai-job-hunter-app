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

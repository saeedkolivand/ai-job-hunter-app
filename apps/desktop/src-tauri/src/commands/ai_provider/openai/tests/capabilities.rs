//! Model-family classification (o-series / gpt-5.x) and the capability/
//! effort/response-format gates built on it.

use super::super::super::{AiProvider, ProviderId};
use super::super::capabilities::{is_gpt5_or_later_reasoning_family, is_reasoning_model};
use super::super::OpenAiClient;

#[test]
fn detects_o_series_including_future_models() {
    for m in ["o1", "o1-mini", "o3", "o3-mini", "o4-mini", "o5", "o9-pro"] {
        assert!(is_reasoning_model(m), "{m} should be a reasoning model");
    }
    for m in [
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-3.5-turbo",
        "omni",
        "chatgpt-4o",
    ] {
        assert!(
            !is_reasoning_model(m),
            "{m} should not be a reasoning model"
        );
    }
}

#[test]
fn gpt5_family_gate_excludes_earlier_majors_and_the_chat_latest_siblings() {
    for m in [
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-4-turbo",
        "gpt-3.5-turbo",
        "gpt-5-chat-latest",
        "gpt-5.1-chat-latest",
        "gpt-5.2-chat-latest",
        "gpt-5.3-chat-latest",
        "not-a-gpt-model",
    ] {
        assert!(
            !is_gpt5_or_later_reasoning_family(m),
            "{m} should not be gpt-5+ reasoning"
        );
    }
    for m in [
        "gpt-5",
        "gpt-5-mini",
        "gpt-5-nano",
        "gpt-5.1",
        "gpt-5.1-codex",
        "gpt-5.4",
        "gpt-5.6-sol",
        "gpt-5.6-terra",
        "gpt-5.6-luna",
        "gpt-6", // not-yet-released — must degrade forward, not backward
    ] {
        assert!(
            is_gpt5_or_later_reasoning_family(m),
            "{m} should be gpt-5+ reasoning"
        );
    }
}

#[test]
fn supports_web_search_gate_only_allows_native_openai() {
    // Regression guard against silently dropping the provider gate in a
    // future refactor: a non-OpenAI id must never reach `/responses` (a
    // generic OpenAI-compatible gateway can't be assumed to support the
    // native `web_search` tool). `web_search_complete` itself can't be
    // driven end to end here — it needs a live `AppHandle`, and this crate
    // has no `tauri::test` mock-app harness (see its doc comment, and the
    // same note on `salary_research::SalaryResearch::enrich`) — so this
    // exercises the pure gate predicate it's built on before any HTTP call.
    assert!(OpenAiClient::new(ProviderId::OpenAi, None).supports_web_search());
    for other in [
        ProviderId::OpenAiCompatible,
        ProviderId::OllamaCloud,
        ProviderId::Ollama,
        ProviderId::Anthropic,
        ProviderId::Gemini,
    ] {
        assert!(
            !OpenAiClient::new(other, None).supports_web_search(),
            "{other:?} must not pass the web_search gate"
        );
    }
}

/// `ModelCapabilities::supports_web_search` (what `ai_research_answer` gates
/// the daily-budget charge on) must mirror the private gate predicate above —
/// this is the field a caller actually reads.
#[test]
fn capabilities_supports_web_search_mirrors_the_gate() {
    assert!(
        OpenAiClient::new(ProviderId::OpenAi, None)
            .capabilities("gpt-4o")
            .supports_web_search
    );
    assert!(
        !OpenAiClient::new(ProviderId::OpenAiCompatible, None)
            .capabilities("some-model")
            .supports_web_search
    );
}

#[test]
fn reasoning_effort_gate_differs_by_provider_id_and_model_catalog() {
    // Native OpenAI: the legacy o-series AND the current gpt-5.x line.
    assert!(OpenAiClient::new(ProviderId::OpenAi, None).supports_reasoning_effort("o3-mini"));
    assert!(!OpenAiClient::new(ProviderId::OpenAi, None).supports_reasoning_effort("gpt-4o"));
    for m in [
        "gpt-5",
        "gpt-5-mini",
        "gpt-5.4",
        "gpt-5.5",
        "gpt-5.6",
        "gpt-5.6-sol",
    ] {
        assert!(
            OpenAiClient::new(ProviderId::OpenAi, None).supports_reasoning_effort(m),
            "{m} should accept reasoning_effort"
        );
    }
    // The `-chat-latest` variant of each gpt-5.x generation is the
    // non-reasoning conversational sibling — must stay excluded.
    for m in ["gpt-5-chat-latest", "gpt-5.1-chat-latest"] {
        assert!(
            !OpenAiClient::new(ProviderId::OpenAi, None).supports_reasoning_effort(m),
            "{m} must not accept reasoning_effort"
        );
    }
    // Ollama Cloud: the Ollama thinking-family catalog, NOT the o-series/gpt-5
    // rule — gpt-oss doesn't match either, and qwen3-coder must stay excluded.
    assert!(
        OpenAiClient::new(ProviderId::OllamaCloud, None).supports_reasoning_effort("gpt-oss:120b")
    );
    assert!(!OpenAiClient::new(ProviderId::OllamaCloud, None)
        .supports_reasoning_effort("qwen3-coder:480b"));
    // A generic OpenAI-compatible gateway is an unknown catalog — never guessed.
    assert!(
        !OpenAiClient::new(ProviderId::OpenAiCompatible, None).supports_reasoning_effort("o3-mini")
    );
}

#[test]
fn effort_levels_mirror_the_reasoning_gate() {
    assert_eq!(
        OpenAiClient::new(ProviderId::OpenAi, None).effort_levels("gpt-5.6"),
        vec!["low", "medium", "high"]
    );
    assert!(OpenAiClient::new(ProviderId::OpenAi, None)
        .effort_levels("gpt-4o")
        .is_empty());
}

#[test]
fn effort_levels_returns_the_verified_universal_set_for_every_reasoning_model() {
    assert_eq!(
        OpenAiClient::new(ProviderId::OpenAi, None).effort_levels("o3-mini"),
        vec!["low", "medium", "high"]
    );
    assert_eq!(
        OpenAiClient::new(ProviderId::OpenAi, None).effort_levels("gpt-5.6"),
        vec!["low", "medium", "high"]
    );
    assert!(OpenAiClient::new(ProviderId::OpenAi, None)
        .effort_levels("gpt-4o")
        .is_empty());
}

#[test]
fn response_format_is_sent_only_to_provider_ids_this_adapter_can_vouch_for() {
    // Native OpenAI defines the field; Ollama Cloud's `/v1` documents it. An
    // arbitrary gateway is an unknown server build — guessing 400s a whole
    // generation, so it takes the prompt-discipline fallback instead.
    assert!(OpenAiClient::new(ProviderId::OpenAi, None).supports_response_format());
    assert!(OpenAiClient::new(ProviderId::OllamaCloud, None).supports_response_format());
    assert!(!OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("http://localhost:1234/v1".into())
    )
    .supports_response_format());
}

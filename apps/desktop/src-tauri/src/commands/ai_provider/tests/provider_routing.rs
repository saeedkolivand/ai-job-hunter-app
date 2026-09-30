//! `resolve_by_name` / `ProviderId` routing: capability-driven web-search and
//! reasoning-effort support, `base_url` validation, the Ollama Cloud /
//! Claude Code wiring, and `validate_model`'s cross-provider guard.

use crate::error::AppError;

use super::super::*;

#[test]
fn web_search_support_is_capability_driven_per_provider() {
    // Exercises the exact path `ai_model_capabilities` takes
    // (`resolve_by_name(..).capabilities(..).supports_web_search`) so the
    // renderer's capability-driven "search company" default stays a read of
    // the Rust matrix, never a TS mirror. Native OpenAI can web-search; a
    // generic OpenAI-compatible gateway cannot — every other provider can.
    let cases = [
        ("anthropic", true),
        ("gemini", true),
        ("ollama", true),
        ("ollama-cloud", true),
        ("openai", true),
        ("openai-compatible", false),
        ("claude-code", true),
        ("codex", true),
        ("gemini-cli", true),
        ("antigravity", true),
    ];
    for (name, expected) in cases {
        let client = resolve_by_name(name, None).unwrap();
        assert_eq!(
            client.capabilities("").supports_web_search,
            expected,
            "{name} web-search support"
        );
    }
    assert!(resolve_by_name("nope", None).is_err());
}

#[test]
fn reasoning_effort_support_is_capability_driven_per_provider_and_model() {
    // Exercises the exact path `ai_model_capabilities` takes for
    // `supportsReasoning` — a model-specific gate (unlike web search, which
    // is per-provider only), so this checks BOTH a capable and a
    // non-capable model per HTTP provider.
    let cases = [
        ("openai", "o3-mini", true),
        ("openai", "gpt-4o", false),
        ("anthropic", "claude-opus-5", true),
        ("anthropic", "claude-3-5-sonnet-20241022", false),
        ("gemini", "gemini-3-pro-preview", true),
        ("gemini", "gemini-2.5-pro", false),
        ("ollama", "gpt-oss:120b", true),
        ("ollama", "llama3.1:8b", false),
        ("ollama-cloud", "gpt-oss:120b", true),
        ("ollama-cloud", "qwen3-coder:480b", false),
        ("openai-compatible", "o3-mini", false),
    ];
    for (provider, model, expected) in cases {
        let client = resolve_by_name(provider, None).unwrap();
        assert_eq!(
            client.capabilities(model).supports_reasoning,
            expected,
            "{provider}/{model} reasoning support"
        );
    }
}

#[test]
fn provider_id_round_trips() {
    for id in [
        ProviderId::Ollama,
        ProviderId::OllamaCloud,
        ProviderId::OpenAi,
        ProviderId::OpenAiCompatible,
        ProviderId::Anthropic,
        ProviderId::Gemini,
        ProviderId::ClaudeCode,
        ProviderId::Codex,
        ProviderId::GeminiCli,
        ProviderId::Antigravity,
    ] {
        assert_eq!(ProviderId::parse(id.as_str()).unwrap(), id);
    }
    assert!(ProviderId::parse("nope").is_err());
}

// ── resolve_by_name: base_url validation (mirrors AiConfigStore::validate_settings) ──
//
// The renderer-facing probe commands (`ai_test_provider_key`/
// `ai_list_provider_models`/`ai_model_capabilities`) hand `resolve_by_name` a
// `base_url` straight off the wire, unlike the settings writer which runs it
// through `AiConfigStore::validate_settings` first. These tests pin the two
// rules `resolve_by_name` now applies itself so the probe path can't regress
// to the unvalidated pre-fix behavior.

#[test]
fn resolve_by_name_rejects_the_cloud_metadata_ip_on_openai_compatible() {
    // `.err().unwrap()`, not `.expect_err(..)`: the `Ok` payload is
    // `Box<dyn AiProvider>`, which is not `Debug` (`expect_err`/`unwrap_err`
    // both require it).
    let err = resolve_by_name(
        "openai-compatible",
        Some("http://169.254.169.254/latest/meta-data".to_string()),
    )
    .err()
    .expect("the cloud-metadata IP literal must be rejected");
    assert!(matches!(err, AppError::Validation(_)));
}

#[test]
fn resolve_by_name_rejects_a_non_http_scheme_on_openai_compatible() {
    let err = resolve_by_name("openai-compatible", Some("file:///etc/passwd".to_string()))
        .err()
        .expect("a non-http(s) scheme must be rejected");
    assert!(matches!(err, AppError::Validation(_)));
}

#[test]
fn resolve_by_name_accepts_a_normal_openai_compatible_base_url() {
    // Sanity: the validation floor must not reject the ordinary case (a local
    // LM Studio / vLLM endpoint) it exists to protect around.
    assert!(resolve_by_name(
        "openai-compatible",
        Some("http://localhost:1234/v1".to_string())
    )
    .is_ok());
}

#[test]
fn resolve_by_name_drops_base_url_for_a_non_openai_compatible_provider_instead_of_erroring() {
    // Mirrors `AiConfigStore::validate_settings`'s scrub: `base_url` is inert
    // for egress on every provider except `OpenAiCompatible`, so a bogus value
    // (here, one that WOULD fail `validate_provider_base_url` if it were
    // checked) must be silently dropped, not surfaced as an error — the same
    // way the persisted-settings path already behaves.
    assert!(resolve_by_name(
        "anthropic",
        Some("http://169.254.169.254/latest/meta-data".to_string())
    )
    .is_ok());
    assert!(resolve_by_name("gemini", Some("not a url at all".to_string())).is_ok());
}

#[test]
fn ollama_cloud_wire_and_credential_key() {
    assert_eq!(ProviderId::OllamaCloud.as_str(), "ollama-cloud");
    assert_eq!(
        ProviderId::parse("ollama-cloud").unwrap(),
        ProviderId::OllamaCloud
    );
    // Shares the `ai:ollama-cloud` credential slot used by Ollama Web Search.
    assert_eq!(ProviderId::OllamaCloud.credential_key(), "ollama-cloud");
    // Cloud, not a local CLI agent.
    assert!(!ProviderId::OllamaCloud.is_cli_agent());
    assert!(!ProviderId::OllamaCloud.is_local());
}

#[test]
fn resolve_ollama_cloud_returns_cloud_client() {
    // Composed client reports its own id (chat is delegated to the inner
    // OpenAI client against ollama.com/v1).
    assert_eq!(
        resolve(ProviderId::OllamaCloud, None).id(),
        ProviderId::OllamaCloud
    );
}

#[test]
fn claude_code_is_a_local_cli_agent() {
    assert!(ProviderId::ClaudeCode.is_cli_agent());
    assert!(ProviderId::ClaudeCode.is_local());
    assert!(!ProviderId::Anthropic.is_cli_agent());
}

#[test]
fn validate_model_allows_unknown_new_names() {
    // A model the code has never heard of must still be accepted, so newly
    // released models work with no code change.
    assert!(ProviderId::OpenAi.validate_model("gpt-6-ultra").is_ok());
    assert!(ProviderId::OpenAi.validate_model("o9-pro").is_ok());
    assert!(ProviderId::Anthropic
        .validate_model("claude-5-haiku")
        .is_ok());
    assert!(ProviderId::Gemini.validate_model("gemini-9-ultra").is_ok());
}

#[test]
fn validate_model_blocks_clear_cross_provider_mistakes() {
    assert!(ProviderId::Anthropic.validate_model("gpt-4o").is_err());
    assert!(ProviderId::OpenAi
        .validate_model("claude-opus-4-7")
        .is_err());
    assert!(ProviderId::Gemini.validate_model("claude-3").is_err());
}

#[test]
fn validate_model_openai_compatible_accepts_any_family() {
    // OpenRouter (openai-compatible) serves anthropic/* and google/* models.
    assert!(ProviderId::OpenAiCompatible
        .validate_model("anthropic/claude-3.5-sonnet")
        .is_ok());
    assert!(ProviderId::OpenAiCompatible
        .validate_model("google/gemini-2.0-flash")
        .is_ok());
}

#[test]
fn validate_model_cli_agent_allows_empty_and_aliases() {
    assert!(ProviderId::ClaudeCode.validate_model("").is_ok());
    assert!(ProviderId::ClaudeCode.validate_model("sonnet").is_ok());
}

#[test]
fn validate_model_cloud_requires_a_model() {
    assert!(ProviderId::OpenAi.validate_model("").is_err());
}

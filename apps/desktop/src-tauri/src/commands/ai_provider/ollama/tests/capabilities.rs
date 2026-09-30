//! Tool-calling + thinking-family model classification gates.

use super::super::super::AiProvider;
use super::super::{ollama_family_supports_thinking, ollama_supports_tools, OllamaClient};

#[test]
fn tool_support_gate_is_conservative() {
    for m in [
        "llama3.1:8b",
        "llama3.3:70b",
        "qwen2.5:7b",
        "mistral-nemo",
        "command-r-plus",
    ] {
        assert!(ollama_supports_tools(m), "{m} should advertise tools");
    }
    // Unknown / non-tool families default off so the turn degrades safely.
    for m in [
        "llama2",
        "phi3",
        "gemma2",
        "nomic-embed-text",
        "deepseek-coder",
    ] {
        assert!(!ollama_supports_tools(m), "{m} must default to no tools");
    }
}

#[test]
fn thinking_family_gate_matches_documented_models_only() {
    for m in [
        "qwen3",
        "qwen3:30b",
        "qwen3:8b",
        "my-qwen3",
        "gpt-oss:20b",
        "gpt-oss:120b",
        "deepseek-r1",
        "deepseek-r1:70b",
        "deepseek-v3.1:671b",
    ] {
        assert!(ollama_family_supports_thinking(m), "{m} should think");
    }
    // qwen3-coder(-plus) matches the "qwen3" substring but is a separate,
    // non-thinking model — must NOT be swept in by a broad substring match.
    for m in [
        "qwen3-coder:480b",
        "qwen3-coder-plus",
        "llama3.1:8b",
        "mistral",
        "nomic-embed-text",
    ] {
        assert!(!ollama_family_supports_thinking(m), "{m} must not think");
    }
}

#[test]
fn coder_exclusion_is_scoped_to_qwen3_not_every_model_containing_coder() {
    // The exclusion is narrow — a hypothetical thinking-capable "coder"
    // variant in an UNRELATED family must not be swept out by name
    // collision alone (only the qwen3 branch excludes "coder").
    assert!(ollama_family_supports_thinking("gpt-oss-coder"));
    assert!(ollama_family_supports_thinking("deepseek-r1-coder"));
}

#[test]
fn effort_levels_mirror_the_thinking_gate() {
    assert_eq!(
        OllamaClient.effort_levels("gpt-oss:120b"),
        vec!["low", "medium", "high"]
    );
    assert!(OllamaClient.effort_levels("llama3.1:8b").is_empty());
}

//! Cheap reasoning effort (`pipeline::low_effort_level` over the REAL
//! adapters' level lists).
//!
//! `Completer::low_effort` takes the FIRST entry of a provider's
//! `effort_levels(model)` and keeps it only when it is `minimal`/`low`. The
//! first half is only correct while every adapter lists its LOWEST tier
//! first, which these tests pin against the live tables — the `Completer`
//! itself needs an `AppHandle` this crate has no harness for, so the free
//! function is what gets driven here.

use super::super::*;

/// The closed effort vocabulary, LOWEST first — the same tier order
/// `timeouts::effort_multiplier`'s table documents (`minimal < low < medium
/// < high < xhigh < max`; `max` is the TOP tier, not `xhigh`). Written out by
/// hand here on purpose: a guard driven off the same list it guards would
/// pass no matter how the adapters reorder theirs.
const TIER_ORDER: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// Every `(what it is, levels)` pair the shipped adapters can currently
/// return, across each per-model branch of each provider's own table.
fn every_providers_effort_levels() -> Vec<(&'static str, Vec<&'static str>)> {
    let openai = OpenAiClient::new(ProviderId::OpenAi, None);
    let compat = OpenAiClient::new(ProviderId::OpenAiCompatible, None);
    let cloud = OllamaCloudClient::new();
    vec![
        (
            "ollama gpt-oss:20b",
            OllamaClient.effort_levels("gpt-oss:20b"),
        ),
        ("ollama qwen3:8b", OllamaClient.effort_levels("qwen3:8b")),
        (
            "ollama llama3.1:8b",
            OllamaClient.effort_levels("llama3.1:8b"),
        ),
        (
            "ollama-cloud gpt-oss:120b",
            cloud.effort_levels("gpt-oss:120b"),
        ),
        (
            "ollama-cloud qwen3-coder:480b",
            cloud.effort_levels("qwen3-coder:480b"),
        ),
        ("openai o3", openai.effort_levels("o3")),
        ("openai gpt-4o", openai.effort_levels("gpt-4o")),
        (
            "openai-compatible local",
            compat.effort_levels("local-model"),
        ),
        (
            "anthropic claude-opus-4-5",
            AnthropicClient.effort_levels("claude-opus-4-5"),
        ),
        (
            "anthropic claude-opus-5",
            AnthropicClient.effort_levels("claude-opus-5"),
        ),
        (
            "anthropic claude-3-5-sonnet-20241022",
            AnthropicClient.effort_levels("claude-3-5-sonnet-20241022"),
        ),
        (
            "gemini gemini-3.1-pro-preview",
            GeminiClient.effort_levels("gemini-3.1-pro-preview"),
        ),
        (
            "gemini gemini-3.6-flash",
            GeminiClient.effort_levels("gemini-3.6-flash"),
        ),
        (
            "gemini gemini-3.1-flash-lite-image",
            GeminiClient.effort_levels("gemini-3.1-flash-lite-image"),
        ),
        (
            "gemini gemini-3.1-flash-lite",
            GeminiClient.effort_levels("gemini-3.1-flash-lite"),
        ),
        (
            "gemini gemini-2.5-pro",
            GeminiClient.effort_levels("gemini-2.5-pro"),
        ),
        (
            "codex CLI",
            cli_agent::CliAgentClient::new(
                cli_agent::backend_for(ProviderId::Codex).expect("codex is a registered backend"),
            )
            .effort_levels(""),
        ),
        (
            "claude-code CLI",
            cli_agent::CliAgentClient::new(
                cli_agent::backend_for(ProviderId::ClaudeCode)
                    .expect("claude-code is a registered backend"),
            )
            .effort_levels(""),
        ),
    ]
}

/// The invariant `low_effort_level`'s `.first()` rests on. Deliberately
/// NOT "the whole list is sorted": Anthropic's is not (it ends `…, "max",
/// "xhigh"`, following its own docs' enumeration rather than tier order).
/// What must hold — and all `.first()` needs — is that entry ZERO is the
/// minimum of the list.
#[test]
fn every_providers_effort_levels_list_its_lowest_tier_first() {
    let rank = |level: &str| {
        TIER_ORDER
            .iter()
            .position(|t| *t == level)
            .unwrap_or_else(|| {
                panic!(
                    "{level:?} is outside the shared effort vocabulary {TIER_ORDER:?} — \
                     add it there (and to the generated EFFORT_TIMEOUT_MULTIPLIER table) \
                     before a provider returns it"
                )
            })
    };

    for (what, levels) in every_providers_effort_levels() {
        let Some(min) = levels.iter().map(|l| rank(l)).min() else {
            continue; // no effort lever at all — nothing to order
        };
        assert_eq!(
            rank(levels[0]),
            min,
            "{what}: {levels:?} must list its LOWEST tier first — \
             `Completer::low_effort` takes entry 0"
        );
    }
}

/// The concrete answers the extension bridge's `answer.assist` depends on,
/// on the provider the defect was measured against, on a model with no lever
/// at all, and on a model whose lowest tier is already an expensive one.
///
/// Mutation check (executed): drop the `minimal`/`low` filter from
/// `low_effort_level` and the `gemini-3.1-flash-lite` case fails (it resolves
/// `Some("high")`); replace the `.first()` with a `"low"` name match and the
/// `minimal`-first Gemini case fails.
#[test]
fn low_effort_level_resolves_low_for_gpt_oss_and_nothing_for_a_model_with_no_cheap_tier() {
    let cheap = |levels: Vec<&'static str>| crate::pipeline::low_effort_level(&levels);

    // Ollama Cloud gpt-oss — the config the empty-length-cut failure was
    // measured on. Local Ollama's own gpt-oss resolves the same way.
    assert_eq!(
        cheap(OllamaCloudClient::new().effort_levels("gpt-oss:20b")),
        Some("low")
    );
    assert_eq!(
        cheap(OllamaClient.effort_levels("gpt-oss:20b")),
        Some("low")
    );

    // No lever → nothing is sent, and nothing is invented.
    assert_eq!(cheap(OllamaClient.effort_levels("llama3.1:8b")), None);
    assert_eq!(cheap(GeminiClient.effort_levels("gemini-2.5-pro")), None);
    assert_eq!(
        cheap(OpenAiClient::new(ProviderId::OpenAiCompatible, None).effort_levels("local-model")),
        None
    );

    // The lowest tier is not always spelled "low" — this one is `minimal`,
    // which is why the resolution is positional rather than a name match.
    let flash = GeminiClient.effort_levels("gemini-3.6-flash");
    assert_eq!(flash.first().copied(), Some("minimal"));
    assert_eq!(cheap(flash), Some("minimal"));

    // …and a model whose ONLY accepted level is an expensive one gets
    // NOTHING, not that level: `answer.assist` asks for a cheap tier to keep
    // reasoning out of its output budget, and `"high"` would both raise the
    // thinking and stretch `stream_deadline` past the baseline (asserted
    // below), holding an `ai_research` slot longer.
    let lite = GeminiClient.effort_levels("gemini-3.1-flash-lite");
    assert_eq!(
        lite.first().copied(),
        Some("high"),
        "the premise: this model's lowest — and only — tier is an expensive one"
    );
    assert_eq!(cheap(lite), None);
}

/// Asking for the lowest tier must never SHORTEN the stream's deadline: the
/// `answer.assist` compose picks an effort purely to keep reasoning from
/// eating the output budget, and a shorter deadline would trade one failure
/// mode for another. `minimal`/`low`/absent all sit at the 1.0 multiplier.
#[test]
fn the_cheap_effort_tiers_keep_the_baseline_stream_deadline() {
    for level in [None, Some("minimal"), Some("low")] {
        assert_eq!(
            timeouts::stream_deadline(level),
            timeouts::STREAM,
            "{level:?} must keep the baseline deadline, never shrink it"
        );
    }
    assert!(
        timeouts::stream_deadline(Some("high")) > timeouts::STREAM,
        "…while a higher tier still extends it"
    );
}

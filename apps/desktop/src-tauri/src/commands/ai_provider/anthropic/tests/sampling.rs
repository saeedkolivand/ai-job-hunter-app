//! `AnthropicClient::sampling_profile` — neutral on adaptive/unrecognized
//! models, real per-intent values everywhere else.

use super::super::super::{
    AiProvider, Intent, SamplingProfile, DETERMINISTIC_TEMPERATURE, PROSE_GROUNDED_TEMPERATURE,
    PROSE_TEMPERATURE, PROSE_TOP_P,
};
use super::super::AnthropicClient;

#[test]
fn sampling_profile_is_neutral_on_adaptive_frontier_and_unrecognized_models_only() {
    // Hard constraint: a Claude 4.7+/5 model receives no sampling params at
    // all, for every intent — `temperature`/`top_p` 400 outright on adaptive
    // models regardless of value. An unrecognized `claude-`-prefixed id
    // fails safe the same way (the same new-family fail-safe
    // `anthropic_supports_temperature` already applies).
    for model in ["claude-opus-5", "claude-opus-4-7", "claude-zephyr-6"] {
        for intent in [
            Intent::Deterministic,
            Intent::Prose,
            Intent::ProseGrounded,
            Intent::Default,
        ] {
            assert_eq!(
                AnthropicClient.sampling_profile(model, intent),
                SamplingProfile::default(),
                "{model} / {intent:?} must stay neutral"
            );
        }
    }
}

#[test]
fn sampling_profile_declares_real_values_for_a_legacy_model_per_intent() {
    // A model `anthropic_supports_temperature` accepts (legacy pre-thinking)
    // declares REAL values reproducing this app's pre-fix shipped numbers —
    // omitting is not a safe fallback on a provider that accepts the field
    // (see `commands/ai_provider/sampling.rs`'s module doc for why).
    let model = "claude-3-5-sonnet-20241022";
    assert_eq!(
        AnthropicClient.sampling_profile(model, Intent::Deterministic),
        SamplingProfile {
            temperature: Some(DETERMINISTIC_TEMPERATURE),
            ..SamplingProfile::default()
        }
    );
    assert_eq!(
        AnthropicClient.sampling_profile(model, Intent::Prose),
        SamplingProfile {
            temperature: Some(PROSE_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            ..SamplingProfile::default()
        }
    );
    assert_eq!(
        AnthropicClient.sampling_profile(model, Intent::ProseGrounded),
        SamplingProfile {
            temperature: Some(PROSE_GROUNDED_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            ..SamplingProfile::default()
        }
    );
    // `Default` (no declared intent) resolves the same as `Deterministic`.
    assert_eq!(
        AnthropicClient.sampling_profile(model, Intent::Default),
        AnthropicClient.sampling_profile(model, Intent::Deterministic)
    );
}

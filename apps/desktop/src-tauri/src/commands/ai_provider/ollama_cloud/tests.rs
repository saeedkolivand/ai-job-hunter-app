use super::super::{
    PROSE_FREQUENCY_PENALTY, PROSE_PRESENCE_PENALTY, PROSE_TEMPERATURE, PROSE_TOP_P,
};
use super::*;

/// Regression guard: the inner `OpenAiClient`'s own `supports_web_search`
/// only passes for `ProviderId::OpenAi`, so a naive delegation would
/// wrongly report `false` for Ollama Cloud — which DOES search via the
/// Ollama Web Search API (see `research_answer` above). The override in
/// `capabilities()` must force it back to `true`.
#[test]
fn capabilities_reports_web_search_support_despite_the_inner_openai_compatible_id() {
    let caps = OllamaCloudClient::new().capabilities("gpt-oss:120b");
    assert!(caps.supports_web_search);
}

#[test]
fn effort_levels_delegate_to_the_inner_client() {
    assert_eq!(
        OllamaCloudClient::new().effort_levels("gpt-oss:120b"),
        vec!["low", "medium", "high"]
    );
    assert!(OllamaCloudClient::new()
        .effort_levels("qwen3-coder:480b")
        .is_empty());
}

/// Regression guard, same shape as the two tests above: a naive
/// delegation to `self.inner` (a generic `OpenAiClient`) would need to
/// reach the SAME `self.id == ProviderId::OllamaCloud` branch inside
/// `OpenAiClient::sampling_profile` that `chat_stream` actually uses —
/// this pins the exact path production traffic takes, not a parallel copy.
#[test]
fn sampling_profile_delegates_to_the_inner_clients_ollama_cloud_table() {
    let gpt_oss = OllamaCloudClient::new().sampling_profile("gpt-oss:120b", Intent::Prose);
    assert_eq!(gpt_oss.temperature, Some(1.0));
    assert_eq!(gpt_oss.top_p, Some(1.0));

    // `ollama_cloud_sampling_profile` returns the gpt-oss 1.0/1.0 override
    // BEFORE it ever inspects `intent` — pin a second, different intent on
    // the SAME model so a future refactor that starts branching gpt-oss on
    // intent (breaking the family-wide override) fails here, not silently.
    let gpt_oss_deterministic =
        OllamaCloudClient::new().sampling_profile("gpt-oss:120b", Intent::Deterministic);
    assert_eq!(gpt_oss_deterministic.temperature, Some(1.0));
    assert_eq!(gpt_oss_deterministic.top_p, Some(1.0));

    // Every OTHER family reuses the SAME per-intent targets native
    // OpenAI does (the `/v1` layer hardcodes 1.0/1.0 on omission for
    // every family, not just gpt-oss — see `ollama_cloud_sampling_profile`'s
    // doc comment) — never left neutral.
    let other = OllamaCloudClient::new().sampling_profile("qwen3-coder:480b", Intent::Prose);
    assert_eq!(
        other,
        SamplingProfile {
            temperature: Some(PROSE_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            frequency_penalty: Some(PROSE_FREQUENCY_PENALTY),
            presence_penalty: Some(PROSE_PRESENCE_PENALTY),
            ..SamplingProfile::default()
        }
    );
}

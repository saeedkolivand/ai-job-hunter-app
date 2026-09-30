//! `OpenAiClient::sampling_profile` + the wire-through into
//! `build_chat_stream_body`, across native OpenAI, generic OpenAI-compatible
//! gateways, and Ollama Cloud's distinct gpt-oss table.

use serde_json::json;

use super::super::super::{
    AiProvider, Intent, ProviderId, SamplingProfile, DETERMINISTIC_TEMPERATURE,
    PROSE_FREQUENCY_PENALTY, PROSE_GROUNDED_TEMPERATURE, PROSE_PRESENCE_PENALTY, PROSE_TEMPERATURE,
    PROSE_TOP_P,
};
use super::super::body::build_chat_stream_body;
use super::super::OpenAiClient;
use super::support::{base_request, body_for, chat_caps, sampling_for};

#[test]
fn chat_stream_body_always_requests_streamed_usage() {
    // AI-spend visibility depends on this flag being sent on every OpenAI
    // Chat Completions stream (native, OpenAI-compatible, and Ollama Cloud).
    let req = base_request();
    let body = build_chat_stream_body(
        &req,
        chat_caps(true),
        sampling_for(ProviderId::OpenAi, &req),
    );
    assert_eq!(body["stream_options"], json!({ "include_usage": true }));
}

#[test]
fn native_openai_wire_body_per_intent() {
    let det = body_for(ProviderId::OpenAi, "gpt-4o", Some("deterministic"));
    assert_eq!(det["temperature"], json!(DETERMINISTIC_TEMPERATURE));
    assert!(det.get("top_p").is_none());
    assert!(det.get("frequency_penalty").is_none());
    assert!(det.get("presence_penalty").is_none());

    let prose = body_for(ProviderId::OpenAi, "gpt-4o", Some("prose"));
    assert_eq!(prose["temperature"], json!(PROSE_TEMPERATURE));
    assert_eq!(prose["top_p"], json!(PROSE_TOP_P));
    assert_eq!(prose["frequency_penalty"], json!(PROSE_FREQUENCY_PENALTY));
    assert_eq!(prose["presence_penalty"], json!(PROSE_PRESENCE_PENALTY));

    let grounded = body_for(ProviderId::OpenAi, "gpt-4o", Some("prose_grounded"));
    assert_eq!(grounded["temperature"], json!(PROSE_GROUNDED_TEMPERATURE));
    assert_eq!(grounded["top_p"], json!(PROSE_TOP_P));
    assert_eq!(
        grounded["frequency_penalty"],
        json!(PROSE_FREQUENCY_PENALTY)
    );
    assert!(
        grounded.get("presence_penalty").is_none(),
        "prose_grounded must never send presence_penalty"
    );

    // `Default` (no declared intent) resolves the same as `Deterministic` —
    // see `Intent`'s own doc comment (`commands/ai_provider/mod.rs`).
    let default = body_for(ProviderId::OpenAi, "gpt-4o", None);
    assert_eq!(default["temperature"], json!(DETERMINISTIC_TEMPERATURE));
    assert!(default.get("top_p").is_none());
    assert!(default.get("frequency_penalty").is_none());
    assert!(default.get("presence_penalty").is_none());
}

#[test]
fn chat_stream_body_skips_sampling_params_on_reasoning_models() {
    // o-series models reject `temperature` entirely — sampling knobs must be
    // skipped alongside it, never sent to a model that 400s on them.
    // `sampling_profile` already returns neutral for these, so this pins the
    // wire body doubly (profile AND `caps.supports_temperature` both agree).
    for intent in ["deterministic", "prose", "prose_grounded"] {
        let body = body_for(ProviderId::OpenAi, "o3-mini", Some(intent));
        assert!(body.get("temperature").is_none(), "{intent}");
        assert!(body.get("top_p").is_none(), "{intent}");
        assert!(body.get("frequency_penalty").is_none(), "{intent}");
        assert!(body.get("presence_penalty").is_none(), "{intent}");
    }
}

#[test]
fn sampling_profile_explicit_override_wins_over_the_prose_profile() {
    // Hard constraint: an explicit user value still wins on every adapter.
    let mut req = base_request();
    req.model = "gpt-4o".to_string();
    req.intent = Some("prose".to_string());
    req.temperature = Some(0.42);
    req.top_p = Some(0.11);
    let merged = sampling_for(ProviderId::OpenAi, &req);
    assert_eq!(merged.temperature, Some(0.42));
    assert_eq!(merged.top_p, Some(0.11));
    // Untouched fields still come from the profile.
    assert_eq!(merged.frequency_penalty, Some(PROSE_FREQUENCY_PENALTY));
}

#[test]
fn sampling_profile_is_neutral_for_a_reasoning_model_regardless_of_intent() {
    // Hard constraint: an OpenAI reasoning model receives no temperature —
    // true for o-series (which also 400s on it) AND gpt-5.x (which technically
    // accepts it but gets no per-task tuning either, see the doc comment on
    // `OpenAiClient::sampling_profile`).
    for model in ["o3-mini", "gpt-5.6"] {
        for intent in [
            Intent::Deterministic,
            Intent::Prose,
            Intent::ProseGrounded,
            Intent::Default,
        ] {
            let profile =
                OpenAiClient::new(ProviderId::OpenAi, None).sampling_profile(model, intent);
            assert_eq!(
                profile,
                SamplingProfile::default(),
                "{model} / {intent:?} must stay neutral"
            );
        }
    }
}

#[test]
fn native_openai_has_no_unknown_model_carve_out_unlike_gemini_or_ollama_cloud() {
    // Native OpenAI's ONLY two neutral cases are the reasoning-model gate
    // and the non-native-id gate (both tested separately) — an
    // unrecognized MODEL NAME under `ProviderId::OpenAi` is not a
    // "family this app can't classify" case (there is no Gemini-style
    // version-prefix or Anthropic-style `claude-` family check for native
    // OpenAI), so it correctly still gets real Deterministic-equivalent
    // values, not neutral.
    let profile = OpenAiClient::new(ProviderId::OpenAi, None)
        .sampling_profile("some-unrecognized-model", Intent::Default);
    assert_eq!(
        profile,
        SamplingProfile {
            temperature: Some(DETERMINISTIC_TEMPERATURE),
            ..SamplingProfile::default()
        }
    );
}

#[test]
fn openai_compatible_wire_body_per_intent() {
    // Mirrors `native_openai_wire_body_per_intent` exactly. LM
    // Studio/vLLM/OpenRouter/custom `OpenAiCompatible` gateways speak the
    // SAME wire protocol as native OpenAI, and `Intent` encodes an APP
    // requirement on the response shape (e.g. the analyze prompt's
    // strict-JSON contract needs a low temperature) — not a guess about an
    // unrecognized MODEL's preferred creative sampling — so it applies
    // here too. This is NOT the unknown-model fail-safe the other adapters
    // use; see `OpenAiClient::sampling_profile`'s doc comment. A prior
    // round wrongly neutralized this path (gated on `self.id ==
    // ProviderId::OpenAi`), silently dropping the JSON-strict analysis
    // surface's low temperature for every gateway — this test pins the
    // regression.
    let det = body_for(
        ProviderId::OpenAiCompatible,
        "gpt-4o",
        Some("deterministic"),
    );
    assert_eq!(det["temperature"], json!(DETERMINISTIC_TEMPERATURE));
    assert!(det.get("top_p").is_none());
    assert!(det.get("frequency_penalty").is_none());
    assert!(det.get("presence_penalty").is_none());

    let prose = body_for(ProviderId::OpenAiCompatible, "gpt-4o", Some("prose"));
    assert_eq!(prose["temperature"], json!(PROSE_TEMPERATURE));
    assert_eq!(prose["top_p"], json!(PROSE_TOP_P));
    assert_eq!(prose["frequency_penalty"], json!(PROSE_FREQUENCY_PENALTY));
    assert_eq!(prose["presence_penalty"], json!(PROSE_PRESENCE_PENALTY));

    let grounded = body_for(
        ProviderId::OpenAiCompatible,
        "gpt-4o",
        Some("prose_grounded"),
    );
    assert_eq!(grounded["temperature"], json!(PROSE_GROUNDED_TEMPERATURE));
    assert_eq!(grounded["top_p"], json!(PROSE_TOP_P));
    assert_eq!(
        grounded["frequency_penalty"],
        json!(PROSE_FREQUENCY_PENALTY)
    );
    assert!(
        grounded.get("presence_penalty").is_none(),
        "prose_grounded must never send presence_penalty"
    );
}

#[test]
fn ollama_cloud_wire_body_gpt_oss_vs_every_other_family() {
    // Source: `github.com/openai/gpt-oss` README, "Recommended Sampling
    // Parameters" — Ollama's `/v1` layer hardcodes 1.0/1.0 when omitted,
    // overriding the Modelfile, so this app must declare it explicitly.
    let gpt_oss = body_for(
        ProviderId::OllamaCloud,
        "gpt-oss:120b",
        Some("deterministic"),
    );
    assert_eq!(gpt_oss["temperature"], json!(1.0));
    assert_eq!(gpt_oss["top_p"], json!(1.0));
    assert!(gpt_oss.get("frequency_penalty").is_none());

    // Every OTHER Ollama Cloud family reuses the SAME per-intent targets
    // native OpenAI does (fix: these used to stay neutral, which is wrong —
    // `/v1` hardcodes 1.0/1.0 on omission for every family, not just gpt-oss).
    for model in ["qwen3-coder:480b", "deepseek-v3.1:671b"] {
        let det = body_for(ProviderId::OllamaCloud, model, Some("deterministic"));
        assert_eq!(
            det["temperature"],
            json!(DETERMINISTIC_TEMPERATURE),
            "{model}"
        );

        let grounded = body_for(ProviderId::OllamaCloud, model, Some("prose_grounded"));
        assert_eq!(
            grounded["temperature"],
            json!(PROSE_GROUNDED_TEMPERATURE),
            "{model}"
        );
        assert!(
            grounded.get("presence_penalty").is_none(),
            "{model}: prose_grounded must never send presence_penalty"
        );
    }
}

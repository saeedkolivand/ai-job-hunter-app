//! `gemini_effective_temperature`/`gemini_omits_sampling_params` and the
//! `GeminiClient::sampling_profile` + `build_chat_stream_body` wire-through.

use serde_json::json;

use super::super::super::{
    AiProvider, Intent, SamplingProfile, DETERMINISTIC_TEMPERATURE, PROSE_FREQUENCY_PENALTY,
    PROSE_GROUNDED_TEMPERATURE, PROSE_PRESENCE_PENALTY, PROSE_TEMPERATURE, PROSE_TOP_P,
};
use super::super::body::build_chat_stream_body;
use super::super::thinking::{gemini_effective_temperature, gemini_omits_sampling_params};
use super::super::GeminiClient;
use super::support::{base_request, sampling_for};

#[test]
fn chat_stream_body_serializes_sampling_params_when_set() {
    let mut req = base_request();
    req.top_p = Some(0.95);
    req.frequency_penalty = Some(0.3);
    req.presence_penalty = Some(0.2);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    let config = &body["generationConfig"];
    assert_eq!(config["topP"], json!(0.95));
    assert_eq!(config["frequencyPenalty"], json!(0.3));
    assert_eq!(config["presencePenalty"], json!(0.2));
}

#[test]
fn gemini_effective_temperature_omits_only_for_v3_with_no_explicit_value() {
    // Explicit value ALWAYS wins, on every model — never overridden.
    assert_eq!(
        gemini_effective_temperature("gemini-3.6-flash", Some(0.3), 0.7),
        Some(0.3)
    );
    assert_eq!(
        gemini_effective_temperature("gemini-1.5-flash", Some(0.3), 0.7),
        Some(0.3)
    );
    // No explicit value, pre-v3: keeps the caller's fallback (unchanged
    // behavior — Google's don't-touch-1.0 guidance is scoped to Gemini 3+).
    assert_eq!(
        gemini_effective_temperature("gemini-1.5-flash", None, 0.7),
        Some(0.7)
    );
    // No explicit value, v3+: omit entirely — `ai.google.dev/gemini-api/docs/gemini-3`
    // (fetched 2026-08-04) warns a below-1.0 value risks looping/degraded
    // performance on complex reasoning tasks; never invent one.
    assert_eq!(
        gemini_effective_temperature("gemini-3.6-flash", None, 0.7),
        None
    );
}

#[test]
fn chat_stream_body_omits_temperature_for_a_v3_model_with_no_explicit_value() {
    let mut req = base_request();
    req.model = "gemini-3.6-flash".to_string();
    req.temperature = None;
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body["generationConfig"].get("temperature").is_none(),
        "must not invent a temperature for a v3+ model — let the API apply its own 1.0"
    );
}

#[test]
fn chat_stream_body_sends_an_explicit_temperature_even_on_a_v3_model() {
    let mut req = base_request();
    req.model = "gemini-3.6-flash".to_string();
    req.temperature = Some(0.3);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["generationConfig"]["temperature"],
        json!(0.3),
        "a deliberate user value must still be honored on a v3+ model"
    );
}

#[test]
fn chat_stream_body_uses_the_deterministic_target_for_a_pre_v3_model_with_no_explicit_value() {
    // No `req.intent` set → `Intent::Default`, which resolves to the SAME
    // numbers as `Intent::Deterministic` on an accepting model (see
    // `Intent`'s own doc comment, `commands/ai_provider/sampling.rs`) — this
    // reproduces the pre-fix renderer's own hardcoded default (`0.3` for the
    // majority of deterministic surfaces), NOT the adapter's old standalone
    // `0.7` fallback (that fallback's job is now done by this profile).
    let mut req = base_request();
    req.model = "gemini-1.5-flash".to_string();
    req.temperature = None;
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["generationConfig"]["temperature"],
        json!(DETERMINISTIC_TEMPERATURE)
    );
}

#[test]
fn chat_stream_body_uses_the_deterministic_target_for_a_non_gemini_prefixed_pre_v3_model() {
    // `gemma-3-27b-it` is what a REAL non-`gemini-`-prefixed pre-v3 id looks
    // like on this provider (`parse_model_page` surfaces it into the model
    // picker exactly like a `gemini-*` id — see `gemini_is_pre_v3`'s doc
    // comment). It must reach the wire with the SAME deterministic
    // temperature as a `gemini-`-prefixed pre-v3 model, not silently drop to
    // neutral.
    let mut req = base_request();
    req.model = "gemma-3-27b-it".to_string();
    req.temperature = None;
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["generationConfig"]["temperature"],
        json!(DETERMINISTIC_TEMPERATURE)
    );
}

#[test]
fn gemini_omits_sampling_params_matches_the_v3_gate() {
    // ONE predicate decides temperature AND topP (and topK, if this file
    // ever wires one up) — pinned directly so the two parameters can't
    // drift apart the way `topP` silently did before this fix.
    assert!(gemini_omits_sampling_params("gemini-3.6-flash"));
    assert!(gemini_omits_sampling_params("gemini-3-pro-preview"));
    assert!(!gemini_omits_sampling_params("gemini-1.5-flash"));
    assert!(!gemini_omits_sampling_params("gemini-2.5-pro"));
}

#[test]
fn chat_stream_body_omits_top_p_for_a_v3_model_even_when_explicit() {
    // Unlike `temperature`, `topP` has no "deliberate user intent" to
    // preserve — it's the renderer's own anti-detection knob (useless on a
    // model that ignores it), so a v3+ model omits it unconditionally, even
    // when the caller set one.
    let mut req = base_request();
    req.model = "gemini-3.6-flash".to_string();
    req.top_p = Some(0.95);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body["generationConfig"].get("topP").is_none(),
        "topP must never reach a v3+ model, even when explicitly set"
    );
}

#[test]
fn chat_stream_body_sends_top_p_for_a_pre_v3_model_when_explicit() {
    let mut req = base_request();
    req.model = "gemini-1.5-flash".to_string();
    req.top_p = Some(0.95);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["generationConfig"]["topP"],
        json!(0.95),
        "unchanged behavior for pre-v3 models"
    );
}

#[test]
fn chat_stream_body_omits_sampling_params_when_none() {
    let req = base_request();
    let body = build_chat_stream_body(&req, sampling_for(&req));
    let config = &body["generationConfig"];
    assert!(config.get("topP").is_none());
    assert!(config.get("frequencyPenalty").is_none());
    assert!(config.get("presencePenalty").is_none());
}

#[test]
fn sampling_profile_is_fully_neutral_on_a_v3_model_for_every_intent() {
    // Hard constraint: a Gemini 3+ model receives NO temperature and NO
    // top_p, even under `Intent::Deterministic` — Google's deprecation
    // notice is unconditional, not intent-scoped.
    for intent in [
        Intent::Deterministic,
        Intent::Prose,
        Intent::ProseGrounded,
        Intent::Default,
    ] {
        let profile = GeminiClient.sampling_profile("gemini-3.6-flash", intent);
        assert_eq!(
            profile,
            SamplingProfile::default(),
            "{intent:?} must stay neutral on a v3+ model"
        );
    }
}

#[test]
fn sampling_profile_declares_real_values_per_intent_on_a_recognized_pre_v3_model() {
    // "gemini-1.5-flash" fails `gemini_is_v3_or_later`, so it is pre-v3 —
    // it declares REAL values reproducing this app's pre-fix shipped numbers,
    // the same [`DETERMINISTIC_TEMPERATURE`]/[`PROSE_TEMPERATURE`]/
    // [`PROSE_GROUNDED_TEMPERATURE`] targets every other accepting adapter
    // uses (this app's pre-fix renderer sent identical numbers to Gemini as
    // every other cloud provider).
    let model = "gemini-1.5-flash";
    assert_eq!(
        GeminiClient.sampling_profile(model, Intent::Deterministic),
        SamplingProfile {
            temperature: Some(DETERMINISTIC_TEMPERATURE),
            ..SamplingProfile::default()
        }
    );
    assert_eq!(
        GeminiClient.sampling_profile(model, Intent::Prose),
        SamplingProfile {
            temperature: Some(PROSE_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            frequency_penalty: Some(PROSE_FREQUENCY_PENALTY),
            presence_penalty: Some(PROSE_PRESENCE_PENALTY),
            ..SamplingProfile::default()
        }
    );
    assert_eq!(
        GeminiClient.sampling_profile(model, Intent::ProseGrounded),
        SamplingProfile {
            temperature: Some(PROSE_GROUNDED_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            frequency_penalty: Some(PROSE_FREQUENCY_PENALTY),
            ..SamplingProfile::default()
        },
        "prose_grounded must never send presence_penalty"
    );
    // `Default` (no declared intent) resolves the same as `Deterministic`.
    assert_eq!(
        GeminiClient.sampling_profile(model, Intent::Default),
        GeminiClient.sampling_profile(model, Intent::Deterministic)
    );
}

#[test]
fn sampling_profile_declares_real_values_for_a_non_gemini_prefixed_pre_v3_model() {
    // `parse_model_page` filters Google's `/v1beta/models` listing only on
    // the `models/` wrapper prefix, not on family, so a `gemma-*`/`learnlm-*`
    // id reaches this app's model picker exactly like a `gemini-*` one and is
    // a real Google model served by the same endpoint — both must get the
    // SAME deterministic profile as a `gemini-`-prefixed pre-v3 model, not
    // fall back to neutral. (A previous version of `gemini_is_pre_v3`
    // required the literal `gemini-` prefix and silently dropped these to
    // neutral — see that function's own doc comment for why that was wrong
    // and reverted; there is no "unrecognized model" case left at this gate
    // to assert neutrality for.)
    for model in ["gemma-3-27b-it", "learnlm-2.0-flash-experimental"] {
        assert_eq!(
            GeminiClient.sampling_profile(model, Intent::Deterministic),
            SamplingProfile {
                temperature: Some(DETERMINISTIC_TEMPERATURE),
                ..SamplingProfile::default()
            },
            "model {model} must get the deterministic profile, not neutral"
        );
    }
}

#[test]
fn gemini_effective_temperature_uses_fallback_for_a_non_gemini_prefixed_model() {
    // `gemini_effective_temperature`'s own gate stays on the wider
    // `gemini_omits_sampling_params` (v3+ only, via `gemini_is_v3_or_later`)
    // — a non-"gemini-"-prefixed id that isn't v3+ still gets its caller's
    // `fallback` here. `GeminiClient::sampling_profile`'s own gate
    // (`gemini_is_pre_v3`) is now the exact same v3 boundary, so the two no
    // longer diverge for any model id.
    assert_eq!(
        gemini_effective_temperature("totally-unrecognized-model-id", None, 0.7),
        Some(0.7)
    );
}

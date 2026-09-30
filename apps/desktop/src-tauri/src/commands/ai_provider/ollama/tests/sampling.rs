//! `OllamaClient::sampling_profile` + the wire-through into
//! `build_chat_stream_body`.

use serde_json::json;

use super::super::super::{
    AiProvider, Intent, SamplingProfile, DETERMINISTIC_TEMPERATURE, PROSE_GROUNDED_TEMPERATURE,
    PROSE_REPEAT_PENALTY, PROSE_TEMPERATURE, PROSE_TOP_P,
};
use super::super::chat::build_chat_stream_body;
use super::super::OllamaClient;
use super::support::{base_request, sampling_for};

#[test]
fn sampling_profile_declares_real_values_for_every_model_no_unknown_model_fail_safe() {
    // Unlike every cloud adapter, Ollama has no "this family 400s" split —
    // every model gets the SAME real per-intent values, including an
    // unrecognized/future model id (there is no unsafe case to fail toward
    // neutral for).
    for model in ["llama3.1:8b", "some-unrecognized-future-model"] {
        assert_eq!(
            OllamaClient.sampling_profile(model, Intent::Deterministic),
            SamplingProfile {
                temperature: Some(DETERMINISTIC_TEMPERATURE),
                ..SamplingProfile::default()
            },
            "{model}"
        );
        assert_eq!(
            OllamaClient.sampling_profile(model, Intent::Prose),
            SamplingProfile {
                temperature: Some(PROSE_TEMPERATURE),
                top_p: Some(PROSE_TOP_P),
                repeat_penalty: Some(PROSE_REPEAT_PENALTY),
                ..SamplingProfile::default()
            },
            "{model}"
        );
        assert_eq!(
            OllamaClient.sampling_profile(model, Intent::ProseGrounded),
            SamplingProfile {
                temperature: Some(PROSE_GROUNDED_TEMPERATURE),
                top_p: Some(PROSE_TOP_P),
                repeat_penalty: Some(PROSE_REPEAT_PENALTY),
                ..SamplingProfile::default()
            },
            "{model}"
        );
        // `Default` (no declared intent) resolves the same as `Deterministic`.
        assert_eq!(
            OllamaClient.sampling_profile(model, Intent::Default),
            OllamaClient.sampling_profile(model, Intent::Deterministic),
            "{model}"
        );
    }
}

#[test]
fn wire_body_carries_the_declared_deterministic_temperature_with_no_explicit_override() {
    // End-to-end: was DEAD before `chat_stream` was wired through
    // `sampling_profile` — `stream_chat` called `build_chat_stream_body(req)`
    // directly, reading `req.temperature` (always `None` from the renderer
    // now) with no fallback at all, silently deferring to whatever the
    // model's Modelfile says (see `sampling_profile`'s doc comment for the
    // empirical live-Ollama evidence this is unsafe). Mutation check: change
    // `DETERMINISTIC_TEMPERATURE` or delete the `Intent::Deterministic` arm
    // in `OllamaClient::sampling_profile` and this must fail.
    let mut req = base_request();
    req.temperature = None;
    req.intent = Some("deterministic".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["options"]["temperature"],
        json!(DETERMINISTIC_TEMPERATURE)
    );
}

#[test]
fn wire_body_prose_grounded_carries_its_own_temperature_not_proses() {
    // Ollama's wire body has no presence_penalty field at all (see
    // `build_chat_stream_body`'s doc comment), so on this adapter the only
    // observable difference between the two prose intents is the temperature.
    // It is HIGHER for grounded, not lower — see `PROSE_GROUNDED_TEMPERATURE`'s
    // doc comment for why that is not the contradiction it looks like.
    let mut req = base_request();
    req.temperature = None;
    req.intent = Some("prose_grounded".to_string());
    let grounded = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        grounded["options"]["temperature"],
        json!(PROSE_GROUNDED_TEMPERATURE)
    );
    assert_eq!(grounded["options"]["top_p"], json!(PROSE_TOP_P));
    assert_eq!(
        grounded["options"]["repeat_penalty"],
        json!(PROSE_REPEAT_PENALTY)
    );

    req.intent = Some("prose".to_string());
    let prose = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(prose["options"]["temperature"], json!(PROSE_TEMPERATURE));
}

#[test]
fn chat_stream_body_serializes_top_p_and_repeat_penalty_when_set() {
    let mut req = base_request();
    req.top_p = Some(0.95);
    req.repeat_penalty = Some(1.15);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    // `base_request()` sets `temperature: Some(0.8)` — the request's
    // explicit override the Settings slider depends on — and it must still
    // win over the intent-derived profile after `.resolve(req)`.
    assert_eq!(body["options"]["temperature"], json!(0.8));
    assert_eq!(body["options"]["top_p"], json!(0.95));
    assert_eq!(body["options"]["repeat_penalty"], json!(1.15));
    // frequency_penalty is never remapped into Ollama's repeat_penalty field.
    assert!(body["options"].get("frequency_penalty").is_none());
}

#[test]
fn chat_stream_body_omits_sampling_options_when_none() {
    let req = base_request();
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body["options"].get("top_p").is_none());
    assert!(body["options"].get("repeat_penalty").is_none());
}

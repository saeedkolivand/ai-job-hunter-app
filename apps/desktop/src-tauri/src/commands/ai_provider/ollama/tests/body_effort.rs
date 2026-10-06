//! `think` gating inside `build_chat_stream_body`.

use serde_json::json;

use super::super::chat::build_chat_stream_body;
use super::support::{base_request, sampling_for};

#[test]
fn chat_stream_body_sends_think_only_for_a_thinking_model_with_effort_set() {
    let mut req = base_request();
    req.model = "gpt-oss:20b".to_string();
    req.effort = Some("high".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["think"], json!("high"));
}

#[test]
fn chat_stream_body_omits_think_for_a_non_thinking_model_even_with_effort_set() {
    let mut req = base_request();
    req.model = "llama3.1:8b".to_string();
    req.effort = Some("high".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("think").is_none());
}

#[test]
fn chat_stream_body_omits_think_outside_the_verified_level_set() {
    // Same class as the Gemini/Anthropic/OpenAI gate: a stale/unrecognized
    // value must never be sent just because the CURRENT model is in the
    // thinking family — `effort` is stored per PROVIDER, not per model.
    let mut req = base_request();
    req.model = "gpt-oss:20b".to_string();
    req.effort = Some("xhigh".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("think").is_none(),
        "xhigh is outside OLLAMA_EFFORT_LEVELS — must not be sent"
    );
}

#[test]
fn chat_stream_body_omits_think_when_effort_not_set() {
    let mut req = base_request();
    req.model = "gpt-oss:20b".to_string();
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("think").is_none());
}

/// The `off` tier (issue #1351): the lowest a thinking model offers. A
/// qwen3-style model takes the boolean (probed against a live daemon: `false`
/// ends the reasoning, a level string only shortens it); gpt-oss ignores
/// `false` (it kept reasoning) and only honours a level, so it gets `"low"`.
///
/// Mutation check (executed): make `think_level` return `json!("low")` for
/// every `off` and the qwen3 case fails; return `json!(false)` for every `off`
/// and the gpt-oss case fails.
#[test]
fn off_sends_think_false_on_qwen3_and_low_on_gpt_oss() {
    for (model, expected) in [
        ("qwen3.8:latest", json!(false)),
        ("deepseek-r1:8b", json!(false)),
        ("gpt-oss:20b", json!("low")),
    ] {
        let mut req = base_request();
        req.model = model.to_string();
        req.effort = Some("off".to_string());
        let body = build_chat_stream_body(&req, sampling_for(&req));
        assert_eq!(body["think"], expected, "{model}: {body}");
    }
}

#[test]
fn off_is_never_sent_to_a_non_thinking_model() {
    let mut req = base_request();
    req.model = "llama3.1:8b".to_string();
    req.effort = Some("off".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("think").is_none(), "{body}");
}

/// `Completer::low_effort` takes entry 0: `off` where thinking can be
/// disabled, `low` where it cannot (gpt-oss).
#[test]
fn the_cheapest_tier_is_off_except_where_thinking_cannot_be_disabled() {
    use super::super::super::OllamaClient;
    use crate::commands::ai_provider::AiProvider;
    assert_eq!(
        OllamaClient.effort_levels("qwen3.8:latest"),
        vec!["off", "low", "medium", "high"]
    );
    assert_eq!(
        OllamaClient.effort_levels("gpt-oss:20b"),
        vec!["low", "medium", "high"]
    );
    for (model, cheapest) in [("qwen3.8:latest", "off"), ("gpt-oss:20b", "low")] {
        assert_eq!(
            crate::pipeline::low_effort_level(&OllamaClient.effort_levels(model)),
            Some(cheapest)
        );
    }
}

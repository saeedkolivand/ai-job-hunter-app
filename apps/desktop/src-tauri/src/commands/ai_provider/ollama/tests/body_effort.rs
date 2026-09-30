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

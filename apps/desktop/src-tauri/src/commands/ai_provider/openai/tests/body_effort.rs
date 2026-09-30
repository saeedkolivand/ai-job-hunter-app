//! `reasoning_effort` gating inside `build_chat_stream_body`.

use serde_json::json;

use super::super::super::{ModelCapabilities, SamplingProfile};
use super::super::body::build_chat_stream_body;
use super::support::{base_request, chat_caps};

#[test]
fn chat_stream_body_sends_reasoning_effort_for_a_reasoning_capable_model() {
    let mut req = base_request();
    req.model = "o3-mini".to_string();
    req.effort = Some("high".to_string());
    let caps = ModelCapabilities {
        supports_reasoning: true,
        ..chat_caps(false)
    };
    let body = build_chat_stream_body(&req, caps, SamplingProfile::default());
    assert_eq!(body["reasoning_effort"], json!("high"));
}

#[test]
fn chat_stream_body_omits_reasoning_effort_for_a_non_reasoning_model() {
    let mut req = base_request();
    req.model = "gpt-4o".to_string();
    req.effort = Some("high".to_string());
    let caps = ModelCapabilities {
        supports_reasoning: false,
        ..chat_caps(true)
    };
    let body = build_chat_stream_body(&req, caps, SamplingProfile::default());
    assert!(body.get("reasoning_effort").is_none());
}

#[test]
fn chat_stream_body_omits_reasoning_effort_when_not_set() {
    let req = base_request();
    let caps = ModelCapabilities {
        supports_reasoning: true,
        ..chat_caps(true)
    };
    let body = build_chat_stream_body(&req, caps, SamplingProfile::default());
    assert!(body.get("reasoning_effort").is_none());
}

#[test]
fn chat_stream_body_omits_reasoning_effort_outside_the_verified_level_set() {
    // OpenAI's real `reasoning_effort` enum has grown to 7 values (see
    // `OPENAI_EFFORT_LEVELS`'s doc comment) but this adapter only exposes the
    // 3 verified-universal ones — a stale/unrecognized value (e.g. carried
    // over from a DIFFERENT provider's richer picker, since `effort` is
    // stored per provider, not per model) must never be sent through just
    // because `caps.supports_reasoning` is true.
    let mut req = base_request();
    req.model = "o3-mini".to_string();
    req.effort = Some("xhigh".to_string());
    let caps = ModelCapabilities {
        supports_reasoning: true,
        ..chat_caps(false)
    };
    let body = build_chat_stream_body(&req, caps, SamplingProfile::default());
    assert!(
        body.get("reasoning_effort").is_none(),
        "xhigh is outside OPENAI_EFFORT_LEVELS — must not be sent"
    );
}

//! `build_chat_stream_body` construction: sampling wire-through, top_p
//! gating around classic/adaptive thinking, and the thinking block shape
//! itself.

use serde_json::json;

use super::super::super::DETERMINISTIC_TEMPERATURE;
use super::super::super::{PROSE_GROUNDED_TEMPERATURE, PROSE_TEMPERATURE, PROSE_TOP_P};
use super::super::body::build_chat_stream_body;
use super::support::{base_request, sampling_for};

#[test]
fn wire_body_carries_the_declared_deterministic_temperature_with_no_explicit_override() {
    // End-to-end: the profile's own temperature reaches the wire when the
    // request carries no explicit override — this is exactly the path that
    // was DEAD before `chat_stream` was wired through `sampling_profile`
    // (previously `build_chat_stream_body` read `req.temperature` directly,
    // defaulting to a bare 0.7 that no test ever exercised). Mutation check:
    // change `DETERMINISTIC_TEMPERATURE` or delete the `Intent::Deterministic`
    // arm in `AnthropicClient::sampling_profile` and this must fail.
    let mut req = base_request("claude-3-5-sonnet-20241022");
    req.temperature = None;
    req.intent = Some("deterministic".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["temperature"], json!(DETERMINISTIC_TEMPERATURE));
}

#[test]
fn wire_body_prose_grounded_never_sends_a_top_p_higher_register_than_declared_and_no_penalty_field_exists(
) {
    // Anthropic has NO frequency/presence penalty parameter at all — so
    // `Intent::ProseGrounded`'s distinguishing feature elsewhere (omitting
    // presence_penalty) has nothing to omit here. What DOES carry over is
    // the lower, more-traceable temperature vs. `Intent::Prose`.
    let mut req = base_request("claude-3-5-sonnet-20241022");
    req.temperature = None;
    req.intent = Some("prose_grounded".to_string());
    let grounded = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(grounded["temperature"], json!(PROSE_GROUNDED_TEMPERATURE));
    assert_eq!(grounded["top_p"], json!(PROSE_TOP_P));
    assert!(!grounded
        .as_object()
        .unwrap()
        .contains_key("presence_penalty"));

    req.intent = Some("prose".to_string());
    let prose = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(prose["temperature"], json!(PROSE_TEMPERATURE));
}

#[test]
fn chat_stream_body_serializes_top_p_when_set_on_a_non_thinking_model() {
    let mut req = base_request("claude-3-5-sonnet-20241022");
    req.top_p = Some(0.95);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["top_p"], json!(0.95));
    assert!(body.get("thinking").is_none());
}

#[test]
fn chat_stream_body_omits_top_p_when_none() {
    let req = base_request("claude-3-5-sonnet-20241022");
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("top_p").is_none());
}

#[test]
fn chat_stream_body_skips_top_p_when_extended_thinking_is_enabled() {
    // The API rejects `top_p` alongside `thinking` — must never be sent
    // together, even if the caller (an application-answer/cover-letter
    // prose call) supplied top_p. `temperature` is omitted entirely on the
    // classic-thinking path too (Anthropic forces it to 1.0 internally;
    // omitting IS that default — see `build_chat_stream_body`'s doc comment).
    let mut req = base_request("claude-opus-4-20250514");
    req.top_p = Some(0.95);
    req.max_tokens = Some(4096); // >= 2048 → thinking budget kicks in
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("thinking").is_some(), "thinking should be enabled");
    assert!(
        body.get("temperature").is_none(),
        "temperature must be omitted, not forced to 1.0, when thinking is enabled"
    );
    assert!(
        body.get("top_p").is_none(),
        "top_p must be omitted when thinking is enabled"
    );
}

#[test]
fn chat_stream_body_sends_adaptive_thinking_with_summarized_display_for_claude_5_family() {
    // End-to-end: the request body builder must attach the adaptive
    // thinking block (opting into visible "summarized" display, which
    // defaults to "omitted"/empty otherwise) and inflate max_tokens the
    // same way the classic-thinking path does.
    for m in [
        "claude-opus-5",
        "claude-sonnet-5",
        "claude-fable-5",
        "claude-fable-5-20260201",
    ] {
        let mut req = base_request(m);
        req.max_tokens = Some(4096);
        let body = build_chat_stream_body(&req, sampling_for(&req));
        assert_eq!(
            body["thinking"],
            json!({ "type": "adaptive", "display": "summarized" }),
            "{m} must opt into summarized display"
        );
        assert_eq!(
            body["max_tokens"],
            json!(4096 + 4096 / 2),
            "{m}: thinking tokens count toward max_tokens on adaptive models too"
        );
        // Anthropic 400s on ANY non-default temperature/top_p for every
        // adaptive-thinking model — both must be entirely omitted.
        assert!(
            body.get("temperature").is_none(),
            "{m} must not send temperature"
        );
        assert!(body.get("top_p").is_none(), "{m} must not send top_p");
    }
}

#[test]
fn chat_stream_body_inflates_max_tokens_for_adaptive_models_below_the_classic_2048_gate() {
    // Regression for the extension bridge's answer-assist flow, which
    // calls with `max_tokens: 1000` (below the classic path's 2048
    // heuristic gate). Adaptive thinking is on by default regardless of
    // the caller's cap, so the inflation must NOT be gated the same way —
    // otherwise the user is billed summarized-thinking tokens out of an
    // un-inflated 1000-token budget and drafts come back short/empty.
    let mut req = base_request("claude-sonnet-5");
    req.max_tokens = Some(1000);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["thinking"],
        json!({ "type": "adaptive", "display": "summarized" })
    );
    assert_eq!(
        body["max_tokens"],
        json!(1000 + 1024),
        "a small cap must get the ~1024-token headroom floor, not a proportional \
         1000/2=500 that leaves too little room for thinking + a visible draft"
    );
}

#[test]
fn chat_stream_body_keeps_the_classic_2048_gate_for_classic_models() {
    // The classic path's inflation/`thinking` key must stay gated on
    // `max_tokens >= 2048` — only the ADAPTIVE path lost that gate.
    let mut req = base_request("claude-opus-4-20250514");
    req.max_tokens = Some(1000);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("thinking").is_none(),
        "classic thinking must stay off below the 2048 gate"
    );
    assert_eq!(
        body["max_tokens"],
        json!(1000),
        "no inflation for a classic model below the 2048 gate"
    );
}

#[test]
fn chat_stream_body_sends_adaptive_thinking_for_opus_4_7_and_4_8() {
    for m in ["claude-opus-4-7", "claude-opus-4-8"] {
        let mut req = base_request(m);
        req.max_tokens = Some(4096);
        let body = build_chat_stream_body(&req, sampling_for(&req));
        assert_eq!(
            body["thinking"],
            json!({ "type": "adaptive", "display": "summarized" }),
            "{m} is adaptive-only — must never get the classic enabled+budget shape"
        );
        assert!(body.get("temperature").is_none());
    }
}

#[test]
fn chat_stream_body_omits_top_p_for_adaptive_models_even_when_caller_supplies_it() {
    let mut req = base_request("claude-sonnet-5");
    req.top_p = Some(0.95);
    req.max_tokens = Some(4096);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("top_p").is_none(),
        "adaptive models 400 on a non-default top_p"
    );
}

#[test]
fn chat_stream_body_sends_no_thinking_key_for_unknown_models() {
    // Unknown/other models get nothing extra: no thinking block, no
    // max_tokens inflation, and the plain temperature/top_p path.
    let mut req = base_request("some-future-claude-model");
    req.max_tokens = Some(8192);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("thinking").is_none());
    assert_eq!(
        body["max_tokens"],
        json!(8192),
        "no inflation for an unknown model"
    );
    assert_eq!(body["temperature"], json!(0.8));
}

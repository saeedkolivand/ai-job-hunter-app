//! `thinkingConfig`/`thinkingLevel` gating inside `build_chat_stream_body`.

use serde_json::json;

use super::super::body::build_chat_stream_body;
use super::support::{base_request, sampling_for};

#[test]
fn chat_stream_body_sends_thinking_level_for_a_v3_model_with_effort_set() {
    // gemini-3.1-pro-preview — LIVE, Preview status
    // (`ai.google.dev/gemini-api/docs/models`, checked 2026-08-04).
    let mut req = base_request();
    req.model = "gemini-3.1-pro-preview".to_string();
    req.effort = Some("low".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        json!("LOW")
    );
}

#[test]
fn chat_stream_body_omits_thinking_level_invalid_for_the_current_model_tier() {
    // The reported model-switch scenario: `effort` is stored PER PROVIDER
    // (`preferences-store.ts`), not per model, and nothing clears it on a
    // model switch. "medium" is valid for gemini-3.1-pro-preview but NOT
    // for gemini-3.1-flash-lite-image (minimal/high only — LIVE, Stable
    // status, `ai.google.dev/gemini-api/docs/models`, checked
    // 2026-08-04) — both are Gemini 3+, so gating on
    // `gemini_is_v3_or_later` alone would ship an invalid level and 400.
    // Must omit `thinkingLevel` entirely rather than send a level the
    // CURRENT model rejects.
    let mut req = base_request();
    req.model = "gemini-3.1-flash-lite-image".to_string();
    req.effort = Some("medium".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body["generationConfig"]["thinkingConfig"]
            .get("thinkingLevel")
            .is_none(),
        "medium is invalid for gemini-3.1-flash-lite-image (minimal/high only) — must not be sent"
    );
}

#[test]
fn chat_stream_body_omits_thinking_level_for_a_pre_v3_model_even_with_effort_set() {
    // `thinkingLevel` is documented "Recommended for Gemini 3 or later
    // models. Use with earlier models results in an error" on THIS
    // endpoint's own REST reference — a pre-v3 model (2.5 and earlier)
    // must never get thinkingLevel/thinkingBudget.
    let mut req = base_request();
    req.model = "gemini-2.5-pro".to_string();
    req.effort = Some("low".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body["generationConfig"]["thinkingConfig"]
        .get("thinkingLevel")
        .is_none());
}

#[test]
fn chat_stream_body_omits_thinking_config_for_a_non_thinking_model_with_no_effort() {
    let mut req = base_request();
    // Pre-v3, non-2.5, non-"*-thinking-*" — genuinely no thinking support.
    req.model = "gemini-2.0-flash".to_string();
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body["generationConfig"].get("thinkingConfig").is_none());
}

#[test]
fn chat_stream_body_keeps_include_thoughts_alongside_thinking_level() {
    // A real Gemini 3 id matches BOTH the (now-widened) "thinking" display
    // gate and the v3 effort gate — both fields must land in the SAME
    // thinkingConfig object, not one clobbering the other. "high" is the
    // one level every row in `gemini_effort_levels`'s table accepts, so
    // it's valid for gemini-3.1-pro-preview specifically too.
    // gemini-3.1-pro-preview — LIVE, Preview status
    // (`ai.google.dev/gemini-api/docs/models`, checked 2026-08-04).
    let mut req = base_request();
    req.model = "gemini-3.1-pro-preview".to_string();
    req.effort = Some("high".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    let tc = &body["generationConfig"]["thinkingConfig"];
    assert_eq!(tc["includeThoughts"], json!(true));
    assert_eq!(tc["thinkingLevel"], json!("HIGH"));
}

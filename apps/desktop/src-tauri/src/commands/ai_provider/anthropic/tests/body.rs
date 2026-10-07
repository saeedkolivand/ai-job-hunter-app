//! Non-streaming `/messages` body builders (`complete`, `structured`,
//! `web_search`, `tools`) plus the `output_config.effort` gating the
//! streaming body shares with them.

use serde_json::json;

use super::super::body::{
    build_chat_stream_body, build_complete_body, build_structured_body, build_tools_body,
    build_web_search_body,
};
use super::super::capabilities::anthropic_structured_effort;
use super::support::{base_request, sampling_for};

#[test]
fn build_complete_body_omits_temperature_and_inflates_max_tokens_for_fable_5() {
    let body = build_complete_body("claude-fable-5", "", "hi", Some(0.8));
    assert!(
        body.get("temperature").is_none(),
        "adaptive models 400 on a non-default temperature"
    );
    assert_eq!(
        body["max_tokens"],
        json!(4096 + 4096 / 2),
        "thinking is on by default and counts toward max_tokens even with no thinking key sent"
    );
    // No thinking-view display concern on this single-shot completion path.
    assert!(body.get("thinking").is_none());
}

#[test]
fn build_complete_body_keeps_temperature_for_a_classic_model() {
    let body = build_complete_body("claude-opus-4-20250514", "sys", "hi", Some(0.3));
    assert_eq!(body["temperature"], json!(0.3));
    assert_eq!(
        body["max_tokens"],
        json!(4096),
        "no inflation for a classic model here"
    );
    assert_eq!(body["system"], json!("sys"));
}

#[test]
fn build_structured_body_merges_output_config_and_still_omits_temperature_on_an_adaptive_model() {
    // The structured path's own body-construction unit test (no HTTP): the
    // merge must land at `output_config.format.type`, and it must NOT disturb
    // `build_complete_body`'s own temperature gate for an adaptive model.
    // Mutation check: drop the `output_config` merge in `build_structured_body`
    // and the first assertion fails; drop `build_complete_body`'s own
    // adaptive-thinking gate and the second one does.
    let output_config =
        json!({ "format": { "type": "json_schema", "schema": { "type": "object" } } });
    let body = build_structured_body(
        "claude-opus-5",
        "sys",
        "hi",
        Some(0.8),
        Some(output_config.clone()),
    );
    assert_eq!(body["output_config"], output_config);
    // Streamed + re-assembled (#1353): the schema rides the stream request.
    assert_eq!(body["stream"], json!(true));
    assert_eq!(
        body["output_config"]["format"]["type"],
        json!("json_schema")
    );
    assert!(
        body.get("temperature").is_none(),
        "adaptive models 400 on a non-default temperature"
    );
}

#[test]
fn build_structured_body_is_build_complete_body_when_output_config_is_absent() {
    // Every caller but the structured path passes `None` — must stay
    // byte-identical to the pre-existing `build_complete_body` alone.
    let body = build_structured_body("claude-opus-4-20250514", "sys", "hi", Some(0.3), None);
    assert_eq!(
        body,
        build_complete_body("claude-opus-4-20250514", "sys", "hi", Some(0.3))
    );
}

#[test]
fn anthropic_structured_effort_keeps_a_level_only_this_models_tier_accepts() {
    // Same table `anthropic_effort_levels` already asserts elsewhere: `xhigh`
    // is on the full (5) tier only. Mutation check: drop the `.contains(e)`
    // filter in `anthropic_structured_effort` and the second assertion fails.
    assert_eq!(
        anthropic_structured_effort("claude-opus-5", Some("xhigh")),
        Some("xhigh")
    );
    assert_eq!(
        anthropic_structured_effort("claude-opus-4-5", Some("xhigh")),
        None,
        "opus-4-5's tier tops out at high — a stale xhigh from another model must not ride along"
    );
    assert_eq!(
        anthropic_structured_effort("claude-opus-5", Some("  ")),
        None
    );
    assert_eq!(anthropic_structured_effort("claude-opus-5", None), None);
}

#[test]
fn build_web_search_body_omits_temperature_and_inflates_max_tokens_for_fable_5() {
    let body = build_web_search_body("claude-fable-5", "sys", "hi");
    assert!(
        body.get("temperature").is_none(),
        "adaptive models 400 on a non-default temperature"
    );
    assert_eq!(
        body["max_tokens"],
        json!(1024 + 1024),
        "the ~1024-token headroom floor applies even to this small hardcoded 1024 cap \
         (a proportional 1024/2=512 would be below the useful-thinking floor)"
    );
}

#[test]
fn build_web_search_body_keeps_its_hardcoded_temperature_for_a_classic_model() {
    let body = build_web_search_body("claude-opus-4-20250514", "sys", "hi");
    assert_eq!(body["temperature"], json!(0.2));
    assert_eq!(body["max_tokens"], json!(1024));
}

#[test]
fn build_web_search_body_uses_the_newer_tool_type_on_a_supported_model() {
    let body = build_web_search_body("claude-sonnet-5", "sys", "hi");
    assert_eq!(body["tools"][0]["type"], json!("web_search_20260209"));
}

#[test]
fn build_web_search_body_keeps_the_classic_tool_type_elsewhere() {
    let body = build_web_search_body("claude-opus-4-5", "sys", "hi");
    assert_eq!(body["tools"][0]["type"], json!("web_search_20250305"));
}

#[test]
fn build_tools_body_omits_temperature_and_inflates_max_tokens_for_fable_5() {
    let body = build_tools_body("claude-fable-5", "", vec![], vec![], Some(0.8));
    assert!(
        body.get("temperature").is_none(),
        "adaptive models 400 on a non-default temperature"
    );
    assert_eq!(body["max_tokens"], json!(4096 + 4096 / 2));
}

#[test]
fn build_tools_body_keeps_temperature_for_a_classic_model() {
    let body = build_tools_body("claude-opus-4-20250514", "", vec![], vec![], Some(0.4));
    assert_eq!(body["temperature"], json!(0.4));
    assert_eq!(body["max_tokens"], json!(4096));
}

#[test]
fn chat_stream_body_sends_output_config_effort_for_an_effort_capable_model() {
    let mut req = base_request("claude-opus-5");
    req.effort = Some("low".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["output_config"], json!({ "effort": "low" }));
}

#[test]
fn chat_stream_body_omits_output_config_for_a_non_effort_capable_model() {
    let mut req = base_request("claude-3-5-sonnet-20241022");
    req.effort = Some("low".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("output_config").is_none());
}

#[test]
fn chat_stream_body_omits_output_config_when_effort_not_set() {
    let req = base_request("claude-opus-5");
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body.get("output_config").is_none());
}

#[test]
fn chat_stream_body_omits_output_config_effort_invalid_for_the_current_model_tier() {
    // The reported model-switch scenario: `effort` is stored PER PROVIDER
    // (`preferences-store.ts`), not per model. "xhigh" is valid on Sonnet 5
    // but NOT on Sonnet 4.6 (max but no xhigh) — both are effort-capable, so
    // gating on `anthropic_supports_effort` alone would ship an invalid
    // level and 400. Must omit `output_config` entirely rather than send a
    // level the CURRENT model rejects.
    let mut req = base_request("claude-sonnet-4-6");
    req.effort = Some("xhigh".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("output_config").is_none(),
        "xhigh is invalid for claude-sonnet-4-6 (max but no xhigh) — must not be sent"
    );
}

#[test]
fn chat_stream_body_omits_output_config_effort_invalid_for_opus_4_5() {
    // Opus 4.5 is the one effort-capable model with NEITHER max nor xhigh.
    let mut req = base_request("claude-opus-4-5");
    req.effort = Some("max".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("output_config").is_none(),
        "max is invalid for claude-opus-4-5 (low/medium/high only) — must not be sent"
    );
}

#[test]
fn chat_stream_body_sends_xhigh_only_on_a_model_that_supports_it() {
    let mut req = base_request("claude-sonnet-5");
    req.effort = Some("xhigh".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["output_config"], json!({ "effort": "xhigh" }));
}

/// `complete_with_effort`'s body (issue #1351): effort rides as
/// `output_config.effort` only where the model's tier accepts it, and the plain
/// path never carries a `thinking` block — with no effort the body is exactly
/// the plain completion body.
///
/// Mutation check (executed): drop the `anthropic_structured_effort` filter in
/// `anthropic_effort_output_config` and the stale-`xhigh` case fails; make it
/// return `None` always and the `low` case fails.
#[test]
fn complete_with_effort_body_gates_the_effort_and_never_sends_thinking() {
    use super::super::capabilities::anthropic_effort_output_config as oc;
    let body = |model: &str, effort: Option<&str>| {
        build_structured_body(model, "sys", "hi", Some(0.3), oc(model, effort))
    };

    let on = body("claude-opus-5", Some("low"));
    assert_eq!(on["output_config"], json!({ "effort": "low" }), "{on}");
    assert!(on.get("thinking").is_none(), "{on}");

    // No effort (or blank): identical to the plain completion body.
    for effort in [None, Some("  ")] {
        let off = body("claude-opus-5", effort);
        assert_eq!(
            off,
            build_complete_body("claude-opus-5", "sys", "hi", Some(0.3))
        );
    }

    // A level this model's tier rejects, and a model with no effort lever, are
    // omitted rather than sent and 400ed.
    assert!(body("claude-opus-4-5", Some("xhigh"))
        .get("output_config")
        .is_none());
    assert!(body("claude-opus-4-20250514", Some("low"))
        .get("output_config")
        .is_none());
}
